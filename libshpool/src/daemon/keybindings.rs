// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The keybindings module implements a system for mapping
//! keybindings to actions. A keybinding is described by
//! a simple language simillar to the one used by other tools,
//! and can be bound to a named action in the config.toml
//! file that shpool loads on startup.
//!
//! One of shpool's design principles is to avoid changing the
//! terminal experience as much as possible, so we try to avoid
//! keybindings, but in the case of detaching from a long-running
//! process, a keybinding is really helpful.
//!
//! ## Keybinding Language
//!
//! The keybinding language has the grammar:
//!
//! ```text
//! sequence ::= chord
//!            | chord ' ' chord
//!
//! chord ::= key
//!         | key '-' chord
//!
//! key ::= mod | sym
//!
//! mod ::= 'Ctrl'
//!
//! sym ::= 'Space' | <lowercase letters> | <numbers>
//! ```
//!
//! chords bind tighter than sequnces. A chord must be pressed all at once
//! while a sequence should have the keys pressed one after another.
//!
//! For now, only fairly limited chords are supported. Chords must either
//! be singletons besides 'Ctrl' or of the form 'Ctrl-x' where
//! x is some non-'Ctrl' key.

use std::{collections::HashMap, fmt};

use anyhow::{anyhow, Context};
use serde_derive::Deserialize;

use super::trie::{Trie, TrieCursor, TrieTab};

//
// Keybindings table
//

// TODO(ethan): should I have some notion of a cooldown time
//              where sequences don't count if they are pressed
//              too slowly?

/// Bindings represents an engine for scanning through user input
/// and occasionally emitting actions that should be acted upon.
pub struct Bindings {
    /// A trie mapping input chunks to all the chords which are part of
    /// our keybindings. We use bytes instead of chars for this trie
    /// because we are going to use it to scan over the raw user input
    /// stream without first parsing that stream into utf8 (since it
    /// might not be utf8).
    chords: Trie<u8, ChordAtom, Vec<Option<usize>>>,
    /// The current match state in the chords trie.
    chords_cursor: TrieCursor,
    /// A trie mapping all the sequence keybindings to actions which
    /// should be performed in response to the sequence.
    sequences: Trie<ChordAtom, Action, Vec<Option<usize>>>,
    /// The current match state in the sequences trie.
    sequences_cursor: TrieCursor,
}

/// The result of advancing the binding engine by a single byte.
#[derive(Debug, Eq, PartialEq)]
pub enum BindingResult {
    NoMatch,
    Partial,
    Match(Action),
}

/// A ChordAtom is a lightweight type that represents a Chord within
/// the keybinding maching engine. We could just directly use chords,
/// but they are fairly heavy nested data structures, and we want our
/// inner match loop to be able to rip through bytes as fast as possible,
/// so we instead map all the chords seen when a Bindings is compiled
/// into a dense set of integers.
#[derive(Eq, PartialEq, Copy, Clone, Hash)]
struct ChordAtom(u8);

impl TrieTab<ChordAtom> for Vec<Option<usize>> {
    fn new() -> Self {
        vec![None; u8::MAX as usize]
    }

    fn get(&self, index: ChordAtom) -> Option<&usize> {
        self[index.0 as usize].as_ref()
    }

    fn set(&mut self, index: ChordAtom, elem: usize) {
        self[index.0 as usize] = Some(elem)
    }
}

impl Bindings {
    /// new builds a bindings matching engine, parsing the given binding->action
    /// mapping and compiling it into the pair of tries that we use to perform
    /// online keybinding matching.
    pub fn new<'a, B: IntoIterator<Item = (&'a str, Action)>>(bindings: B) -> anyhow::Result<Self> {
        let mut chords = Trie::new();
        let mut sequences = Trie::new();

        let mut chord_atom_counter: usize = 0;
        let mut chord_atom_tab = HashMap::new();

        let tokenizer = Lexer::new();
        for (binding_src, action) in bindings.into_iter() {
            let tokens =
                tokenizer.tokenize(binding_src.chars()).context("tokenizing keybinding")?;
            let sequence = parse(tokens).context("parsing keybinding")?;
            for chord in sequence.0.iter() {
                // resolving the key code will also check the validity
                let code = chord.key_code()?;

                let chord_atom = chord_atom_tab.entry(chord.clone()).or_insert_with(|| {
                    let atom = ChordAtom(chord_atom_counter as u8);
                    chord_atom_counter += 1;
                    atom
                });
                if chord_atom_counter >= u8::MAX as usize {
                    return Err(anyhow!(
                        "shpool only supports up to {} unique chords at a time",
                        u8::MAX
                    ));
                }

                chords.insert(vec![code].into_iter(), *chord_atom);
            }
            sequences
                .insert(sequence.0.iter().map(|chord| *chord_atom_tab.get(chord).unwrap()), action);
        }

        Ok(Bindings {
            chords,
            chords_cursor: TrieCursor::Start,
            sequences,
            sequences_cursor: TrieCursor::Start,
        })
    }

    /// transition takes the next byte in an input stream and mutates the
    /// bindings engine while possibly emitting an action that the caller
    /// should perform in response to a keybinding that has just been completed.
    pub fn transition(&mut self, byte: u8) -> BindingResult {
        self.chords_cursor = self.chords.advance(self.chords_cursor, byte);
        if let Some(chord_atom) = self.chords.get(self.chords_cursor) {
            self.chords_cursor = TrieCursor::Start;

            self.sequences_cursor = self.sequences.advance(self.sequences_cursor, *chord_atom);
            match self.sequences_cursor {
                TrieCursor::Match { is_partial, .. } if is_partial => BindingResult::Partial,
                TrieCursor::Match { .. } => {
                    let cursor = self.sequences_cursor;
                    self.sequences_cursor = TrieCursor::Start;
                    if let Some(action) = self.sequences.get(cursor) {
                        BindingResult::Match(*action)
                    } else {
                        BindingResult::NoMatch
                    }
                }
                _ => {
                    self.sequences_cursor = TrieCursor::Start;
                    BindingResult::NoMatch
                }
            }
        } else {
            match self.chords_cursor {
                TrieCursor::Match { is_partial, .. } if is_partial => BindingResult::Partial,
                _ => {
                    // no match, reset
                    self.sequences_cursor = TrieCursor::Start;
                    self.chords_cursor = TrieCursor::Start;
                    BindingResult::NoMatch
                }
            }
        }
    }
}

#[derive(Eq, PartialEq, Debug, Deserialize, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// detaches the current shpool session
    Detach,
    /// does nothing, useful for testing the keybinding engine and not much else
    NoOp,
}

//
// Parser
//

/// A list of chords that need to be pressed one after another
#[derive(Eq, PartialEq, Debug)]
pub struct Sequence(Vec<Chord>);

/// a list of keys that need to be held down all together
#[derive(Eq, PartialEq, Debug, Hash, Clone)]
pub struct Chord(Vec<String>);

impl Chord {
    /// Make sure the chord is valid.
    ///
    /// Valid forms are:
    ///   sym
    ///   Ctrl-sym
    fn check_valid(&self) -> anyhow::Result<()> {
        for key in self.0.iter() {
            if !Self::is_key(key) {
                return Err(anyhow!("invalid chord: {}: invalid key", self));
            }
        }

        if self.0.len() == 1 {
            if Self::is_ctrl(&self.0[0]) {
                return Err(anyhow!("invalid chord: {}: Ctrl is not a cord", self));
            }
        } else if self.0.len() == 2 {
            if !Self::is_ctrl(&self.0[0]) {
                return Err(anyhow!("invalid chord: {}: Ctrl is the only supported mod key", self));
            }
            if Self::is_ctrl(&self.0[1]) {
                return Err(anyhow!("invalid chord: {}: Ctrl cannot be repeated", self));
            }
        } else {
            return Err(anyhow!("invalid chord: {}", self));
        }
        Ok(())
    }

    /// key_code returns the byte that this chord generates when pressed.
    ///
    /// Eventually, we might want to extend this to support chords that
    /// generate multiple codes, but for now we only support single-code
    /// chords.
    fn key_code(&self) -> anyhow::Result<u8> {
        self.check_valid()?;

        if self.0.len() == 1 && Self::is_sym(&self.0[0]) {
            if self.0[0] == "Space" {
                return Ok(b' ');
            }
            let c = self.0[0].chars().next().unwrap();
            return Ok(c as u32 as u8);
        }

        if self.0.len() == 2 {
            let ctrl_chord = format!("{self}");
            for (chord, code) in CONTROL_CODES.iter() {
                if ctrl_chord == *chord {
                    return Ok(*code);
                }
            }
        }

        Err(anyhow!("unknown key code for chord: {}", self))
    }

    fn is_key(key: &str) -> bool {
        Self::is_ctrl(key) || Self::is_sym(key)
    }

    fn is_ctrl(key: &str) -> bool {
        key == "Ctrl"
    }

    fn is_sym(key: &str) -> bool {
        if key == "Space" {
            return true;
        }

        if matches!(key, "\\" | "[" | "]" | "@" | "^" | "_" | "?") {
            return true;
        }

        if key.len() != 1 {
            return false;
        }

        let c = key.chars().next().unwrap();

        // If we expanded our alphabet size a bit, we can include the
        // uppercase letters using this method if we wanted to.
        c.is_digit(10 + 26)
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.join("-"))?;
        Ok(())
    }
}

fn parse<T: IntoIterator<Item = Token>>(tokens: T) -> anyhow::Result<Sequence> {
    let mut chords = vec![];
    let mut keys = vec![];
    let mut saw_dash = true;
    for token in tokens.into_iter() {
        match token {
            Token::Key(key) => {
                if saw_dash {
                    keys.push(key);
                    saw_dash = false;
                } else {
                    chords.push(Chord(keys.clone()));

                    keys.clear();
                    keys.push(key);
                }
            }
            Token::Dash => {
                if saw_dash {
                    return Err(anyhow!("unexpected DASH token"));
                } else {
                    saw_dash = true;
                }
            }
        }
    }

    if !keys.is_empty() {
        chords.push(Chord(keys));
    }

    Ok(Sequence(chords))
}

//
// Lexer
//

struct Lexer {
    words_trie: Trie<char, (), HashMap<char, usize>>,
}

#[derive(Eq, PartialEq, Debug)]
enum Token {
    Key(String),
    Dash,
}

impl Lexer {
    fn new() -> Self {
        let words = vec!["Ctrl", "Space"];
        let mut words_trie = Trie::new();
        for word in words {
            words_trie.insert(word.chars(), ());
        }
        Lexer { words_trie }
    }

    fn tokenize<S: Iterator<Item = char>>(&self, src: S) -> anyhow::Result<Vec<Token>> {
        let mut tokens = vec![];
        let mut word_chars = vec![];
        let mut cursor = TrieCursor::Start;
        for c in src {
            if c.is_whitespace() {
                continue;
            }

            let new_cursor = self.words_trie.advance(cursor, c);
            match new_cursor {
                TrieCursor::Start => return Err(anyhow!("internal error: trie bug")),
                TrieCursor::NoMatch => {
                    cursor = TrieCursor::Start;

                    word_chars.push(c);
                    for c in word_chars.iter() {
                        match *c {
                            '-' => tokens.push(Token::Dash),
                            '\\' => tokens.push(Token::Key(String::from("\\"))),
                            '[' => tokens.push(Token::Key(String::from("["))),
                            ']' => tokens.push(Token::Key(String::from("]"))),
                            '@' => tokens.push(Token::Key(String::from("@"))),
                            '^' => tokens.push(Token::Key(String::from("^"))),
                            '_' => tokens.push(Token::Key(String::from("_"))),
                            '?' => tokens.push(Token::Key(String::from("?"))),
                            '0'..='9' => tokens.push(Token::Key(String::from(*c))),
                            'a'..='z' => tokens.push(Token::Key(String::from(*c))),
                            _ => return Err(anyhow!("unexpected char: '{}'", *c)),
                        }
                    }
                    word_chars.clear();
                    continue;
                }
                TrieCursor::Match { is_partial, .. } => {
                    word_chars.push(c);
                    if is_partial {
                        cursor = new_cursor;
                    } else {
                        tokens.push(Token::Key(word_chars.iter().collect()));

                        // reset match state
                        cursor = TrieCursor::Start;
                        word_chars.clear();
                        continue;
                    }
                }
            }
        }

        Ok(tokens)
    }
}

//
// Data Tables
//

// This table was generated experimentally by logging the key
// codes the shpool daemon receives and pressing the Ctrl-<key>
// combo for all the lower-case letters, numbers, some symbols,
// and the space bar.
const CONTROL_CODES: [(&str, u8); 42] = [
    ("Ctrl-Space", 0),
    ("Ctrl-a", 1),
    ("Ctrl-b", 2),
    ("Ctrl-c", 3),
    ("Ctrl-d", 4),
    ("Ctrl-e", 5),
    ("Ctrl-f", 6),
    ("Ctrl-g", 7),
    ("Ctrl-h", 8),
    ("Ctrl-i", 9),
    ("Ctrl-j", 10),
    ("Ctrl-k", 11),
    ("Ctrl-l", 12),
    ("Ctrl-m", 13),
    ("Ctrl-n", 14),
    ("Ctrl-o", 15),
    ("Ctrl-p", 16),
    ("Ctrl-q", 17),
    ("Ctrl-r", 18),
    ("Ctrl-s", 19),
    ("Ctrl-t", 20),
    ("Ctrl-u", 21),
    ("Ctrl-v", 22),
    ("Ctrl-w", 23),
    ("Ctrl-y", 24),
    ("Ctrl-x", 25),
    ("Ctrl-z", 26),
    ("Ctrl-@", 0),
    ("Ctrl-2", 0),
    ("Ctrl-[", 27),
    ("Ctrl-3", 27),
    ("Ctrl-\\", 28),
    ("Ctrl-4", 28),
    ("Ctrl-]", 29),
    ("Ctrl-5", 29),
    ("Ctrl-^", 30),
    ("Ctrl-6", 30),
    ("Ctrl-_", 31),
    ("Ctrl-7", 31),
    ("Ctrl-?", 127),
    ("Ctrl-8", 127),
    ("Ctrl-0", 127),
];

//
// Unit Tests
//

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bindings() -> anyhow::Result<()> {
        let cases = vec![
            (
                // the bindings mapping
                vec![("a", Action::Detach)],
                // the keypresses to scan over
                ['a'].iter().map(|c| *c as u32 as u8).collect::<Vec<_>>(),
                BindingResult::Match(Action::Detach), // the final output from the engine
            ),
            (
                // the bindings mapping
                vec![("a", Action::Detach)],
                // the keypresses to scan over
                ['b', 'x', 'y', 'a'].iter().map(|c| *c as u32 as u8).collect::<Vec<_>>(),
                BindingResult::Match(Action::Detach), // the final output from the engine
            ),
            (
                vec![("a", Action::Detach)],
                ['b'].iter().map(|c| *c as u32 as u8).collect::<Vec<_>>(),
                BindingResult::NoMatch,
            ),
            (
                vec![("a", Action::Detach)],
                ['a', 'a', 'x', 'a', 'b'].iter().map(|c| *c as u32 as u8).collect::<Vec<_>>(),
                BindingResult::NoMatch,
            ),
            (vec![("Ctrl-a", Action::Detach)], vec![1], BindingResult::Match(Action::Detach)),
            (vec![("Ctrl-Space", Action::Detach)], vec![0], BindingResult::Match(Action::Detach)),
            (
                vec![("Ctrl-Space Ctrl-d", Action::Detach)],
                vec![0, 4],
                BindingResult::Match(Action::Detach),
            ),
            (vec![("Ctrl-Space Ctrl-d", Action::Detach)], vec![0, 20, 4], BindingResult::NoMatch),
            (vec![("Ctrl-Space Ctrl-d", Action::Detach)], vec![0, 4, 20], BindingResult::NoMatch),
            (
                vec![("a b c", Action::Detach)],
                ['a', 'b'].iter().map(|c| *c as u32 as u8).collect::<Vec<_>>(),
                BindingResult::Partial,
            ),
            (vec![("Ctrl-0", Action::Detach)], vec![127], BindingResult::Match(Action::Detach)),
            (vec![("Ctrl-\\", Action::Detach)], vec![28], BindingResult::Match(Action::Detach)),
            (
                vec![("Ctrl-\\ d", Action::Detach)],
                vec![28, b'd'],
                BindingResult::Match(Action::Detach),
            ),
            (vec![("Ctrl-\\ d", Action::Detach)], vec![28], BindingResult::Partial),
        ];

        for (bindings_mapping, keypresses, final_output) in cases.into_iter() {
            let mut bindings = Bindings::new(bindings_mapping)?;

            let mut actual_final_output = BindingResult::NoMatch;
            for byte in keypresses.into_iter() {
                actual_final_output = bindings.transition(byte);
            }
            assert_eq!(actual_final_output, final_output);
        }

        Ok(())
    }

    #[test]
    fn test_cord_validity() -> anyhow::Result<()> {
        let cases = vec![
            ("Ctrl-x", ""),
            ("a-a", "Ctrl is the only supported mod key"),
            ("Ctrl-a-x", "invalid chord"),
            ("a-Ctrl", "Ctrl is the only supported mod key"),
            ("Ctrl-Ctrl", "Ctrl cannot be repeated"),
        ];

        let tokenizer = Lexer::new();
        for (src, errstr) in cases.into_iter() {
            let tokens = tokenizer.tokenize(src.chars())?;
            let seq = parse(tokens)?;
            let chord = seq.0[0].clone();

            if errstr.is_empty() {
                chord.check_valid()?;
            } else if let Err(e) = chord.check_valid() {
                let got = format!("{e:?}");
                assert!(got.contains(errstr));
            } else {
                panic!("bad success, want err with: {errstr}");
            }
        }

        Ok(())
    }

    #[test]
    fn test_parse_ok() -> anyhow::Result<()> {
        let cases = vec![
            (
                "Ctrl-x a",
                Sequence(vec![
                    Chord(vec![String::from("Ctrl"), String::from("x")]),
                    Chord(vec![String::from("a")]),
                ]),
            ),
            (
                "Ctrl-x-a",
                Sequence(vec![Chord(vec![
                    String::from("Ctrl"),
                    String::from("x"),
                    String::from("a"),
                ])]),
            ),
            (
                "Ctrl Ctrl b c",
                Sequence(vec![
                    Chord(vec![String::from("Ctrl")]),
                    Chord(vec![String::from("Ctrl")]),
                    Chord(vec![String::from("b")]),
                    Chord(vec![String::from("c")]),
                ]),
            ),
        ];

        let tokenizer = Lexer::new();
        for (src, want) in cases.into_iter() {
            let tokens = tokenizer.tokenize(src.chars())?;
            let got = parse(tokens)?;
            assert_eq!(got, want);
        }

        Ok(())
    }

    #[test]
    fn test_tokenize_ok() -> anyhow::Result<()> {
        let cases = vec![
            ("-", vec![Token::Dash]),
            ("- ", vec![Token::Dash]),
            ("-\t", vec![Token::Dash]),
            (" -\t", vec![Token::Dash]),
            (" \t-\t ", vec![Token::Dash]),
            ("a", vec![Token::Key(String::from("a"))]),
            ("a a", vec![Token::Key(String::from("a")), Token::Key(String::from("a"))]),
            ("aa", vec![Token::Key(String::from("a")), Token::Key(String::from("a"))]),
            ("Ctrl", vec![Token::Key(String::from("Ctrl"))]),
            (
                "Ctrl-a",
                vec![Token::Key(String::from("Ctrl")), Token::Dash, Token::Key(String::from("a"))],
            ),
            (
                "Ctrl-0",
                vec![Token::Key(String::from("Ctrl")), Token::Dash, Token::Key(String::from("0"))],
            ),
            (
                "Ctrl-\\",
                vec![Token::Key(String::from("Ctrl")), Token::Dash, Token::Key(String::from("\\"))],
            ),
            (
                "Ctrl-\\ d",
                vec![
                    Token::Key(String::from("Ctrl")),
                    Token::Dash,
                    Token::Key(String::from("\\")),
                    Token::Key(String::from("d")),
                ],
            ),
        ];

        let tokenizer = Lexer::new();
        for (src, want) in cases.into_iter() {
            let got = tokenizer.tokenize(src.chars())?;
            assert_eq!(got, want);
        }

        Ok(())
    }

    #[test]
    fn test_tokenize_err() -> anyhow::Result<()> {
        let cases = vec![("CtrCtrl", "unexpected char"), ("Ctrc", "unexpected char")];

        let tokenizer = Lexer::new();
        for (src, errsubstr) in cases.into_iter() {
            if let Err(err) = tokenizer.tokenize(src.chars()) {
                let errstr = format!("{err:?}");
                assert!(errstr.contains(errsubstr));
            } else {
                panic!("expected an error")
            }
        }

        Ok(())
    }

    #[test]
    fn test_trie_contains() {
        let cases =
            vec![vec!["word"], vec![""], vec!["word", "words", "blah", "blip", "foo", "bar"]];

        for words in cases.into_iter() {
            let mut trie: Trie<_, _, HashMap<char, usize>> = Trie::new();
            for word in words.iter() {
                trie.insert(word.chars(), ());
            }
            for word in words.iter() {
                assert!(trie.contains(word.chars()));
            }
        }
    }
}
