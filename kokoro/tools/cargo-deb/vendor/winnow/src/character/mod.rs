//! Character specific parsers and combinators
//!
//! Functions recognizing specific characters

#![allow(deprecated)] // will just become `pub(crate)` later

#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub mod complete;
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub mod streaming;
#[cfg(test)]
mod tests;

use crate::lib::std::ops::{Add, Shl};

use crate::combinator::opt;
use crate::error::ParseError;
use crate::error::{ErrMode, ErrorKind, Needed};
use crate::stream::Compare;
use crate::stream::{AsBStr, AsChar, Offset, ParseSlice, Stream, StreamIsPartial};
use crate::trace::trace;
use crate::IResult;
use crate::Parser;

/// Recognizes the string "\r\n".
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult};
/// # use winnow::character::crlf;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     crlf(input)
/// }
///
/// assert_eq!(parser("\r\nc"), Ok(("c", "\r\n")));
/// assert_eq!(parser("ab\r\nc"), Err(ErrMode::Backtrack(Error::new("ab\r\nc", ErrorKind::CrLf))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::CrLf))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::crlf;
/// assert_eq!(crlf::<_, Error<_>>(Partial::new("\r\nc")), Ok((Partial::new("c"), "\r\n")));
/// assert_eq!(crlf::<_, Error<_>>(Partial::new("ab\r\nc")), Err(ErrMode::Backtrack(Error::new(Partial::new("ab\r\nc"), ErrorKind::CrLf))));
/// assert_eq!(crlf::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn crlf<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<&'static str>,
{
    trace("crlf", move |input: I| {
        if input.is_partial() {
            streaming::crlf(input)
        } else {
            complete::crlf(input)
        }
    })(input)
}

/// Recognizes a string of any char except '\r\n' or '\n'.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::not_line_ending;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     not_line_ending(input)
/// }
///
/// assert_eq!(parser("ab\r\nc"), Ok(("\r\nc", "ab")));
/// assert_eq!(parser("ab\nc"), Ok(("\nc", "ab")));
/// assert_eq!(parser("abc"), Ok(("", "abc")));
/// assert_eq!(parser(""), Ok(("", "")));
/// assert_eq!(parser("a\rb\nc"), Err(ErrMode::Backtrack(Error { input: "a\rb\nc", kind: ErrorKind::Tag })));
/// assert_eq!(parser("a\rbc"), Err(ErrMode::Backtrack(Error { input: "a\rbc", kind: ErrorKind::Tag })));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::not_line_ending;
/// assert_eq!(not_line_ending::<_, Error<_>>(Partial::new("ab\r\nc")), Ok((Partial::new("\r\nc"), "ab")));
/// assert_eq!(not_line_ending::<_, Error<_>>(Partial::new("abc")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(not_line_ending::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(not_line_ending::<_, Error<_>>(Partial::new("a\rb\nc")), Err(ErrMode::Backtrack(Error::new(Partial::new("a\rb\nc"), ErrorKind::Tag ))));
/// assert_eq!(not_line_ending::<_, Error<_>>(Partial::new("a\rbc")), Err(ErrMode::Backtrack(Error::new(Partial::new("a\rbc"), ErrorKind::Tag ))));
/// ```
#[inline(always)]
pub fn not_line_ending<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream + AsBStr,
    I: Compare<&'static str>,
    <I as Stream>::Token: AsChar,
{
    trace("not_line_ending", move |input: I| {
        if input.is_partial() {
            streaming::not_line_ending(input)
        } else {
            complete::not_line_ending(input)
        }
    })(input)
}

/// Recognizes an end of line (both '\n' and '\r\n').
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::line_ending;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     line_ending(input)
/// }
///
/// assert_eq!(parser("\r\nc"), Ok(("c", "\r\n")));
/// assert_eq!(parser("ab\r\nc"), Err(ErrMode::Backtrack(Error::new("ab\r\nc", ErrorKind::CrLf))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::CrLf))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::line_ending;
/// assert_eq!(line_ending::<_, Error<_>>(Partial::new("\r\nc")), Ok((Partial::new("c"), "\r\n")));
/// assert_eq!(line_ending::<_, Error<_>>(Partial::new("ab\r\nc")), Err(ErrMode::Backtrack(Error::new(Partial::new("ab\r\nc"), ErrorKind::CrLf))));
/// assert_eq!(line_ending::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn line_ending<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<&'static str>,
{
    trace("line_ending", move |input: I| {
        if input.is_partial() {
            streaming::line_ending(input)
        } else {
            complete::line_ending(input)
        }
    })(input)
}

/// Matches a newline character '\n'.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::newline;
/// fn parser(input: &str) -> IResult<&str, char> {
///     newline(input)
/// }
///
/// assert_eq!(parser("\nc"), Ok(("c", '\n')));
/// assert_eq!(parser("\r\nc"), Err(ErrMode::Backtrack(Error::new("\r\nc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Char))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::newline;
/// assert_eq!(newline::<_, Error<_>>(Partial::new("\nc")), Ok((Partial::new("c"), '\n')));
/// assert_eq!(newline::<_, Error<_>>(Partial::new("\r\nc")), Err(ErrMode::Backtrack(Error::new(Partial::new("\r\nc"), ErrorKind::Char))));
/// assert_eq!(newline::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn newline<I, Error: ParseError<I>>(input: I) -> IResult<I, char, Error>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("newline", move |input: I| {
        if input.is_partial() {
            streaming::newline(input)
        } else {
            complete::newline(input)
        }
    })(input)
}

/// Matches a tab character '\t'.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::tab;
/// fn parser(input: &str) -> IResult<&str, char> {
///     tab(input)
/// }
///
/// assert_eq!(parser("\tc"), Ok(("c", '\t')));
/// assert_eq!(parser("\r\nc"), Err(ErrMode::Backtrack(Error::new("\r\nc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Char))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::tab;
/// assert_eq!(tab::<_, Error<_>>(Partial::new("\tc")), Ok((Partial::new("c"), '\t')));
/// assert_eq!(tab::<_, Error<_>>(Partial::new("\r\nc")), Err(ErrMode::Backtrack(Error::new(Partial::new("\r\nc"), ErrorKind::Char))));
/// assert_eq!(tab::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn tab<I, Error: ParseError<I>>(input: I) -> IResult<I, char, Error>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("tag", move |input: I| {
        if input.is_partial() {
            streaming::tab(input)
        } else {
            complete::tab(input)
        }
    })(input)
}

/// Recognizes zero or more lowercase and uppercase ASCII alphabetic characters: a-z, A-Z
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphabetic character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphabetic character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::character::alpha0;
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
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::alpha0;
/// assert_eq!(alpha0::<_, Error<_>>(Partial::new("ab1c")), Ok((Partial::new("1c"), "ab")));
/// assert_eq!(alpha0::<_, Error<_>>(Partial::new("1c")), Ok((Partial::new("1c"), "")));
/// assert_eq!(alpha0::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alpha0<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("alpha0", move |input: I| {
        if input.is_partial() {
            streaming::alpha0(input)
        } else {
            complete::alpha0(input)
        }
    })(input)
}

/// Recognizes one or more lowercase and uppercase ASCII alphabetic characters: a-z, A-Z
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found  (a non alphabetic character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphabetic character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::alpha1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alpha1(input)
/// }
///
/// assert_eq!(parser("aB1c"), Ok(("1c", "aB")));
/// assert_eq!(parser("1c"), Err(ErrMode::Backtrack(Error::new("1c", ErrorKind::Alpha))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Alpha))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::alpha1;
/// assert_eq!(alpha1::<_, Error<_>>(Partial::new("aB1c")), Ok((Partial::new("1c"), "aB")));
/// assert_eq!(alpha1::<_, Error<_>>(Partial::new("1c")), Err(ErrMode::Backtrack(Error::new(Partial::new("1c"), ErrorKind::Alpha))));
/// assert_eq!(alpha1::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alpha1<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("alpha1", move |input: I| {
        if input.is_partial() {
            streaming::alpha1(input)
        } else {
            complete::alpha1(input)
        }
    })(input)
}

/// Recognizes zero or more ASCII numerical characters: 0-9
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non digit character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::character::digit0;
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
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::digit0;
/// assert_eq!(digit0::<_, Error<_>>(Partial::new("21c")), Ok((Partial::new("c"), "21")));
/// assert_eq!(digit0::<_, Error<_>>(Partial::new("a21c")), Ok((Partial::new("a21c"), "")));
/// assert_eq!(digit0::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn digit0<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("digit0", move |input: I| {
        if input.is_partial() {
            streaming::digit0(input)
        } else {
            complete::digit0(input)
        }
    })(input)
}

/// Recognizes one or more ASCII numerical characters: 0-9
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non digit character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     digit1(input)
/// }
///
/// assert_eq!(parser("21c"), Ok(("c", "21")));
/// assert_eq!(parser("c1"), Err(ErrMode::Backtrack(Error::new("c1", ErrorKind::Digit))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Digit))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::digit1;
/// assert_eq!(digit1::<_, Error<_>>(Partial::new("21c")), Ok((Partial::new("c"), "21")));
/// assert_eq!(digit1::<_, Error<_>>(Partial::new("c1")), Err(ErrMode::Backtrack(Error::new(Partial::new("c1"), ErrorKind::Digit))));
/// assert_eq!(digit1::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// ## Parsing an integer
///
/// You can use `digit1` in combination with [`Parser::map_res`][crate::Parser::map_res] to parse an integer:
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed, Parser};
/// # use winnow::character::digit1;
/// fn parser(input: &str) -> IResult<&str, u32> {
///   digit1.map_res(str::parse).parse_next(input)
/// }
///
/// assert_eq!(parser("416"), Ok(("", 416)));
/// assert_eq!(parser("12b"), Ok(("b", 12)));
/// assert!(parser("b").is_err());
/// ```
#[inline(always)]
pub fn digit1<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("digit1", move |input: I| {
        if input.is_partial() {
            streaming::digit1(input)
        } else {
            complete::digit1(input)
        }
    })(input)
}

/// Recognizes zero or more ASCII hexadecimal numerical characters: 0-9, A-F, a-f
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non hexadecimal digit character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non hexadecimal digit character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::character::hex_digit0;
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
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::hex_digit0;
/// assert_eq!(hex_digit0::<_, Error<_>>(Partial::new("21cZ")), Ok((Partial::new("Z"), "21c")));
/// assert_eq!(hex_digit0::<_, Error<_>>(Partial::new("Z21c")), Ok((Partial::new("Z21c"), "")));
/// assert_eq!(hex_digit0::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn hex_digit0<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("hex_digit0", move |input: I| {
        if input.is_partial() {
            streaming::hex_digit0(input)
        } else {
            complete::hex_digit0(input)
        }
    })(input)
}

/// Recognizes one or more ASCII hexadecimal numerical characters: 0-9, A-F, a-f
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non hexadecimal digit character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non hexadecimal digit character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::hex_digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     hex_digit1(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(parser("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::HexDigit))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::HexDigit))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::hex_digit1;
/// assert_eq!(hex_digit1::<_, Error<_>>(Partial::new("21cZ")), Ok((Partial::new("Z"), "21c")));
/// assert_eq!(hex_digit1::<_, Error<_>>(Partial::new("H2")), Err(ErrMode::Backtrack(Error::new(Partial::new("H2"), ErrorKind::HexDigit))));
/// assert_eq!(hex_digit1::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn hex_digit1<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("hex_digit1", move |input: I| {
        if input.is_partial() {
            streaming::hex_digit1(input)
        } else {
            complete::hex_digit1(input)
        }
    })(input)
}

/// Recognizes zero or more octal characters: 0-7
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non octal
/// digit character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non octal digit character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::character::oct_digit0;
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
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::oct_digit0;
/// assert_eq!(oct_digit0::<_, Error<_>>(Partial::new("21cZ")), Ok((Partial::new("cZ"), "21")));
/// assert_eq!(oct_digit0::<_, Error<_>>(Partial::new("Z21c")), Ok((Partial::new("Z21c"), "")));
/// assert_eq!(oct_digit0::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn oct_digit0<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("oct_digit0", move |input: I| {
        if input.is_partial() {
            streaming::oct_digit0(input)
        } else {
            complete::oct_digit0(input)
        }
    })(input)
}

/// Recognizes one or more octal characters: 0-7
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non octal digit character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non octal digit character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::oct_digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     oct_digit1(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("cZ", "21")));
/// assert_eq!(parser("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::OctDigit))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::OctDigit))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::oct_digit1;
/// assert_eq!(oct_digit1::<_, Error<_>>(Partial::new("21cZ")), Ok((Partial::new("cZ"), "21")));
/// assert_eq!(oct_digit1::<_, Error<_>>(Partial::new("H2")), Err(ErrMode::Backtrack(Error::new(Partial::new("H2"), ErrorKind::OctDigit))));
/// assert_eq!(oct_digit1::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn oct_digit1<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("oct_digit0", move |input: I| {
        if input.is_partial() {
            streaming::oct_digit1(input)
        } else {
            complete::oct_digit1(input)
        }
    })(input)
}

/// Recognizes zero or more ASCII numerical and alphabetic characters: 0-9, a-z, A-Z
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphanumerical character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphanumerical character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::character::alphanumeric0;
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
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::alphanumeric0;
/// assert_eq!(alphanumeric0::<_, Error<_>>(Partial::new("21cZ%1")), Ok((Partial::new("%1"), "21cZ")));
/// assert_eq!(alphanumeric0::<_, Error<_>>(Partial::new("&Z21c")), Ok((Partial::new("&Z21c"), "")));
/// assert_eq!(alphanumeric0::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alphanumeric0<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("alphanumeric0", move |input: I| {
        if input.is_partial() {
            streaming::alphanumeric0(input)
        } else {
            complete::alphanumeric0(input)
        }
    })(input)
}

/// Recognizes one or more ASCII numerical and alphabetic characters: 0-9, a-z, A-Z
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non alphanumerical character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphanumerical character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::alphanumeric1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alphanumeric1(input)
/// }
///
/// assert_eq!(parser("21cZ%1"), Ok(("%1", "21cZ")));
/// assert_eq!(parser("&H2"), Err(ErrMode::Backtrack(Error::new("&H2", ErrorKind::AlphaNumeric))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::AlphaNumeric))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::alphanumeric1;
/// assert_eq!(alphanumeric1::<_, Error<_>>(Partial::new("21cZ%1")), Ok((Partial::new("%1"), "21cZ")));
/// assert_eq!(alphanumeric1::<_, Error<_>>(Partial::new("&H2")), Err(ErrMode::Backtrack(Error::new(Partial::new("&H2"), ErrorKind::AlphaNumeric))));
/// assert_eq!(alphanumeric1::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alphanumeric1<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("alphanumeric1", move |input: I| {
        if input.is_partial() {
            streaming::alphanumeric1(input)
        } else {
            complete::alphanumeric1(input)
        }
    })(input)
}

/// Recognizes zero or more spaces and tabs.
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non space
/// character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::space0;
/// assert_eq!(space0::<_, Error<_>>(Partial::new(" \t21c")), Ok((Partial::new("21c"), " \t")));
/// assert_eq!(space0::<_, Error<_>>(Partial::new("Z21c")), Ok((Partial::new("Z21c"), "")));
/// assert_eq!(space0::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn space0<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("space0", move |input: I| {
        if input.is_partial() {
            streaming::space0(input)
        } else {
            complete::space0(input)
        }
    })(input)
}

/// Recognizes one or more spaces and tabs.
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::space1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     space1(input)
/// }
///
/// assert_eq!(parser(" \t21c"), Ok(("21c", " \t")));
/// assert_eq!(parser("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::Space))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Space))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::space1;
/// assert_eq!(space1::<_, Error<_>>(Partial::new(" \t21c")), Ok((Partial::new("21c"), " \t")));
/// assert_eq!(space1::<_, Error<_>>(Partial::new("H2")), Err(ErrMode::Backtrack(Error::new(Partial::new("H2"), ErrorKind::Space))));
/// assert_eq!(space1::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn space1<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("space1", move |input: I| {
        if input.is_partial() {
            streaming::space1(input)
        } else {
            complete::space1(input)
        }
    })(input)
}

/// Recognizes zero or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return the whole input if no terminating token is found (a non space
/// character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::character::multispace0;
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
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::multispace0;
/// assert_eq!(multispace0::<_, Error<_>>(Partial::new(" \t\n\r21c")), Ok((Partial::new("21c"), " \t\n\r")));
/// assert_eq!(multispace0::<_, Error<_>>(Partial::new("Z21c")), Ok((Partial::new("Z21c"), "")));
/// assert_eq!(multispace0::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn multispace0<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("multispace0", move |input: I| {
        if input.is_partial() {
            streaming::multispace0(input)
        } else {
            complete::multispace0(input)
        }
    })(input)
}

/// Recognizes one or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::multispace1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     multispace1(input)
/// }
///
/// assert_eq!(parser(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert_eq!(parser("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::MultiSpace))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::MultiSpace))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// # use winnow::character::multispace1;
/// assert_eq!(multispace1::<_, Error<_>>(Partial::new(" \t\n\r21c")), Ok((Partial::new("21c"), " \t\n\r")));
/// assert_eq!(multispace1::<_, Error<_>>(Partial::new("H2")), Err(ErrMode::Backtrack(Error::new(Partial::new("H2"), ErrorKind::MultiSpace))));
/// assert_eq!(multispace1::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn multispace1<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    trace("multispace1", move |input: I| {
        if input.is_partial() {
            streaming::multispace1(input)
        } else {
            complete::multispace1(input)
        }
    })(input)
}

/// Decode a decimal unsigned integer
///
/// *Complete version*: can parse until the end of input.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
#[doc(alias = "u8")]
#[doc(alias = "u16")]
#[doc(alias = "u32")]
#[doc(alias = "u64")]
#[doc(alias = "u128")]
pub fn dec_uint<I, O, E: ParseError<I>>(input: I) -> IResult<I, O, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar + Copy,
    O: Uint,
{
    trace("dec_uint", move |input: I| {
        let i = input.clone();

        if i.eof_offset() == 0 {
            if input.is_partial() {
                return Err(ErrMode::Incomplete(Needed::new(1)));
            } else {
                return Err(ErrMode::from_error_kind(input, ErrorKind::Digit));
            }
        }

        let mut value = O::default();
        for (offset, c) in i.iter_offsets() {
            match c.as_char().to_digit(10) {
                Some(d) => match value.checked_mul(10, sealed::SealedMarker).and_then(|v| {
                    let d = d as u8;
                    v.checked_add(d, sealed::SealedMarker)
                }) {
                    None => return Err(ErrMode::from_error_kind(input, ErrorKind::Digit)),
                    Some(v) => value = v,
                },
                None => {
                    if offset == 0 {
                        return Err(ErrMode::from_error_kind(input, ErrorKind::Digit));
                    } else {
                        return Ok((i.next_slice(offset).0, value));
                    }
                }
            }
        }

        if input.is_partial() {
            Err(ErrMode::Incomplete(Needed::new(1)))
        } else {
            Ok((i.next_slice(i.eof_offset()).0, value))
        }
    })(input)
}

/// Metadata for parsing unsigned integers, see [`dec_uint`]
pub trait Uint: Default {
    #[doc(hidden)]
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self>;
    #[doc(hidden)]
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self>;
}

impl Uint for u8 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for u16 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for u32 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for u64 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for u128 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for i8 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for i16 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for i32 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for i64 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

impl Uint for i128 {
    fn checked_mul(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_mul(by as Self)
    }
    fn checked_add(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_add(by as Self)
    }
}

/// Decode a decimal signed integer
///
/// *Complete version*: can parse until the end of input.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
#[doc(alias = "i8")]
#[doc(alias = "i16")]
#[doc(alias = "i32")]
#[doc(alias = "i64")]
#[doc(alias = "i128")]
pub fn dec_int<I, O, E: ParseError<I>>(input: I) -> IResult<I, O, E>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar + Copy,
    O: Int,
{
    trace("dec_int", move |input: I| {
        let i = input.clone();

        fn sign(token: impl AsChar) -> bool {
            let token = token.as_char();
            token == '+' || token == '-'
        }
        let (i, sign) = opt(crate::bytes::one_of(sign).map(AsChar::as_char))
            .map(|c| c != Some('-'))
            .parse_next(i)?;

        if i.eof_offset() == 0 {
            if input.is_partial() {
                return Err(ErrMode::Incomplete(Needed::new(1)));
            } else {
                return Err(ErrMode::from_error_kind(input, ErrorKind::Digit));
            }
        }

        let mut value = O::default();
        for (offset, c) in i.iter_offsets() {
            match c.as_char().to_digit(10) {
                Some(d) => match value.checked_mul(10, sealed::SealedMarker).and_then(|v| {
                    let d = d as u8;
                    if sign {
                        v.checked_add(d, sealed::SealedMarker)
                    } else {
                        v.checked_sub(d, sealed::SealedMarker)
                    }
                }) {
                    None => return Err(ErrMode::from_error_kind(input, ErrorKind::Digit)),
                    Some(v) => value = v,
                },
                None => {
                    if offset == 0 {
                        return Err(ErrMode::from_error_kind(input, ErrorKind::Digit));
                    } else {
                        return Ok((i.next_slice(offset).0, value));
                    }
                }
            }
        }

        if input.is_partial() {
            Err(ErrMode::Incomplete(Needed::new(1)))
        } else {
            Ok((i.next_slice(i.eof_offset()).0, value))
        }
    })(input)
}

/// Metadata for parsing signed integers, see [`dec_int`]
pub trait Int: Uint {
    #[doc(hidden)]
    fn checked_sub(self, by: u8, _: sealed::SealedMarker) -> Option<Self>;
}

impl Int for i8 {
    fn checked_sub(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_sub(by as Self)
    }
}

impl Int for i16 {
    fn checked_sub(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_sub(by as Self)
    }
}

impl Int for i32 {
    fn checked_sub(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_sub(by as Self)
    }
}

impl Int for i64 {
    fn checked_sub(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_sub(by as Self)
    }
}

impl Int for i128 {
    fn checked_sub(self, by: u8, _: sealed::SealedMarker) -> Option<Self> {
        self.checked_sub(by as Self)
    }
}

/// Decode a variable-width hexadecimal integer.
///
/// *Complete version*: Will parse until the end of input if it has fewer characters than the type
/// supports.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if end-of-input
/// is hit before a hard boundary (non-hex character, more characters than supported).
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// use winnow::character::hex_uint;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u32> {
///   hex_uint(s)
/// }
///
/// assert_eq!(parser(&b"01AE"[..]), Ok((&b""[..], 0x01AE)));
/// assert_eq!(parser(&b"abc"[..]), Ok((&b""[..], 0x0ABC)));
/// assert_eq!(parser(&b"ggg"[..]), Err(ErrMode::Backtrack(Error::new(&b"ggg"[..], ErrorKind::IsA))));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::character::hex_uint;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u32> {
///   hex_uint(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"01AE;"[..])), Ok((Partial::new(&b";"[..]), 0x01AE)));
/// assert_eq!(parser(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(parser(Partial::new(&b"ggg"[..])), Err(ErrMode::Backtrack(Error::new(Partial::new(&b"ggg"[..]), ErrorKind::IsA))));
/// ```
#[inline]
pub fn hex_uint<I, O, E: ParseError<I>>(input: I) -> IResult<I, O, E>
where
    I: StreamIsPartial,
    I: Stream,
    O: HexUint,
    <I as Stream>::Token: AsChar,
    <I as Stream>::Slice: AsBStr,
{
    trace("hex_uint", move |input: I| {
        let invalid_offset = input
            .offset_for(|c| {
                let c = c.as_char();
                !"0123456789abcdefABCDEF".contains(c)
            })
            .unwrap_or_else(|| input.eof_offset());
        let max_nibbles = O::max_nibbles(sealed::SealedMarker);
        let max_offset = input.offset_at(max_nibbles);
        let offset = match max_offset {
            Ok(max_offset) => {
                if max_offset < invalid_offset {
                    // Overflow
                    return Err(ErrMode::from_error_kind(input, ErrorKind::IsA));
                } else {
                    invalid_offset
                }
            }
            Err(_) => {
                if input.is_partial() && invalid_offset == input.eof_offset() {
                    // Only the next byte is guaranteed required
                    return Err(ErrMode::Incomplete(Needed::new(1)));
                } else {
                    invalid_offset
                }
            }
        };
        if offset == 0 {
            // Must be at least one digit
            return Err(ErrMode::from_error_kind(input, ErrorKind::IsA));
        }
        let (remaining, parsed) = input.next_slice(offset);

        let mut res = O::default();
        for c in parsed.as_bstr() {
            let nibble = *c as char;
            let nibble = nibble.to_digit(16).unwrap_or(0) as u8;
            let nibble = O::from(nibble);
            res = (res << O::from(4)) + nibble;
        }

        Ok((remaining, res))
    })(input)
}

/// Metadata for parsing hex numbers, see [`hex_uint`]
pub trait HexUint:
    Default + Shl<Self, Output = Self> + Add<Self, Output = Self> + From<u8>
{
    #[doc(hidden)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize;
}

impl HexUint for u8 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        2
    }
}

impl HexUint for u16 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        4
    }
}

impl HexUint for u32 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        8
    }
}

impl HexUint for u64 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        16
    }
}

impl HexUint for u128 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        32
    }
}

/// Recognizes floating point number in text format and returns a f32 or f64.
///
/// *Complete version*: Can parse until the end of input.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::character::float;
///
/// fn parser(s: &str) -> IResult<&str, f64> {
///   float(s)
/// }
///
/// assert_eq!(parser("11e-1"), Ok(("", 1.1)));
/// assert_eq!(parser("123E-02"), Ok(("", 1.23)));
/// assert_eq!(parser("123K-01"), Ok(("K-01", 123.0)));
/// assert_eq!(parser("abc"), Err(ErrMode::Backtrack(Error::new("abc", ErrorKind::Float))));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::character::float;
///
/// fn parser(s: Partial<&str>) -> IResult<Partial<&str>, f64> {
///   float(s)
/// }
///
/// assert_eq!(parser(Partial::new("11e-1 ")), Ok((Partial::new(" "), 1.1)));
/// assert_eq!(parser(Partial::new("11e-1")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(parser(Partial::new("123E-02")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(parser(Partial::new("123K-01")), Ok((Partial::new("K-01"), 123.0)));
/// assert_eq!(parser(Partial::new("abc")), Err(ErrMode::Backtrack(Error::new(Partial::new("abc"), ErrorKind::Float))));
/// ```
#[inline(always)]
#[doc(alias = "f32")]
#[doc(alias = "double")]
pub fn float<I, O, E: ParseError<I>>(input: I) -> IResult<I, O, E>
where
    I: StreamIsPartial,
    I: Stream,
    I: Offset + Compare<&'static str>,
    <I as Stream>::Slice: ParseSlice<O>,
    <I as Stream>::Token: AsChar + Copy,
    <I as Stream>::IterOffsets: Clone,
    I: AsBStr,
{
    trace("float", move |input: I| {
        let (i, s) = if input.is_partial() {
            crate::number::streaming::recognize_float_or_exceptions(input)?
        } else {
            crate::number::complete::recognize_float_or_exceptions(input)?
        };
        match s.parse_slice() {
            Some(f) => Ok((i, f)),
            None => Err(ErrMode::from_error_kind(i, ErrorKind::Float)),
        }
    })(input)
}

/// Matches a byte string with escaped characters.
///
/// * The first argument matches the normal characters (it must not accept the control character)
/// * The second argument is the control character (like `\` in most languages)
/// * The third argument matches the escaped characters
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed, IResult};
/// # use winnow::character::digit1;
/// use winnow::character::escaped;
/// use winnow::bytes::one_of;
///
/// fn esc(s: &str) -> IResult<&str, &str> {
///   escaped(digit1, '\\', one_of(r#""n\"#))(s)
/// }
///
/// assert_eq!(esc("123;"), Ok((";", "123")));
/// assert_eq!(esc(r#"12\"34;"#), Ok((";", r#"12\"34"#)));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed, IResult};
/// # use winnow::character::digit1;
/// # use winnow::Partial;
/// use winnow::character::escaped;
/// use winnow::bytes::one_of;
///
/// fn esc(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   escaped(digit1, '\\', one_of("\"n\\"))(s)
/// }
///
/// assert_eq!(esc(Partial::new("123;")), Ok((Partial::new(";"), "123")));
/// assert_eq!(esc(Partial::new("12\\\"34;")), Ok((Partial::new(";"), "12\\\"34")));
/// ```
#[inline(always)]
pub fn escaped<'a, I: 'a, Error, F, G, O1, O2>(
    mut normal: F,
    control_char: char,
    mut escapable: G,
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + Offset,
    <I as Stream>::Token: crate::stream::AsChar,
    F: Parser<I, O1, Error>,
    G: Parser<I, O2, Error>,
    Error: ParseError<I>,
{
    trace("escaped", move |input: I| {
        if input.is_partial() {
            crate::bytes::streaming::escaped_internal(
                input,
                &mut normal,
                control_char,
                &mut escapable,
            )
        } else {
            crate::bytes::complete::escaped_internal(
                input,
                &mut normal,
                control_char,
                &mut escapable,
            )
        }
    })
}

/// Matches a byte string with escaped characters.
///
/// * The first argument matches the normal characters (it must not match the control character)
/// * The second argument is the control character (like `\` in most languages)
/// * The third argument matches the escaped characters and transforms them
///
/// As an example, the chain `abc\tdef` could be `abc    def` (it also consumes the control character)
///
/// # Example
///
/// ```
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use std::str::from_utf8;
/// use winnow::bytes::tag;
/// use winnow::character::escaped_transform;
/// use winnow::character::alpha1;
/// use winnow::branch::alt;
/// use winnow::combinator::value;
///
/// fn parser(input: &str) -> IResult<&str, String> {
///   escaped_transform(
///     alpha1,
///     '\\',
///     alt((
///       tag("\\").value("\\"),
///       tag("\"").value("\""),
///       tag("n").value("\n"),
///     ))
///   )(input)
/// }
///
/// assert_eq!(parser("ab\\\"cd"), Ok(("", String::from("ab\"cd"))));
/// assert_eq!(parser("ab\\ncd"), Ok(("", String::from("ab\ncd"))));
/// ```
///
/// ```
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use std::str::from_utf8;
/// # use winnow::Partial;
/// use winnow::bytes::tag;
/// use winnow::character::escaped_transform;
/// use winnow::character::alpha1;
/// use winnow::branch::alt;
/// use winnow::combinator::value;
///
/// fn parser(input: Partial<&str>) -> IResult<Partial<&str>, String> {
///   escaped_transform(
///     alpha1,
///     '\\',
///     alt((
///       tag("\\").value("\\"),
///       tag("\"").value("\""),
///       tag("n").value("\n"),
///     ))
///   )(input)
/// }
///
/// assert_eq!(parser(Partial::new("ab\\\"cd\"")), Ok((Partial::new("\""), String::from("ab\"cd"))));
/// ```
#[cfg(feature = "alloc")]
#[inline(always)]
pub fn escaped_transform<I, Error, F, G, Output>(
    mut normal: F,
    control_char: char,
    mut transform: G,
) -> impl FnMut(I) -> IResult<I, Output, Error>
where
    I: StreamIsPartial,
    I: Stream + Offset,
    <I as Stream>::Token: crate::stream::AsChar,
    Output: crate::stream::Accumulate<<I as Stream>::Slice>,
    F: Parser<I, <I as Stream>::Slice, Error>,
    G: Parser<I, <I as Stream>::Slice, Error>,
    Error: ParseError<I>,
{
    trace("escaped_transform", move |input: I| {
        if input.is_partial() {
            crate::bytes::streaming::escaped_transform_internal(
                input,
                &mut normal,
                control_char,
                &mut transform,
            )
        } else {
            crate::bytes::complete::escaped_transform_internal(
                input,
                &mut normal,
                control_char,
                &mut transform,
            )
        }
    })
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "0.1.0", note = "Replaced with `AsChar::is_alpha`")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub fn is_alphabetic(chr: u8) -> bool {
    matches!(chr, 0x41..=0x5A | 0x61..=0x7A)
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "0.1.0", note = "Replaced with `AsChar::is_dec_digit`")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub fn is_digit(chr: u8) -> bool {
    matches!(chr, 0x30..=0x39)
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "0.1.0", note = "Replaced with `AsChar::is_hex_digit`")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub fn is_hex_digit(chr: u8) -> bool {
    matches!(chr, 0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66)
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "0.1.0", note = "Replaced with `AsChar::is_oct_digit`")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub fn is_oct_digit(chr: u8) -> bool {
    matches!(chr, 0x30..=0x37)
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "0.1.0", note = "Replaced with `AsChar::is_alphanum`")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub fn is_alphanumeric(chr: u8) -> bool {
    #![allow(deprecated)]
    is_alphabetic(chr) || is_digit(chr)
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "0.1.0", note = "Replaced with `AsChar::is_space`")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub fn is_space(chr: u8) -> bool {
    chr == b' ' || chr == b'\t'
}

#[inline]
#[doc(hidden)]
#[deprecated(since = "0.1.0", note = "Replaced with `AsChar::is_newline`")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub fn is_newline(chr: u8) -> bool {
    chr == b'\n'
}

mod sealed {
    pub struct SealedMarker;
}
