//! Stream capability for combinators to parse
//!
//! Stream types include:
//! - `&[u8]` and [`Bytes`] for binary data
//! - `&str` (aliased as [`Str`]) and [`BStr`] for UTF-8 data
//! - [`Located`] can track the location within the original buffer to report
//!   [spans][crate::Parser::with_span]
//! - [`Stateful`] to thread global state through your parsers
//! - [`Partial`] can mark an input as partial buffer that is being streamed into
//! - [Custom stream types][crate::_topic::stream]

use core::num::NonZeroUsize;

use crate::error::{ErrMode, ErrorKind, Needed, ParseError};
use crate::lib::std::iter::{Cloned, Enumerate};
use crate::lib::std::ops::{
    Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
};
use crate::lib::std::slice::Iter;
use crate::lib::std::str::from_utf8;
use crate::lib::std::str::CharIndices;
use crate::lib::std::str::FromStr;
use crate::IResult;

#[cfg(feature = "alloc")]
use crate::lib::std::collections::BTreeMap;
#[cfg(feature = "std")]
use crate::lib::std::collections::HashMap;
#[cfg(feature = "alloc")]
use crate::lib::std::string::String;
#[cfg(feature = "alloc")]
use crate::lib::std::vec::Vec;

mod impls;
#[cfg(test)]
mod tests;

/// UTF-8 Stream
pub type Str<'i> = &'i str;

/// Improved `Debug` experience for `&[u8]` byte streams
#[allow(clippy::derive_hash_xor_eq)]
#[derive(Hash)]
#[repr(transparent)]
pub struct Bytes([u8]);

impl Bytes {
    /// Make a stream out of a byte slice-like.
    #[inline]
    pub fn new<B: ?Sized + AsRef<[u8]>>(bytes: &B) -> &Self {
        Self::from_bytes(bytes.as_ref())
    }

    #[inline]
    fn from_bytes(slice: &[u8]) -> &Self {
        unsafe { crate::lib::std::mem::transmute(slice) }
    }

    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Improved `Debug` experience for `&[u8]` UTF-8-ish streams
#[allow(clippy::derive_hash_xor_eq)]
#[derive(Hash)]
#[repr(transparent)]
pub struct BStr([u8]);

impl BStr {
    /// Make a stream out of a byte slice-like.
    #[inline]
    pub fn new<B: ?Sized + AsRef<[u8]>>(bytes: &B) -> &Self {
        Self::from_bytes(bytes.as_ref())
    }

    #[inline]
    fn from_bytes(slice: &[u8]) -> &Self {
        unsafe { crate::lib::std::mem::transmute(slice) }
    }

    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Allow collecting the span of a parsed token
///
/// See [`Parser::span`][crate::Parser::span] and [`Parser::with_span`][crate::Parser::with_span] for more details
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Located<I> {
    initial: I,
    input: I,
}

impl<I> Located<I>
where
    I: Clone + Offset,
{
    /// Wrap another Stream with span tracking
    pub fn new(input: I) -> Self {
        let initial = input.clone();
        Self { initial, input }
    }

    fn location(&self) -> usize {
        self.initial.offset_to(&self.input)
    }
}

impl<I> AsRef<I> for Located<I> {
    #[inline(always)]
    fn as_ref(&self) -> &I {
        &self.input
    }
}

impl<I> crate::lib::std::ops::Deref for Located<I> {
    type Target = I;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<I: crate::lib::std::fmt::Display> crate::lib::std::fmt::Display for Located<I> {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        self.input.fmt(f)
    }
}

/// Thread global state through your parsers
///
/// Use cases
/// - Recursion checks
/// - Error recovery
/// - Debugging
///
/// # Example
///
/// ```
/// # use std::cell::Cell;
/// # use winnow::prelude::*;
/// # use winnow::stream::Stateful;
/// # use winnow::character::alpha1;
/// # type Error = ();
///
/// #[derive(Clone, Debug)]
/// struct State<'s>(&'s Cell<u32>);
///
/// impl<'s> State<'s> {
///     fn count(&self) {
///         self.0.set(self.0.get() + 1);
///     }
/// }
///
/// type Stream<'is> = Stateful<&'is str, State<'is>>;
///
/// fn word(i: Stream<'_>) -> IResult<Stream<'_>, &str> {
///   i.state.count();
///   alpha1(i)
/// }
///
/// let data = "Hello";
/// let state = Cell::new(0);
/// let input = Stream { input: data, state: State(&state) };
/// let output = word.parse(input).unwrap();
/// assert_eq!(state.get(), 1);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stateful<I, S> {
    /// Inner input being wrapped in state
    pub input: I,
    /// User-provided state
    pub state: S,
}

impl<I, S> AsRef<I> for Stateful<I, S> {
    #[inline(always)]
    fn as_ref(&self) -> &I {
        &self.input
    }
}

impl<I, S> crate::lib::std::ops::Deref for Stateful<I, S> {
    type Target = I;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<I: crate::lib::std::fmt::Display, S> crate::lib::std::fmt::Display for Stateful<I, S> {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        self.input.fmt(f)
    }
}

/// Mark the input as a partial buffer for streaming input.
///
/// Complete input means that we already have all of the data.  This will be the common case with
/// small files that can be read entirely to memory.
///
/// In contrast, streaming input assumes that we might not have all of the data.
/// This can happen with some network protocol or large file parsers, where the
/// input buffer can be full and need to be resized or refilled.
/// - [`ErrMode::Incomplete`] will report how much more data is needed.
/// - [`Parser::complete_err`][crate::Parser::complete_err] transform [`ErrMode::Incomplete`] to
///   [`ErrMode::Backtrack`]
///
/// See also [`StreamIsPartial`] to tell whether the input supports complete or partial parsing.
///
/// See also [Special Topics: Parsing Partial Input][crate::_topic::partial].
///
/// # Example
///
/// Here is how it works in practice:
///
/// ```rust
/// # use winnow::{IResult, error::ErrMode, error::Needed, error::{Error, ErrorKind}, bytes, character, stream::Partial};
/// # use winnow::prelude::*;
///
/// fn take_partial(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
///   bytes::take(4u8).parse_next(i)
/// }
///
/// fn take_complete(i: &[u8]) -> IResult<&[u8], &[u8]> {
///   bytes::take(4u8).parse_next(i)
/// }
///
/// // both parsers will take 4 bytes as expected
/// assert_eq!(take_partial(Partial::new(&b"abcde"[..])), Ok((Partial::new(&b"e"[..]), &b"abcd"[..])));
/// assert_eq!(take_complete(&b"abcde"[..]), Ok((&b"e"[..], &b"abcd"[..])));
///
/// // if the input is smaller than 4 bytes, the partial parser
/// // will return `Incomplete` to indicate that we need more data
/// assert_eq!(take_partial(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// // but the complete parser will return an error
/// assert_eq!(take_complete(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
///
/// // the alpha0 function recognizes 0 or more alphabetic characters
/// fn alpha0_partial(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   character::alpha0(i)
/// }
///
/// fn alpha0_complete(i: &str) -> IResult<&str, &str> {
///   character::alpha0(i)
/// }
///
/// // if there's a clear limit to the recognized characters, both parsers work the same way
/// assert_eq!(alpha0_partial(Partial::new("abcd;")), Ok((Partial::new(";"), "abcd")));
/// assert_eq!(alpha0_complete("abcd;"), Ok((";", "abcd")));
///
/// // but when there's no limit, the partial version returns `Incomplete`, because it cannot
/// // know if more input data should be recognized. The whole input could be "abcd;", or
/// // "abcde;"
/// assert_eq!(alpha0_partial(Partial::new("abcd")), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// // while the complete version knows that all of the data is there
/// assert_eq!(alpha0_complete("abcd"), Ok(("", "abcd")));
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Partial<I> {
    input: I,
    partial: bool,
}

impl<I> Partial<I>
where
    I: StreamIsPartial,
{
    /// Create a partial input
    pub fn new(input: I) -> Self {
        debug_assert!(
            !I::is_partial_supported(),
            "`Partial` can only wrap complete sources"
        );
        let partial = true;
        Self { input, partial }
    }

    /// Extract the original [`Stream`]
    #[inline(always)]
    pub fn into_inner(self) -> I {
        self.input
    }
}

impl<I> Default for Partial<I>
where
    I: Default + StreamIsPartial,
{
    fn default() -> Self {
        Self::new(I::default())
    }
}

impl<I> crate::lib::std::ops::Deref for Partial<I> {
    type Target = I;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<I: crate::lib::std::fmt::Display> crate::lib::std::fmt::Display for Partial<I> {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        self.input.fmt(f)
    }
}

/// Abstract method to calculate the input length
pub trait SliceLen {
    /// Calculates the input length, as indicated by its name,
    /// and the name of the trait itself
    fn slice_len(&self) -> usize;
}

impl<'a, T> SliceLen for &'a [T] {
    #[inline]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<T, const LEN: usize> SliceLen for [T; LEN] {
    #[inline]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<'a, T, const LEN: usize> SliceLen for &'a [T; LEN] {
    #[inline]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<'a> SliceLen for &'a str {
    #[inline]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<'a> SliceLen for &'a Bytes {
    #[inline]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<'a> SliceLen for &'a BStr {
    #[inline]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<I> SliceLen for (I, usize, usize)
where
    I: SliceLen,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.0.slice_len() * 8 + self.2 - self.1
    }
}

impl<I> SliceLen for Located<I>
where
    I: SliceLen,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.input.slice_len()
    }
}

impl<I, S> SliceLen for Stateful<I, S>
where
    I: SliceLen,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.input.slice_len()
    }
}

impl<I> SliceLen for Partial<I>
where
    I: SliceLen,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.input.slice_len()
    }
}

/// Core definition for parser input state
pub trait Stream: Offset + Clone + crate::lib::std::fmt::Debug {
    /// The smallest unit being parsed
    ///
    /// Example: `u8` for `&[u8]` or `char` for `&str`
    type Token: crate::lib::std::fmt::Debug;
    /// Sequence of `Token`s
    ///
    /// Example: `&[u8]` for `Located<&[u8]>` or `&str` for `Located<&str>`
    type Slice: crate::lib::std::fmt::Debug;

    /// Iterate with the offset from the current location
    type IterOffsets: Iterator<Item = (usize, Self::Token)>;

    /// Iterate with the offset from the current location
    fn iter_offsets(&self) -> Self::IterOffsets;
    /// Returns the offaet to the end of the input
    fn eof_offset(&self) -> usize;

    /// Split off the next token from the input
    fn next_token(&self) -> Option<(Self, Self::Token)>;

    /// Finds the offset of the next matching token
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool;
    /// Get the offset for the number of `tokens` into the stream
    ///
    /// This means "0 tokens" will return `0` offset
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed>;
    /// Split off a slice of tokens from the input
    ///
    /// **NOTE:** For inputs with variable width tokens, like `&str`'s `char`, `offset` might not correspond
    /// with the number of tokens.  To get a valid offset, use:
    /// - [`Stream::eof_offset`]
    /// - [`Stream::iter_offsets`]
    /// - [`Stream::offset_for`]
    /// - [`Stream::offset_at`]
    ///
    /// # Panic
    ///
    /// This will panic if
    ///
    /// * Indexes must be within bounds of the original input;
    /// * Indexes must uphold invariants of the stream, like for `str` they must lie on UTF-8
    ///   sequence boundaries.
    ///
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice);
}

impl<'i, T> Stream for &'i [T]
where
    T: Clone + crate::lib::std::fmt::Debug,
{
    type Token = T;
    type Slice = &'i [T];

    type IterOffsets = Enumerate<Cloned<Iter<'i, T>>>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.iter().cloned().enumerate()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn next_token(&self) -> Option<(Self, Self::Token)> {
        self.split_first()
            .map(|(token, next)| (next, token.clone()))
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.iter().position(|b| predicate(b.clone()))
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        if let Some(needed) = tokens.checked_sub(self.len()).and_then(NonZeroUsize::new) {
            Err(Needed::Size(needed))
        } else {
            Ok(tokens)
        }
    }
    #[inline(always)]
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice) {
        let (slice, next) = self.split_at(offset);
        (next, slice)
    }
}

impl<'i> Stream for &'i str {
    type Token = char;
    type Slice = &'i str;

    type IterOffsets = CharIndices<'i>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.char_indices()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn next_token(&self) -> Option<(Self, Self::Token)> {
        let c = self.chars().next()?;
        let offset = c.len();
        Some((&self[offset..], c))
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        for (o, c) in self.iter_offsets() {
            if predicate(c) {
                return Some(o);
            }
        }
        None
    }
    #[inline]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        let mut cnt = 0;
        for (offset, _) in self.iter_offsets() {
            if cnt == tokens {
                return Ok(offset);
            }
            cnt += 1;
        }

        if cnt == tokens {
            Ok(self.eof_offset())
        } else {
            Err(Needed::Unknown)
        }
    }
    #[inline(always)]
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice) {
        let (slice, next) = self.split_at(offset);
        (next, slice)
    }
}

impl<'i> Stream for &'i Bytes {
    type Token = u8;
    type Slice = &'i [u8];

    type IterOffsets = Enumerate<Cloned<Iter<'i, u8>>>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.iter().cloned().enumerate()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn next_token(&self) -> Option<(Self, Self::Token)> {
        if self.is_empty() {
            None
        } else {
            Some((Bytes::from_bytes(&self[1..]), self[0]))
        }
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.iter().position(|b| predicate(*b))
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        if let Some(needed) = tokens.checked_sub(self.len()).and_then(NonZeroUsize::new) {
            Err(Needed::Size(needed))
        } else {
            Ok(tokens)
        }
    }
    #[inline(always)]
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice) {
        let (next, slice) = (&self.0).next_slice(offset);
        (Bytes::from_bytes(next), slice)
    }
}

impl<'i> Stream for &'i BStr {
    type Token = u8;
    type Slice = &'i [u8];

    type IterOffsets = Enumerate<Cloned<Iter<'i, u8>>>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.iter().cloned().enumerate()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn next_token(&self) -> Option<(Self, Self::Token)> {
        if self.is_empty() {
            None
        } else {
            Some((BStr::from_bytes(&self[1..]), self[0]))
        }
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.iter().position(|b| predicate(*b))
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        if let Some(needed) = tokens.checked_sub(self.len()).and_then(NonZeroUsize::new) {
            Err(Needed::Size(needed))
        } else {
            Ok(tokens)
        }
    }
    #[inline(always)]
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice) {
        let (next, slice) = (&self.0).next_slice(offset);
        (BStr::from_bytes(next), slice)
    }
}

impl<I> Stream for (I, usize)
where
    I: Stream<Token = u8>,
{
    type Token = bool;
    type Slice = (I::Slice, usize, usize);

    type IterOffsets = BitOffsets<I>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        BitOffsets {
            i: self.clone(),
            o: 0,
        }
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        let offset = self.0.eof_offset() * 8;
        if offset == 0 {
            0
        } else {
            offset - self.1
        }
    }

    #[inline(always)]
    fn next_token(&self) -> Option<(Self, Self::Token)> {
        next_bit(self)
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.iter_offsets()
            .find_map(|(o, b)| predicate(b).then(|| o))
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        if let Some(needed) = tokens
            .checked_sub(self.eof_offset())
            .and_then(NonZeroUsize::new)
        {
            Err(Needed::Size(needed))
        } else {
            Ok(tokens)
        }
    }
    #[inline(always)]
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice) {
        let byte_offset = (offset + self.1) / 8;
        let end_offset = (offset + self.1) % 8;
        let (i, s) = self.0.next_slice(byte_offset);
        ((i, end_offset), (s, self.1, end_offset))
    }
}

/// Iterator for [bit][crate::bits] stream (`(I, usize)`)
pub struct BitOffsets<I> {
    i: (I, usize),
    o: usize,
}

impl<I> Iterator for BitOffsets<I>
where
    I: Stream<Token = u8>,
{
    type Item = (usize, bool);
    fn next(&mut self) -> Option<Self::Item> {
        let (next, b) = next_bit(&self.i)?;
        let o = self.o;

        self.i = next;
        self.o += 1;

        Some((o, b))
    }
}

fn next_bit<I>(i: &(I, usize)) -> Option<((I, usize), bool)>
where
    I: Stream<Token = u8>,
{
    if i.eof_offset() == 0 {
        return None;
    }

    let i = i.clone();
    let (next_i, byte) = i.0.next_token()?;
    let bit = (byte >> i.1) & 0x1 == 0x1;

    let next_offset = i.1 + 1;
    if next_offset == 8 {
        Some(((next_i, 0), bit))
    } else {
        Some(((i.0, next_offset), bit))
    }
}

impl<I: Stream> Stream for Located<I> {
    type Token = <I as Stream>::Token;
    type Slice = <I as Stream>::Slice;

    type IterOffsets = <I as Stream>::IterOffsets;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.input.iter_offsets()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.input.eof_offset()
    }

    #[inline(always)]
    fn next_token(&self) -> Option<(Self, Self::Token)> {
        let (next, token) = self.input.next_token()?;
        Some((
            Self {
                initial: self.initial.clone(),
                input: next,
            },
            token,
        ))
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.input.offset_for(predicate)
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        self.input.offset_at(tokens)
    }
    #[inline(always)]
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice) {
        let (next, slice) = self.input.next_slice(offset);
        (
            Self {
                initial: self.initial.clone(),
                input: next,
            },
            slice,
        )
    }
}

impl<I: Stream, S: Clone + crate::lib::std::fmt::Debug> Stream for Stateful<I, S> {
    type Token = <I as Stream>::Token;
    type Slice = <I as Stream>::Slice;

    type IterOffsets = <I as Stream>::IterOffsets;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.input.iter_offsets()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.input.eof_offset()
    }

    #[inline(always)]
    fn next_token(&self) -> Option<(Self, Self::Token)> {
        let (next, token) = self.input.next_token()?;
        Some((
            Self {
                input: next,
                state: self.state.clone(),
            },
            token,
        ))
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.input.offset_for(predicate)
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        self.input.offset_at(tokens)
    }
    #[inline(always)]
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice) {
        let (next, slice) = self.input.next_slice(offset);
        (
            Self {
                input: next,
                state: self.state.clone(),
            },
            slice,
        )
    }
}

impl<I: Stream> Stream for Partial<I> {
    type Token = <I as Stream>::Token;
    type Slice = <I as Stream>::Slice;

    type IterOffsets = <I as Stream>::IterOffsets;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.input.iter_offsets()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.input.eof_offset()
    }

    #[inline(always)]
    fn next_token(&self) -> Option<(Self, Self::Token)> {
        let (next, token) = self.input.next_token()?;
        Some((
            Partial {
                input: next,
                partial: self.partial,
            },
            token,
        ))
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.input.offset_for(predicate)
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        self.input.offset_at(tokens)
    }
    #[inline(always)]
    fn next_slice(&self, offset: usize) -> (Self, Self::Slice) {
        let (next, slice) = self.input.next_slice(offset);
        (
            Partial {
                input: next,
                partial: self.partial,
            },
            slice,
        )
    }
}

/// Number of indices input has advanced since start of parsing
pub trait Location {
    /// Number of indices input has advanced since start of parsing
    fn location(&self) -> usize;
}

impl<I> Location for Located<I>
where
    I: Clone + Offset,
{
    #[inline(always)]
    fn location(&self) -> usize {
        self.location()
    }
}

impl<I, S> Location for Stateful<I, S>
where
    I: Location,
{
    #[inline(always)]
    fn location(&self) -> usize {
        self.input.location()
    }
}

impl<I> Location for Partial<I>
where
    I: Location,
{
    #[inline(always)]
    fn location(&self) -> usize {
        self.input.location()
    }
}

/// Marks the input as being the complete buffer or a partial buffer for streaming input
///
/// See [`Partial`] for marking a presumed complete buffer type as a streaming buffer.
pub trait StreamIsPartial: Sized {
    /// Whether the stream is currently partial or complete
    type PartialState;

    /// Mark the stream is complete
    #[must_use]
    fn complete(&mut self) -> Self::PartialState;

    /// Restore the stream back to its previous state
    fn restore_partial(&mut self, state: Self::PartialState);

    /// Report whether the [`Stream`] is can ever be incomplete
    fn is_partial_supported() -> bool;

    /// Report whether the [`Stream`] is currently incomplete
    #[inline(always)]
    fn is_partial(&self) -> bool {
        Self::is_partial_supported()
    }
}

impl<'a, T> StreamIsPartial for &'a [T] {
    type PartialState = ();

    fn complete(&mut self) -> Self::PartialState {}

    fn restore_partial(&mut self, _state: Self::PartialState) {}

    #[inline(always)]
    fn is_partial_supported() -> bool {
        false
    }
}

impl<'a> StreamIsPartial for &'a str {
    type PartialState = ();

    fn complete(&mut self) -> Self::PartialState {
        // Already complete
    }

    fn restore_partial(&mut self, _state: Self::PartialState) {}

    #[inline(always)]
    fn is_partial_supported() -> bool {
        false
    }
}

impl<'a> StreamIsPartial for &'a Bytes {
    type PartialState = ();

    fn complete(&mut self) -> Self::PartialState {
        // Already complete
    }

    fn restore_partial(&mut self, _state: Self::PartialState) {}

    #[inline(always)]
    fn is_partial_supported() -> bool {
        false
    }
}

impl<'a> StreamIsPartial for &'a BStr {
    type PartialState = ();

    fn complete(&mut self) -> Self::PartialState {
        // Already complete
    }

    fn restore_partial(&mut self, _state: Self::PartialState) {}

    #[inline(always)]
    fn is_partial_supported() -> bool {
        false
    }
}

impl<I> StreamIsPartial for (I, usize)
where
    I: StreamIsPartial,
{
    type PartialState = I::PartialState;

    fn complete(&mut self) -> Self::PartialState {
        self.0.complete()
    }

    fn restore_partial(&mut self, state: Self::PartialState) {
        self.0.restore_partial(state);
    }

    #[inline(always)]
    fn is_partial_supported() -> bool {
        I::is_partial_supported()
    }

    #[inline(always)]
    fn is_partial(&self) -> bool {
        self.0.is_partial()
    }
}

impl<I> StreamIsPartial for Located<I>
where
    I: StreamIsPartial,
{
    type PartialState = I::PartialState;

    fn complete(&mut self) -> Self::PartialState {
        self.input.complete()
    }

    fn restore_partial(&mut self, state: Self::PartialState) {
        self.input.restore_partial(state);
    }

    #[inline(always)]
    fn is_partial_supported() -> bool {
        I::is_partial_supported()
    }

    #[inline(always)]
    fn is_partial(&self) -> bool {
        self.input.is_partial()
    }
}

impl<I, S> StreamIsPartial for Stateful<I, S>
where
    I: StreamIsPartial,
{
    type PartialState = I::PartialState;

    fn complete(&mut self) -> Self::PartialState {
        self.input.complete()
    }

    fn restore_partial(&mut self, state: Self::PartialState) {
        self.input.restore_partial(state);
    }

    #[inline(always)]
    fn is_partial_supported() -> bool {
        I::is_partial_supported()
    }

    #[inline(always)]
    fn is_partial(&self) -> bool {
        self.input.is_partial()
    }
}

impl<I> StreamIsPartial for Partial<I>
where
    I: StreamIsPartial,
{
    type PartialState = bool;

    fn complete(&mut self) -> Self::PartialState {
        core::mem::replace(&mut self.partial, false)
    }

    fn restore_partial(&mut self, state: Self::PartialState) {
        self.partial = state;
    }

    #[inline(always)]
    fn is_partial_supported() -> bool {
        true
    }

    #[inline(always)]
    fn is_partial(&self) -> bool {
        self.partial
    }
}

/// Useful functions to calculate the offset between slices and show a hexdump of a slice
pub trait Offset {
    /// Offset between the first byte of self and the first byte of the argument
    fn offset_to(&self, second: &Self) -> usize;
}

impl<'a, T> Offset for &'a [T] {
    #[inline(always)]
    fn offset_to(&self, second: &Self) -> usize {
        (*self).offset_to(*second)
    }
}

/// Convenience implementation to accept `&[T]` instead of `&&[T]` as above
impl<T> Offset for [T] {
    #[inline]
    fn offset_to(&self, second: &Self) -> usize {
        let fst = self.as_ptr();
        let snd = second.as_ptr();

        debug_assert!(
            fst <= snd,
            "`Offset::offset_to` only accepts slices of `self`"
        );
        snd as usize - fst as usize
    }
}

impl<'a> Offset for &'a str {
    #[inline(always)]
    fn offset_to(&self, second: &Self) -> usize {
        self.as_bytes().offset_to(second.as_bytes())
    }
}

/// Convenience implementation to accept `&str` instead of `&&str` as above
impl Offset for str {
    #[inline(always)]
    fn offset_to(&self, second: &Self) -> usize {
        self.as_bytes().offset_to(second.as_bytes())
    }
}

impl Offset for Bytes {
    #[inline(always)]
    fn offset_to(&self, second: &Self) -> usize {
        self.as_bytes().offset_to(second.as_bytes())
    }
}

impl<'a> Offset for &'a Bytes {
    #[inline(always)]
    fn offset_to(&self, second: &Self) -> usize {
        self.as_bytes().offset_to(second.as_bytes())
    }
}

impl Offset for BStr {
    #[inline(always)]
    fn offset_to(&self, second: &Self) -> usize {
        self.as_bytes().offset_to(second.as_bytes())
    }
}

impl<'a> Offset for &'a BStr {
    #[inline(always)]
    fn offset_to(&self, second: &Self) -> usize {
        self.as_bytes().offset_to(second.as_bytes())
    }
}

impl<I> Offset for (I, usize)
where
    I: Offset,
{
    #[inline(always)]
    fn offset_to(&self, other: &Self) -> usize {
        self.0.offset_to(&other.0) * 8 + other.1 - self.1
    }
}

impl<I> Offset for Located<I>
where
    I: Offset,
{
    #[inline(always)]
    fn offset_to(&self, other: &Self) -> usize {
        self.input.offset_to(&other.input)
    }
}

impl<I, S> Offset for Stateful<I, S>
where
    I: Offset,
{
    #[inline(always)]
    fn offset_to(&self, other: &Self) -> usize {
        self.input.offset_to(&other.input)
    }
}

impl<I> Offset for Partial<I>
where
    I: Offset,
{
    #[inline(always)]
    fn offset_to(&self, second: &Self) -> usize {
        self.input.offset_to(&second.input)
    }
}

/// Helper trait for types that can be viewed as a byte slice
pub trait AsBytes {
    /// Casts the input type to a byte slice
    fn as_bytes(&self) -> &[u8];
}

impl<'a> AsBytes for &'a [u8] {
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

impl<'a> AsBytes for &'a Bytes {
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        (*self).as_bytes()
    }
}

impl<I> AsBytes for Located<I>
where
    I: AsBytes,
{
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self.input.as_bytes()
    }
}

impl<I, S> AsBytes for Stateful<I, S>
where
    I: AsBytes,
{
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self.input.as_bytes()
    }
}

impl<I> AsBytes for Partial<I>
where
    I: AsBytes,
{
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self.input.as_bytes()
    }
}

/// Helper trait for types that can be viewed as a byte slice
pub trait AsBStr {
    /// Casts the input type to a byte slice
    fn as_bstr(&self) -> &[u8];
}

impl<'a> AsBStr for &'a [u8] {
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self
    }
}

impl<'a> AsBStr for &'a BStr {
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        (*self).as_bytes()
    }
}

impl<'a> AsBStr for &'a str {
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        (*self).as_bytes()
    }
}

impl<I> AsBStr for Located<I>
where
    I: AsBStr,
{
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self.input.as_bstr()
    }
}

impl<I, S> AsBStr for Stateful<I, S>
where
    I: AsBStr,
{
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self.input.as_bstr()
    }
}

impl<I> AsBStr for Partial<I>
where
    I: AsBStr,
{
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self.input.as_bstr()
    }
}

/// Result of [`Compare::compare`]
#[derive(Debug, Eq, PartialEq)]
pub enum CompareResult {
    /// Comparison was successful
    Ok,
    /// We need more data to be sure
    Incomplete,
    /// Comparison failed
    Error,
}

/// Abstracts comparison operations
pub trait Compare<T> {
    /// Compares self to another value for equality
    fn compare(&self, t: T) -> CompareResult;
    /// Compares self to another value for equality
    /// independently of the case.
    ///
    /// Warning: for `&str`, the comparison is done
    /// by lowercasing both strings and comparing
    /// the result. This is a temporary solution until
    /// a better one appears
    fn compare_no_case(&self, t: T) -> CompareResult;
}

fn lowercase_byte(c: u8) -> u8 {
    match c {
        b'A'..=b'Z' => c - b'A' + b'a',
        _ => c,
    }
}

impl<'a, 'b> Compare<&'b [u8]> for &'a [u8] {
    #[inline]
    fn compare(&self, t: &'b [u8]) -> CompareResult {
        let pos = self.iter().zip(t.iter()).position(|(a, b)| a != b);

        match pos {
            Some(_) => CompareResult::Error,
            None => {
                if self.len() >= t.len() {
                    CompareResult::Ok
                } else {
                    CompareResult::Incomplete
                }
            }
        }
    }

    #[inline]
    fn compare_no_case(&self, t: &'b [u8]) -> CompareResult {
        if self
            .iter()
            .zip(t)
            .any(|(a, b)| lowercase_byte(*a) != lowercase_byte(*b))
        {
            CompareResult::Error
        } else if self.len() < t.len() {
            CompareResult::Incomplete
        } else {
            CompareResult::Ok
        }
    }
}

impl<'a, const LEN: usize> Compare<[u8; LEN]> for &'a [u8] {
    #[inline(always)]
    fn compare(&self, t: [u8; LEN]) -> CompareResult {
        self.compare(&t[..])
    }

    #[inline(always)]
    fn compare_no_case(&self, t: [u8; LEN]) -> CompareResult {
        self.compare_no_case(&t[..])
    }
}

impl<'a, 'b, const LEN: usize> Compare<&'b [u8; LEN]> for &'a [u8] {
    #[inline(always)]
    fn compare(&self, t: &'b [u8; LEN]) -> CompareResult {
        self.compare(&t[..])
    }

    #[inline(always)]
    fn compare_no_case(&self, t: &'b [u8; LEN]) -> CompareResult {
        self.compare_no_case(&t[..])
    }
}

impl<'a, 'b> Compare<&'b str> for &'a [u8] {
    #[inline(always)]
    fn compare(&self, t: &'b str) -> CompareResult {
        self.compare(t.as_bytes())
    }
    #[inline(always)]
    fn compare_no_case(&self, t: &'b str) -> CompareResult {
        self.compare_no_case(t.as_bytes())
    }
}

impl<'a, 'b> Compare<&'b str> for &'a str {
    #[inline(always)]
    fn compare(&self, t: &'b str) -> CompareResult {
        self.as_bytes().compare(t.as_bytes())
    }

    //FIXME: this version is too simple and does not use the current locale
    #[inline]
    fn compare_no_case(&self, t: &'b str) -> CompareResult {
        let pos = self
            .chars()
            .zip(t.chars())
            .position(|(a, b)| a.to_lowercase().ne(b.to_lowercase()));

        match pos {
            Some(_) => CompareResult::Error,
            None => {
                if self.len() >= t.len() {
                    CompareResult::Ok
                } else {
                    CompareResult::Incomplete
                }
            }
        }
    }
}

impl<'a, 'b> Compare<&'b [u8]> for &'a str {
    #[inline(always)]
    fn compare(&self, t: &'b [u8]) -> CompareResult {
        AsBStr::as_bstr(self).compare(t)
    }
    #[inline(always)]
    fn compare_no_case(&self, t: &'b [u8]) -> CompareResult {
        AsBStr::as_bstr(self).compare_no_case(t)
    }
}

impl<'a, T> Compare<T> for &'a Bytes
where
    &'a [u8]: Compare<T>,
{
    #[inline(always)]
    fn compare(&self, t: T) -> CompareResult {
        let bytes = (*self).as_bytes();
        bytes.compare(t)
    }

    #[inline(always)]
    fn compare_no_case(&self, t: T) -> CompareResult {
        let bytes = (*self).as_bytes();
        bytes.compare_no_case(t)
    }
}

impl<'a, T> Compare<T> for &'a BStr
where
    &'a [u8]: Compare<T>,
{
    #[inline(always)]
    fn compare(&self, t: T) -> CompareResult {
        let bytes = (*self).as_bytes();
        bytes.compare(t)
    }

    #[inline(always)]
    fn compare_no_case(&self, t: T) -> CompareResult {
        let bytes = (*self).as_bytes();
        bytes.compare_no_case(t)
    }
}

impl<I, U> Compare<U> for Located<I>
where
    I: Compare<U>,
{
    #[inline(always)]
    fn compare(&self, other: U) -> CompareResult {
        self.input.compare(other)
    }

    #[inline(always)]
    fn compare_no_case(&self, other: U) -> CompareResult {
        self.input.compare_no_case(other)
    }
}

impl<I, S, U> Compare<U> for Stateful<I, S>
where
    I: Compare<U>,
{
    #[inline(always)]
    fn compare(&self, other: U) -> CompareResult {
        self.input.compare(other)
    }

    #[inline(always)]
    fn compare_no_case(&self, other: U) -> CompareResult {
        self.input.compare_no_case(other)
    }
}

impl<I, T> Compare<T> for Partial<I>
where
    I: Compare<T>,
{
    #[inline(always)]
    fn compare(&self, t: T) -> CompareResult {
        self.input.compare(t)
    }

    #[inline(always)]
    fn compare_no_case(&self, t: T) -> CompareResult {
        self.input.compare_no_case(t)
    }
}

/// Look for a slice in self
pub trait FindSlice<T> {
    /// Returns the offset of the slice if it is found
    fn find_slice(&self, substr: T) -> Option<usize>;
}

impl<'i, 's> FindSlice<&'s [u8]> for &'i [u8] {
    #[inline(always)]
    fn find_slice(&self, substr: &'s [u8]) -> Option<usize> {
        memmem(self, substr)
    }
}

impl<'i> FindSlice<u8> for &'i [u8] {
    #[inline(always)]
    fn find_slice(&self, substr: u8) -> Option<usize> {
        memchr(substr, self)
    }
}

impl<'i, 's> FindSlice<&'s str> for &'i [u8] {
    #[inline(always)]
    fn find_slice(&self, substr: &'s str) -> Option<usize> {
        self.find_slice(substr.as_bytes())
    }
}

impl<'i, 's> FindSlice<&'s str> for &'i str {
    #[inline(always)]
    fn find_slice(&self, substr: &'s str) -> Option<usize> {
        self.find(substr)
    }
}

impl<'i> FindSlice<char> for &'i str {
    #[inline(always)]
    fn find_slice(&self, substr: char) -> Option<usize> {
        self.find(substr)
    }
}

impl<'i, S> FindSlice<S> for &'i Bytes
where
    &'i [u8]: FindSlice<S>,
{
    #[inline(always)]
    fn find_slice(&self, substr: S) -> Option<usize> {
        let bytes = (*self).as_bytes();
        let offset = bytes.find_slice(substr);
        offset
    }
}

impl<'i, S> FindSlice<S> for &'i BStr
where
    &'i [u8]: FindSlice<S>,
{
    #[inline(always)]
    fn find_slice(&self, substr: S) -> Option<usize> {
        let bytes = (*self).as_bytes();
        let offset = bytes.find_slice(substr);
        offset
    }
}

impl<I, T> FindSlice<T> for Located<I>
where
    I: FindSlice<T>,
{
    #[inline(always)]
    fn find_slice(&self, substr: T) -> Option<usize> {
        self.input.find_slice(substr)
    }
}

impl<I, S, T> FindSlice<T> for Stateful<I, S>
where
    I: FindSlice<T>,
{
    #[inline(always)]
    fn find_slice(&self, substr: T) -> Option<usize> {
        self.input.find_slice(substr)
    }
}

impl<I, T> FindSlice<T> for Partial<I>
where
    I: FindSlice<T>,
{
    #[inline(always)]
    fn find_slice(&self, substr: T) -> Option<usize> {
        self.input.find_slice(substr)
    }
}

/// Used to integrate `str`'s `parse()` method
pub trait ParseSlice<R> {
    /// Succeeds if `parse()` succeededThe
    ///
    /// The byte slice implementation will first convert it to a `&str`, then apply the `parse()`
    /// function
    fn parse_slice(&self) -> Option<R>;
}

impl<'a, R: FromStr> ParseSlice<R> for &'a [u8] {
    #[inline(always)]
    fn parse_slice(&self) -> Option<R> {
        from_utf8(self).ok().and_then(|s| s.parse().ok())
    }
}

impl<'a, R: FromStr> ParseSlice<R> for &'a str {
    #[inline(always)]
    fn parse_slice(&self) -> Option<R> {
        self.parse().ok()
    }
}

/// Convert a `Stream` into an appropriate `Output` type
pub trait UpdateSlice: Stream {
    /// Convert an `Output` type to be used as `Stream`
    fn update_slice(self, inner: Self::Slice) -> Self;
}

impl<'a, T> UpdateSlice for &'a [T]
where
    T: Clone + crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        inner
    }
}

impl<'a> UpdateSlice for &'a str {
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        inner
    }
}

impl<'a> UpdateSlice for &'a Bytes {
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        Bytes::new(inner)
    }
}

impl<'a> UpdateSlice for &'a BStr {
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        BStr::new(inner)
    }
}

impl<I> UpdateSlice for Located<I>
where
    I: UpdateSlice,
{
    #[inline(always)]
    fn update_slice(mut self, inner: Self::Slice) -> Self {
        self.input = I::update_slice(self.input, inner);
        self
    }
}

impl<I, S> UpdateSlice for Stateful<I, S>
where
    I: UpdateSlice,
    S: Clone + crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn update_slice(mut self, inner: Self::Slice) -> Self {
        self.input = I::update_slice(self.input, inner);
        self
    }
}

impl<I> UpdateSlice for Partial<I>
where
    I: UpdateSlice,
{
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        Partial {
            input: I::update_slice(self.input, inner),
            partial: self.partial,
        }
    }
}

/// Abstracts something which can extend an `Extend`.
/// Used to build modified input slices in `escaped_transform`
pub trait Accumulate<T>: Sized {
    /// Create a new `Extend` of the correct type
    fn initial(capacity: Option<usize>) -> Self;
    /// Accumulate the input into an accumulator
    fn accumulate(&mut self, acc: T);
}

impl<T> Accumulate<T> for () {
    #[inline(always)]
    fn initial(_capacity: Option<usize>) -> Self {}
    #[inline(always)]
    fn accumulate(&mut self, _acc: T) {}
}

impl<T> Accumulate<T> for usize {
    #[inline(always)]
    fn initial(_capacity: Option<usize>) -> Self {
        0
    }
    #[inline(always)]
    fn accumulate(&mut self, _acc: T) {
        *self += 1;
    }
}

#[cfg(feature = "alloc")]
impl<T> Accumulate<T> for Vec<T> {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => Vec::with_capacity(clamp_capacity::<T>(capacity)),
            None => Vec::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: T) {
        self.push(acc);
    }
}

#[cfg(feature = "alloc")]
impl<'i, T: Clone> Accumulate<&'i [T]> for Vec<T> {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => Vec::with_capacity(clamp_capacity::<T>(capacity)),
            None => Vec::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: &'i [T]) {
        self.extend(acc.iter().cloned());
    }
}

#[cfg(feature = "alloc")]
impl Accumulate<char> for String {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => String::with_capacity(clamp_capacity::<char>(capacity)),
            None => String::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: char) {
        self.push(acc);
    }
}

#[cfg(feature = "alloc")]
impl<'i> Accumulate<&'i str> for String {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => String::with_capacity(clamp_capacity::<char>(capacity)),
            None => String::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: &'i str) {
        self.push_str(acc);
    }
}

#[cfg(feature = "alloc")]
impl<K, V> Accumulate<(K, V)> for BTreeMap<K, V>
where
    K: crate::lib::std::cmp::Ord,
{
    #[inline(always)]
    fn initial(_capacity: Option<usize>) -> Self {
        BTreeMap::new()
    }
    #[inline(always)]
    fn accumulate(&mut self, (key, value): (K, V)) {
        self.insert(key, value);
    }
}

#[cfg(feature = "std")]
impl<K, V> Accumulate<(K, V)> for HashMap<K, V>
where
    K: crate::lib::std::cmp::Eq + crate::lib::std::hash::Hash,
{
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => HashMap::with_capacity(clamp_capacity::<(K, V)>(capacity)),
            None => HashMap::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, (key, value): (K, V)) {
        self.insert(key, value);
    }
}

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn clamp_capacity<T>(capacity: usize) -> usize {
    /// Don't pre-allocate more than 64KiB when calling `Vec::with_capacity`.
    ///
    /// Pre-allocating memory is a nice optimization but count fields can't
    /// always be trusted. We should clamp initial capacities to some reasonable
    /// amount. This reduces the risk of a bogus count value triggering a panic
    /// due to an OOM error.
    ///
    /// This does not affect correctness. `winnow` will always read the full number
    /// of elements regardless of the capacity cap.
    const MAX_INITIAL_CAPACITY_BYTES: usize = 65536;

    let max_initial_capacity =
        MAX_INITIAL_CAPACITY_BYTES / crate::lib::std::mem::size_of::<T>().max(1);
    capacity.min(max_initial_capacity)
}

/// Helper trait to convert numbers to usize.
///
/// By default, usize implements `From<u8>` and `From<u16>` but not
/// `From<u32>` and `From<u64>` because that would be invalid on some
/// platforms. This trait implements the conversion for platforms
/// with 32 and 64 bits pointer platforms
pub trait ToUsize {
    /// converts self to usize
    fn to_usize(&self) -> usize;
}

impl ToUsize for u8 {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self as usize
    }
}

impl ToUsize for u16 {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self as usize
    }
}

impl ToUsize for usize {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self
    }
}

#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
impl ToUsize for u32 {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self as usize
    }
}

#[cfg(target_pointer_width = "64")]
impl ToUsize for u64 {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self as usize
    }
}

/// Transforms a token into a char for basic string parsing
#[allow(clippy::len_without_is_empty)]
#[allow(clippy::wrong_self_convention)]
pub trait AsChar {
    /// Makes a char from self
    ///
    /// # Example
    ///
    /// ```
    /// use winnow::stream::AsChar as _;
    ///
    /// assert_eq!('a'.as_char(), 'a');
    /// assert_eq!(u8::MAX.as_char(), std::char::from_u32(u8::MAX as u32).unwrap());
    /// ```
    fn as_char(self) -> char;

    /// Tests that self is an alphabetic character
    ///
    /// **Warning:** for `&str` it recognizes alphabetic
    /// characters outside of the 52 ASCII letters
    fn is_alpha(self) -> bool;

    /// Tests that self is an alphabetic character
    /// or a decimal digit
    fn is_alphanum(self) -> bool;
    /// Tests that self is a decimal digit
    fn is_dec_digit(self) -> bool;
    /// Tests that self is an hex digit
    fn is_hex_digit(self) -> bool;
    /// Tests that self is an octal digit
    fn is_oct_digit(self) -> bool;
    /// Gets the len in bytes for self
    fn len(self) -> usize;
    /// Tests that self is ASCII space or tab
    fn is_space(self) -> bool;
    /// Tests if byte is ASCII newline: \n
    fn is_newline(self) -> bool;
}

impl AsChar for u8 {
    #[inline(always)]
    fn as_char(self) -> char {
        self as char
    }
    #[inline]
    fn is_alpha(self) -> bool {
        matches!(self, 0x41..=0x5A | 0x61..=0x7A)
    }
    #[inline]
    fn is_alphanum(self) -> bool {
        self.is_alpha() || self.is_dec_digit()
    }
    #[inline]
    fn is_dec_digit(self) -> bool {
        matches!(self, 0x30..=0x39)
    }
    #[inline]
    fn is_hex_digit(self) -> bool {
        matches!(self, 0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66)
    }
    #[inline]
    fn is_oct_digit(self) -> bool {
        matches!(self, 0x30..=0x37)
    }
    #[inline]
    fn len(self) -> usize {
        1
    }
    #[inline]
    fn is_space(self) -> bool {
        self == b' ' || self == b'\t'
    }
    fn is_newline(self) -> bool {
        self == b'\n'
    }
}
impl<'a> AsChar for &'a u8 {
    #[inline(always)]
    fn as_char(self) -> char {
        *self as char
    }
    #[inline]
    fn is_alpha(self) -> bool {
        matches!(*self, 0x41..=0x5A | 0x61..=0x7A)
    }
    #[inline]
    fn is_alphanum(self) -> bool {
        self.is_alpha() || self.is_dec_digit()
    }
    #[inline]
    fn is_dec_digit(self) -> bool {
        matches!(*self, 0x30..=0x39)
    }
    #[inline]
    fn is_hex_digit(self) -> bool {
        matches!(*self, 0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66)
    }
    #[inline]
    fn is_oct_digit(self) -> bool {
        matches!(*self, 0x30..=0x37)
    }
    #[inline]
    fn len(self) -> usize {
        1
    }
    #[inline]
    fn is_space(self) -> bool {
        *self == b' ' || *self == b'\t'
    }
    fn is_newline(self) -> bool {
        *self == b'\n'
    }
}

impl AsChar for char {
    #[inline(always)]
    fn as_char(self) -> char {
        self
    }
    #[inline]
    fn is_alpha(self) -> bool {
        self.is_ascii_alphabetic()
    }
    #[inline]
    fn is_alphanum(self) -> bool {
        self.is_alpha() || self.is_dec_digit()
    }
    #[inline]
    fn is_dec_digit(self) -> bool {
        self.is_ascii_digit()
    }
    #[inline]
    fn is_hex_digit(self) -> bool {
        self.is_ascii_hexdigit()
    }
    #[inline]
    fn is_oct_digit(self) -> bool {
        self.is_digit(8)
    }
    #[inline]
    fn len(self) -> usize {
        self.len_utf8()
    }
    #[inline]
    fn is_space(self) -> bool {
        self == ' ' || self == '\t'
    }
    fn is_newline(self) -> bool {
        self == '\n'
    }
}

impl<'a> AsChar for &'a char {
    #[inline(always)]
    fn as_char(self) -> char {
        *self
    }
    #[inline]
    fn is_alpha(self) -> bool {
        self.is_ascii_alphabetic()
    }
    #[inline]
    fn is_alphanum(self) -> bool {
        self.is_alpha() || self.is_dec_digit()
    }
    #[inline]
    fn is_dec_digit(self) -> bool {
        self.is_ascii_digit()
    }
    #[inline]
    fn is_hex_digit(self) -> bool {
        self.is_ascii_hexdigit()
    }
    #[inline]
    fn is_oct_digit(self) -> bool {
        self.is_digit(8)
    }
    #[inline]
    fn len(self) -> usize {
        self.len_utf8()
    }
    #[inline]
    fn is_space(self) -> bool {
        *self == ' ' || *self == '\t'
    }
    fn is_newline(self) -> bool {
        *self == '\n'
    }
}

/// Check if a token in in a set of possible tokens
///
/// This is generally implemented on patterns that a token may match and supports `u8` and `char`
/// tokens along with the following patterns
/// - `b'c'` and `'c'`
/// - `b""` and `""`
/// - `|c| true`
/// - `b'a'..=b'z'`, `'a'..='z'` (etc for each [range type][std::ops])
/// - `(pattern1, pattern2, ...)`
///
/// # Example
///
/// For example, you could implement `hex_digit0` as:
/// ```
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::bytes::take_while1;
/// fn hex_digit1(input: &str) -> IResult<&str, &str> {
///     take_while1(('a'..='f', 'A'..='F', '0'..='9')).parse_next(input)
/// }
///
/// assert_eq!(hex_digit1("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(hex_digit1("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::Slice))));
/// assert_eq!(hex_digit1(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
/// ```
pub trait ContainsToken<T> {
    /// Returns true if self contains the token
    fn contains_token(&self, token: T) -> bool;
}

impl ContainsToken<u8> for u8 {
    #[inline(always)]
    fn contains_token(&self, token: u8) -> bool {
        *self == token
    }
}

impl<'a> ContainsToken<&'a u8> for u8 {
    #[inline(always)]
    fn contains_token(&self, token: &u8) -> bool {
        self.contains_token(*token)
    }
}

impl ContainsToken<char> for u8 {
    #[inline(always)]
    fn contains_token(&self, token: char) -> bool {
        self.as_char() == token
    }
}

impl<'a> ContainsToken<&'a char> for u8 {
    #[inline(always)]
    fn contains_token(&self, token: &char) -> bool {
        self.contains_token(*token)
    }
}

impl<C: AsChar> ContainsToken<C> for char {
    #[inline(always)]
    fn contains_token(&self, token: C) -> bool {
        *self == token.as_char()
    }
}

impl<C: AsChar, F: Fn(C) -> bool> ContainsToken<C> for F {
    #[inline(always)]
    fn contains_token(&self, token: C) -> bool {
        self(token)
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1> for Range<C2> {
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let start = self.start.clone().as_char();
        let end = self.end.clone().as_char();
        (start..end).contains(&token.as_char())
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1> for RangeInclusive<C2> {
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let start = self.start().clone().as_char();
        let end = self.end().clone().as_char();
        (start..=end).contains(&token.as_char())
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1> for RangeFrom<C2> {
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let start = self.start.clone().as_char();
        (start..).contains(&token.as_char())
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1> for RangeTo<C2> {
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let end = self.end.clone().as_char();
        (..end).contains(&token.as_char())
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1> for RangeToInclusive<C2> {
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let end = self.end.clone().as_char();
        (..=end).contains(&token.as_char())
    }
}

impl<C1: AsChar> ContainsToken<C1> for RangeFull {
    #[inline(always)]
    fn contains_token(&self, _token: C1) -> bool {
        true
    }
}

impl<C: AsChar> ContainsToken<C> for &'_ [u8] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| t.as_char() == token)
    }
}

impl<C: AsChar> ContainsToken<C> for &'_ [char] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| *t == token)
    }
}

impl<const LEN: usize, C: AsChar> ContainsToken<C> for &'_ [u8; LEN] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| t.as_char() == token)
    }
}

impl<const LEN: usize, C: AsChar> ContainsToken<C> for &'_ [char; LEN] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| *t == token)
    }
}

impl<const LEN: usize, C: AsChar> ContainsToken<C> for [u8; LEN] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| t.as_char() == token)
    }
}

impl<const LEN: usize, C: AsChar> ContainsToken<C> for [char; LEN] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| *t == token)
    }
}

impl<C: AsChar> ContainsToken<C> for &'_ str {
    #[inline(always)]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.chars().any(|i| i == token)
    }
}

impl<T> ContainsToken<T> for () {
    #[inline(always)]
    fn contains_token(&self, _token: T) -> bool {
        false
    }
}

macro_rules! impl_contains_token_for_tuple {
  ($($haystack:ident),+) => (
    #[allow(non_snake_case)]
    impl<T, $($haystack),+> ContainsToken<T> for ($($haystack),+,)
    where
    T: Clone,
      $($haystack: ContainsToken<T>),+
    {
    #[inline]
      fn contains_token(&self, token: T) -> bool {
        let ($(ref $haystack),+,) = *self;
        $($haystack.contains_token(token.clone()) || )+ false
      }
    }
  )
}

macro_rules! impl_contains_token_for_tuples {
    ($haystack1:ident, $($haystack:ident),+) => {
        impl_contains_token_for_tuples!(__impl $haystack1; $($haystack),+);
    };
    (__impl $($haystack:ident),+; $haystack1:ident $(,$haystack2:ident)*) => {
        impl_contains_token_for_tuple!($($haystack),+);
        impl_contains_token_for_tuples!(__impl $($haystack),+, $haystack1; $($haystack2),*);
    };
    (__impl $($haystack:ident),+;) => {
        impl_contains_token_for_tuple!($($haystack),+);
    }
}

impl_contains_token_for_tuples!(
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21
);

/// Looks for the first element of the input type for which the condition returns true,
/// and returns the input up to this position.
///
/// *Partial version*: If no element is found matching the condition, this will return `Incomplete`
pub(crate) fn split_at_offset_partial<P, I: Stream, E: ParseError<I>>(
    input: &I,
    predicate: P,
) -> IResult<I, <I as Stream>::Slice, E>
where
    P: Fn(I::Token) -> bool,
{
    let offset = input
        .offset_for(predicate)
        .ok_or_else(|| ErrMode::Incomplete(Needed::new(1)))?;
    Ok(input.next_slice(offset))
}

/// Looks for the first element of the input type for which the condition returns true
/// and returns the input up to this position.
///
/// Fails if the produced slice is empty.
///
/// *Partial version*: If no element is found matching the condition, this will return `Incomplete`
pub(crate) fn split_at_offset1_partial<P, I: Stream, E: ParseError<I>>(
    input: &I,
    predicate: P,
    e: ErrorKind,
) -> IResult<I, <I as Stream>::Slice, E>
where
    P: Fn(I::Token) -> bool,
{
    let offset = input
        .offset_for(predicate)
        .ok_or_else(|| ErrMode::Incomplete(Needed::new(1)))?;
    if offset == 0 {
        Err(ErrMode::from_error_kind(input.clone(), e))
    } else {
        Ok(input.next_slice(offset))
    }
}

/// Looks for the first element of the input type for which the condition returns true,
/// and returns the input up to this position.
///
/// *Complete version*: If no element is found matching the condition, this will return the whole input
pub(crate) fn split_at_offset_complete<P, I: Stream, E: ParseError<I>>(
    input: &I,
    predicate: P,
) -> IResult<I, <I as Stream>::Slice, E>
where
    P: Fn(I::Token) -> bool,
{
    let offset = input
        .offset_for(predicate)
        .unwrap_or_else(|| input.eof_offset());
    Ok(input.next_slice(offset))
}

/// Looks for the first element of the input type for which the condition returns true
/// and returns the input up to this position.
///
/// Fails if the produced slice is empty.
///
/// *Complete version*: If no element is found matching the condition, this will return the whole input
pub(crate) fn split_at_offset1_complete<P, I: Stream, E: ParseError<I>>(
    input: &I,
    predicate: P,
    e: ErrorKind,
) -> IResult<I, <I as Stream>::Slice, E>
where
    P: Fn(I::Token) -> bool,
{
    let offset = input
        .offset_for(predicate)
        .unwrap_or_else(|| input.eof_offset());
    if offset == 0 {
        Err(ErrMode::from_error_kind(input.clone(), e))
    } else {
        Ok(input.next_slice(offset))
    }
}

#[cfg(feature = "simd")]
#[inline(always)]
fn memchr(token: u8, slice: &[u8]) -> Option<usize> {
    memchr::memchr(token, slice)
}

#[cfg(not(feature = "simd"))]
#[inline(always)]
fn memchr(token: u8, slice: &[u8]) -> Option<usize> {
    slice.iter().position(|t| *t == token)
}

#[cfg(feature = "simd")]
#[inline(always)]
fn memmem(slice: &[u8], tag: &[u8]) -> Option<usize> {
    memchr::memmem::find(slice, tag)
}

#[cfg(not(feature = "simd"))]
fn memmem(slice: &[u8], tag: &[u8]) -> Option<usize> {
    for i in 0..slice.len() {
        let subslice = &slice[i..];
        if subslice.starts_with(tag) {
            return Some(i);
        }
    }
    None
}
