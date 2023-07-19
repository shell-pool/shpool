//! Character specific parsers and combinators
//!
//! Functions recognizing specific characters

#![allow(deprecated)] // will just become `pub(crate)` later

pub mod complete;
pub mod streaming;
#[cfg(test)]
mod tests;

use crate::error::ParseError;
use crate::input::Compare;
use crate::input::{
  AsBytes, AsChar, InputIsStreaming, InputIter, InputLength, InputTake, InputTakeAtPosition,
  IntoOutput, Offset, ParseTo, Slice,
};
use crate::lib::std::ops::{Range, RangeFrom, RangeTo};
use crate::IResult;

/// Recognizes the string "\r\n".
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult};
/// # use nom8::character::crlf;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     crlf(input)
/// }
///
/// assert_eq!(parser("\r\nc"), Ok(("c", "\r\n")));
/// assert_eq!(parser("ab\r\nc"), Err(Err::Error(Error::new("ab\r\nc", ErrorKind::CrLf))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::CrLf))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::crlf;
/// assert_eq!(crlf::<_, (_, ErrorKind), true>(Streaming("\r\nc")), Ok((Streaming("c"), "\r\n")));
/// assert_eq!(crlf::<_, (_, ErrorKind), true>(Streaming("ab\r\nc")), Err(Err::Error((Streaming("ab\r\nc"), ErrorKind::CrLf))));
/// assert_eq!(crlf::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn crlf<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: Slice<Range<usize>> + Slice<RangeFrom<usize>> + Slice<RangeTo<usize>>,
  T: InputIter + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  T: Compare<&'static str>,
{
  if STREAMING {
    streaming::crlf(input)
  } else {
    complete::crlf(input)
  }
}

/// Recognizes a string of any char except '\r\n' or '\n'.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::not_line_ending;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     not_line_ending(input)
/// }
///
/// assert_eq!(parser("ab\r\nc"), Ok(("\r\nc", "ab")));
/// assert_eq!(parser("ab\nc"), Ok(("\nc", "ab")));
/// assert_eq!(parser("abc"), Ok(("", "abc")));
/// assert_eq!(parser(""), Ok(("", "")));
/// assert_eq!(parser("a\rb\nc"), Err(Err::Error(Error { input: "a\rb\nc", code: ErrorKind::Tag })));
/// assert_eq!(parser("a\rbc"), Err(Err::Error(Error { input: "a\rbc", code: ErrorKind::Tag })));
/// ```
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::not_line_ending;
/// assert_eq!(not_line_ending::<_, (_, ErrorKind), true>(Streaming("ab\r\nc")), Ok((Streaming("\r\nc"), "ab")));
/// assert_eq!(not_line_ending::<_, (_, ErrorKind), true>(Streaming("abc")), Err(Err::Incomplete(Needed::Unknown)));
/// assert_eq!(not_line_ending::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::Unknown)));
/// assert_eq!(not_line_ending::<_, (_, ErrorKind), true>(Streaming("a\rb\nc")), Err(Err::Error((Streaming("a\rb\nc"), ErrorKind::Tag ))));
/// assert_eq!(not_line_ending::<_, (_, ErrorKind), true>(Streaming("a\rbc")), Err(Err::Error((Streaming("a\rbc"), ErrorKind::Tag ))));
/// ```
#[inline(always)]
pub fn not_line_ending<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: Slice<Range<usize>> + Slice<RangeFrom<usize>> + Slice<RangeTo<usize>>,
  T: InputIter + InputLength + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  T: Compare<&'static str>,
  <T as InputIter>::Item: AsChar,
  <T as InputIter>::Item: AsChar,
{
  if STREAMING {
    streaming::not_line_ending(input)
  } else {
    complete::not_line_ending(input)
  }
}

/// Recognizes an end of line (both '\n' and '\r\n').
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::line_ending;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     line_ending(input)
/// }
///
/// assert_eq!(parser("\r\nc"), Ok(("c", "\r\n")));
/// assert_eq!(parser("ab\r\nc"), Err(Err::Error(Error::new("ab\r\nc", ErrorKind::CrLf))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::CrLf))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::line_ending;
/// assert_eq!(line_ending::<_, (_, ErrorKind), true>(Streaming("\r\nc")), Ok((Streaming("c"), "\r\n")));
/// assert_eq!(line_ending::<_, (_, ErrorKind), true>(Streaming("ab\r\nc")), Err(Err::Error((Streaming("ab\r\nc"), ErrorKind::CrLf))));
/// assert_eq!(line_ending::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn line_ending<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: Slice<Range<usize>> + Slice<RangeFrom<usize>> + Slice<RangeTo<usize>>,
  T: InputIter + InputLength + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  T: Compare<&'static str>,
{
  if STREAMING {
    streaming::line_ending(input)
  } else {
    complete::line_ending(input)
  }
}

/// Matches a newline character '\n'.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::newline;
/// fn parser(input: &str) -> IResult<&str, char> {
///     newline(input)
/// }
///
/// assert_eq!(parser("\nc"), Ok(("c", '\n')));
/// assert_eq!(parser("\r\nc"), Err(Err::Error(Error::new("\r\nc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Char))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::newline;
/// assert_eq!(newline::<_, (_, ErrorKind), true>(Streaming("\nc")), Ok((Streaming("c"), '\n')));
/// assert_eq!(newline::<_, (_, ErrorKind), true>(Streaming("\r\nc")), Err(Err::Error((Streaming("\r\nc"), ErrorKind::Char))));
/// assert_eq!(newline::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn newline<I, Error: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter + InputLength + InputIsStreaming<STREAMING>,
  <I as InputIter>::Item: AsChar,
{
  if STREAMING {
    streaming::newline(input)
  } else {
    complete::newline(input)
  }
}

/// Matches a tab character '\t'.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::tab;
/// fn parser(input: &str) -> IResult<&str, char> {
///     tab(input)
/// }
///
/// assert_eq!(parser("\tc"), Ok(("c", '\t')));
/// assert_eq!(parser("\r\nc"), Err(Err::Error(Error::new("\r\nc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Char))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::tab;
/// assert_eq!(tab::<_, (_, ErrorKind), true>(Streaming("\tc")), Ok((Streaming("c"), '\t')));
/// assert_eq!(tab::<_, (_, ErrorKind), true>(Streaming("\r\nc")), Err(Err::Error((Streaming("\r\nc"), ErrorKind::Char))));
/// assert_eq!(tab::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn tab<I, Error: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter + InputLength + InputIsStreaming<STREAMING>,
  <I as InputIter>::Item: AsChar,
{
  if STREAMING {
    streaming::tab(input)
  } else {
    complete::tab(input)
  }
}

/// Recognizes zero or more lowercase and uppercase ASCII alphabetic characters: a-z, A-Z
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphabetic character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphabetic character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::alpha0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alpha0(input)
/// }
///
/// assert_eq!(parser("ab1c"), Ok(("1c", "ab")));
/// assert_eq!(parser("1c"), Ok(("1c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::alpha0;
/// assert_eq!(alpha0::<_, (_, ErrorKind), true>(Streaming("ab1c")), Ok((Streaming("1c"), "ab")));
/// assert_eq!(alpha0::<_, (_, ErrorKind), true>(Streaming("1c")), Ok((Streaming("1c"), "")));
/// assert_eq!(alpha0::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alpha0<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::alpha0(input)
  } else {
    complete::alpha0(input)
  }
}

/// Recognizes one or more lowercase and uppercase ASCII alphabetic characters: a-z, A-Z
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found  (a non alphabetic character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphabetic character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::alpha1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alpha1(input)
/// }
///
/// assert_eq!(parser("aB1c"), Ok(("1c", "aB")));
/// assert_eq!(parser("1c"), Err(Err::Error(Error::new("1c", ErrorKind::Alpha))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Alpha))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::alpha1;
/// assert_eq!(alpha1::<_, (_, ErrorKind), true>(Streaming("aB1c")), Ok((Streaming("1c"), "aB")));
/// assert_eq!(alpha1::<_, (_, ErrorKind), true>(Streaming("1c")), Err(Err::Error((Streaming("1c"), ErrorKind::Alpha))));
/// assert_eq!(alpha1::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alpha1<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::alpha1(input)
  } else {
    complete::alpha1(input)
  }
}

/// Recognizes zero or more ASCII numerical characters: 0-9
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non digit character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::digit0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     digit0(input)
/// }
///
/// assert_eq!(parser("21c"), Ok(("c", "21")));
/// assert_eq!(parser("21"), Ok(("", "21")));
/// assert_eq!(parser("a21c"), Ok(("a21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::digit0;
/// assert_eq!(digit0::<_, (_, ErrorKind), true>(Streaming("21c")), Ok((Streaming("c"), "21")));
/// assert_eq!(digit0::<_, (_, ErrorKind), true>(Streaming("a21c")), Ok((Streaming("a21c"), "")));
/// assert_eq!(digit0::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn digit0<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::digit0(input)
  } else {
    complete::digit0(input)
  }
}

/// Recognizes one or more ASCII numerical characters: 0-9
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non digit character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     digit1(input)
/// }
///
/// assert_eq!(parser("21c"), Ok(("c", "21")));
/// assert_eq!(parser("c1"), Err(Err::Error(Error::new("c1", ErrorKind::Digit))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Digit))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::digit1;
/// assert_eq!(digit1::<_, (_, ErrorKind), true>(Streaming("21c")), Ok((Streaming("c"), "21")));
/// assert_eq!(digit1::<_, (_, ErrorKind), true>(Streaming("c1")), Err(Err::Error((Streaming("c1"), ErrorKind::Digit))));
/// assert_eq!(digit1::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
///
/// ## Parsing an integer
///
/// You can use `digit1` in combination with [`Parser::map_res`][crate::Parser::map_res] to parse an integer:
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed, Parser};
/// # use nom8::character::digit1;
/// fn parser(input: &str) -> IResult<&str, u32> {
///   digit1.map_res(str::parse).parse(input)
/// }
///
/// assert_eq!(parser("416"), Ok(("", 416)));
/// assert_eq!(parser("12b"), Ok(("b", 12)));
/// assert!(parser("b").is_err());
/// ```
#[inline(always)]
pub fn digit1<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::digit1(input)
  } else {
    complete::digit1(input)
  }
}

/// Recognizes zero or more ASCII hexadecimal numerical characters: 0-9, A-F, a-f
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non hexadecimal digit character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non hexadecimal digit character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::hex_digit0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     hex_digit0(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::hex_digit0;
/// assert_eq!(hex_digit0::<_, (_, ErrorKind), true>(Streaming("21cZ")), Ok((Streaming("Z"), "21c")));
/// assert_eq!(hex_digit0::<_, (_, ErrorKind), true>(Streaming("Z21c")), Ok((Streaming("Z21c"), "")));
/// assert_eq!(hex_digit0::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn hex_digit0<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::hex_digit0(input)
  } else {
    complete::hex_digit0(input)
  }
}

/// Recognizes one or more ASCII hexadecimal numerical characters: 0-9, A-F, a-f
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non hexadecimal digit character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non hexadecimal digit character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::hex_digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     hex_digit1(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(parser("H2"), Err(Err::Error(Error::new("H2", ErrorKind::HexDigit))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::HexDigit))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::hex_digit1;
/// assert_eq!(hex_digit1::<_, (_, ErrorKind), true>(Streaming("21cZ")), Ok((Streaming("Z"), "21c")));
/// assert_eq!(hex_digit1::<_, (_, ErrorKind), true>(Streaming("H2")), Err(Err::Error((Streaming("H2"), ErrorKind::HexDigit))));
/// assert_eq!(hex_digit1::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn hex_digit1<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::hex_digit1(input)
  } else {
    complete::hex_digit1(input)
  }
}

/// Recognizes zero or more octal characters: 0-7
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non octal
/// digit character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non octal digit character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::oct_digit0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     oct_digit0(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("cZ", "21")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::oct_digit0;
/// assert_eq!(oct_digit0::<_, (_, ErrorKind), true>(Streaming("21cZ")), Ok((Streaming("cZ"), "21")));
/// assert_eq!(oct_digit0::<_, (_, ErrorKind), true>(Streaming("Z21c")), Ok((Streaming("Z21c"), "")));
/// assert_eq!(oct_digit0::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn oct_digit0<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::oct_digit0(input)
  } else {
    complete::oct_digit0(input)
  }
}

/// Recognizes one or more octal characters: 0-7
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non octal digit character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non octal digit character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::oct_digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     oct_digit1(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("cZ", "21")));
/// assert_eq!(parser("H2"), Err(Err::Error(Error::new("H2", ErrorKind::OctDigit))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::OctDigit))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::oct_digit1;
/// assert_eq!(oct_digit1::<_, (_, ErrorKind), true>(Streaming("21cZ")), Ok((Streaming("cZ"), "21")));
/// assert_eq!(oct_digit1::<_, (_, ErrorKind), true>(Streaming("H2")), Err(Err::Error((Streaming("H2"), ErrorKind::OctDigit))));
/// assert_eq!(oct_digit1::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn oct_digit1<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::oct_digit1(input)
  } else {
    complete::oct_digit1(input)
  }
}

/// Recognizes zero or more ASCII numerical and alphabetic characters: 0-9, a-z, A-Z
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphanumerical character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphanumerical character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::alphanumeric0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alphanumeric0(input)
/// }
///
/// assert_eq!(parser("21cZ%1"), Ok(("%1", "21cZ")));
/// assert_eq!(parser("&Z21c"), Ok(("&Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::alphanumeric0;
/// assert_eq!(alphanumeric0::<_, (_, ErrorKind), true>(Streaming("21cZ%1")), Ok((Streaming("%1"), "21cZ")));
/// assert_eq!(alphanumeric0::<_, (_, ErrorKind), true>(Streaming("&Z21c")), Ok((Streaming("&Z21c"), "")));
/// assert_eq!(alphanumeric0::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alphanumeric0<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::alphanumeric0(input)
  } else {
    complete::alphanumeric0(input)
  }
}

/// Recognizes one or more ASCII numerical and alphabetic characters: 0-9, a-z, A-Z
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non alphanumerical character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphanumerical character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::alphanumeric1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alphanumeric1(input)
/// }
///
/// assert_eq!(parser("21cZ%1"), Ok(("%1", "21cZ")));
/// assert_eq!(parser("&H2"), Err(Err::Error(Error::new("&H2", ErrorKind::AlphaNumeric))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::AlphaNumeric))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::alphanumeric1;
/// assert_eq!(alphanumeric1::<_, (_, ErrorKind), true>(Streaming("21cZ%1")), Ok((Streaming("%1"), "21cZ")));
/// assert_eq!(alphanumeric1::<_, (_, ErrorKind), true>(Streaming("&H2")), Err(Err::Error((Streaming("&H2"), ErrorKind::AlphaNumeric))));
/// assert_eq!(alphanumeric1::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alphanumeric1<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    streaming::alphanumeric1(input)
  } else {
    complete::alphanumeric1(input)
  }
}

/// Recognizes zero or more spaces and tabs.
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non space
/// character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::space0;
/// assert_eq!(space0::<_, (_, ErrorKind), true>(Streaming(" \t21c")), Ok((Streaming("21c"), " \t")));
/// assert_eq!(space0::<_, (_, ErrorKind), true>(Streaming("Z21c")), Ok((Streaming("Z21c"), "")));
/// assert_eq!(space0::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn space0<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar + Clone,
{
  if STREAMING {
    streaming::space0(input)
  } else {
    complete::space0(input)
  }
}

/// Recognizes one or more spaces and tabs.
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::space1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     space1(input)
/// }
///
/// assert_eq!(parser(" \t21c"), Ok(("21c", " \t")));
/// assert_eq!(parser("H2"), Err(Err::Error(Error::new("H2", ErrorKind::Space))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Space))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::space1;
/// assert_eq!(space1::<_, (_, ErrorKind), true>(Streaming(" \t21c")), Ok((Streaming("21c"), " \t")));
/// assert_eq!(space1::<_, (_, ErrorKind), true>(Streaming("H2")), Err(Err::Error((Streaming("H2"), ErrorKind::Space))));
/// assert_eq!(space1::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn space1<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar + Clone,
{
  if STREAMING {
    streaming::space1(input)
  } else {
    complete::space1(input)
  }
}

/// Recognizes zero or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return the whole input if no terminating token is found (a non space
/// character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::multispace0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     multispace0(input)
/// }
///
/// assert_eq!(parser(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::multispace0;
/// assert_eq!(multispace0::<_, (_, ErrorKind), true>(Streaming(" \t\n\r21c")), Ok((Streaming("21c"), " \t\n\r")));
/// assert_eq!(multispace0::<_, (_, ErrorKind), true>(Streaming("Z21c")), Ok((Streaming("Z21c"), "")));
/// assert_eq!(multispace0::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn multispace0<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar + Clone,
{
  if STREAMING {
    streaming::multispace0(input)
  } else {
    complete::multispace0(input)
  }
}

/// Recognizes one or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::multispace1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     multispace1(input)
/// }
///
/// assert_eq!(parser(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert_eq!(parser("H2"), Err(Err::Error(Error::new("H2", ErrorKind::MultiSpace))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::MultiSpace))));
/// ```
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::input::Streaming;
/// # use nom8::character::multispace1;
/// assert_eq!(multispace1::<_, (_, ErrorKind), true>(Streaming(" \t\n\r21c")), Ok((Streaming("21c"), " \t\n\r")));
/// assert_eq!(multispace1::<_, (_, ErrorKind), true>(Streaming("H2")), Err(Err::Error((Streaming("H2"), ErrorKind::MultiSpace))));
/// assert_eq!(multispace1::<_, (_, ErrorKind), true>(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn multispace1<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar + Clone,
{
  if STREAMING {
    streaming::multispace1(input)
  } else {
    complete::multispace1(input)
  }
}

#[doc(hidden)]
macro_rules! ints {
    ($($t:tt)+) => {
        $(
        /// will parse a number in text form to a number
        ///
        /// *Complete version*: can parse until the end of input.
        ///
        /// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data.
        #[inline(always)]
        pub fn $t<T, E: ParseError<T>, const STREAMING: bool>(input: T) -> IResult<T, $t, E>
            where
            T: InputIter + Slice<RangeFrom<usize>> + InputLength + InputTake + Clone + InputIsStreaming<STREAMING>,
            T: IntoOutput,
            <T as InputIter>::Item: AsChar,
            T: for <'a> Compare<&'a[u8]>,
            {
                if STREAMING {
                  streaming::$t(input)
                } else {
                  complete::$t(input)
                }
            }
        )+
    }
}

ints! { i8 i16 i32 i64 i128 }

#[doc(hidden)]
macro_rules! uints {
    ($($t:tt)+) => {
        $(
        /// will parse a number in text form to a number
        ///
        /// *Complete version*: can parse until the end of input.
        ///
        /// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there's not enough input data.
        #[inline(always)]
        pub fn $t<T, E: ParseError<T>, const STREAMING: bool>(input: T) -> IResult<T, $t, E>
            where
            T: InputIter + Slice<RangeFrom<usize>> + InputLength + InputIsStreaming<STREAMING>,
            T: IntoOutput,
            <T as InputIter>::Item: AsChar,
            {
                if STREAMING {
                  streaming::$t(input)
                } else {
                  complete::$t(input)
                }
            }
        )+
    }
}

uints! { u8 u16 u32 u64 u128 }

/// Recognizes floating point number in text format and returns a f32.
///
/// *Complete version*: Can parse until the end of input.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::character::f32;
///
/// let parser = |s| {
///   f32(s)
/// };
///
/// assert_eq!(parser("11e-1"), Ok(("", 1.1)));
/// assert_eq!(parser("123E-02"), Ok(("", 1.23)));
/// assert_eq!(parser("123K-01"), Ok(("K-01", 123.0)));
/// assert_eq!(parser("abc"), Err(Err::Error(("abc", ErrorKind::Float))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::character::f32;
///
/// let parser = |s| {
///   f32(s)
/// };
///
/// assert_eq!(parser(Streaming("11e-1 ")), Ok((Streaming(" "), 1.1)));
/// assert_eq!(parser(Streaming("11e-1")), Err(Err::Incomplete(Needed::new(1))));
/// assert_eq!(parser(Streaming("123E-02")), Err(Err::Incomplete(Needed::new(1))));
/// assert_eq!(parser(Streaming("123K-01")), Ok((Streaming("K-01"), 123.0)));
/// assert_eq!(parser(Streaming("abc")), Err(Err::Error((Streaming("abc"), ErrorKind::Float))));
/// ```
#[inline(always)]
pub fn f32<T, E: ParseError<T>, const STREAMING: bool>(input: T) -> IResult<T, f32, E>
where
  T: Slice<RangeFrom<usize>> + Slice<RangeTo<usize>> + Slice<Range<usize>>,
  T: Clone + Offset + Compare<&'static str>,
  T: InputIter + InputLength + InputTake + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as IntoOutput>::Output: ParseTo<f32>,
  <T as InputIter>::Item: AsChar + Copy,
  <T as InputIter>::IterElem: Clone,
  T: InputTakeAtPosition,
  <T as InputTakeAtPosition>::Item: AsChar,
  T: AsBytes,
  T: for<'a> Compare<&'a [u8]>,
{
  if STREAMING {
    crate::number::streaming::float(input)
  } else {
    crate::number::complete::float(input)
  }
}

/// Recognizes floating point number in text format and returns a f64.
///
/// *Complete version*: Can parse until the end of input.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::character::f64;
///
/// let parser = |s| {
///   f64(s)
/// };
///
/// assert_eq!(parser("11e-1"), Ok(("", 1.1)));
/// assert_eq!(parser("123E-02"), Ok(("", 1.23)));
/// assert_eq!(parser("123K-01"), Ok(("K-01", 123.0)));
/// assert_eq!(parser("abc"), Err(Err::Error(("abc", ErrorKind::Float))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::character::f64;
///
/// let parser = |s| {
///   f64(s)
/// };
///
/// assert_eq!(parser(Streaming("11e-1 ")), Ok((Streaming(" "), 1.1)));
/// assert_eq!(parser(Streaming("11e-1")), Err(Err::Incomplete(Needed::new(1))));
/// assert_eq!(parser(Streaming("123E-02")), Err(Err::Incomplete(Needed::new(1))));
/// assert_eq!(parser(Streaming("123K-01")), Ok((Streaming("K-01"), 123.0)));
/// assert_eq!(parser(Streaming("abc")), Err(Err::Error((Streaming("abc"), ErrorKind::Float))));
/// ```
#[inline(always)]
pub fn f64<T, E: ParseError<T>, const STREAMING: bool>(input: T) -> IResult<T, f64, E>
where
  T: Slice<RangeFrom<usize>> + Slice<RangeTo<usize>> + Slice<Range<usize>>,
  T: Clone + Offset + Compare<&'static str>,
  T: InputIter + InputLength + InputTake + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as IntoOutput>::Output: ParseTo<f64>,
  <T as InputIter>::Item: AsChar + Copy,
  <T as InputIter>::IterElem: Clone,
  T: InputTakeAtPosition,
  <T as InputTakeAtPosition>::Item: AsChar,
  T: AsBytes,
  T: for<'a> Compare<&'a [u8]>,
{
  if STREAMING {
    crate::number::streaming::double(input)
  } else {
    crate::number::complete::double(input)
  }
}

/// Recognizes floating point number in a byte string and returns the corresponding slice.
///
/// *Complete version*: Can parse until the end of input.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::character::recognize_float;
///
/// let parser = |s| {
///   recognize_float(s)
/// };
///
/// assert_eq!(parser("11e-1"), Ok(("", "11e-1")));
/// assert_eq!(parser("123E-02"), Ok(("", "123E-02")));
/// assert_eq!(parser("123K-01"), Ok(("K-01", "123")));
/// assert_eq!(parser("abc"), Err(Err::Error(("abc", ErrorKind::Char))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::character::recognize_float;
///
/// let parser = |s| {
///   recognize_float(s)
/// };
///
/// assert_eq!(parser(Streaming("11e-1;")), Ok((Streaming(";"), "11e-1")));
/// assert_eq!(parser(Streaming("123E-02;")), Ok((Streaming(";"), "123E-02")));
/// assert_eq!(parser(Streaming("123K-01")), Ok((Streaming("K-01"), "123")));
/// assert_eq!(parser(Streaming("abc")), Err(Err::Error((Streaming("abc"), ErrorKind::Char))));
/// ```
#[inline(always)]
pub fn recognize_float<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: Slice<RangeFrom<usize>> + Slice<RangeTo<usize>>,
  T: Clone + Offset,
  T: InputIter + InputLength + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputIter>::Item: AsChar,
  T: InputTakeAtPosition,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  if STREAMING {
    crate::number::streaming::recognize_float(input)
  } else {
    crate::number::complete::recognize_float(input)
  }
}

/// Recognizes a floating point number in text format
///
/// It returns a tuple of (`sign`, `integer part`, `fraction part` and `exponent`) of the input
/// data.
///
/// *Complete version*: Can parse until the end of input.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
#[inline(always)]
pub fn recognize_float_parts<T, E: ParseError<T>, const STREAMING: bool>(
  input: T,
) -> IResult<
  T,
  (
    bool,
    <T as IntoOutput>::Output,
    <T as IntoOutput>::Output,
    i32,
  ),
  E,
>
where
  T: Slice<RangeFrom<usize>> + Slice<RangeTo<usize>> + Slice<Range<usize>>,
  T: Clone + Offset,
  T: InputIter + InputTake + InputIsStreaming<STREAMING>,
  T: IntoOutput,
  <T as InputIter>::Item: AsChar + Copy,
  T: InputTakeAtPosition + InputLength,
  <T as InputTakeAtPosition>::Item: AsChar,
  T: for<'a> Compare<&'a [u8]>,
  T: AsBytes,
{
  if STREAMING {
    crate::number::streaming::recognize_float_parts(input)
  } else {
    crate::number::complete::recognize_float_parts(input)
  }
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "8.0.0", note = "Replaced with `AsChar::is_alpha`")]
pub fn is_alphabetic(chr: u8) -> bool {
  (chr >= 0x41 && chr <= 0x5A) || (chr >= 0x61 && chr <= 0x7A)
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "8.0.0", note = "Replaced with `AsChar::is_dec_digit`")]
pub fn is_digit(chr: u8) -> bool {
  chr >= 0x30 && chr <= 0x39
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "8.0.0", note = "Replaced with `AsChar::is_hex_digit`")]
pub fn is_hex_digit(chr: u8) -> bool {
  (chr >= 0x30 && chr <= 0x39) || (chr >= 0x41 && chr <= 0x46) || (chr >= 0x61 && chr <= 0x66)
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "8.0.0", note = "Replaced with `AsChar::is_oct_digit`")]
pub fn is_oct_digit(chr: u8) -> bool {
  chr >= 0x30 && chr <= 0x37
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "8.0.0", note = "Replaced with `AsChar::is_alphanum`")]
pub fn is_alphanumeric(chr: u8) -> bool {
  #![allow(deprecated)]
  is_alphabetic(chr) || is_digit(chr)
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "8.0.0", note = "Replaced with `AsChar::is_space`")]
pub fn is_space(chr: u8) -> bool {
  chr == b' ' || chr == b'\t'
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "8.0.0", note = "Replaced with `AsChar::is_newline`")]
pub fn is_newline(chr: u8) -> bool {
  chr == b'\n'
}
