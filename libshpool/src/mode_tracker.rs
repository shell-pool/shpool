// Copyright 2026 Google LLC
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

//! Tracks the terminal settings that programs in a session change, keeping
//! the latest value of each, so that a reattaching terminal can be brought to
//! the same state:
//!
//! - DEC private modes (`CSI ? Pm h/l`), with XTSAVE/XTRESTORE (`CSI ? Pm
//!   s/r`);
//! - kitty keyboard flags (`CSI > u`, `CSI < u`, `CSI = u`), a stack per
//!   screen;
//! - xterm key modifier options (`CSI > Pp ; Pv m`, `CSI > Pp n`);
//! - the cursor style (`CSI Ps SP q`).
//!
//! RIS (`ESC c`) clears them all, DECSTR (`CSI ! p`) all but the screen.
//! Application cursor keys and keypad, bracketed paste and the mouse are left
//! to vt100's `input_mode_formatted`: mouse tracking and mouse encoding are
//! each one setting in a terminal, not independent modes.
//!
//! The terminal's own leftovers are cleared with `TERMINAL_RESET`, sent before
//! a restore and by the client when it detaches.

use std::collections::BTreeMap;

/// Kitty keyboard stack entries kept per screen (kitty itself keeps 8).
const MAX_KEY_STACK: usize = 16;
/// DEC private modes kept: the lowest-numbered ones, since real modes have
/// small numbers and the rest is junk such as a binary file written out.
const MAX_DEC_MODES: usize = 256;

/// DEC private modes not replayed by `modes_formatted`: VT52 (2), 132 columns
/// (3), origin mode and left/right margins (6, 69; relative to margins that
/// aren't tracked), Tektronix (38); the screen (47, 1047, 1049), set by
/// `screen_and_keys_formatted`; actions or frames rather than settings: save
/// cursor (1048), highlight mouse tracking (1001), synchronized output (2026);
/// the modes vt100 restores, and the settings they share: the keypad (66, the
/// same as `ESC =`/`ESC >`) and the mouse encodings vt100 doesn't track (1015,
/// 1016; one setting with 1005/1006, so replaying them alone would override
/// vt100's).
const NOT_REPLAYED: &[u16] = &[
    2, 3, 6, 38, 69, 47, 1047, 1049, 1048, 1001, 2026, // see above
    1, 66, 2004, 9, 1000, 1002, 1003, 1005, 1006, 1015, 1016, // vt100
];

/// Turns off what sessions commonly leave on in a terminal: the alternate
/// screen (left inside DECSC/DECRC, since Ghostty and xterm restore a saved
/// cursor on leaving it even when they are not on it); the kitty keyboard
/// flags of both screens (the alternate screen's are only reachable on it);
/// mouse tracking and encodings, focus reports, bracketed paste, application
/// cursor keys and keypad, modifyOtherKeys; a hidden cursor.
pub(crate) const TERMINAL_RESET: &[u8] = b"\x1b7\x1b[?1049l\x1b8\x1b[<99u\x1b[=0;1u\
    \x1b[?1049h\x1b[<99u\x1b[=0;1u\x1b[?1049l\
    \x1b[?9l\x1b[?1000l\x1b[?1002l\x1b[?1003l\
    \x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1016l\
    \x1b[?1004l\x1b[?2004l\x1b[?1l\x1b>\x1b[>4m\x1b[?25h";

/// Kitty keyboard protocol state of one screen.
#[derive(Default)]
struct KeyFlags {
    /// Flags set with `CSI = u` while nothing is pushed. kitty keeps them in
    /// the bottom slot of the stack, so popping the pushed entries keeps them.
    base: u32,
    stack: Vec<u32>,
}

impl KeyFlags {
    fn current(&mut self) -> &mut u32 {
        self.stack.last_mut().unwrap_or(&mut self.base)
    }

    fn push(&mut self, flags: u32) {
        if self.stack.len() == MAX_KEY_STACK {
            self.stack.remove(0);
        }
        self.stack.push(flags);
    }

    fn pop(&mut self, n: usize) {
        if n > self.stack.len() {
            // "If a pop request is received that empties the stack, all flags
            // are reset."
            self.base = 0;
        }
        self.stack.truncate(self.stack.len().saturating_sub(n));
    }

    fn set(&mut self, flags: u32, mode: u16) {
        let current = self.current();
        match mode {
            1 => *current = flags,
            2 => *current |= flags,
            3 => *current &= !flags,
            _ => {} // undefined; kitty ignores the request
        }
    }

    /// Rebuild this stack on a terminal whose stack is empty.
    fn write_formatted(&self, buf: &mut Vec<u8>) {
        if self.base != 0 {
            buf.extend(format!("\x1b[={};1u", self.base).into_bytes());
        }
        for flags in &self.stack {
            buf.extend(format!("\x1b[>{flags}u").into_bytes());
        }
    }
}

#[derive(Default)]
struct Settings {
    dec_modes: BTreeMap<u16, bool>,
    /// DEC private modes saved with XTSAVE: their value then, None if unset.
    saved_modes: BTreeMap<u16, Option<bool>>,
    alt_screen: bool,
    /// Kitty keyboard flags: [normal screen, alternate screen].
    keys: [KeyFlags; 2],
    /// The sequence that sets each xterm key modifier option, by resource.
    key_modifiers: BTreeMap<u16, String>,
    cursor_style: Option<u16>,
}

impl Settings {
    fn set_dec_mode(&mut self, mode: u16, on: bool) {
        self.dec_modes.insert(mode, on);
        if self.dec_modes.len() > MAX_DEC_MODES {
            self.dec_modes.pop_last();
        }
    }

    fn save_dec_mode(&mut self, mode: u16) {
        if self.saved_modes.len() < MAX_DEC_MODES || self.saved_modes.contains_key(&mode) {
            self.saved_modes.insert(mode, self.dec_modes.get(&mode).copied());
        }
    }

    fn restore_dec_mode(&mut self, mode: u16) {
        match self.saved_modes.get(&mode) {
            Some(&Some(on)) => self.set_dec_mode(mode, on),
            Some(None) => {
                self.dec_modes.remove(&mode);
            }
            None => {}
        }
    }

    fn screen_keys(&mut self) -> &mut KeyFlags {
        &mut self.keys[self.alt_screen as usize]
    }
}

impl vte::Perform for Settings {
    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        if intermediates.is_empty() && byte == b'c' {
            *self = Settings::default(); // RIS
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool, // too many parameters: vt100 uses the first ones too
        action: char,
    ) {
        let param = |i| params.iter().nth(i).and_then(|p| p.first().copied());
        let modes = || params.iter().filter_map(|p| p.first().copied());
        match (intermediates, action) {
            ([b'?'], 'h' | 'l') => {
                modes().for_each(|m| self.set_dec_mode(m, action == 'h'));
                // The screen switches exactly as vt100 sees them (it ignores
                // 1047, subparameters and XTRESTORE), since its contents are
                // what the restore draws.
                if params.iter().any(|p| p == [47] || p == [1049]) {
                    self.alt_screen = action == 'h';
                }
            }
            ([b'?'], 's') => modes().for_each(|m| self.save_dec_mode(m)),
            ([b'?'], 'r') => modes().for_each(|m| self.restore_dec_mode(m)),
            ([b'>'], 'u') => self.screen_keys().push(param(0).unwrap_or(0) as u32),
            ([b'<'], 'u') => self.screen_keys().pop(param(0).unwrap_or(0).max(1) as usize),
            ([b'='], 'u') => {
                let mode = param(1).filter(|m| *m != 0).unwrap_or(1);
                self.screen_keys().set(param(0).unwrap_or(0) as u32, mode);
            }
            // vte reports `CSI > m` (all to default) as `CSI > 0 m` (only
            // modifyKeyboard); treat both as resetting all.
            ([b'>'], 'm') => match (param(0), param(1)) {
                (None | Some(0), None) => self.key_modifiers.clear(),
                (Some(r), None) if r <= 7 => {
                    self.key_modifiers.insert(r, format!("\x1b[>{r}m"));
                }
                (Some(r), Some(v)) if r <= 7 => {
                    self.key_modifiers.insert(r, format!("\x1b[>{r};{v}m"));
                }
                _ => {}
            },
            ([b'>'], 'n') => {
                if let Some(r) = param(0).filter(|r| *r <= 7) {
                    self.key_modifiers.insert(r, format!("\x1b[>{r}n"));
                }
            }
            ([b'!'], 'p') => *self = Settings { alt_screen: self.alt_screen, ..Default::default() },
            ([b' '], 'q') => self.cursor_style = Some(param(0).unwrap_or(0)),
            _ => {}
        }
    }
}

#[derive(Default)]
pub(crate) struct ModeTracker {
    parser: vte::Parser,
    settings: Settings,
}

impl ModeTracker {
    /// Feed output from the session's pty. Sequences may be split across calls.
    ///
    /// This parses the output a second time, next to vt100's parser, which
    /// doesn't expose the sequences it ignores. Like it, vte buffers OSC
    /// payloads (bounded only by their length) while it reads them.
    pub(crate) fn process(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.settings, bytes);
    }

    /// The start of a restore: reset the attached terminal (it may come from
    /// another session), set the normal screen's keyboard flags, then if
    /// `alt_screen` (vt100's view, since its contents come next) switch to the
    /// alternate screen and set its flags.
    pub(crate) fn screen_and_keys_formatted(&self, alt_screen: bool) -> Vec<u8> {
        let mut buf = TERMINAL_RESET.to_vec();
        self.settings.keys[0].write_formatted(&mut buf);
        if alt_screen {
            buf.extend_from_slice(b"\x1b[?1049h");
            self.settings.keys[1].write_formatted(&mut buf);
        }
        buf
    }

    /// The end of a restore, after the contents: every other setting.
    pub(crate) fn modes_formatted(&self) -> Vec<u8> {
        let settings = &self.settings;
        let mut buf: Vec<u8> = settings.key_modifiers.values().flat_map(|s| s.bytes()).collect();
        for (mode, on) in &settings.dec_modes {
            if !NOT_REPLAYED.contains(mode) {
                buf.extend(format!("\x1b[?{mode}{}", if *on { 'h' } else { 'l' }).into_bytes());
            }
        }
        if let Some(style) = settings.cursor_style {
            buf.extend(format!("\x1b[{style} q").into_bytes());
        }
        buf
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn tracked(chunks: &[&[u8]]) -> ModeTracker {
        let mut t = ModeTracker::default();
        chunks.iter().for_each(|chunk| t.process(chunk));
        t
    }

    fn s(bytes: &[u8]) -> String {
        String::from_utf8(bytes.to_vec()).unwrap().replace('\x1b', "ESC")
    }

    fn modes(chunks: &[&[u8]]) -> String {
        s(&tracked(chunks).modes_formatted())
    }

    /// screen_and_keys_formatted without TERMINAL_RESET.
    fn keys(chunks: &[&[u8]], alt_screen: bool) -> String {
        let out = tracked(chunks).screen_and_keys_formatted(alt_screen);
        s(out.strip_prefix(TERMINAL_RESET).unwrap())
    }

    #[test]
    fn replays_only_the_latest_value() {
        let out = modes(&[b"\x1b[?1004h\x1b[?1004l\x1b[?1004h\x1b[?7l\x1b[?7h"]);
        assert_eq!(out, "ESC[?7hESC[?1004h");
        assert_eq!(modes(&[b"\x1b[?1004;7;25l"]), "ESC[?7lESC[?25lESC[?1004l");
    }

    #[test]
    fn sequences_split_across_reads() {
        assert_eq!(modes(&[b"text\x1b[?10", b"04", b"h more"]), "ESC[?1004h");
        assert_eq!(modes(&[b"\x1b", b"[", b">", b"4;2", b"m"]), "ESC[>4;2m");
    }

    #[test]
    fn not_replayed() {
        let out = modes(&[
            b"\x1b[?2l\x1b[?3h\x1b[?6h\x1b[?38h\x1b[?69h",
            b"\x1b[?2026h\x1b[?1048h\x1b[?1049h\x1b[?7l",
            // left to vt100: crossterm's mouse disable, then vim's enable
            b"\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1015h\x1b[?1006h",
            b"\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l",
            b"\x1b[?1002h\x1b[?1006h\x1b[?1h\x1b[?66h\x1b[?2004h",
        ]);
        assert_eq!(out, "ESC[?7l");
    }

    #[test]
    fn full_reset_clears_everything() {
        let t = tracked(&[b"\x1b[?1049h\x1b[>1u\x1b[?1004h\x1b[>4;2m\x1b[5 q\x1bc"]);
        assert_eq!(s(&t.modes_formatted()), "");
        assert_eq!(keys(&[b"\x1b[?1049h\x1b[>1u\x1bc"], false), "");
        // the normal screen's keyboard flags are the current ones again
        let chunks: &[&[u8]] = &[b"\x1b[?1049h\x1bc\x1b[>1u"];
        assert_eq!(keys(chunks, false), "ESC[>1u");
        assert!(!tracked(chunks).settings.alt_screen);
    }

    #[test]
    fn soft_reset_keeps_the_screen() {
        let chunks: &[&[u8]] = &[b"\x1b[?1049h\x1b[?25l\x1b[>1u\x1b[>4;2m\x1b[!p"];
        assert_eq!(modes(chunks), "");
        assert_eq!(keys(chunks, true), "ESC[?1049h");
        assert!(tracked(chunks).settings.alt_screen);
    }

    #[test]
    fn saved_modes_are_restored() {
        assert_eq!(modes(&[b"\x1b[?1004s\x1b[?1004h\x1b[?1004r"]), "");
        assert_eq!(modes(&[b"\x1b[?1004l\x1b[?1004s\x1b[?1004h\x1b[?1004r"]), "ESC[?1004l");
        assert_eq!(modes(&[b"\x1b[?7h\x1b[?7s\x1b[?7l\x1b[?7r"]), "ESC[?7h");
    }

    #[test]
    fn alternate_screen_and_keyboard_stacks() {
        // Claude Code: push on the normal screen, enter the alternate screen,
        // push there too.
        let chunks: &[&[u8]] = &[b"\x1b[<u\x1b[>5u\x1b[?1049h\x1b[>5u"];
        assert_eq!(keys(chunks, true), "ESC[>5uESC[?1049hESC[>5u");
        // leaving the alternate screen keeps the normal screen's stack
        let chunks: &[&[u8]] = &[b"\x1b[>5u\x1b[?1049h\x1b[>1u\x1b[<u\x1b[?1049l"];
        assert_eq!(keys(chunks, false), "ESC[>5u");
        // what isn't a screen switch for vt100 isn't one for the tracker: 1047,
        // subparameters, XTRESTORE
        for (chunk, alt) in [
            (&b"\x1b[?1047h"[..], false),
            (b"\x1b[?1049:1h", false),
            (b"\x1b[?1049s\x1b[?1049h\x1b[?1049r", true),
        ] {
            assert_eq!(tracked(&[chunk]).settings.alt_screen, alt, "{chunk:?}");
        }
    }

    #[test]
    fn keyboard_pop_and_set() {
        // popping more than was pushed empties the stack and resets the flags
        let chunks: &[&[u8]] = &[b"\x1b[=3u\x1b[>1u\x1b[>5u\x1b[<5u"];
        assert_eq!(keys(chunks, false), "");
        // popping what was pushed keeps the flags set before
        let chunks: &[&[u8]] = &[b"\x1b[=3u\x1b[>1u\x1b[<u"];
        assert_eq!(keys(chunks, false), "ESC[=3;1u");
        // set modes: 1 replace, 2 or, 3 and-not, others ignored
        let chunks: &[&[u8]] = &[b"\x1b[=1u\x1b[=4;2u\x1b[=1;3u\x1b[=9;7u"];
        assert_eq!(keys(chunks, false), "ESC[=4;1u");
        // a pushed entry is modified in place
        assert_eq!(keys(&[b"\x1b[>1u\x1b[=8;2u"], false), "ESC[>9u");
    }

    #[test]
    fn key_modifiers_and_cursor_style() {
        assert_eq!(modes(&[b"\x1b[>4;2m\x1b[5 q"]), "ESC[>4;2mESC[5 q");
        assert_eq!(modes(&[b"\x1b[>4;2m\x1b[>4m"]), "ESC[>4m");
        assert_eq!(modes(&[b"\x1b[>4;2m\x1b[>4n"]), "ESC[>4n");
        assert_eq!(modes(&[b"\x1b[>4;2m\x1b[>1;1m\x1b[>m"]), "");
        assert_eq!(modes(&[b"\x1b[4m\x1b[38;5;1m"]), ""); // SGR
    }

    #[test]
    fn too_many_modes() {
        // junk mode numbers fill the table; a real one still gets in
        let mut t = ModeTracker::default();
        (3000..3300).for_each(|mode| t.process(format!("\x1b[?{mode}h").as_bytes()));
        t.process(b"\x1b[?1004h");
        assert_eq!(t.settings.dec_modes.len(), MAX_DEC_MODES);
        assert!(s(&t.modes_formatted()).starts_with("ESC[?1004hESC[?3000h"));
    }
}
