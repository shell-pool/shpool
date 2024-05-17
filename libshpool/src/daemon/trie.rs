use std::{collections::HashMap, hash};

#[derive(Debug)]
pub struct Trie<Sym, V, TT> {
    // The nodes which form the tree. The first node is the root
    // node, afterwards the order is undefined.
    nodes: Vec<TrieNode<Sym, V, TT>>,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum TrieCursor {
    /// A cursor to use to start a char-wise match
    Start,
    /// Represents a state in the middle or end of a match
    Match { idx: usize, is_partial: bool },
    /// A terminal state indicating a failure to match
    NoMatch,
}

#[derive(Debug)]
pub struct TrieNode<Sym, V, TT> {
    // We need to store a phantom symbol here so we can have the
    // Sym type parameter available for the TrieTab trait constraint
    // in the impl block. Apologies for the type tetris.
    phantom: std::marker::PhantomData<Sym>,
    value: Option<V>,
    tab: TT,
}

impl<Sym, V, TT> Trie<Sym, V, TT>
where
    TT: TrieTab<Sym>,
    Sym: Copy,
{
    pub fn new() -> Self {
        Trie { nodes: vec![TrieNode::new(None)] }
    }

    /// Insert a seq, value pair into the trie
    pub fn insert<Seq: Iterator<Item = Sym>>(&mut self, seq: Seq, value: V) {
        let mut current_node = 0;
        for sym in seq {
            current_node = if let Some(next_node) = self.nodes[current_node].tab.get(sym) {
                *next_node
            } else {
                let idx = self.nodes.len();
                self.nodes.push(TrieNode::new(None));
                self.nodes[current_node].tab.set(sym, idx);
                idx
            };
        }
        self.nodes[current_node].value = Some(value);
    }

    /// Check if the given sequence exists in the trie, used by tests.
    #[allow(dead_code)]
    pub fn contains<Seq: Iterator<Item = Sym>>(&self, seq: Seq) -> bool {
        let mut match_state = TrieCursor::Start;
        for sym in seq {
            match_state = self.advance(match_state, sym);
            if let TrieCursor::NoMatch = match_state {
                return false;
            }
        }
        if let TrieCursor::Start = match_state {
            return self.nodes[0].value.is_some();
        }

        if let TrieCursor::Match { is_partial, .. } = match_state { !is_partial } else { false }
    }

    /// Process a single token of input, returning the current state.
    /// To start a new match, use TrieCursor::Start.
    pub fn advance(&self, cursor: TrieCursor, sym: Sym) -> TrieCursor {
        let node = match cursor {
            TrieCursor::Start => &self.nodes[0],
            TrieCursor::Match { idx, .. } => &self.nodes[idx],
            TrieCursor::NoMatch => return TrieCursor::NoMatch,
        };

        if let Some(idx) = node.tab.get(sym) {
            TrieCursor::Match { idx: *idx, is_partial: self.nodes[*idx].value.is_none() }
        } else {
            TrieCursor::NoMatch
        }
    }

    /// Get the value for a match cursor.
    pub fn get(&self, cursor: TrieCursor) -> Option<&V> {
        if let TrieCursor::Match { idx, .. } = cursor {
            self.nodes[idx].value.as_ref()
        } else {
            None
        }
    }
}

impl<Sym, V, TT> TrieNode<Sym, V, TT>
where
    TT: TrieTab<Sym>,
{
    fn new(value: Option<V>) -> Self {
        TrieNode { phantom: std::marker::PhantomData, value, tab: TT::new() }
    }
}

/// The backing table the trie uses to associate symbols with state
/// indexes. This is basically `std::ops::IndexMut` plus a `new` function.
/// We can't just make this a sub-trait of `IndexMut` because u8 does
/// not implement IndexMut for vectors.
pub trait TrieTab<Idx> {
    fn new() -> Self;
    fn get(&self, index: Idx) -> Option<&usize>;
    fn set(&mut self, index: Idx, elem: usize);
}

impl<Sym> TrieTab<Sym> for HashMap<Sym, usize>
where
    Sym: hash::Hash + Eq + PartialEq,
{
    fn new() -> Self {
        HashMap::new()
    }

    fn get(&self, index: Sym) -> Option<&usize> {
        self.get(&index)
    }

    fn set(&mut self, index: Sym, elem: usize) {
        self.insert(index, elem);
    }
}

impl TrieTab<u8> for Vec<Option<usize>> {
    fn new() -> Self {
        vec![None; u8::MAX as usize]
    }

    fn get(&self, index: u8) -> Option<&usize> {
        self[index as usize].as_ref()
    }

    fn set(&mut self, index: u8, elem: usize) {
        self[index as usize] = Some(elem)
    }
}
