//! Input capability for nom combinators to parse
//!
//! Input types include:
//! - `&str` and `&[u8]` are the standard input types
//! - [`Located`] can track the location within the original buffer to report
//!   [spans][crate::Parser::with_span]
//! - [`Stateful`] to thread global state through your parsers
//! - [`Streaming`] can mark an input as partial buffer that is being streamed into
//!
//! # How do a parse a custom input type?
//!
//! While historically, nom has worked mainly on `&[u8]` and `&str`, it can actually
//! use any type as input, as long as they follow a specific set of traits.
//! Those traits were developed first to abstract away the differences between
//! `&[u8]` and `&str`, but were then employed for more interesting types,
//! like [nom_locate](https://github.com/fflorent/nom_locate), a wrapper type
//! that can carry line and column information, or to parse
//! [a list of tokens](https://github.com/Rydgel/monkey-rust/blob/master/lib/parser/mod.rs).
//!
//! ## Implementing a custom type
//!
//! Let's assume we have an input type we'll call `MyInput`. `MyInput` is a sequence of `MyItem` type.
//! The goal is to define nom parsers with this signature: `MyInput -> IResult<MyInput, Output>`.
//!
//! ```rust,ignore
//! fn parser(i: MyInput) -> IResult<MyInput, Output> {
//!     tag("test")(i)
//! }
//! ```
//!
//! Here are the traits we have to implement for `MyInput`:
//!
//! | trait | usage |
//! |---|---|
//! | [InputIsStreaming] | Marks the input as being the complete buffer or a partial buffer for streaming input |
//! | [AsBytes] |Casts the input type to a byte slice|
//! | [Compare] |Character comparison operations|
//! | [ExtendInto] |Abstracts something which can extend an `Extend`|
//! | [FindSubstring] |Look for a substring in self|
//! | [FindToken] |Look for self in the given input stream|
//! | [InputIter] |Common iteration operations on the input type|
//! | [InputLength] |Calculate the input length|
//! | [IntoOutput] |Adapt a captired `Input` into an appropriate type|
//! | [Location] |Calculate location within initial input|
//! | [InputTake] |Slicing operations|
//! | [InputTakeAtPosition] |Look for a specific token and split at its position|
//! | [Offset] |Calculate the offset between slices|
//! | [ParseTo] |Used to integrate `&str`'s `parse()` method|
//! | [Slice] |Slicing operations using ranges|
//!
//! Here are the traits we have to implement for `MyItem`:
//!
//! | trait | usage |
//! |---|---|
//! | [AsChar][AsChar] |Transforms common types to a char for basic token parsing|

use core::num::NonZeroUsize;

use crate::error::{ErrorKind, ParseError};
use crate::lib::std::iter::{Copied, Enumerate};
use crate::lib::std::ops::{
  Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
};
use crate::lib::std::slice::Iter;
use crate::lib::std::str::from_utf8;
use crate::lib::std::str::CharIndices;
use crate::lib::std::str::Chars;
use crate::lib::std::str::FromStr;
use crate::{Err, IResult, Needed};

#[cfg(feature = "alloc")]
use crate::lib::std::string::String;
#[cfg(feature = "alloc")]
use crate::lib::std::vec::Vec;

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
  I: Clone + IntoOutput + Offset,
{
  /// Wrap another Input with span tracking
  pub fn new(input: I) -> Self {
    let initial = input.clone();
    Self { initial, input }
  }

  fn location(&self) -> usize {
    self.initial.offset(&self.input)
  }
}

impl<I> AsRef<I> for Located<I> {
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

/// Thread global state through your parsers
///
/// Use cases
/// - Recusion checks
/// - Errror recovery
/// - Debugging
///
/// # Example
///
/// ```
/// # use std::cell::Cell;
/// # use nom8::prelude::*;
/// # use nom8::input::Stateful;
/// # use nom8::character::alpha1;
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
/// type Input<'is> = Stateful<&'is str, State<'is>>;
///
/// fn word(i: Input<'_>) -> IResult<Input<'_>, &str> {
///   i.state.count();
///   alpha1(i)
/// }
///
/// let data = "Hello";
/// let state = Cell::new(0);
/// let input = Input { input: data, state: State(&state) };
/// let output = word.parse(input).finish().unwrap();
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
  fn as_ref(&self) -> &I {
    &self.input
  }
}

impl<I, S> crate::lib::std::ops::Deref for Stateful<I, S> {
  type Target = I;

  fn deref(&self) -> &Self::Target {
    self.as_ref()
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
/// - [`Err::Incomplete`] will report how much more data is needed.
/// - [`Parser::complete`][crate::Parser::complete] transform [`Err::Incomplete`] to
///   [`Err::Error`]
///
/// See also [`InputIsStreaming`] to tell whether the input supports complete or streaming parsing.
///
/// # Example
///
/// Here is how it works in practice:
///
/// ```rust
/// use nom8::{IResult, Err, Needed, error::{Error, ErrorKind}, bytes, character, input::Streaming};
///
/// fn take_streaming(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
///   bytes::take(4u8)(i)
/// }
///
/// fn take_complete(i: &[u8]) -> IResult<&[u8], &[u8]> {
///   bytes::take(4u8)(i)
/// }
///
/// // both parsers will take 4 bytes as expected
/// assert_eq!(take_streaming(Streaming(&b"abcde"[..])), Ok((Streaming(&b"e"[..]), &b"abcd"[..])));
/// assert_eq!(take_complete(&b"abcde"[..]), Ok((&b"e"[..], &b"abcd"[..])));
///
/// // if the input is smaller than 4 bytes, the streaming parser
/// // will return `Incomplete` to indicate that we need more data
/// assert_eq!(take_streaming(Streaming(&b"abc"[..])), Err(Err::Incomplete(Needed::new(1))));
///
/// // but the complete parser will return an error
/// assert_eq!(take_complete(&b"abc"[..]), Err(Err::Error(Error::new(&b"abc"[..], ErrorKind::Eof))));
///
/// // the alpha0 function recognizes 0 or more alphabetic characters
/// fn alpha0_streaming(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
///   character::alpha0(i)
/// }
///
/// fn alpha0_complete(i: &str) -> IResult<&str, &str> {
///   character::alpha0(i)
/// }
///
/// // if there's a clear limit to the recognized characters, both parsers work the same way
/// assert_eq!(alpha0_streaming(Streaming("abcd;")), Ok((Streaming(";"), "abcd")));
/// assert_eq!(alpha0_complete("abcd;"), Ok((";", "abcd")));
///
/// // but when there's no limit, the streaming version returns `Incomplete`, because it cannot
/// // know if more input data should be recognized. The whole input could be "abcd;", or
/// // "abcde;"
/// assert_eq!(alpha0_streaming(Streaming("abcd")), Err(Err::Incomplete(Needed::new(1))));
///
/// // while the complete version knows that all of the data is there
/// assert_eq!(alpha0_complete("abcd"), Ok(("", "abcd")));
/// ```
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Streaming<I>(pub I);

impl<I> Streaming<I> {
  /// Convert to complete counterpart
  #[inline(always)]
  pub fn into_complete(self) -> I {
    self.0
  }
}

impl<I> crate::lib::std::ops::Deref for Streaming<I> {
  type Target = I;

  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// Number of indices input has advanced since start of parsing
pub trait Location {
  /// Number of indices input has advanced since start of parsing
  fn location(&self) -> usize;
}

impl<I> Location for Located<I>
where
  I: Clone + IntoOutput + Offset,
{
  fn location(&self) -> usize {
    self.location()
  }
}

impl<I, S> Location for Stateful<I, S>
where
  I: Location,
{
  fn location(&self) -> usize {
    self.input.location()
  }
}

impl<I> Location for Streaming<I>
where
  I: Location,
{
  fn location(&self) -> usize {
    self.0.location()
  }
}

/// Marks the input as being the complete buffer or a partial buffer for streaming input
///
/// See [Streaming] for marking a presumed complete buffer type as a streaming buffer.
pub trait InputIsStreaming<const YES: bool>: Sized {
  /// Complete counterpart
  ///
  /// - Set to `Self` if this is a complete buffer.
  /// - Set to [`std::convert::Infallible`] if there isn't an associated complete buffer type
  type Complete: InputIsStreaming<false>;
  /// Streaming counterpart
  ///
  /// - Set to `Self` if this is a streaming buffer.
  /// - Set to [`std::convert::Infallible`] if there isn't an associated streaming buffer type
  type Streaming: InputIsStreaming<true>;

  /// Convert to complete counterpart
  fn into_complete(self) -> Self::Complete;
  /// Convert to streaming counterpart
  fn into_streaming(self) -> Self::Streaming;
}

impl<I> InputIsStreaming<true> for Located<I>
where
  I: InputIsStreaming<true>,
{
  type Complete = Located<<I as InputIsStreaming<true>>::Complete>;
  type Streaming = Self;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    Located {
      initial: self.initial.into_complete(),
      input: self.input.into_complete(),
    }
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    self
  }
}

impl<I> InputIsStreaming<false> for Located<I>
where
  I: InputIsStreaming<false>,
{
  type Complete = Self;
  type Streaming = Located<<I as InputIsStreaming<false>>::Streaming>;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    Located {
      initial: self.initial.into_streaming(),
      input: self.input.into_streaming(),
    }
  }
}

impl<I, S> InputIsStreaming<true> for Stateful<I, S>
where
  I: InputIsStreaming<true>,
{
  type Complete = Stateful<<I as InputIsStreaming<true>>::Complete, S>;
  type Streaming = Self;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    Stateful {
      input: self.input.into_complete(),
      state: self.state,
    }
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    self
  }
}

impl<I, S> InputIsStreaming<false> for Stateful<I, S>
where
  I: InputIsStreaming<false>,
{
  type Complete = Self;
  type Streaming = Stateful<<I as InputIsStreaming<false>>::Streaming, S>;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    Stateful {
      input: self.input.into_streaming(),
      state: self.state,
    }
  }
}

impl<I> InputIsStreaming<true> for Streaming<I>
where
  I: InputIsStreaming<false>,
{
  type Complete = I;
  type Streaming = Self;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self.0
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    self
  }
}

impl<'a, T> InputIsStreaming<false> for &'a [T] {
  type Complete = Self;
  type Streaming = Streaming<Self>;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    Streaming(self)
  }
}

impl<T, const L: usize> InputIsStreaming<false> for [T; L] {
  type Complete = Self;
  type Streaming = Streaming<Self>;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    Streaming(self)
  }
}

impl<'a, T, const L: usize> InputIsStreaming<false> for &'a [T; L] {
  type Complete = Self;
  type Streaming = Streaming<Self>;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    Streaming(self)
  }
}

impl<'a> InputIsStreaming<false> for &'a str {
  type Complete = Self;
  type Streaming = Streaming<Self>;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    Streaming(self)
  }
}

impl<'a> InputIsStreaming<false> for (&'a [u8], usize) {
  type Complete = Self;
  type Streaming = Streaming<Self>;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    Streaming(self)
  }
}

impl<const YES: bool> InputIsStreaming<YES> for crate::lib::std::convert::Infallible {
  type Complete = Self;
  type Streaming = Self;

  #[inline(always)]
  fn into_complete(self) -> Self::Complete {
    self
  }
  #[inline(always)]
  fn into_streaming(self) -> Self::Streaming {
    self
  }
}

/// Abstract method to calculate the input length
pub trait InputLength {
  /// Calculates the input length, as indicated by its name,
  /// and the name of the trait itself
  fn input_len(&self) -> usize;
}

impl<I> InputLength for Located<I>
where
  I: InputLength,
{
  fn input_len(&self) -> usize {
    self.input.input_len()
  }
}

impl<I, S> InputLength for Stateful<I, S>
where
  I: InputLength,
{
  fn input_len(&self) -> usize {
    self.input.input_len()
  }
}

impl<I> InputLength for Streaming<I>
where
  I: InputLength,
{
  #[inline(always)]
  fn input_len(&self) -> usize {
    self.0.input_len()
  }
}

impl<'a, T> InputLength for &'a [T] {
  #[inline]
  fn input_len(&self) -> usize {
    self.len()
  }
}

impl<const LEN: usize> InputLength for [u8; LEN] {
  #[inline]
  fn input_len(&self) -> usize {
    self.len()
  }
}

impl<'a, const LEN: usize> InputLength for &'a [u8; LEN] {
  #[inline]
  fn input_len(&self) -> usize {
    self.len()
  }
}

impl<'a> InputLength for &'a str {
  #[inline]
  fn input_len(&self) -> usize {
    self.len()
  }
}

impl<'a> InputLength for (&'a [u8], usize) {
  #[inline]
  fn input_len(&self) -> usize {
    self.0.len() * 8 - self.1
  }
}

/// Useful functions to calculate the offset between slices and show a hexdump of a slice
pub trait Offset {
  /// Offset between the first byte of self and the first byte of the argument
  fn offset(&self, second: &Self) -> usize;
}

impl<I> Offset for Located<I>
where
  I: Offset,
{
  fn offset(&self, other: &Self) -> usize {
    self.input.offset(&other.input)
  }
}

impl<I, S> Offset for Stateful<I, S>
where
  I: Offset,
{
  fn offset(&self, other: &Self) -> usize {
    self.input.offset(&other.input)
  }
}

impl<I> Offset for Streaming<I>
where
  I: Offset,
{
  #[inline(always)]
  fn offset(&self, second: &Self) -> usize {
    self.0.offset(&second.0)
  }
}

impl Offset for [u8] {
  fn offset(&self, second: &Self) -> usize {
    let fst = self.as_ptr();
    let snd = second.as_ptr();

    snd as usize - fst as usize
  }
}

impl<'a> Offset for &'a [u8] {
  fn offset(&self, second: &Self) -> usize {
    let fst = self.as_ptr();
    let snd = second.as_ptr();

    snd as usize - fst as usize
  }
}

impl Offset for str {
  fn offset(&self, second: &Self) -> usize {
    let fst = self.as_ptr();
    let snd = second.as_ptr();

    snd as usize - fst as usize
  }
}

impl<'a> Offset for &'a str {
  fn offset(&self, second: &Self) -> usize {
    let fst = self.as_ptr();
    let snd = second.as_ptr();

    snd as usize - fst as usize
  }
}

/// Helper trait for types that can be viewed as a byte slice
pub trait AsBytes {
  /// Casts the input type to a byte slice
  fn as_bytes(&self) -> &[u8];
}

impl<I> AsBytes for Located<I>
where
  I: AsBytes,
{
  fn as_bytes(&self) -> &[u8] {
    self.input.as_bytes()
  }
}

impl<I, S> AsBytes for Stateful<I, S>
where
  I: AsBytes,
{
  fn as_bytes(&self) -> &[u8] {
    self.input.as_bytes()
  }
}

impl<I> AsBytes for Streaming<I>
where
  I: AsBytes,
{
  #[inline(always)]
  fn as_bytes(&self) -> &[u8] {
    self.0.as_bytes()
  }
}

impl AsBytes for [u8] {
  #[inline(always)]
  fn as_bytes(&self) -> &[u8] {
    self
  }
}

impl<'a> AsBytes for &'a [u8] {
  #[inline(always)]
  fn as_bytes(&self) -> &[u8] {
    *self
  }
}

impl<const LEN: usize> AsBytes for [u8; LEN] {
  #[inline(always)]
  fn as_bytes(&self) -> &[u8] {
    self
  }
}

impl<'a, const LEN: usize> AsBytes for &'a [u8; LEN] {
  #[inline(always)]
  fn as_bytes(&self) -> &[u8] {
    *self
  }
}

impl<'a> AsBytes for &'a str {
  #[inline(always)]
  fn as_bytes(&self) -> &[u8] {
    (*self).as_bytes()
  }
}

impl AsBytes for str {
  #[inline(always)]
  fn as_bytes(&self) -> &[u8] {
    self.as_ref()
  }
}

/// Transforms common types to a char for basic token parsing
pub trait AsChar {
  /// Makes a char from self
  ///
  /// ```
  /// use nom8::input::AsChar as _;
  ///
  /// assert_eq!('a'.as_char(), 'a');
  /// assert_eq!(u8::MAX.as_char(), std::char::from_u32(u8::MAX as u32).unwrap());
  /// ```
  fn as_char(self) -> char;

  /// Tests that self is an alphabetic character
  ///
  /// Warning: for `&str` it recognizes alphabetic
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
  #[inline]
  fn as_char(self) -> char {
    self as char
  }
  #[inline]
  fn is_alpha(self) -> bool {
    (self >= 0x41 && self <= 0x5A) || (self >= 0x61 && self <= 0x7A)
  }
  #[inline]
  fn is_alphanum(self) -> bool {
    self.is_alpha() || self.is_dec_digit()
  }
  #[inline]
  fn is_dec_digit(self) -> bool {
    self >= 0x30 && self <= 0x39
  }
  #[inline]
  fn is_hex_digit(self) -> bool {
    (self >= 0x30 && self <= 0x39)
      || (self >= 0x41 && self <= 0x46)
      || (self >= 0x61 && self <= 0x66)
  }
  #[inline]
  fn is_oct_digit(self) -> bool {
    self >= 0x30 && self <= 0x37
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
  #[inline]
  fn as_char(self) -> char {
    *self as char
  }
  #[inline]
  fn is_alpha(self) -> bool {
    (*self >= 0x41 && *self <= 0x5A) || (*self >= 0x61 && *self <= 0x7A)
  }
  #[inline]
  fn is_alphanum(self) -> bool {
    self.is_alpha() || self.is_dec_digit()
  }
  #[inline]
  fn is_dec_digit(self) -> bool {
    *self >= 0x30 && *self <= 0x39
  }
  #[inline]
  fn is_hex_digit(self) -> bool {
    (*self >= 0x30 && *self <= 0x39)
      || (*self >= 0x41 && *self <= 0x46)
      || (*self >= 0x61 && *self <= 0x66)
  }
  #[inline]
  fn is_oct_digit(self) -> bool {
    *self >= 0x30 && *self <= 0x37
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
  #[inline]
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
  #[inline]
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

/// Abstracts common iteration operations on the input type
pub trait InputIter {
  /// The current input type is a sequence of that `Item` type.
  ///
  /// Example: `u8` for `&[u8]` or `char` for `&str`
  type Item;
  /// An iterator over the input type, producing the item and its position
  /// for use with [Slice]. If we're iterating over `&str`, the position
  /// corresponds to the byte index of the character
  type Iter: Iterator<Item = (usize, Self::Item)>;

  /// An iterator over the input type, producing the item
  type IterElem: Iterator<Item = Self::Item>;

  /// Returns an iterator over the elements and their byte offsets
  fn iter_indices(&self) -> Self::Iter;
  /// Returns an iterator over the elements
  fn iter_elements(&self) -> Self::IterElem;
  /// Finds the byte position of the element
  fn position<P>(&self, predicate: P) -> Option<usize>
  where
    P: Fn(Self::Item) -> bool;
  /// Get the byte offset from the element's position in the stream
  fn slice_index(&self, count: usize) -> Result<usize, Needed>;
}

impl<I> InputIter for Located<I>
where
  I: InputIter,
{
  type Item = I::Item;
  type Iter = I::Iter;
  type IterElem = I::IterElem;

  fn iter_indices(&self) -> Self::Iter {
    self.input.iter_indices()
  }

  fn iter_elements(&self) -> Self::IterElem {
    self.input.iter_elements()
  }

  fn position<P>(&self, predicate: P) -> Option<usize>
  where
    P: Fn(Self::Item) -> bool,
  {
    self.input.position(predicate)
  }

  fn slice_index(&self, count: usize) -> Result<usize, Needed> {
    self.input.slice_index(count)
  }
}

impl<I, S> InputIter for Stateful<I, S>
where
  I: InputIter,
{
  type Item = I::Item;
  type Iter = I::Iter;
  type IterElem = I::IterElem;

  fn iter_indices(&self) -> Self::Iter {
    self.input.iter_indices()
  }

  fn iter_elements(&self) -> Self::IterElem {
    self.input.iter_elements()
  }

  fn position<P>(&self, predicate: P) -> Option<usize>
  where
    P: Fn(Self::Item) -> bool,
  {
    self.input.position(predicate)
  }

  fn slice_index(&self, count: usize) -> Result<usize, Needed> {
    self.input.slice_index(count)
  }
}

impl<I> InputIter for Streaming<I>
where
  I: InputIter,
{
  type Item = I::Item;
  type Iter = I::Iter;
  type IterElem = I::IterElem;

  #[inline(always)]
  fn iter_indices(&self) -> Self::Iter {
    self.0.iter_indices()
  }
  #[inline(always)]
  fn iter_elements(&self) -> Self::IterElem {
    self.0.iter_elements()
  }
  #[inline(always)]
  fn position<P>(&self, predicate: P) -> Option<usize>
  where
    P: Fn(Self::Item) -> bool,
  {
    self.0.position(predicate)
  }
  #[inline(always)]
  fn slice_index(&self, count: usize) -> Result<usize, Needed> {
    self.0.slice_index(count)
  }
}
impl<'a> InputIter for &'a [u8] {
  type Item = u8;
  type Iter = Enumerate<Self::IterElem>;
  type IterElem = Copied<Iter<'a, u8>>;

  #[inline]
  fn iter_indices(&self) -> Self::Iter {
    self.iter_elements().enumerate()
  }
  #[inline]
  fn iter_elements(&self) -> Self::IterElem {
    self.iter().copied()
  }
  #[inline]
  fn position<P>(&self, predicate: P) -> Option<usize>
  where
    P: Fn(Self::Item) -> bool,
  {
    self.iter().position(|b| predicate(*b))
  }
  #[inline]
  fn slice_index(&self, count: usize) -> Result<usize, Needed> {
    if let Some(needed) = count.checked_sub(self.len()).and_then(NonZeroUsize::new) {
      Err(Needed::Size(needed))
    } else {
      Ok(count)
    }
  }
}

impl<'a, const LEN: usize> InputIter for &'a [u8; LEN] {
  type Item = u8;
  type Iter = Enumerate<Self::IterElem>;
  type IterElem = Copied<Iter<'a, u8>>;

  fn iter_indices(&self) -> Self::Iter {
    (&self[..]).iter_indices()
  }

  fn iter_elements(&self) -> Self::IterElem {
    (&self[..]).iter_elements()
  }

  fn position<P>(&self, predicate: P) -> Option<usize>
  where
    P: Fn(Self::Item) -> bool,
  {
    (&self[..]).position(predicate)
  }

  fn slice_index(&self, count: usize) -> Result<usize, Needed> {
    (&self[..]).slice_index(count)
  }
}

impl<'a> InputIter for &'a str {
  type Item = char;
  type Iter = CharIndices<'a>;
  type IterElem = Chars<'a>;
  #[inline]
  fn iter_indices(&self) -> Self::Iter {
    self.char_indices()
  }
  #[inline]
  fn iter_elements(&self) -> Self::IterElem {
    self.chars()
  }
  fn position<P>(&self, predicate: P) -> Option<usize>
  where
    P: Fn(Self::Item) -> bool,
  {
    for (o, c) in self.char_indices() {
      if predicate(c) {
        return Some(o);
      }
    }
    None
  }
  #[inline]
  fn slice_index(&self, count: usize) -> Result<usize, Needed> {
    let mut cnt = 0;
    for (index, _) in self.char_indices() {
      if cnt == count {
        return Ok(index);
      }
      cnt += 1;
    }
    if cnt == count {
      return Ok(self.len());
    }
    Err(Needed::Unknown)
  }
}

/// Abstracts slicing operations
pub trait InputTake: Sized {
  /// Returns a slice of `count` bytes. panics if count > length
  fn take(&self, count: usize) -> Self;
  /// Split the stream at the `count` byte offset. panics if count > length
  fn take_split(&self, count: usize) -> (Self, Self);
}

impl<I> InputTake for Located<I>
where
  I: InputTake + Clone,
{
  fn take(&self, count: usize) -> Self {
    Self {
      initial: self.initial.clone(),
      input: self.input.take(count),
    }
  }

  fn take_split(&self, count: usize) -> (Self, Self) {
    let (left, right) = self.input.take_split(count);
    (
      Self {
        initial: self.initial.clone(),
        input: left,
      },
      Self {
        initial: self.initial.clone(),
        input: right,
      },
    )
  }
}

impl<I, S> InputTake for Stateful<I, S>
where
  I: InputTake,
  S: Clone,
{
  fn take(&self, count: usize) -> Self {
    Self {
      input: self.input.take(count),
      state: self.state.clone(),
    }
  }

  fn take_split(&self, count: usize) -> (Self, Self) {
    let (left, right) = self.input.take_split(count);
    (
      Self {
        input: left,
        state: self.state.clone(),
      },
      Self {
        input: right,
        state: self.state.clone(),
      },
    )
  }
}

impl<I> InputTake for Streaming<I>
where
  I: InputTake,
{
  #[inline(always)]
  fn take(&self, count: usize) -> Self {
    Streaming(self.0.take(count))
  }
  #[inline(always)]
  fn take_split(&self, count: usize) -> (Self, Self) {
    let (start, end) = self.0.take_split(count);
    (Streaming(start), Streaming(end))
  }
}

impl<'a> InputTake for &'a [u8] {
  #[inline]
  fn take(&self, count: usize) -> Self {
    &self[0..count]
  }
  #[inline]
  fn take_split(&self, count: usize) -> (Self, Self) {
    let (prefix, suffix) = self.split_at(count);
    (suffix, prefix)
  }
}

impl<'a> InputTake for &'a str {
  #[inline]
  fn take(&self, count: usize) -> Self {
    &self[..count]
  }

  // return byte index
  #[inline]
  fn take_split(&self, count: usize) -> (Self, Self) {
    let (prefix, suffix) = self.split_at(count);
    (suffix, prefix)
  }
}

/// Dummy trait used for default implementations (currently only used for `InputTakeAtPosition` and `Compare`).
///
/// When implementing a custom input type, it is possible to use directly the
/// default implementation: If the input type implements `InputLength`, `InputIter`,
/// `InputTake` and `Clone`, you can implement `UnspecializedInput` and get
/// a default version of `InputTakeAtPosition` and `Compare`.
///
/// For performance reasons, you might want to write a custom implementation of
/// `InputTakeAtPosition` (like the one for `&[u8]`).
pub trait UnspecializedInput {}

/// Methods to take as much input as possible until the provided function returns true for the current element.
///
/// A large part of nom's basic parsers are built using this trait.
pub trait InputTakeAtPosition: Sized {
  /// The current input type is a sequence of that `Item` type.
  ///
  /// Example: `u8` for `&[u8]` or `char` for `&str`
  type Item;

  /// Looks for the first element of the input type for which the condition returns true,
  /// and returns the input up to this position.
  ///
  /// *streaming version*: If no element is found matching the condition, this will return `Incomplete`
  fn split_at_position_streaming<P, E: ParseError<Self>>(
    &self,
    predicate: P,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool;

  /// Looks for the first element of the input type for which the condition returns true
  /// and returns the input up to this position.
  ///
  /// Fails if the produced slice is empty.
  ///
  /// *streaming version*: If no element is found matching the condition, this will return `Incomplete`
  fn split_at_position1_streaming<P, E: ParseError<Self>>(
    &self,
    predicate: P,
    e: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool;

  /// Looks for the first element of the input type for which the condition returns true,
  /// and returns the input up to this position.
  ///
  /// *complete version*: If no element is found matching the condition, this will return the whole input
  fn split_at_position_complete<P, E: ParseError<Self>>(
    &self,
    predicate: P,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool;

  /// Looks for the first element of the input type for which the condition returns true
  /// and returns the input up to this position.
  ///
  /// Fails if the produced slice is empty.
  ///
  /// *complete version*: If no element is found matching the condition, this will return the whole input
  fn split_at_position1_complete<P, E: ParseError<Self>>(
    &self,
    predicate: P,
    e: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool;
}

impl<I> InputTakeAtPosition for Located<I>
where
  I: InputTakeAtPosition + Clone,
{
  type Item = <I as InputTakeAtPosition>::Item;

  fn split_at_position_complete<P, E>(&self, predicate: P) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    located_clone_map_result(self, move |data| data.split_at_position_complete(predicate))
  }

  fn split_at_position_streaming<P, E>(&self, predicate: P) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    located_clone_map_result(self, move |data| {
      data.split_at_position_streaming(predicate)
    })
  }

  fn split_at_position1_streaming<P, E>(
    &self,
    predicate: P,
    kind: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    located_clone_map_result(self, move |data| {
      data.split_at_position1_streaming(predicate, kind)
    })
  }

  fn split_at_position1_complete<P, E>(
    &self,
    predicate: P,
    kind: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    located_clone_map_result(self, move |data| {
      data.split_at_position1_complete(predicate, kind)
    })
  }
}

fn located_clone_map_result<I, E, F>(input: &Located<I>, f: F) -> IResult<Located<I>, Located<I>, E>
where
  I: Clone,
  E: ParseError<Located<I>>,
  F: FnOnce(&I) -> IResult<I, I>,
{
  let map_error = |error: crate::error::Error<I>| {
    E::from_error_kind(
      Located {
        initial: input.initial.clone(),
        input: error.input,
      },
      error.code,
    )
  };
  f(&input.input)
    .map(|(remaining, output)| {
      (
        Located {
          initial: input.initial.clone(),
          input: remaining,
        },
        Located {
          initial: input.initial.clone(),
          input: output,
        },
      )
    })
    .map_err(|error| match error {
      Err::Error(error) => Err::Error(map_error(error)),
      Err::Failure(error) => Err::Failure(map_error(error)),
      Err::Incomplete(needed) => Err::Incomplete(needed),
    })
}

impl<I, S> InputTakeAtPosition for Stateful<I, S>
where
  I: InputTakeAtPosition,
  S: Clone,
{
  type Item = <I as InputTakeAtPosition>::Item;

  fn split_at_position_complete<P, E>(&self, predicate: P) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    stateful_clone_map_result(self, move |data| data.split_at_position_complete(predicate))
  }

  fn split_at_position_streaming<P, E>(&self, predicate: P) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    stateful_clone_map_result(self, move |data| {
      data.split_at_position_streaming(predicate)
    })
  }

  fn split_at_position1_streaming<P, E>(
    &self,
    predicate: P,
    kind: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    stateful_clone_map_result(self, move |data| {
      data.split_at_position1_streaming(predicate, kind)
    })
  }

  fn split_at_position1_complete<P, E>(
    &self,
    predicate: P,
    kind: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    stateful_clone_map_result(self, move |data| {
      data.split_at_position1_complete(predicate, kind)
    })
  }
}

fn stateful_clone_map_result<I, S, E, F>(
  input: &Stateful<I, S>,
  f: F,
) -> IResult<Stateful<I, S>, Stateful<I, S>, E>
where
  E: ParseError<Stateful<I, S>>,
  S: Clone,
  F: FnOnce(&I) -> IResult<I, I>,
{
  let map_error = |error: crate::error::Error<I>| {
    E::from_error_kind(
      Stateful {
        input: error.input,
        state: input.state.clone(),
      },
      error.code,
    )
  };
  f(&input.input)
    .map(|(remaining, output)| {
      (
        Stateful {
          input: remaining,
          state: input.state.clone(),
        },
        Stateful {
          input: output,
          state: input.state.clone(),
        },
      )
    })
    .map_err(|error| match error {
      Err::Error(error) => Err::Error(map_error(error)),
      Err::Failure(error) => Err::Failure(map_error(error)),
      Err::Incomplete(needed) => Err::Incomplete(needed),
    })
}

impl<I> InputTakeAtPosition for Streaming<I>
where
  I: InputTakeAtPosition,
{
  type Item = <I as InputTakeAtPosition>::Item;

  fn split_at_position_complete<P, E>(&self, predicate: P) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    streaming_clone_map_result(self, move |data| data.split_at_position_complete(predicate))
  }

  fn split_at_position_streaming<P, E>(&self, predicate: P) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    streaming_clone_map_result(self, move |data| {
      data.split_at_position_streaming(predicate)
    })
  }

  fn split_at_position1_streaming<P, E>(
    &self,
    predicate: P,
    kind: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    streaming_clone_map_result(self, move |data| {
      data.split_at_position1_streaming(predicate, kind)
    })
  }

  fn split_at_position1_complete<P, E>(
    &self,
    predicate: P,
    kind: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
    E: ParseError<Self>,
  {
    streaming_clone_map_result(self, move |data| {
      data.split_at_position1_complete(predicate, kind)
    })
  }
}

fn streaming_clone_map_result<I, E, F>(
  input: &Streaming<I>,
  f: F,
) -> IResult<Streaming<I>, Streaming<I>, E>
where
  E: ParseError<Streaming<I>>,
  F: FnOnce(&I) -> IResult<I, I>,
{
  let map_error =
    |error: crate::error::Error<I>| E::from_error_kind(Streaming(error.input), error.code);
  f(&input.0)
    .map(|(remaining, output)| (Streaming(remaining), Streaming(output)))
    .map_err(|error| match error {
      Err::Error(error) => Err::Error(map_error(error)),
      Err::Failure(error) => Err::Failure(map_error(error)),
      Err::Incomplete(needed) => Err::Incomplete(needed),
    })
}

impl<I: InputLength + InputIter + InputTake + Clone + UnspecializedInput> InputTakeAtPosition
  for I
{
  type Item = <I as InputIter>::Item;

  fn split_at_position_streaming<P, E: ParseError<Self>>(
    &self,
    predicate: P,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.position(predicate) {
      Some(n) => Ok(self.take_split(n)),
      None => Err(Err::Incomplete(Needed::new(1))),
    }
  }

  fn split_at_position1_streaming<P, E: ParseError<Self>>(
    &self,
    predicate: P,
    e: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.position(predicate) {
      Some(0) => Err(Err::Error(E::from_error_kind(self.clone(), e))),
      Some(n) => Ok(self.take_split(n)),
      None => Err(Err::Incomplete(Needed::new(1))),
    }
  }

  fn split_at_position_complete<P, E: ParseError<Self>>(
    &self,
    predicate: P,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.position(predicate) {
      Some(n) => Ok(self.take_split(n)),
      None => Ok(self.take_split(self.input_len())),
    }
  }

  fn split_at_position1_complete<P, E: ParseError<Self>>(
    &self,
    predicate: P,
    e: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.position(predicate) {
      Some(0) => Err(Err::Error(E::from_error_kind(self.clone(), e))),
      Some(n) => Ok(self.take_split(n)),
      None => {
        if self.input_len() == 0 {
          Err(Err::Error(E::from_error_kind(self.clone(), e)))
        } else {
          Ok(self.take_split(self.input_len()))
        }
      }
    }
  }
}

impl<'a> InputTakeAtPosition for &'a [u8] {
  type Item = u8;

  fn split_at_position_streaming<P, E: ParseError<Self>>(
    &self,
    predicate: P,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.iter().position(|c| predicate(*c)) {
      Some(i) => Ok(self.take_split(i)),
      None => Err(Err::Incomplete(Needed::new(1))),
    }
  }

  fn split_at_position1_streaming<P, E: ParseError<Self>>(
    &self,
    predicate: P,
    e: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.iter().position(|c| predicate(*c)) {
      Some(0) => Err(Err::Error(E::from_error_kind(self, e))),
      Some(i) => Ok(self.take_split(i)),
      None => Err(Err::Incomplete(Needed::new(1))),
    }
  }

  fn split_at_position_complete<P, E: ParseError<Self>>(
    &self,
    predicate: P,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.iter().position(|c| predicate(*c)) {
      Some(i) => Ok(self.take_split(i)),
      None => Ok(self.take_split(self.input_len())),
    }
  }

  fn split_at_position1_complete<P, E: ParseError<Self>>(
    &self,
    predicate: P,
    e: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.iter().position(|c| predicate(*c)) {
      Some(0) => Err(Err::Error(E::from_error_kind(self, e))),
      Some(i) => Ok(self.take_split(i)),
      None => {
        if self.is_empty() {
          Err(Err::Error(E::from_error_kind(self, e)))
        } else {
          Ok(self.take_split(self.input_len()))
        }
      }
    }
  }
}

impl<'a> InputTakeAtPosition for &'a str {
  type Item = char;

  fn split_at_position_streaming<P, E: ParseError<Self>>(
    &self,
    predicate: P,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.find(predicate) {
      // find() returns a byte index that is already in the slice at a char boundary
      Some(i) => unsafe { Ok((self.get_unchecked(i..), self.get_unchecked(..i))) },
      None => Err(Err::Incomplete(Needed::new(1))),
    }
  }

  fn split_at_position1_streaming<P, E: ParseError<Self>>(
    &self,
    predicate: P,
    e: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.find(predicate) {
      Some(0) => Err(Err::Error(E::from_error_kind(self, e))),
      // find() returns a byte index that is already in the slice at a char boundary
      Some(i) => unsafe { Ok((self.get_unchecked(i..), self.get_unchecked(..i))) },
      None => Err(Err::Incomplete(Needed::new(1))),
    }
  }

  fn split_at_position_complete<P, E: ParseError<Self>>(
    &self,
    predicate: P,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.find(predicate) {
      // find() returns a byte index that is already in the slice at a char boundary
      Some(i) => unsafe { Ok((self.get_unchecked(i..), self.get_unchecked(..i))) },
      // the end of slice is a char boundary
      None => unsafe {
        Ok((
          self.get_unchecked(self.len()..),
          self.get_unchecked(..self.len()),
        ))
      },
    }
  }

  fn split_at_position1_complete<P, E: ParseError<Self>>(
    &self,
    predicate: P,
    e: ErrorKind,
  ) -> IResult<Self, Self, E>
  where
    P: Fn(Self::Item) -> bool,
  {
    match self.find(predicate) {
      Some(0) => Err(Err::Error(E::from_error_kind(self, e))),
      // find() returns a byte index that is already in the slice at a char boundary
      Some(i) => unsafe { Ok((self.get_unchecked(i..), self.get_unchecked(..i))) },
      None => {
        if self.is_empty() {
          Err(Err::Error(E::from_error_kind(self, e)))
        } else {
          // the end of slice is a char boundary
          unsafe {
            Ok((
              self.get_unchecked(self.len()..),
              self.get_unchecked(..self.len()),
            ))
          }
        }
      }
    }
  }
}

/// Indicates whether a comparison was successful, an error, or
/// if more data was needed
#[derive(Debug, PartialEq)]
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

impl<I, U> Compare<U> for Located<I>
where
  I: Compare<U>,
{
  fn compare(&self, other: U) -> CompareResult {
    self.input.compare(other)
  }

  fn compare_no_case(&self, other: U) -> CompareResult {
    self.input.compare_no_case(other)
  }
}

impl<I, S, U> Compare<U> for Stateful<I, S>
where
  I: Compare<U>,
{
  fn compare(&self, other: U) -> CompareResult {
    self.input.compare(other)
  }

  fn compare_no_case(&self, other: U) -> CompareResult {
    self.input.compare_no_case(other)
  }
}

impl<I, T> Compare<T> for Streaming<I>
where
  I: Compare<T>,
{
  #[inline(always)]
  fn compare(&self, t: T) -> CompareResult {
    self.0.compare(t)
  }

  #[inline(always)]
  fn compare_no_case(&self, t: T) -> CompareResult {
    self.0.compare_no_case(t)
  }
}

fn lowercase_byte(c: u8) -> u8 {
  match c {
    b'A'..=b'Z' => c - b'A' + b'a',
    _ => c,
  }
}

impl<'a, 'b> Compare<&'b [u8]> for &'a [u8] {
  #[inline(always)]
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

  #[inline(always)]
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

impl<
    T: InputLength + InputIter<Item = u8> + InputTake + UnspecializedInput,
    O: InputLength + InputIter<Item = u8> + InputTake,
  > Compare<O> for T
{
  #[inline(always)]
  fn compare(&self, t: O) -> CompareResult {
    let pos = self
      .iter_elements()
      .zip(t.iter_elements())
      .position(|(a, b)| a != b);

    match pos {
      Some(_) => CompareResult::Error,
      None => {
        if self.input_len() >= t.input_len() {
          CompareResult::Ok
        } else {
          CompareResult::Incomplete
        }
      }
    }
  }

  #[inline(always)]
  fn compare_no_case(&self, t: O) -> CompareResult {
    if self
      .iter_elements()
      .zip(t.iter_elements())
      .any(|(a, b)| lowercase_byte(a) != lowercase_byte(b))
    {
      CompareResult::Error
    } else if self.input_len() < t.input_len() {
      CompareResult::Incomplete
    } else {
      CompareResult::Ok
    }
  }
}

impl<'a, 'b> Compare<&'b str> for &'a [u8] {
  #[inline(always)]
  fn compare(&self, t: &'b str) -> CompareResult {
    self.compare(AsBytes::as_bytes(t))
  }
  #[inline(always)]
  fn compare_no_case(&self, t: &'b str) -> CompareResult {
    self.compare_no_case(AsBytes::as_bytes(t))
  }
}

impl<'a, 'b> Compare<&'b str> for &'a str {
  #[inline(always)]
  fn compare(&self, t: &'b str) -> CompareResult {
    self.as_bytes().compare(t.as_bytes())
  }

  //FIXME: this version is too simple and does not use the current locale
  #[inline(always)]
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
    AsBytes::as_bytes(self).compare(t)
  }
  #[inline(always)]
  fn compare_no_case(&self, t: &'b [u8]) -> CompareResult {
    AsBytes::as_bytes(self).compare_no_case(t)
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
/// For example, you could implement `hex_digit0` as:
/// ```
/// # use nom8::prelude::*;
/// # use nom8::{Err, error::ErrorKind, error::Error, Needed};
/// # use nom8::bytes::take_while1;
/// fn hex_digit1(input: &str) -> IResult<&str, &str> {
///     take_while1(('a'..='f', 'A'..='F', '0'..='9')).parse(input)
/// }
///
/// assert_eq!(hex_digit1("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(hex_digit1("H2"), Err(Err::Error(Error::new("H2", ErrorKind::TakeWhile1))));
/// assert_eq!(hex_digit1(""), Err(Err::Error(Error::new("", ErrorKind::TakeWhile1))));
/// ```
pub trait FindToken<T> {
  /// Returns true if self contains the token
  fn find_token(&self, token: T) -> bool;
}

impl<I, T> FindToken<T> for Streaming<I>
where
  I: FindToken<T>,
{
  #[inline(always)]
  fn find_token(&self, token: T) -> bool {
    self.0.find_token(token)
  }
}

impl FindToken<u8> for u8 {
  fn find_token(&self, token: u8) -> bool {
    *self == token
  }
}

impl<'a> FindToken<&'a u8> for u8 {
  fn find_token(&self, token: &u8) -> bool {
    self.find_token(*token)
  }
}

impl FindToken<char> for u8 {
  fn find_token(&self, token: char) -> bool {
    self.as_char() == token
  }
}

impl<'a> FindToken<&'a char> for u8 {
  fn find_token(&self, token: &char) -> bool {
    self.find_token(*token)
  }
}

impl<C: AsChar> FindToken<C> for char {
  fn find_token(&self, token: C) -> bool {
    *self == token.as_char()
  }
}

impl<C: AsChar, F: Fn(C) -> bool> FindToken<C> for F {
  fn find_token(&self, token: C) -> bool {
    self(token)
  }
}

impl<C1: AsChar, C2: AsChar + Clone> FindToken<C1> for Range<C2> {
  fn find_token(&self, token: C1) -> bool {
    let start = self.start.clone().as_char();
    let end = self.end.clone().as_char();
    (start..end).contains(&token.as_char())
  }
}

impl<C1: AsChar, C2: AsChar + Clone> FindToken<C1> for RangeInclusive<C2> {
  fn find_token(&self, token: C1) -> bool {
    let start = self.start().clone().as_char();
    let end = self.end().clone().as_char();
    (start..=end).contains(&token.as_char())
  }
}

impl<C1: AsChar, C2: AsChar + Clone> FindToken<C1> for RangeFrom<C2> {
  fn find_token(&self, token: C1) -> bool {
    let start = self.start.clone().as_char();
    (start..).contains(&token.as_char())
  }
}

impl<C1: AsChar, C2: AsChar + Clone> FindToken<C1> for RangeTo<C2> {
  fn find_token(&self, token: C1) -> bool {
    let end = self.end.clone().as_char();
    (..end).contains(&token.as_char())
  }
}

impl<C1: AsChar, C2: AsChar + Clone> FindToken<C1> for RangeToInclusive<C2> {
  fn find_token(&self, token: C1) -> bool {
    let end = self.end.clone().as_char();
    (..=end).contains(&token.as_char())
  }
}

impl<C1: AsChar> FindToken<C1> for RangeFull {
  fn find_token(&self, _token: C1) -> bool {
    true
  }
}

impl<'a> FindToken<u8> for &'a [u8] {
  fn find_token(&self, token: u8) -> bool {
    memchr::memchr(token, self).is_some()
  }
}

impl<'a, 'b> FindToken<&'a u8> for &'b [u8] {
  fn find_token(&self, token: &u8) -> bool {
    self.find_token(*token)
  }
}

impl<'a> FindToken<char> for &'a [u8] {
  fn find_token(&self, token: char) -> bool {
    self.iter().any(|i| i.as_char() == token)
  }
}

impl<'a, 'b> FindToken<&'a char> for &'b [u8] {
  fn find_token(&self, token: &char) -> bool {
    self.find_token(*token)
  }
}

impl<const LEN: usize> FindToken<u8> for [u8; LEN] {
  fn find_token(&self, token: u8) -> bool {
    memchr::memchr(token, &self[..]).is_some()
  }
}

impl<'a, const LEN: usize> FindToken<&'a u8> for [u8; LEN] {
  fn find_token(&self, token: &u8) -> bool {
    self.find_token(*token)
  }
}

impl<'a, const LEN: usize> FindToken<char> for [u8; LEN] {
  fn find_token(&self, token: char) -> bool {
    self.iter().any(|i| i.as_char() == token)
  }
}

impl<'a, const LEN: usize> FindToken<&'a char> for [u8; LEN] {
  fn find_token(&self, token: &char) -> bool {
    self.find_token(*token)
  }
}

impl<'a> FindToken<u8> for &'a str {
  fn find_token(&self, token: u8) -> bool {
    self.as_bytes().find_token(token)
  }
}

impl<'a, 'b> FindToken<&'a u8> for &'b str {
  fn find_token(&self, token: &u8) -> bool {
    self.as_bytes().find_token(token)
  }
}

impl<'a> FindToken<char> for &'a str {
  fn find_token(&self, token: char) -> bool {
    self.chars().any(|i| i == token)
  }
}

impl<'a, 'b> FindToken<&'a char> for &'b str {
  fn find_token(&self, token: &char) -> bool {
    self.find_token(*token)
  }
}

impl<'a> FindToken<u8> for &'a [char] {
  fn find_token(&self, token: u8) -> bool {
    self.iter().any(|i| *i == token.as_char())
  }
}

impl<'a, 'b> FindToken<&'a u8> for &'b [char] {
  fn find_token(&self, token: &u8) -> bool {
    self.find_token(*token)
  }
}

impl<'a> FindToken<char> for &'a [char] {
  fn find_token(&self, token: char) -> bool {
    self.iter().any(|i| *i == token)
  }
}

impl<'a, 'b> FindToken<&'a char> for &'b [char] {
  fn find_token(&self, token: &char) -> bool {
    self.find_token(*token)
  }
}

impl<T> FindToken<T> for () {
  fn find_token(&self, _token: T) -> bool {
    false
  }
}

macro_rules! impl_find_token_for_tuple {
  ($($haystack:ident),+) => (
    #[allow(non_snake_case)]
    impl<T, $($haystack),+> FindToken<T> for ($($haystack),+,)
    where
    T: Clone,
      $($haystack: FindToken<T>),+
    {
      fn find_token(&self, token: T) -> bool {
        let ($(ref $haystack),+,) = *self;
        $($haystack.find_token(token.clone()) || )+ false
      }
    }
  )
}

macro_rules! impl_find_token_for_tuples {
    ($haystack1:ident, $($haystack:ident),+) => {
        impl_find_token_for_tuples!(__impl $haystack1; $($haystack),+);
    };
    (__impl $($haystack:ident),+; $haystack1:ident $(,$haystack2:ident)*) => {
        impl_find_token_for_tuple!($($haystack),+);
        impl_find_token_for_tuples!(__impl $($haystack),+, $haystack1; $($haystack2),*);
    };
    (__impl $($haystack:ident),+;) => {
        impl_find_token_for_tuple!($($haystack),+);
    }
}

impl_find_token_for_tuples!(
  F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21
);

/// Look for a substring in self
pub trait FindSubstring<T> {
  /// Returns the byte position of the substring if it is found
  fn find_substring(&self, substr: T) -> Option<usize>;
}

impl<I, T> FindSubstring<T> for Located<I>
where
  I: FindSubstring<T>,
{
  #[inline(always)]
  fn find_substring(&self, substr: T) -> Option<usize> {
    self.input.find_substring(substr)
  }
}

impl<I, S, T> FindSubstring<T> for Stateful<I, S>
where
  I: FindSubstring<T>,
{
  #[inline(always)]
  fn find_substring(&self, substr: T) -> Option<usize> {
    self.input.find_substring(substr)
  }
}

impl<I, T> FindSubstring<T> for Streaming<I>
where
  I: FindSubstring<T>,
{
  #[inline(always)]
  fn find_substring(&self, substr: T) -> Option<usize> {
    self.0.find_substring(substr)
  }
}

impl<'a, 'b> FindSubstring<&'b [u8]> for &'a [u8] {
  fn find_substring(&self, substr: &'b [u8]) -> Option<usize> {
    if substr.len() > self.len() {
      return None;
    }

    let (&substr_first, substr_rest) = match substr.split_first() {
      Some(split) => split,
      // an empty substring is found at position 0
      // This matches the behavior of str.find("").
      None => return Some(0),
    };

    if substr_rest.is_empty() {
      return memchr::memchr(substr_first, self);
    }

    let mut offset = 0;
    let haystack = &self[..self.len() - substr_rest.len()];

    while let Some(position) = memchr::memchr(substr_first, &haystack[offset..]) {
      offset += position;
      let next_offset = offset + 1;
      if &self[next_offset..][..substr_rest.len()] == substr_rest {
        return Some(offset);
      }

      offset = next_offset;
    }

    None
  }
}

impl<'a, 'b> FindSubstring<&'b str> for &'a [u8] {
  fn find_substring(&self, substr: &'b str) -> Option<usize> {
    self.find_substring(AsBytes::as_bytes(substr))
  }
}

impl<'a, 'b> FindSubstring<&'b str> for &'a str {
  //returns byte index
  fn find_substring(&self, substr: &'b str) -> Option<usize> {
    self.find(substr)
  }
}

/// Used to integrate `str`'s `parse()` method
pub trait ParseTo<R> {
  /// Succeeds if `parse()` succeeded. The byte slice implementation
  /// will first convert it to a `&str`, then apply the `parse()` function
  fn parse_to(&self) -> Option<R>;
}

impl<I, R> ParseTo<R> for Located<I>
where
  I: ParseTo<R>,
{
  #[inline(always)]
  fn parse_to(&self) -> Option<R> {
    self.input.parse_to()
  }
}

impl<I, S, R> ParseTo<R> for Stateful<I, S>
where
  I: ParseTo<R>,
{
  #[inline(always)]
  fn parse_to(&self) -> Option<R> {
    self.input.parse_to()
  }
}

impl<I, R> ParseTo<R> for Streaming<I>
where
  I: ParseTo<R>,
{
  #[inline(always)]
  fn parse_to(&self) -> Option<R> {
    self.0.parse_to()
  }
}

impl<'a, R: FromStr> ParseTo<R> for &'a [u8] {
  fn parse_to(&self) -> Option<R> {
    from_utf8(self).ok().and_then(|s| s.parse().ok())
  }
}

impl<'a, R: FromStr> ParseTo<R> for &'a str {
  fn parse_to(&self) -> Option<R> {
    self.parse().ok()
  }
}

/// Slicing operations using ranges.
///
/// This trait is loosely based on
/// `Index`, but can actually return
/// something else than a `&[T]` or `&str`
pub trait Slice<R> {
  /// Slices self according to the range argument
  fn slice(&self, range: R) -> Self;
}

impl<I, R> Slice<R> for Located<I>
where
  I: Slice<R> + Clone,
{
  #[inline(always)]
  fn slice(&self, range: R) -> Self {
    Located {
      initial: self.initial.clone(),
      input: self.input.slice(range),
    }
  }
}

impl<I, S, R> Slice<R> for Stateful<I, S>
where
  I: Slice<R>,
  S: Clone,
{
  #[inline(always)]
  fn slice(&self, range: R) -> Self {
    Self {
      input: self.input.slice(range),
      state: self.state.clone(),
    }
  }
}

impl<I, R> Slice<R> for Streaming<I>
where
  I: Slice<R>,
{
  #[inline(always)]
  fn slice(&self, range: R) -> Self {
    Streaming(self.0.slice(range))
  }
}

macro_rules! impl_fn_slice {
  ( $ty:ty ) => {
    fn slice(&self, range: $ty) -> Self {
      &self[range]
    }
  };
}

macro_rules! slice_range_impl {
  ( [ $for_type:ident ], $ty:ty ) => {
    impl<'a, $for_type> Slice<$ty> for &'a [$for_type] {
      impl_fn_slice!($ty);
    }
  };
  ( $for_type:ty, $ty:ty ) => {
    impl<'a> Slice<$ty> for &'a $for_type {
      impl_fn_slice!($ty);
    }
  };
}

macro_rules! slice_ranges_impl {
  ( [ $for_type:ident ] ) => {
    slice_range_impl! {[$for_type], Range<usize>}
    slice_range_impl! {[$for_type], RangeTo<usize>}
    slice_range_impl! {[$for_type], RangeFrom<usize>}
    slice_range_impl! {[$for_type], RangeFull}
  };
  ( $for_type:ty ) => {
    slice_range_impl! {$for_type, Range<usize>}
    slice_range_impl! {$for_type, RangeTo<usize>}
    slice_range_impl! {$for_type, RangeFrom<usize>}
    slice_range_impl! {$for_type, RangeFull}
  };
}

slice_ranges_impl! {[T]}
slice_ranges_impl! {str}

/// Convert an `Input` into an appropriate `Output` type
pub trait IntoOutput {
  /// Output type
  type Output;
  /// Convert an `Input` into an appropriate `Output` type
  fn into_output(self) -> Self::Output;
  /// Convert an `Output` type to be used as `Input`
  fn merge_output(self, inner: Self::Output) -> Self;
}

impl<I> IntoOutput for Located<I>
where
  I: IntoOutput,
{
  type Output = I::Output;
  #[inline]
  fn into_output(self) -> Self::Output {
    self.input.into_output()
  }
  #[inline]
  fn merge_output(mut self, inner: Self::Output) -> Self {
    self.input = I::merge_output(self.input, inner);
    self
  }
}

impl<I, S> IntoOutput for Stateful<I, S>
where
  I: IntoOutput,
{
  type Output = I::Output;
  #[inline]
  fn into_output(self) -> Self::Output {
    self.input.into_output()
  }
  #[inline]
  fn merge_output(mut self, inner: Self::Output) -> Self {
    self.input = I::merge_output(self.input, inner);
    self
  }
}

impl<I> IntoOutput for Streaming<I>
where
  I: IntoOutput,
{
  type Output = I::Output;
  #[inline]
  fn into_output(self) -> Self::Output {
    self.into_complete().into_output()
  }
  #[inline]
  fn merge_output(self, inner: Self::Output) -> Self {
    Streaming(I::merge_output(self.0, inner))
  }
}

impl<'a, T> IntoOutput for &'a [T] {
  type Output = Self;
  #[inline]
  fn into_output(self) -> Self::Output {
    self
  }
  #[inline]
  fn merge_output(self, inner: Self::Output) -> Self {
    inner
  }
}

impl<const LEN: usize> IntoOutput for [u8; LEN] {
  type Output = Self;
  #[inline]
  fn into_output(self) -> Self::Output {
    self
  }
  #[inline]
  fn merge_output(self, inner: Self::Output) -> Self {
    inner
  }
}

impl<'a, const LEN: usize> IntoOutput for &'a [u8; LEN] {
  type Output = Self;
  #[inline]
  fn into_output(self) -> Self::Output {
    self
  }
  #[inline]
  fn merge_output(self, inner: Self::Output) -> Self {
    inner
  }
}

impl<'a> IntoOutput for &'a str {
  type Output = Self;
  #[inline]
  fn into_output(self) -> Self::Output {
    self
  }
  #[inline]
  fn merge_output(self, inner: Self::Output) -> Self {
    inner
  }
}

impl<'a> IntoOutput for (&'a [u8], usize) {
  type Output = Self;
  #[inline]
  fn into_output(self) -> Self::Output {
    self
  }
  #[inline]
  fn merge_output(self, inner: Self::Output) -> Self {
    inner
  }
}

/// Abstracts something which can extend an `Extend`.
/// Used to build modified input slices in `escaped_transform`
pub trait ExtendInto {
  /// The current input type is a sequence of that `Item` type.
  ///
  /// Example: `u8` for `&[u8]` or `char` for `&str`
  type Item;

  /// The type that will be produced
  type Extender;

  /// Create a new `Extend` of the correct type
  fn new_builder(&self) -> Self::Extender;
  /// Accumulate the input into an accumulator
  fn extend_into(&self, acc: &mut Self::Extender);
}

impl<I> ExtendInto for Located<I>
where
  I: ExtendInto,
{
  type Item = I::Item;
  type Extender = I::Extender;

  fn new_builder(&self) -> Self::Extender {
    self.input.new_builder()
  }

  fn extend_into(&self, extender: &mut Self::Extender) {
    self.input.extend_into(extender)
  }
}

impl<I, S> ExtendInto for Stateful<I, S>
where
  I: ExtendInto,
{
  type Item = I::Item;
  type Extender = I::Extender;

  fn new_builder(&self) -> Self::Extender {
    self.input.new_builder()
  }

  fn extend_into(&self, extender: &mut Self::Extender) {
    self.input.extend_into(extender)
  }
}

impl<I> ExtendInto for Streaming<I>
where
  I: ExtendInto,
{
  type Item = I::Item;
  type Extender = I::Extender;

  #[inline(always)]
  fn new_builder(&self) -> Self::Extender {
    self.0.new_builder()
  }
  #[inline(always)]
  fn extend_into(&self, acc: &mut Self::Extender) {
    self.0.extend_into(acc)
  }
}

#[cfg(feature = "alloc")]
impl ExtendInto for [u8] {
  type Item = u8;
  type Extender = Vec<u8>;

  #[inline]
  fn new_builder(&self) -> Vec<u8> {
    Vec::new()
  }
  #[inline]
  fn extend_into(&self, acc: &mut Vec<u8>) {
    acc.extend(self.iter().cloned());
  }
}

#[cfg(feature = "alloc")]
impl ExtendInto for &[u8] {
  type Item = u8;
  type Extender = Vec<u8>;

  #[inline]
  fn new_builder(&self) -> Vec<u8> {
    Vec::new()
  }
  #[inline]
  fn extend_into(&self, acc: &mut Vec<u8>) {
    acc.extend_from_slice(self);
  }
}

#[cfg(feature = "alloc")]
impl ExtendInto for str {
  type Item = char;
  type Extender = String;

  #[inline]
  fn new_builder(&self) -> String {
    String::new()
  }
  #[inline]
  fn extend_into(&self, acc: &mut String) {
    acc.push_str(self);
  }
}

#[cfg(feature = "alloc")]
impl ExtendInto for &str {
  type Item = char;
  type Extender = String;

  #[inline]
  fn new_builder(&self) -> String {
    String::new()
  }
  #[inline]
  fn extend_into(&self, acc: &mut String) {
    acc.push_str(self);
  }
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
  #[inline]
  fn to_usize(&self) -> usize {
    *self as usize
  }
}

impl ToUsize for u16 {
  #[inline]
  fn to_usize(&self) -> usize {
    *self as usize
  }
}

impl ToUsize for usize {
  #[inline]
  fn to_usize(&self) -> usize {
    *self
  }
}

#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
impl ToUsize for u32 {
  #[inline]
  fn to_usize(&self) -> usize {
    *self as usize
  }
}

#[cfg(target_pointer_width = "64")]
impl ToUsize for u64 {
  #[inline]
  fn to_usize(&self) -> usize {
    *self as usize
  }
}

/// Equivalent From implementation to avoid orphan rules in bits parsers
pub trait ErrorConvert<E> {
  /// Transform to another error type
  fn convert(self) -> E;
}

impl<I> ErrorConvert<(I, ErrorKind)> for ((I, usize), ErrorKind) {
  fn convert(self) -> (I, ErrorKind) {
    ((self.0).0, self.1)
  }
}

impl<I> ErrorConvert<((I, usize), ErrorKind)> for (I, ErrorKind) {
  fn convert(self) -> ((I, usize), ErrorKind) {
    ((self.0, 0), self.1)
  }
}

use crate::error;
impl<I> ErrorConvert<error::Error<I>> for error::Error<(I, usize)> {
  fn convert(self) -> error::Error<I> {
    error::Error {
      input: self.input.0,
      code: self.code,
    }
  }
}

impl<I> ErrorConvert<error::Error<(I, usize)>> for error::Error<I> {
  fn convert(self) -> error::Error<(I, usize)> {
    error::Error {
      input: (self.input, 0),
      code: self.code,
    }
  }
}

#[cfg(feature = "alloc")]
impl<I> ErrorConvert<error::VerboseError<I>> for error::VerboseError<(I, usize)> {
  fn convert(self) -> error::VerboseError<I> {
    error::VerboseError {
      errors: self.errors.into_iter().map(|(i, e)| (i.0, e)).collect(),
    }
  }
}

#[cfg(feature = "alloc")]
impl<I> ErrorConvert<error::VerboseError<(I, usize)>> for error::VerboseError<I> {
  fn convert(self) -> error::VerboseError<(I, usize)> {
    error::VerboseError {
      errors: self.errors.into_iter().map(|(i, e)| ((i, 0), e)).collect(),
    }
  }
}

/// Helper trait to show a byte slice as a hex dump
#[cfg(feature = "std")]
pub trait HexDisplay {
  /// Converts the value of `self` to a hex dump, returning the owned
  /// `String`.
  fn to_hex(&self, chunk_size: usize) -> String;

  /// Converts the value of `self` to a hex dump beginning at `from` address, returning the owned
  /// `String`.
  fn to_hex_from(&self, chunk_size: usize, from: usize) -> String;
}

#[cfg(feature = "std")]
static CHARS: &[u8] = b"0123456789abcdef";

#[cfg(feature = "std")]
impl<I> HexDisplay for Located<I>
where
  I: HexDisplay,
{
  #[inline(always)]
  fn to_hex(&self, chunk_size: usize) -> String {
    self.input.to_hex(chunk_size)
  }

  #[inline(always)]
  fn to_hex_from(&self, chunk_size: usize, from: usize) -> String {
    self.input.to_hex_from(chunk_size, from)
  }
}

#[cfg(feature = "std")]
impl<I, S> HexDisplay for Stateful<I, S>
where
  I: HexDisplay,
{
  #[inline(always)]
  fn to_hex(&self, chunk_size: usize) -> String {
    self.input.to_hex(chunk_size)
  }

  #[inline(always)]
  fn to_hex_from(&self, chunk_size: usize, from: usize) -> String {
    self.input.to_hex_from(chunk_size, from)
  }
}

#[cfg(feature = "std")]
impl<I> HexDisplay for Streaming<I>
where
  I: HexDisplay,
{
  #[inline(always)]
  fn to_hex(&self, chunk_size: usize) -> String {
    self.0.to_hex(chunk_size)
  }

  #[inline(always)]
  fn to_hex_from(&self, chunk_size: usize, from: usize) -> String {
    self.0.to_hex_from(chunk_size, from)
  }
}

#[cfg(feature = "std")]
impl HexDisplay for [u8] {
  #[allow(unused_variables)]
  fn to_hex(&self, chunk_size: usize) -> String {
    self.to_hex_from(chunk_size, 0)
  }

  #[allow(unused_variables)]
  fn to_hex_from(&self, chunk_size: usize, from: usize) -> String {
    let mut v = Vec::with_capacity(self.len() * 3);
    let mut i = from;
    for chunk in self.chunks(chunk_size) {
      let s = format!("{:08x}", i);
      for &ch in s.as_bytes().iter() {
        v.push(ch);
      }
      v.push(b'\t');

      i += chunk_size;

      for &byte in chunk {
        v.push(CHARS[(byte >> 4) as usize]);
        v.push(CHARS[(byte & 0xf) as usize]);
        v.push(b' ');
      }
      if chunk_size > chunk.len() {
        for j in 0..(chunk_size - chunk.len()) {
          v.push(b' ');
          v.push(b' ');
          v.push(b' ');
        }
      }
      v.push(b'\t');

      for &byte in chunk {
        if (byte >= 32 && byte <= 126) || byte >= 128 {
          v.push(byte);
        } else {
          v.push(b'.');
        }
      }
      v.push(b'\n');
    }

    String::from_utf8_lossy(&v[..]).into_owned()
  }
}

#[cfg(feature = "std")]
impl HexDisplay for str {
  #[allow(unused_variables)]
  fn to_hex(&self, chunk_size: usize) -> String {
    self.to_hex_from(chunk_size, 0)
  }

  #[allow(unused_variables)]
  fn to_hex_from(&self, chunk_size: usize, from: usize) -> String {
    self.as_bytes().to_hex_from(chunk_size, from)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_offset_u8() {
    let s = b"abcd123";
    let a = &s[..];
    let b = &a[2..];
    let c = &a[..4];
    let d = &a[3..5];
    assert_eq!(a.offset(b), 2);
    assert_eq!(a.offset(c), 0);
    assert_eq!(a.offset(d), 3);
  }

  #[test]
  fn test_offset_str() {
    let s = "abcřèÂßÇd123";
    let a = &s[..];
    let b = &a[7..];
    let c = &a[..5];
    let d = &a[5..9];
    assert_eq!(a.offset(b), 7);
    assert_eq!(a.offset(c), 0);
    assert_eq!(a.offset(d), 5);
  }
}
