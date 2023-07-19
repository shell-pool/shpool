/*!
Provides a noncontiguous NFA implementation of Aho-Corasick.

This is a low-level API that generally only needs to be used in niche
circumstances. When possible, prefer using [`AhoCorasick`](crate::AhoCorasick)
instead of a noncontiguous NFA directly. Using an `NFA` directly is typically
only necessary when one needs access to the [`Automaton`] trait implementation.
*/

use alloc::{
    collections::{BTreeSet, VecDeque},
    vec,
    vec::Vec,
};

use crate::{
    automaton::Automaton,
    util::{
        alphabet::{ByteClassSet, ByteClasses},
        error::{BuildError, MatchError},
        prefilter::{self, opposite_ascii_case, Prefilter},
        primitives::{IteratorIndexExt, PatternID, SmallIndex, StateID},
        remapper::Remapper,
        search::{Anchored, MatchKind},
        special::Special,
    },
};

/// A noncontiguous NFA implementation of Aho-Corasick.
///
/// When possible, prefer using [`AhoCorasick`](crate::AhoCorasick) instead of
/// this type directly. Using an `NFA` directly is typically only necessary
/// when one needs access to the [`Automaton`] trait implementation.
///
/// This NFA represents the "core" implementation of Aho-Corasick in this
/// crate. Namely, constructing this NFA involving building a trie and then
/// filling in the failure transitions between states, similar to what is
/// described in any standard textbook description of Aho-Corasick.
///
/// In order to minimize heap usage and to avoid additional construction costs,
/// this implementation represents the transitions of all states as distinct
/// sparse memory allocations. This is where it gets its name from. That is,
/// this NFA has no contiguous memory allocation for its transition table. Each
/// state gets its own allocation.
///
/// While the sparse representation keeps memory usage to somewhat reasonable
/// levels, it is still quite large and also results in somewhat mediocre
/// search performance. For this reason, it is almost always a good idea to
/// use a [`contiguous::NFA`](crate::nfa::contiguous::NFA) instead. It is
/// marginally slower to build, but has higher throughput and can sometimes use
/// an order of magnitude less memory. The main reason to use a noncontiguous
/// NFA is when you need the fastest possible construction time, or when a
/// contiguous NFA does not have the desired capacity. (The total number of NFA
/// states it can have is fewer than a noncontiguous NFA.)
///
/// # Example
///
/// This example shows how to build an `NFA` directly and use it to execute
/// [`Automaton::try_find`]:
///
/// ```
/// use aho_corasick::{
///     automaton::Automaton,
///     nfa::noncontiguous::NFA,
///     Input, Match,
/// };
///
/// let patterns = &["b", "abc", "abcd"];
/// let haystack = "abcd";
///
/// let nfa = NFA::new(patterns).unwrap();
/// assert_eq!(
///     Some(Match::must(0, 1..2)),
///     nfa.try_find(&Input::new(haystack))?,
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// It is also possible to implement your own version of `try_find`. See the
/// [`Automaton`] documentation for an example.
#[derive(Clone)]
pub struct NFA {
    /// The match semantics built into this NFA.
    match_kind: MatchKind,
    /// A set of states. Each state defines its own transitions, a fail
    /// transition and a set of indices corresponding to matches.
    ///
    /// The first state is always the fail state, which is used only as a
    /// sentinel. Namely, in the final NFA, no transition into the fail state
    /// exists. (Well, they do, but they aren't followed. Instead, the state's
    /// failure transition is followed.)
    ///
    /// The second state (index 1) is always the dead state. Dead states are
    /// in every automaton, but only used when leftmost-{first,longest} match
    /// semantics are enabled. Specifically, they instruct search to stop
    /// at specific points in order to report the correct match location. In
    /// the standard Aho-Corasick construction, there are no transitions to
    /// the dead state.
    ///
    /// The third state (index 2) is generally intended to be the starting or
    /// "root" state.
    states: Vec<State>,
    /// The length, in bytes, of each pattern in this NFA. This slice is
    /// indexed by `PatternID`.
    ///
    /// The number of entries in this vector corresponds to the total number of
    /// patterns in this automaton.
    pattern_lens: Vec<SmallIndex>,
    /// A prefilter for quickly skipping to candidate matches, if pertinent.
    prefilter: Option<Prefilter>,
    /// A set of equivalence classes in terms of bytes. We compute this while
    /// building the NFA, but don't use it in the NFA's states. Instead, we
    /// use this for building the DFA. We store it on the NFA since it's easy
    /// to compute while visiting the patterns.
    byte_classes: ByteClasses,
    /// The length, in bytes, of the shortest pattern in this automaton. This
    /// information is useful for detecting whether an automaton matches the
    /// empty string or not.
    min_pattern_len: usize,
    /// The length, in bytes, of the longest pattern in this automaton. This
    /// information is useful for keeping correct buffer sizes when searching
    /// on streams.
    max_pattern_len: usize,
    /// The information required to deduce which states are "special" in this
    /// NFA.
    ///
    /// Since the DEAD and FAIL states are always the first two states and
    /// there are only ever two start states (which follow all of the match
    /// states), it follows that we can determine whether a state is a fail,
    /// dead, match or start with just a few comparisons on the ID itself:
    ///
    ///    is_dead(sid): sid == NFA::DEAD
    ///    is_fail(sid): sid == NFA::FAIL
    ///   is_match(sid): NFA::FAIL < sid && sid <= max_match_id
    ///   is_start(sid): sid == start_unanchored_id || sid == start_anchored_id
    ///
    /// Note that this only applies to the NFA after it has been constructed.
    /// During construction, the start states are the first ones added and the
    /// match states are inter-leaved with non-match states. Once all of the
    /// states have been added, the states are shuffled such that the above
    /// predicates hold.
    special: Special,
    /// The number of bytes of heap used by this sparse NFA.
    memory_usage: usize,
}

impl NFA {
    /// Create a new Aho-Corasick noncontiguous NFA using the default
    /// configuration.
    ///
    /// Use a [`Builder`] if you want to change the configuration.
    pub fn new<I, P>(patterns: I) -> Result<NFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        NFA::builder().build(patterns)
    }

    /// A convenience method for returning a new Aho-Corasick noncontiguous NFA
    /// builder.
    ///
    /// This usually permits one to just import the `NFA` type.
    pub fn builder() -> Builder {
        Builder::new()
    }
}

impl NFA {
    /// The DEAD state is a sentinel state like the FAIL state. The DEAD state
    /// instructs any search to stop and return any currently recorded match,
    /// or no match otherwise. Generally speaking, it is impossible for an
    /// unanchored standard search to enter a DEAD state. But an anchored
    /// search can, and so to can a leftmost search.
    ///
    /// We put DEAD before FAIL so that DEAD is always 0. We repeat this
    /// decision across the other Aho-Corasicm automata, so that DEAD
    /// states there are always 0 too. It's not that we need all of the
    /// implementations to agree, but rather, the contiguous NFA and the DFA
    /// use a sort of "premultiplied" state identifier where the only state
    /// whose ID is always known and constant is the first state. Subsequent
    /// state IDs depend on how much space has already been used in the
    /// transition table.
    pub(crate) const DEAD: StateID = StateID::new_unchecked(0);
    /// The FAIL state mostly just corresponds to the ID of any transition on a
    /// state that isn't explicitly defined. When one transitions into the FAIL
    /// state, one must follow the previous state's failure transition before
    /// doing the next state lookup. In this way, FAIL is more of a sentinel
    /// than a state that one actually transitions into. In particular, it is
    /// never exposed in the `Automaton` interface.
    pub(crate) const FAIL: StateID = StateID::new_unchecked(1);

    /// Returns the equivalence classes of bytes found while constructing
    /// this NFA.
    ///
    /// Note that the NFA doesn't actually make use of these equivalence
    /// classes. Instead, these are useful for building the DFA when desired.
    pub(crate) fn byte_classes(&self) -> &ByteClasses {
        &self.byte_classes
    }

    /// Returns a slice containing the length of each pattern in this searcher.
    /// It is indexed by `PatternID` and has length `NFA::patterns_len`.
    ///
    /// This is exposed for convenience when building a contiguous NFA. But it
    /// can be reconstructed from the `Automaton` API if necessary.
    pub(crate) fn pattern_lens_raw(&self) -> &[SmallIndex] {
        &self.pattern_lens
    }

    /// Returns a slice of all states in this non-contiguous NFA.
    pub(crate) fn states(&self) -> &[State] {
        &self.states
    }

    /// Returns the underlying "special" state information for this NFA.
    pub(crate) fn special(&self) -> &Special {
        &self.special
    }

    /// Swaps the states at `id1` and `id2`.
    ///
    /// This does not update the transitions of any state to account for the
    /// state swap.
    pub(crate) fn swap_states(&mut self, id1: StateID, id2: StateID) {
        self.states.swap(id1.as_usize(), id2.as_usize());
    }

    /// Re-maps all state IDs in this NFA according to the `map` function
    /// given.
    pub(crate) fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
        for state in self.states.iter_mut() {
            state.fail = map(state.fail);
            for (_, ref mut sid) in state.trans.iter_mut() {
                *sid = map(*sid);
            }
        }
    }
}

// SAFETY: 'start_state' always returns a valid state ID, 'next_state' always
// returns a valid state ID given a valid state ID. We otherwise claim that
// all other methods are correct as well.
unsafe impl Automaton for NFA {
    #[inline(always)]
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError> {
        match anchored {
            Anchored::No => Ok(self.special.start_unanchored_id),
            Anchored::Yes => Ok(self.special.start_anchored_id),
        }
    }

    #[inline(always)]
    fn next_state(
        &self,
        anchored: Anchored,
        mut sid: StateID,
        byte: u8,
    ) -> StateID {
        // This terminates since:
        //
        // 1. state.fail never points to the FAIL state.
        // 2. All state.fail values point to a state closer to the start state.
        // 3. The start state has no transitions to the FAIL state.
        loop {
            let state = &self.states[sid];
            let next = state.next_state(byte);
            if next != NFA::FAIL {
                return next;
            }
            // For an anchored search, we never follow failure transitions
            // because failure transitions lead us down a path to matching
            // a *proper* suffix of the path we were on. Thus, it can only
            // produce matches that appear after the beginning of the search.
            if anchored.is_anchored() {
                return NFA::DEAD;
            }
            sid = state.fail;
        }
    }

    #[inline(always)]
    fn is_special(&self, sid: StateID) -> bool {
        sid <= self.special.max_special_id
    }

    #[inline(always)]
    fn is_dead(&self, sid: StateID) -> bool {
        sid == NFA::DEAD
    }

    #[inline(always)]
    fn is_match(&self, sid: StateID) -> bool {
        // N.B. This returns true when sid==NFA::FAIL but that's okay because
        // NFA::FAIL is not actually a valid state ID from the perspective of
        // the Automaton trait. Namely, it is never returned by 'start_state'
        // or by 'next_state'. So we don't need to care about it here.
        !self.is_dead(sid) && sid <= self.special.max_match_id
    }

    #[inline(always)]
    fn is_start(&self, sid: StateID) -> bool {
        sid == self.special.start_unanchored_id
            || sid == self.special.start_anchored_id
    }

    #[inline(always)]
    fn match_kind(&self) -> MatchKind {
        self.match_kind
    }

    #[inline(always)]
    fn patterns_len(&self) -> usize {
        self.pattern_lens.len()
    }

    #[inline(always)]
    fn pattern_len(&self, pid: PatternID) -> usize {
        self.pattern_lens[pid].as_usize()
    }

    #[inline(always)]
    fn min_pattern_len(&self) -> usize {
        self.min_pattern_len
    }

    #[inline(always)]
    fn max_pattern_len(&self) -> usize {
        self.max_pattern_len
    }

    #[inline(always)]
    fn match_len(&self, sid: StateID) -> usize {
        self.states[sid].matches.len()
    }

    #[inline(always)]
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID {
        self.states[sid].matches[index]
    }

    #[inline(always)]
    fn memory_usage(&self) -> usize {
        self.memory_usage
            + self.prefilter.as_ref().map_or(0, |p| p.memory_usage())
    }

    #[inline(always)]
    fn prefilter(&self) -> Option<&Prefilter> {
        self.prefilter.as_ref()
    }
}

/// A representation of a sparse NFA state for an Aho-Corasick automaton.
///
/// It contains the transitions to the next state, a failure transition for
/// cases where there exists no other transition for the current input byte
/// and the matches implied by visiting this state (if any).
#[derive(Clone)]
pub(crate) struct State {
    /// The set of defined transitions for this state sorted by `u8`. In an
    /// unanchored search, if a byte is not in this set of transitions, then
    /// it should transition to `fail`. In an anchored search, it should
    /// transition to the special DEAD state.
    pub(crate) trans: Vec<(u8, StateID)>,
    /// The patterns that match once this state is entered. Note that order
    /// is important in the leftmost case. For example, if one adds 'foo' and
    /// 'foo' (duplicate patterns are not disallowed), then in a leftmost-first
    /// search, only the first 'foo' will ever match.
    pub(crate) matches: Vec<PatternID>,
    /// The state that should be transitioned to if the current byte in the
    /// haystack does not have a corresponding transition defined in this
    /// state.
    pub(crate) fail: StateID,
    /// The depth of this state. Specifically, this is the distance from this
    /// state to the starting state. (For the special sentinel states DEAD and
    /// FAIL, their depth is always 0.) The depth of a starting state is 0.
    ///
    /// Note that depth is currently not used in this non-contiguous NFA. It
    /// may in the future, but it is used in the contiguous NFA. Namely, it
    /// permits an optimization where states near the starting state have their
    /// transitions stored in a dense fashion, but all other states have their
    /// transitions stored in a sparse fashion. (This non-contiguous NFA uses
    /// a sparse representation for all states unconditionally.) In any case,
    /// this is really the only convenient place to compute and store this
    /// information, which we need when building the contiguous NFA.
    pub(crate) depth: SmallIndex,
}

impl State {
    /// Return the heap memory used by this state. Note that if `State` is
    /// itself on the heap, then callers need to call this in addition to
    /// `size_of::<State>()` to get the full heap memory used.
    fn memory_usage(&self) -> usize {
        use core::mem::size_of;

        (self.trans.len() * size_of::<(u8, StateID)>())
            + (self.matches.len() * size_of::<PatternID>())
    }

    /// Return true if and only if this state is a match state.
    fn is_match(&self) -> bool {
        !self.matches.is_empty()
    }

    /// Return the next state by following the transition for the given byte.
    /// If no transition for the given byte is defined, then the FAIL state ID
    /// is returned.
    #[inline(always)]
    fn next_state(&self, byte: u8) -> StateID {
        // This is a special case that targets the unanchored starting state.
        // By construction, the unanchored starting state is actually a dense
        // state, because every possible transition is defined on it. Any
        // transitions that weren't added as part of initial trie construction
        // get explicitly added as a self-transition back to itself. Thus, we
        // can treat it as if it were dense and do a constant time lookup.
        //
        // This has *massive* benefit when executing searches because the
        // unanchored starting state is by far the hottest state and is
        // frequently visited. Moreover, the 'for' loop below that works
        // decently on an actually sparse state is disastrous on a state that
        // is nearly or completely dense.
        //
        // This optimization also works in general, including for non-starting
        // states that happen to have every transition defined. Namely, it
        // is impossible for 'self.trans' to have duplicate transitions (by
        // construction) and transitions are always in sorted ascending order.
        // So if a state has 256 transitions, it is, by construction, dense and
        // amenable to constant time indexing.
        if self.trans.len() == 256 {
            self.trans[usize::from(byte)].1
        } else {
            for &(b, id) in self.trans.iter() {
                if b == byte {
                    return id;
                }
            }
            NFA::FAIL
        }
    }

    /// Set the transition for the given byte to the state ID given.
    ///
    /// Note that one should not set transitions to the FAIL state. It is not
    /// technically incorrect, but it wastes space. If a transition is not
    /// defined, then it is automatically assumed to lead to the FAIL state.
    fn set_next_state(&mut self, byte: u8, next: StateID) {
        match self.trans.binary_search_by_key(&byte, |&(b, _)| b) {
            Ok(i) => self.trans[i] = (byte, next),
            Err(i) => self.trans.insert(i, (byte, next)),
        }
    }
}

impl core::fmt::Debug for State {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::{automaton::sparse_transitions, util::debug::DebugByte};

        let it = sparse_transitions(self.trans.iter().copied()).enumerate();
        for (i, (start, end, sid)) in it {
            if i > 0 {
                write!(f, ", ")?;
            }
            if start == end {
                write!(f, "{:?} => {:?}", DebugByte(start), sid.as_usize())?;
            } else {
                write!(
                    f,
                    "{:?}-{:?} => {:?}",
                    DebugByte(start),
                    DebugByte(end),
                    sid.as_usize()
                )?;
            }
        }
        Ok(())
    }
}

/// A builder for configuring an Aho-Corasick noncontiguous NFA.
///
/// This builder has a subset of the options available to a
/// [`AhoCorasickBuilder`](crate::AhoCorasickBuilder). Of the shared options,
/// their behavior is identical.
#[derive(Clone, Debug)]
pub struct Builder {
    match_kind: MatchKind,
    prefilter: bool,
    ascii_case_insensitive: bool,
}

impl Default for Builder {
    fn default() -> Builder {
        Builder {
            match_kind: MatchKind::default(),
            prefilter: true,
            ascii_case_insensitive: false,
        }
    }
}

impl Builder {
    /// Create a new builder for configuring an Aho-Corasick noncontiguous NFA.
    pub fn new() -> Builder {
        Builder::default()
    }

    /// Build an Aho-Corasick noncontiguous NFA from the given iterator of
    /// patterns.
    ///
    /// A builder may be reused to create more NFAs.
    pub fn build<I, P>(&self, patterns: I) -> Result<NFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        debug!("building non-contiguous NFA");
        let nfa = Compiler::new(self)?.compile(patterns)?;
        debug!(
            "non-contiguous NFA built, <states: {:?}, size: {:?}>",
            nfa.states.len(),
            nfa.memory_usage()
        );
        Ok(nfa)
    }

    /// Set the desired match semantics.
    ///
    /// See
    /// [`AhoCorasickBuilder::match_kind`](crate::AhoCorasickBuilder::match_kind)
    /// for more documentation and examples.
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut Builder {
        self.match_kind = kind;
        self
    }

    /// Enable ASCII-aware case insensitive matching.
    ///
    /// See
    /// [`AhoCorasickBuilder::ascii_case_insensitive`](crate::AhoCorasickBuilder::ascii_case_insensitive)
    /// for more documentation and examples.
    pub fn ascii_case_insensitive(&mut self, yes: bool) -> &mut Builder {
        self.ascii_case_insensitive = yes;
        self
    }

    /// Enable heuristic prefilter optimizations.
    ///
    /// See
    /// [`AhoCorasickBuilder::prefilter`](crate::AhoCorasickBuilder::prefilter)
    /// for more documentation and examples.
    pub fn prefilter(&mut self, yes: bool) -> &mut Builder {
        self.prefilter = yes;
        self
    }
}

/// A compiler uses a builder configuration and builds up the NFA formulation
/// of an Aho-Corasick automaton. This roughly corresponds to the standard
/// formulation described in textbooks, with some tweaks to support leftmost
/// searching.
#[derive(Debug)]
struct Compiler<'a> {
    builder: &'a Builder,
    prefilter: prefilter::Builder,
    nfa: NFA,
    byteset: ByteClassSet,
}

impl<'a> Compiler<'a> {
    fn new(builder: &'a Builder) -> Result<Compiler<'a>, BuildError> {
        let prefilter = prefilter::Builder::new(builder.match_kind)
            .ascii_case_insensitive(builder.ascii_case_insensitive);
        Ok(Compiler {
            builder,
            prefilter,
            nfa: NFA {
                match_kind: builder.match_kind,
                states: vec![],
                pattern_lens: vec![],
                prefilter: None,
                byte_classes: ByteClasses::singletons(),
                min_pattern_len: usize::MAX,
                max_pattern_len: 0,
                special: Special::zero(),
                memory_usage: 0,
            },
            byteset: ByteClassSet::empty(),
        })
    }

    fn compile<I, P>(mut self, patterns: I) -> Result<NFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        // the dead state, only used for leftmost and fixed to id==0
        self.add_state(0)?;
        // the fail state, which is never entered and fixed to id==1
        self.add_state(0)?;
        // unanchored start state, initially fixed to id==2 but later shuffled
        // to appear after all non-start match states.
        self.nfa.special.start_unanchored_id = self.add_state(0)?;
        // anchored start state, initially fixed to id==3 but later shuffled
        // to appear after unanchored start state.
        self.nfa.special.start_anchored_id = self.add_state(0)?;
        // Initialize the unanchored starting state in order to make it dense,
        // and thus make transition lookups on this state faster.
        self.init_unanchored_start_state();
        // Build the base trie from the given patterns.
        self.build_trie(patterns)?;
        // Add transitions (and maybe matches) to the anchored starting state.
        // The anchored starting state is used for anchored searches. The only
        // mechanical difference between it and the unanchored start state is
        // that missing transitions map to the DEAD state instead of the FAIL
        // state.
        self.set_anchored_start_state();
        // Rewrite transitions to the FAIL state on the unanchored start state
        // as self-transitions. This keeps the start state active at all times.
        self.add_unanchored_start_state_loop();
        // Set all transitions on the DEAD state to point to itself. This way,
        // the DEAD state can never be escaped. It MUST be used as a sentinel
        // in any correct search.
        self.add_dead_state_loop();
        // The meat of the Aho-Corasick algorithm: compute and write failure
        // transitions. i.e., the state to move to when a transition isn't
        // defined in the current state. These are epsilon transitions and thus
        // make this formulation an NFA.
        self.fill_failure_transitions();
        // Handle a special case under leftmost semantics when at least one
        // of the patterns is the empty string.
        self.close_start_state_loop_for_leftmost();
        // Shuffle states so that we have DEAD, FAIL, MATCH, ..., START, START,
        // NON-MATCH, ... This permits us to very quickly query the type of
        // the state we're currently in during a search.
        self.shuffle();
        // Turn our set of bytes into equivalent classes. This NFA
        // implementation doesn't use byte classes directly, but any
        // Aho-Corasick searcher built from this one might.
        self.nfa.byte_classes = self.byteset.byte_classes();
        self.nfa.prefilter = self.prefilter.build();
        self.calculate_memory_usage();
        // Store the maximum ID of all *relevant* special states. Start states
        // are only relevant when we have a prefilter, otherwise, there is zero
        // reason to care about whether a state is a start state or not during
        // a search. Indeed, without a prefilter, we are careful to explicitly
        // NOT care about start states, otherwise the search can ping pong
        // between the unrolled loop and the handling of special-status states
        // and destroy perf.
        self.nfa.special.max_special_id = if self.nfa.prefilter.is_some() {
            // Why the anchored starting state? Because we always put it
            // after the unanchored starting state and it is therefore the
            // maximum. Why put unanchored followed by anchored? No particular
            // reason, but that's how the states are logically organized in the
            // Thompson NFA implementation found in regex-automata. ¯\_(ツ)_/¯
            self.nfa.special.start_anchored_id
        } else {
            self.nfa.special.max_match_id
        };
        Ok(self.nfa)
    }

    /// This sets up the initial prefix trie that makes up the Aho-Corasick
    /// automaton. Effectively, it creates the basic structure of the
    /// automaton, where every pattern given has a path from the start state to
    /// the end of the pattern.
    fn build_trie<I, P>(&mut self, patterns: I) -> Result<(), BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        'PATTERNS: for (i, pat) in patterns.into_iter().enumerate() {
            let pid = PatternID::new(i).map_err(|e| {
                BuildError::pattern_id_overflow(
                    PatternID::MAX.as_u64(),
                    e.attempted(),
                )
            })?;
            let pat = pat.as_ref();
            let patlen = SmallIndex::new(pat.len())
                .map_err(|_| BuildError::pattern_too_long(pid, pat.len()))?;
            self.nfa.min_pattern_len =
                core::cmp::min(self.nfa.min_pattern_len, pat.len());
            self.nfa.max_pattern_len =
                core::cmp::max(self.nfa.max_pattern_len, pat.len());
            assert_eq!(
                i,
                self.nfa.pattern_lens.len(),
                "expected number of patterns to match pattern ID"
            );
            self.nfa.pattern_lens.push(patlen);
            // We add the pattern to the prefilter here because the pattern
            // ID in the prefilter is determined with respect to the patterns
            // added to the prefilter. That is, it isn't the ID we have here,
            // but the one determined by its own accounting of patterns.
            // To ensure they line up, we add every pattern we see to the
            // prefilter, even if some patterns ultimately are impossible to
            // match (in leftmost-first semantics specifically).
            //
            // Another way of doing this would be to expose an API in the
            // prefilter to permit setting your own pattern IDs. Or to just use
            // our own map and go between them. But this case is sufficiently
            // rare that we don't bother and just make sure they're in sync.
            if self.builder.prefilter {
                self.prefilter.add(pat);
            }

            let mut prev = self.nfa.special.start_unanchored_id;
            let mut saw_match = false;
            for (depth, &b) in pat.iter().enumerate() {
                // When leftmost-first match semantics are requested, we
                // specifically stop adding patterns when a previously added
                // pattern is a prefix of it. We avoid adding it because
                // leftmost-first semantics imply that the pattern can never
                // match. This is not just an optimization to save space! It
                // is necessary for correctness. In fact, this is the only
                // difference in the automaton between the implementations for
                // leftmost-first and leftmost-longest.
                saw_match = saw_match || self.nfa.states[prev].is_match();
                if self.builder.match_kind.is_leftmost_first() && saw_match {
                    // Skip to the next pattern immediately. This avoids
                    // incorrectly adding a match after this loop terminates.
                    continue 'PATTERNS;
                }

                // Add this byte to our equivalence classes. We don't use these
                // for NFA construction. These are instead used only if we're
                // building a DFA. They would technically be useful for the
                // NFA, but it would require a second pass over the patterns.
                self.byteset.set_range(b, b);
                if self.builder.ascii_case_insensitive {
                    let b = opposite_ascii_case(b);
                    self.byteset.set_range(b, b);
                }

                // If the transition from prev using the current byte already
                // exists, then just move through it. Otherwise, add a new
                // state. We track the depth here so that we can determine
                // how to represent transitions. States near the start state
                // use a dense representation that uses more memory but is
                // faster. Other states use a sparse representation that uses
                // less memory but is slower.
                let next = self.nfa.states[prev].next_state(b);
                if next != NFA::FAIL {
                    prev = next;
                } else {
                    let next = self.add_state(depth)?;
                    self.nfa.states[prev].set_next_state(b, next);
                    if self.builder.ascii_case_insensitive {
                        let b = opposite_ascii_case(b);
                        self.nfa.states[prev].set_next_state(b, next);
                    }
                    prev = next;
                }
            }
            // Once the pattern has been added, log the match in the final
            // state that it reached.
            self.nfa.states[prev].matches.push(pid);
        }
        Ok(())
    }

    /// This routine creates failure transitions according to the standard
    /// textbook formulation of the Aho-Corasick algorithm, with a couple small
    /// tweaks to support "leftmost" semantics.
    ///
    /// Building failure transitions is the most interesting part of building
    /// the Aho-Corasick automaton, because they are what allow searches to
    /// be performed in linear time. Specifically, a failure transition is
    /// a single transition associated with each state that points back to
    /// the longest proper suffix of the pattern being searched. The failure
    /// transition is followed whenever there exists no transition on the
    /// current state for the current input byte. If there is no other proper
    /// suffix, then the failure transition points back to the starting state.
    ///
    /// For example, let's say we built an Aho-Corasick automaton with the
    /// following patterns: 'abcd' and 'cef'. The trie looks like this:
    ///
    /// ```ignore
    ///          a - S1 - b - S2 - c - S3 - d - S4*
    ///         /
    ///     S0 - c - S5 - e - S6 - f - S7*
    /// ```
    ///
    /// At this point, it should be fairly straight-forward to see how this
    /// trie can be used in a simplistic way. At any given position in the
    /// text we're searching (called the "subject" string), all we need to do
    /// is follow the transitions in the trie by consuming one transition for
    /// each byte in the subject string. If we reach a match state, then we can
    /// report that location as a match.
    ///
    /// The trick comes when searching a subject string like 'abcef'. We'll
    /// initially follow the transition from S0 to S1 and wind up in S3 after
    /// observng the 'c' byte. At this point, the next byte is 'e' but state
    /// S3 has no transition for 'e', so the search fails. We then would need
    /// to restart the search at the next position in 'abcef', which
    /// corresponds to 'b'. The match would fail, but the next search starting
    /// at 'c' would finally succeed. The problem with this approach is that
    /// we wind up searching the subject string potentially many times. In
    /// effect, this makes the algorithm have worst case `O(n * m)` complexity,
    /// where `n ~ len(subject)` and `m ~ len(all patterns)`. We would instead
    /// like to achieve a `O(n + m)` worst case complexity.
    ///
    /// This is where failure transitions come in. Instead of dying at S3 in
    /// the first search, the automaton can instruct the search to move to
    /// another part of the automaton that corresponds to a suffix of what
    /// we've seen so far. Recall that we've seen 'abc' in the subject string,
    /// and the automaton does indeed have a non-empty suffix, 'c', that could
    /// potentially lead to another match. Thus, the actual Aho-Corasick
    /// automaton for our patterns in this case looks like this:
    ///
    /// ```ignore
    ///          a - S1 - b - S2 - c - S3 - d - S4*
    ///         /                      /
    ///        /       ----------------
    ///       /       /
    ///     S0 - c - S5 - e - S6 - f - S7*
    /// ```
    ///
    /// That is, we have a failure transition from S3 to S5, which is followed
    /// exactly in cases when we are in state S3 but see any byte other than
    /// 'd' (that is, we've "failed" to find a match in this portion of our
    /// trie). We know we can transition back to S5 because we've already seen
    /// a 'c' byte, so we don't need to re-scan it. We can then pick back up
    /// with the search starting at S5 and complete our match.
    ///
    /// Adding failure transitions to a trie is fairly simple, but subtle. The
    /// key issue is that you might have multiple failure transition that you
    /// need to follow. For example, look at the trie for the patterns
    /// 'abcd', 'b', 'bcd' and 'cd':
    ///
    /// ```ignore
    ///          - a - S1 - b - S2* - c - S3 - d - S4*
    ///         /               /         /
    ///        /         -------   -------
    ///       /         /         /
    ///     S0 --- b - S5* - c - S6 - d - S7*
    ///       \                  /
    ///        \         --------
    ///         \       /
    ///          - c - S8 - d - S9*
    /// ```
    ///
    /// The failure transitions for this trie are defined from S2 to S5,
    /// S3 to S6 and S6 to S8. Moreover, state S2 needs to track that it
    /// corresponds to a match, since its failure transition to S5 is itself
    /// a match state.
    ///
    /// Perhaps simplest way to think about adding these failure transitions
    /// is recursively. That is, if you know the failure transitions for every
    /// possible previous state that could be visited (e.g., when computing the
    /// failure transition for S3, you already know the failure transitions
    /// for S0, S1 and S2), then you can simply follow the failure transition
    /// of the previous state and check whether the incoming transition is
    /// defined after following the failure transition.
    ///
    /// For example, when determining the failure state for S3, by our
    /// assumptions, we already know that there is a failure transition from
    /// S2 (the previous state) to S5. So we follow that transition and check
    /// whether the transition connecting S2 to S3 is defined. Indeed, it is,
    /// as there is a transition from S5 to S6 for the byte 'c'. If no such
    /// transition existed, we could keep following the failure transitions
    /// until we reach the start state, which is the failure transition for
    /// every state that has no corresponding proper suffix.
    ///
    /// We don't actually use recursion to implement this, but instead, use a
    /// breadth first search of the automaton. Our base case is the start
    /// state, whose failure transition is just a transition to itself.
    ///
    /// When building a leftmost automaton, we proceed as above, but only
    /// include a subset of failure transitions. Namely, we omit any failure
    /// transitions that appear after a match state in the trie. This is
    /// because failure transitions always point back to a proper suffix of
    /// what has been seen so far. Thus, following a failure transition after
    /// a match implies looking for a match that starts after the one that has
    /// already been seen, which is of course therefore not the leftmost match.
    ///
    /// N.B. I came up with this algorithm on my own, and after scouring all of
    /// the other AC implementations I know of (Perl, Snort, many on GitHub).
    /// I couldn't find any that implement leftmost semantics like this.
    /// Perl of course needs leftmost-first semantics, but they implement it
    /// with a seeming hack at *search* time instead of encoding it into the
    /// automaton. There are also a couple Java libraries that support leftmost
    /// longest semantics, but they do it by building a queue of matches at
    /// search time, which is even worse than what Perl is doing. ---AG
    fn fill_failure_transitions(&mut self) {
        let is_leftmost = self.builder.match_kind.is_leftmost();
        let start_uid = self.nfa.special.start_unanchored_id;
        // Initialize the queue for breadth first search with all transitions
        // out of the start state. We handle the start state specially because
        // we only want to follow non-self transitions. If we followed self
        // transitions, then this would never terminate.
        let mut queue = VecDeque::new();
        let mut seen = self.queued_set();
        for i in 0..self.nfa.states[start_uid].trans.len() {
            let (_, next) = self.nfa.states[start_uid].trans[i];
            // Skip anything we've seen before and any self-transitions on the
            // start state.
            if next == start_uid || seen.contains(next) {
                continue;
            }
            queue.push_back(next);
            seen.insert(next);
            // Under leftmost semantics, if a state immediately following
            // the start state is a match state, then we never want to
            // follow its failure transition since the failure transition
            // necessarily leads back to the start state, which we never
            // want to do for leftmost matching after a match has been
            // found.
            //
            // We apply the same logic to non-start states below as well.
            if is_leftmost && self.nfa.states[next].is_match() {
                self.nfa.states[next].fail = NFA::DEAD;
            }
        }
        while let Some(id) = queue.pop_front() {
            for i in 0..self.nfa.states[id].trans.len() {
                let (b, next) = self.nfa.states[id].trans[i];
                if seen.contains(next) {
                    // The only way to visit a duplicate state in a transition
                    // list is when ASCII case insensitivity is enabled. In
                    // this case, we want to skip it since it's redundant work.
                    // But it would also end up duplicating matches, which
                    // results in reporting duplicate matches in some cases.
                    // See the 'acasei010' regression test.
                    continue;
                }
                queue.push_back(next);
                seen.insert(next);

                // As above for start states, under leftmost semantics, once
                // we see a match all subsequent states should have no failure
                // transitions because failure transitions always imply looking
                // for a match that is a suffix of what has been seen so far
                // (where "seen so far" corresponds to the string formed by
                // following the transitions from the start state to the
                // current state). Under leftmost semantics, we specifically do
                // not want to allow this to happen because we always want to
                // report the match found at the leftmost position.
                //
                // The difference between leftmost-first and leftmost-longest
                // occurs previously while we build the trie. For
                // leftmost-first, we simply omit any entries that would
                // otherwise require passing through a match state.
                //
                // Note that for correctness, the failure transition has to be
                // set to the dead state for ALL states following a match, not
                // just the match state itself. However, by setting the failure
                // transition to the dead state on all match states, the dead
                // state will automatically propagate to all subsequent states
                // via the failure state computation below.
                if is_leftmost && self.nfa.states[next].is_match() {
                    self.nfa.states[next].fail = NFA::DEAD;
                    continue;
                }
                let mut fail = self.nfa.states[id].fail;
                while self.nfa.states[fail].next_state(b) == NFA::FAIL {
                    fail = self.nfa.states[fail].fail;
                }
                fail = self.nfa.states[fail].next_state(b);
                self.nfa.states[next].fail = fail;
                self.copy_matches(fail, next);
            }
            // If the start state is a match state, then this automaton can
            // match the empty string. This implies all states are match states
            // since every position matches the empty string, so copy the
            // matches from the start state to every state. Strictly speaking,
            // this is only necessary for overlapping matches since each
            // non-empty non-start match state needs to report empty matches
            // in addition to its own. For the non-overlapping case, such
            // states only report the first match, which is never empty since
            // it isn't a start state.
            if !is_leftmost {
                self.copy_matches(self.nfa.special.start_unanchored_id, id);
            }
        }
    }

    /// Shuffle the states so that they appear in this sequence:
    ///
    ///   DEAD, FAIL, MATCH..., START, START, NON-MATCH...
    ///
    /// The idea here is that if we know how special states are laid out in our
    /// transition table, then we can determine what "kind" of state we're in
    /// just by comparing our current state ID with a particular value. In this
    /// way, we avoid doing extra memory lookups.
    ///
    /// Before shuffling begins, our states look something like this:
    ///
    ///   DEAD, FAIL, START, START, (MATCH | NON-MATCH)...
    ///
    /// So all we need to do is move all of the MATCH states so that they
    /// all appear before any NON-MATCH state, like so:
    ///
    ///   DEAD, FAIL, START, START, MATCH... NON-MATCH...
    ///
    /// Then it's just a simple matter of swapping the two START states with
    /// the last two MATCH states.
    ///
    /// (This is the same technique used for fully compiled DFAs in
    /// regex-automata.)
    fn shuffle(&mut self) {
        let old_start_uid = self.nfa.special.start_unanchored_id;
        let old_start_aid = self.nfa.special.start_anchored_id;
        assert!(old_start_uid < old_start_aid);
        assert_eq!(
            3,
            old_start_aid.as_usize(),
            "anchored start state should be at index 3"
        );
        // We implement shuffling by a sequence of pairwise swaps of states.
        // Since we have a number of things referencing states via their
        // IDs and swapping them changes their IDs, we need to record every
        // swap we make so that we can remap IDs. The remapper handles this
        // book-keeping for us.
        let mut remapper = Remapper::new(&self.nfa, 0);
        // The way we proceed here is by moving all match states so that
        // they directly follow the start states. So it will go: DEAD, FAIL,
        // START-UNANCHORED, START-ANCHORED, MATCH, ..., NON-MATCH, ...
        //
        // To do that, we proceed forward through all states after
        // START-ANCHORED and swap match states so that they appear before all
        // non-match states.
        let mut next_avail = StateID::from(4u8);
        for i in next_avail.as_usize()..self.nfa.states.len() {
            let sid = StateID::new(i).unwrap();
            if !self.nfa.states[sid].is_match() {
                continue;
            }
            remapper.swap(&mut self.nfa, sid, next_avail);
            // The key invariant here is that only non-match states exist
            // between 'next_avail' and 'sid' (with them being potentially
            // equivalent). Thus, incrementing 'next_avail' by 1 is guaranteed
            // to land on the leftmost non-match state. (Unless 'next_avail'
            // and 'sid' are equivalent, in which case, a swap will occur but
            // it is a no-op.)
            next_avail = StateID::new(next_avail.one_more()).unwrap();
        }
        // Now we'd like to move the start states to immediately following the
        // match states. (The start states may themselves be match states, but
        // we'll handle that later.) We arrange the states this way so that we
        // don't necessarily need to check whether a state is a start state or
        // not before checking whether a state is a match state. For example,
        // we'd like to be able to write this as our state machine loop:
        //
        //   sid = start()
        //   for byte in haystack:
        //     sid = next(sid, byte)
        //     if sid <= nfa.max_start_id:
        //       if sid <= nfa.max_dead_id:
        //         # search complete
        //       elif sid <= nfa.max_match_id:
        //         # found match
        //
        // The important context here is that we might not want to look for
        // start states at all. Namely, if a searcher doesn't have a prefilter,
        // then there is no reason to care about whether we're in a start state
        // or not. And indeed, if we did check for it, this very hot loop would
        // ping pong between the special state handling and the main state
        // transition logic. This in turn stalls the CPU by killing branch
        // prediction.
        //
        // So essentially, we really want to be able to "forget" that start
        // states even exist and this is why we put them at the end.
        let new_start_aid =
            StateID::new(next_avail.as_usize().checked_sub(1).unwrap())
                .unwrap();
        remapper.swap(&mut self.nfa, old_start_aid, new_start_aid);
        let new_start_uid =
            StateID::new(next_avail.as_usize().checked_sub(2).unwrap())
                .unwrap();
        remapper.swap(&mut self.nfa, old_start_uid, new_start_uid);
        let new_max_match_id =
            StateID::new(next_avail.as_usize().checked_sub(3).unwrap())
                .unwrap();
        self.nfa.special.max_match_id = new_max_match_id;
        self.nfa.special.start_unanchored_id = new_start_uid;
        self.nfa.special.start_anchored_id = new_start_aid;
        // If one start state is a match state, then they both are.
        if self.nfa.states[self.nfa.special.start_anchored_id].is_match() {
            self.nfa.special.max_match_id = self.nfa.special.start_anchored_id;
        }
        remapper.remap(&mut self.nfa);
    }

    /// Returns a set that tracked queued states.
    ///
    /// This is only necessary when ASCII case insensitivity is enabled, since
    /// it is the only way to visit the same state twice. Otherwise, this
    /// returns an inert set that nevers adds anything and always reports
    /// `false` for every member test.
    fn queued_set(&self) -> QueuedSet {
        if self.builder.ascii_case_insensitive {
            QueuedSet::active()
        } else {
            QueuedSet::inert()
        }
    }

    /// Initializes the unanchored start state by making it dense. This is
    /// achieved by explicitly setting every transition to the FAIL state.
    /// This isn't necessary for correctness, since any missing transition is
    /// automatically assumed to be mapped to the FAIL state. We do this to
    /// make the unanchored starting state dense, and thus in turn make
    /// transition lookups on it faster. (Which is worth doing because it's
    /// the most active state.)
    fn init_unanchored_start_state(&mut self) {
        let start_uid = self.nfa.special.start_unanchored_id;
        for byte in 0..=255 {
            self.nfa.states[start_uid].set_next_state(byte, NFA::FAIL);
        }
    }

    /// Setup the anchored start state by copying all of the transitions and
    /// matches from the unanchored starting state with one change: the failure
    /// transition is changed to the DEAD state, so that for any undefined
    /// transitions, the search will stop.
    fn set_anchored_start_state(&mut self) {
        let start_uid = self.nfa.special.start_unanchored_id;
        let start_aid = self.nfa.special.start_anchored_id;
        self.nfa.states[start_aid].trans =
            self.nfa.states[start_uid].trans.clone();
        self.copy_matches(start_uid, start_aid);
        // This is the main difference between the unanchored and anchored
        // starting states. If a lookup on an anchored starting state fails,
        // then the search should stop.
        //
        // N.B. This assumes that the loop on the unanchored starting state
        // hasn't been created yet.
        self.nfa.states[start_aid].fail = NFA::DEAD;
    }

    /// Set the failure transitions on the start state to loop back to the
    /// start state. This effectively permits the Aho-Corasick automaton to
    /// match at any position. This is also required for finding the next
    /// state to terminate, namely, finding the next state should never return
    /// a fail_id.
    ///
    /// This must be done after building the initial trie, since trie
    /// construction depends on transitions to `fail_id` to determine whether a
    /// state already exists or not.
    fn add_unanchored_start_state_loop(&mut self) {
        let start_uid = self.nfa.special.start_unanchored_id;
        let start = &mut self.nfa.states[start_uid];
        for b in 0..=255 {
            if start.next_state(b) == NFA::FAIL {
                start.set_next_state(b, start_uid);
            }
        }
    }

    /// Remove the start state loop by rewriting any transitions on the start
    /// state back to the start state with transitions to the dead state.
    ///
    /// The loop is only closed when two conditions are met: the start state
    /// is a match state and the match kind is leftmost-first or
    /// leftmost-longest.
    ///
    /// The reason for this is that under leftmost semantics, a start state
    /// that is also a match implies that we should never restart the search
    /// process. We allow normal transitions out of the start state, but if
    /// none exist, we transition to the dead state, which signals that
    /// searching should stop.
    fn close_start_state_loop_for_leftmost(&mut self) {
        let start_uid = self.nfa.special.start_unanchored_id;
        let start = &mut self.nfa.states[start_uid];
        if self.builder.match_kind.is_leftmost() && start.is_match() {
            for b in 0..=255 {
                if start.next_state(b) == start_uid {
                    start.set_next_state(b, NFA::DEAD);
                }
            }
        }
    }

    /// Sets all transitions on the dead state to point back to the dead state.
    /// Normally, missing transitions map back to the failure state, but the
    /// point of the dead state is to act as a sink that can never be escaped.
    fn add_dead_state_loop(&mut self) {
        let dead = &mut self.nfa.states[NFA::DEAD];
        for b in 0..=255 {
            dead.set_next_state(b, NFA::DEAD);
        }
    }

    /// Copy matches from the `src` state to the `dst` state. This is useful
    /// when a match state can be reached via a failure transition. In which
    /// case, you'll want to copy the matches (if any) from the state reached
    /// by the failure transition to the original state you were at.
    fn copy_matches(&mut self, src: StateID, dst: StateID) {
        let (src, dst) =
            get_two_mut(&mut self.nfa.states, src.as_usize(), dst.as_usize());
        dst.matches.extend_from_slice(&src.matches);
    }

    /// Allocate and add a fresh state to the underlying NFA and return its
    /// ID (guaranteed to be one more than the ID of the previously allocated
    /// state). If the ID would overflow `StateID`, then this returns an error.
    fn add_state(&mut self, depth: usize) -> Result<StateID, BuildError> {
        // This is OK because we error when building the trie if we see a
        // pattern whose length cannot fit into a 'SmallIndex', and the longest
        // possible depth corresponds to the length of the longest pattern.
        let depth = SmallIndex::new(depth)
            .expect("patterns longer than SmallIndex::MAX are not allowed");
        let id = StateID::new(self.nfa.states.len()).map_err(|e| {
            BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted())
        })?;
        self.nfa.states.push(State {
            trans: vec![],
            matches: vec![],
            fail: self.nfa.special.start_unanchored_id,
            depth,
        });
        Ok(id)
    }

    /// Computes the total amount of heap used by this NFA in bytes.
    fn calculate_memory_usage(&mut self) {
        use core::mem::size_of;

        for state in self.nfa.states.iter() {
            self.nfa.memory_usage += size_of::<State>() + state.memory_usage();
        }
    }
}

/// A set of state identifiers used to avoid revisiting the same state multiple
/// times when filling in failure transitions.
///
/// This set has an "inert" and an "active" mode. When inert, the set never
/// stores anything and always returns `false` for every member test. This is
/// useful to avoid the performance and memory overhead of maintaining this
/// set when it is not needed.
#[derive(Debug)]
struct QueuedSet {
    set: Option<BTreeSet<StateID>>,
}

impl QueuedSet {
    /// Return an inert set that returns `false` for every state ID membership
    /// test.
    fn inert() -> QueuedSet {
        QueuedSet { set: None }
    }

    /// Return an active set that tracks state ID membership.
    fn active() -> QueuedSet {
        QueuedSet { set: Some(BTreeSet::new()) }
    }

    /// Inserts the given state ID into this set. (If the set is inert, then
    /// this is a no-op.)
    fn insert(&mut self, state_id: StateID) {
        if let Some(ref mut set) = self.set {
            set.insert(state_id);
        }
    }

    /// Returns true if and only if the given state ID is in this set. If the
    /// set is inert, this always returns false.
    fn contains(&self, state_id: StateID) -> bool {
        match self.set {
            None => false,
            Some(ref set) => set.contains(&state_id),
        }
    }
}

impl core::fmt::Debug for NFA {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use crate::automaton::fmt_state_indicator;

        writeln!(f, "noncontiguous::NFA(")?;
        for (sid, state) in self.states.iter().with_state_ids() {
            // The FAIL state doesn't actually have space for a state allocated
            // for it, so we have to treat it as a special case.
            if sid == NFA::FAIL {
                writeln!(f, "F {:06}:", sid.as_usize())?;
                continue;
            }
            fmt_state_indicator(f, self, sid)?;
            write!(
                f,
                "{:06}({:06}): ",
                sid.as_usize(),
                state.fail.as_usize()
            )?;
            state.fmt(f)?;
            write!(f, "\n")?;
            if self.is_match(sid) {
                write!(f, "         matches: ")?;
                for (i, pid) in state.matches.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", pid.as_usize())?;
                }
                write!(f, "\n")?;
            }
        }
        writeln!(f, "match kind: {:?}", self.match_kind)?;
        writeln!(f, "prefilter: {:?}", self.prefilter.is_some())?;
        writeln!(f, "state length: {:?}", self.states.len())?;
        writeln!(f, "pattern length: {:?}", self.patterns_len())?;
        writeln!(f, "shortest pattern length: {:?}", self.min_pattern_len)?;
        writeln!(f, "longest pattern length: {:?}", self.max_pattern_len)?;
        writeln!(f, "memory usage: {:?}", self.memory_usage())?;
        writeln!(f, ")")?;
        Ok(())
    }
}

/// Safely return two mutable borrows to two different locations in the given
/// slice.
///
/// This panics if i == j.
fn get_two_mut<T>(xs: &mut [T], i: usize, j: usize) -> (&mut T, &mut T) {
    assert!(i != j, "{} must not be equal to {}", i, j);
    if i < j {
        let (before, after) = xs.split_at_mut(j);
        (&mut before[i], &mut after[0])
    } else {
        let (before, after) = xs.split_at_mut(i);
        (&mut after[0], &mut before[j])
    }
}
