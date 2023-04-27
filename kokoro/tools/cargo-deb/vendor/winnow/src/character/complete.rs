//! Character specific parsers and combinators, complete input version.
//!
//! Functions recognizing specific characters.

#![allow(deprecated)]

use crate::combinator::opt;
use crate::error::ErrMode;
use crate::error::ErrorKind;
use crate::error::ParseError;
use crate::stream::{
    split_at_offset1_complete, split_at_offset_complete, AsBStr, AsChar, ContainsToken, Stream,
};
use crate::stream::{Compare, CompareResult};
use crate::IResult;

/// Recognizes one character.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{ErrorKind, Error}, IResult};
/// # use winnow::character::complete::char;
/// fn parser(i: &str) -> IResult<&str, char> {
///     char('a')(i)
/// }
/// assert_eq!(parser("abc"), Ok(("bc", 'a')));
/// assert_eq!(parser(" abc"), Err(ErrMode::Backtrack(Error::new(" abc", ErrorKind::Char))));
/// assert_eq!(parser("bc"), Err(ErrMode::Backtrack(Error::new("bc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Char))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::one_of`][crate::bytes::one_of]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::bytes::one_of`")]
pub fn char<I, Error: ParseError<I>>(c: char) -> impl Fn(I) -> IResult<I, char, Error>
where
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    move |i: I| char_internal(i, c)
}

pub(crate) fn char_internal<I, Error: ParseError<I>>(i: I, c: char) -> IResult<I, char, Error>
where
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    i.next_token()
        .map(|(i, t)| (i, t.as_char()))
        .filter(|(_, t)| *t == c)
        .ok_or_else(|| ErrMode::Backtrack(Error::from_char(i, c)))
}

/// Recognizes one character and checks that it satisfies a predicate
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{ErrorKind, Error}, error::Needed, IResult};
/// # use winnow::character::complete::satisfy;
/// fn parser(i: &str) -> IResult<&str, char> {
///     satisfy(|c| c == 'a' || c == 'b')(i)
/// }
/// assert_eq!(parser("abc"), Ok(("bc", 'a')));
/// assert_eq!(parser("cd"), Err(ErrMode::Backtrack(Error::new("cd", ErrorKind::Satisfy))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Satisfy))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::one_of`][crate::bytes::one_of]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::bytes::one_of`")]
pub fn satisfy<F, I, Error: ParseError<I>>(cond: F) -> impl Fn(I) -> IResult<I, char, Error>
where
    I: Stream,
    <I as Stream>::Token: AsChar,
    F: Fn(char) -> bool,
{
    move |i: I| satisfy_internal(i, &cond)
}

pub(crate) fn satisfy_internal<F, I, Error: ParseError<I>>(
    i: I,
    cond: &F,
) -> IResult<I, char, Error>
where
    I: Stream,
    <I as Stream>::Token: AsChar,
    F: Fn(char) -> bool,
{
    i.next_token()
        .map(|(i, t)| (i, t.as_char()))
        .filter(|(_, t)| cond(*t))
        .ok_or_else(|| ErrMode::from_error_kind(i, ErrorKind::Satisfy))
}

/// Recognizes one of the provided characters.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::character::complete::one_of;
/// assert_eq!(one_of::<_, _, Error<_>>("abc")("b"), Ok(("", 'b')));
/// assert_eq!(one_of::<_, _, Error<_>>("a")("bc"), Err(ErrMode::Backtrack(Error::new("bc", ErrorKind::OneOf))));
/// assert_eq!(one_of::<_, _, Error<_>>("a")(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::OneOf))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::one_of`][crate::bytes::one_of]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::bytes::one_of`")]
pub fn one_of<I, T, Error: ParseError<I>>(list: T) -> impl Fn(I) -> IResult<I, char, Error>
where
    I: Stream,
    <I as Stream>::Token: AsChar + Copy,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| crate::bytes::complete::one_of_internal(i, &list).map(|(i, c)| (i, c.as_char()))
}

/// Recognizes a character that is not in the provided characters.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::character::complete::none_of;
/// assert_eq!(none_of::<_, _, Error<_>>("abc")("z"), Ok(("", 'z')));
/// assert_eq!(none_of::<_, _, Error<_>>("ab")("a"), Err(ErrMode::Backtrack(Error::new("a", ErrorKind::NoneOf))));
/// assert_eq!(none_of::<_, _, Error<_>>("a")(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::NoneOf))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::none_of`][crate::bytes::none_of]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::bytes::none_of`")]
pub fn none_of<I, T, Error: ParseError<I>>(list: T) -> impl Fn(I) -> IResult<I, char, Error>
where
    I: Stream,
    <I as Stream>::Token: AsChar + Copy,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| crate::bytes::complete::none_of_internal(i, &list).map(|(i, c)| (i, c.as_char()))
}

/// Recognizes the string "\r\n".
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult};
/// # use winnow::character::complete::crlf;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     crlf(input)
/// }
///
/// assert_eq!(parser("\r\nc"), Ok(("c", "\r\n")));
/// assert_eq!(parser("ab\r\nc"), Err(ErrMode::Backtrack(Error::new("ab\r\nc", ErrorKind::CrLf))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::CrLf))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::crlf`][crate::character::crlf]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::crlf`")]
pub fn crlf<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    T: Compare<&'static str>,
{
    const CRLF: &str = "\r\n";
    match input.compare(CRLF) {
        CompareResult::Ok => Ok(input.next_slice(CRLF.len())),
        CompareResult::Incomplete | CompareResult::Error => {
            let e: ErrorKind = ErrorKind::CrLf;
            Err(ErrMode::from_error_kind(input, e))
        }
    }
}

//FIXME: there's still an incomplete
/// Recognizes a string of any char except '\r\n' or '\n'.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::not_line_ending;
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
/// **WARNING:** Deprecated, replaced with [`winnow::character::not_line_ending`][crate::character::not_line_ending]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::not_line_ending`"
)]
pub fn not_line_ending<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream + AsBStr,
    T: Compare<&'static str>,
    <T as Stream>::Token: AsChar,
{
    match input.offset_for(|item| {
        let c = item.as_char();
        c == '\r' || c == '\n'
    }) {
        None => Ok(input.next_slice(input.eof_offset())),
        Some(offset) => {
            let (new_input, res) = input.next_slice(offset);
            let bytes = new_input.as_bstr();
            let nth = bytes[0];
            if nth == b'\r' {
                let comp = new_input.compare("\r\n");
                match comp {
                    //FIXME: calculate the right index
                    CompareResult::Ok => {}
                    CompareResult::Incomplete | CompareResult::Error => {
                        let e: ErrorKind = ErrorKind::Tag;
                        return Err(ErrMode::from_error_kind(input, e));
                    }
                }
            }
            Ok((new_input, res))
        }
    }
}

/// Recognizes an end of line (both '\n' and '\r\n').
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::line_ending;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     line_ending(input)
/// }
///
/// assert_eq!(parser("\r\nc"), Ok(("c", "\r\n")));
/// assert_eq!(parser("ab\r\nc"), Err(ErrMode::Backtrack(Error::new("ab\r\nc", ErrorKind::CrLf))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::CrLf))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::line_ending`][crate::character::line_ending]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::line_ending`"
)]
pub fn line_ending<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    T: Compare<&'static str>,
{
    const LF: &str = "\n";
    const CRLF: &str = "\r\n";
    match input.compare(LF) {
        CompareResult::Ok => Ok(input.next_slice(LF.len())),
        CompareResult::Incomplete => Err(ErrMode::from_error_kind(input, ErrorKind::CrLf)),
        CompareResult::Error => match input.compare("\r\n") {
            CompareResult::Ok => Ok(input.next_slice(CRLF.len())),
            CompareResult::Incomplete | CompareResult::Error => Err(ErrMode::Backtrack(
                E::from_error_kind(input, ErrorKind::CrLf),
            )),
        },
    }
}

/// Matches a newline character '\n'.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::newline;
/// fn parser(input: &str) -> IResult<&str, char> {
///     newline(input)
/// }
///
/// assert_eq!(parser("\nc"), Ok(("c", '\n')));
/// assert_eq!(parser("\r\nc"), Err(ErrMode::Backtrack(Error::new("\r\nc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Char))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::newline`][crate::character::newline]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::newline`")]
pub fn newline<I, Error: ParseError<I>>(input: I) -> IResult<I, char, Error>
where
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    char('\n')(input)
}

/// Matches a tab character '\t'.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::tab;
/// fn parser(input: &str) -> IResult<&str, char> {
///     tab(input)
/// }
///
/// assert_eq!(parser("\tc"), Ok(("c", '\t')));
/// assert_eq!(parser("\r\nc"), Err(ErrMode::Backtrack(Error::new("\r\nc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Char))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::tab`][crate::character::tab]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::tab`")]
pub fn tab<I, Error: ParseError<I>>(input: I) -> IResult<I, char, Error>
where
    I: Stream,
    <I as Stream>::Token: AsChar,
{
    char('\t')(input)
}

/// Matches one byte as a character. Note that the input type will
/// accept a `str`, but not a `&[u8]`, unlike many other parsers.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use winnow::{character::complete::anychar, error::ErrMode, error::{Error, ErrorKind}, IResult};
/// fn parser(input: &str) -> IResult<&str, char> {
///     anychar(input)
/// }
///
/// assert_eq!(parser("abc"), Ok(("bc",'a')));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Eof))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::any`][crate::bytes::any]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::bytes::any`")]
pub fn anychar<T, E: ParseError<T>>(input: T) -> IResult<T, char, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    crate::bytes::complete::any(input).map(|(i, c)| (i, c.as_char()))
}

/// Recognizes zero or more lowercase and uppercase ASCII alphabetic characters: a-z, A-Z
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphabetic character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, IResult, error::Needed};
/// # use winnow::character::complete::alpha0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alpha0(input)
/// }
///
/// assert_eq!(parser("ab1c"), Ok(("1c", "ab")));
/// assert_eq!(parser("1c"), Ok(("1c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::alpha0`][crate::character::alpha0]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::alpha0`")]
pub fn alpha0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset_complete(&input, |item| !item.is_alpha())
}

/// Recognizes one or more lowercase and uppercase ASCII alphabetic characters: a-z, A-Z
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found  (a non alphabetic character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::alpha1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alpha1(input)
/// }
///
/// assert_eq!(parser("aB1c"), Ok(("1c", "aB")));
/// assert_eq!(parser("1c"), Err(ErrMode::Backtrack(Error::new("1c", ErrorKind::Alpha))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Alpha))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::alpha1`][crate::character::alpha1]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::alpha1`")]
pub fn alpha1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset1_complete(&input, |item| !item.is_alpha(), ErrorKind::Alpha)
}

/// Recognizes zero or more ASCII numerical characters: 0-9
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, IResult, error::Needed};
/// # use winnow::character::complete::digit0;
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
/// **WARNING:** Deprecated, replaced with [`winnow::character::digit0`][crate::character::digit0]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::digit0`")]
pub fn digit0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset_complete(&input, |item| !item.is_dec_digit())
}

/// Recognizes one or more ASCII numerical characters: 0-9
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     digit1(input)
/// }
///
/// assert_eq!(parser("21c"), Ok(("c", "21")));
/// assert_eq!(parser("c1"), Err(ErrMode::Backtrack(Error::new("c1", ErrorKind::Digit))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Digit))));
/// ```
///
/// ## Parsing an integer
/// You can use `digit1` in combination with [`map_res`] to parse an integer:
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::combinator::map_res;
/// # use winnow::character::complete::digit1;
/// fn parser(input: &str) -> IResult<&str, u32> {
///   map_res(digit1, str::parse)(input)
/// }
///
/// assert_eq!(parser("416"), Ok(("", 416)));
/// assert_eq!(parser("12b"), Ok(("b", 12)));
/// assert!(parser("b").is_err());
/// ```
///
/// [`map_res`]: crate::combinator::map_res
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::digit1`][crate::character::digit1]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::digit1`")]
pub fn digit1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset1_complete(&input, |item| !item.is_dec_digit(), ErrorKind::Digit)
}

/// Recognizes zero or more ASCII hexadecimal numerical characters: 0-9, A-F, a-f
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non hexadecimal digit character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, IResult, error::Needed};
/// # use winnow::character::complete::hex_digit0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     hex_digit0(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::hex_digit0`][crate::character::hex_digit0]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::hex_digit0`"
)]
pub fn hex_digit0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset_complete(&input, |item| !item.is_hex_digit())
}

/// Recognizes one or more ASCII hexadecimal numerical characters: 0-9, A-F, a-f
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non hexadecimal digit character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::hex_digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     hex_digit1(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(parser("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::HexDigit))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::HexDigit))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::hex_digit1`][crate::character::hex_digit1]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::hex_digit1`"
)]
pub fn hex_digit1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset1_complete(&input, |item| !item.is_hex_digit(), ErrorKind::HexDigit)
}

/// Recognizes zero or more octal characters: 0-7
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non octal
/// digit character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, IResult, error::Needed};
/// # use winnow::character::complete::oct_digit0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     oct_digit0(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("cZ", "21")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::oct_digit0`][crate::character::oct_digit0]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::oct_digit0`"
)]
pub fn oct_digit0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset_complete(&input, |item| !item.is_oct_digit())
}

/// Recognizes one or more octal characters: 0-7
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non octal digit character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::oct_digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     oct_digit1(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("cZ", "21")));
/// assert_eq!(parser("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::OctDigit))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::OctDigit))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::oct_digit1`][crate::character::oct_digit1]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::oct_digit1`"
)]
pub fn oct_digit1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset1_complete(&input, |item| !item.is_oct_digit(), ErrorKind::OctDigit)
}

/// Recognizes zero or more ASCII numerical and alphabetic characters: 0-9, a-z, A-Z
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphanumerical character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, IResult, error::Needed};
/// # use winnow::character::complete::alphanumeric0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alphanumeric0(input)
/// }
///
/// assert_eq!(parser("21cZ%1"), Ok(("%1", "21cZ")));
/// assert_eq!(parser("&Z21c"), Ok(("&Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::alphanumeric0`][crate::character::alphanumeric0]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::alphanumeric0`"
)]
pub fn alphanumeric0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset_complete(&input, |item| !item.is_alphanum())
}

/// Recognizes one or more ASCII numerical and alphabetic characters: 0-9, a-z, A-Z
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non alphanumerical character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::alphanumeric1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alphanumeric1(input)
/// }
///
/// assert_eq!(parser("21cZ%1"), Ok(("%1", "21cZ")));
/// assert_eq!(parser("&H2"), Err(ErrMode::Backtrack(Error::new("&H2", ErrorKind::AlphaNumeric))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::AlphaNumeric))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::alphanumeric1`][crate::character::alphanumeric1]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::alphanumeric1`"
)]
pub fn alphanumeric1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset1_complete(&input, |item| !item.is_alphanum(), ErrorKind::AlphaNumeric)
}

/// Recognizes zero or more spaces and tabs.
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non space
/// character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, IResult, error::Needed};
/// # use winnow::character::complete::space0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     space0(input)
/// }
///
/// assert_eq!(parser(" \t21c"), Ok(("21c", " \t")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::space0`][crate::character::space0]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::space0`")]
pub fn space0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset_complete(&input, |item| {
        let c = item.as_char();
        !(c == ' ' || c == '\t')
    })
}

/// Recognizes one or more spaces and tabs.
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::space1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     space1(input)
/// }
///
/// assert_eq!(parser(" \t21c"), Ok(("21c", " \t")));
/// assert_eq!(parser("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::Space))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Space))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::space1`][crate::character::space1]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::space1`")]
pub fn space1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset1_complete(
        &input,
        |item| {
            let c = item.as_char();
            !(c == ' ' || c == '\t')
        },
        ErrorKind::Space,
    )
}

/// Recognizes zero or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return the whole input if no terminating token is found (a non space
/// character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, IResult, error::Needed};
/// # use winnow::character::complete::multispace0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     multispace0(input)
/// }
///
/// assert_eq!(parser(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::multispace0`][crate::character::multispace0]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::multispace0`"
)]
pub fn multispace0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset_complete(&input, |item| {
        let c = item.as_char();
        !(c == ' ' || c == '\t' || c == '\r' || c == '\n')
    })
}

/// Recognizes one or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult, error::Needed};
/// # use winnow::character::complete::multispace1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     multispace1(input)
/// }
///
/// assert_eq!(parser(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert_eq!(parser("H2"), Err(ErrMode::Backtrack(Error::new("H2", ErrorKind::MultiSpace))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::MultiSpace))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::multispace1`][crate::character::multispace1]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::multispace1`"
)]
pub fn multispace1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar,
{
    split_at_offset1_complete(
        &input,
        |item| {
            let c = item.as_char();
            !(c == ' ' || c == '\t' || c == '\r' || c == '\n')
        },
        ErrorKind::MultiSpace,
    )
}

pub(crate) fn sign<T, E: ParseError<T>>(input: T) -> IResult<T, bool, E>
where
    T: Stream,
    <T as Stream>::Token: AsChar + Copy,
{
    fn sign(token: impl AsChar) -> bool {
        let token = token.as_char();
        token == '+' || token == '-'
    }

    let (i, sign) = opt(|input| crate::bytes::complete::one_of_internal(input, &sign))(input)?;
    let sign = sign.map(AsChar::as_char) != Some('-');

    Ok((i, sign))
}

#[doc(hidden)]
macro_rules! ints {
    ($($t:tt)+) => {
        $(
        /// will parse a number in text form to a number
        ///
        /// *Complete version*: can parse until the end of input.
        ///
        /// **WARNING:** Deprecated, replaced with
        /// [`winnow::character::dec_uint`][crate::character::dec_int]
        #[deprecated(
          since = "0.1.0",
          note = "Replaced with `winnow::character::dec_int`"
        )]
        pub fn $t<T, E: ParseError<T>>(input: T) -> IResult<T, $t, E>
            where
              T: Stream,
              <T as Stream>::Token: AsChar + Copy,
            {
                let (i, sign) = sign(input.clone())?;

                if i.eof_offset() == 0 {
                    return Err(ErrMode::from_error_kind(input, ErrorKind::Digit));
                }

                let mut value: $t = 0;
                for (offset, c) in i.iter_offsets() {
                    match c.as_char().to_digit(10) {
                        None => {
                            if offset == 0 {
                                return Err(ErrMode::from_error_kind(input, ErrorKind::Digit));
                            } else {
                                return Ok((i.next_slice(offset).0, value));
                            }
                        },
                        Some(d) => match value.checked_mul(10).and_then(|v| {
                            if sign {
                                v.checked_add(d as $t)
                            } else {
                               v.checked_sub(d as $t)
                            }
                        }) {
                            None => return Err(ErrMode::from_error_kind(input, ErrorKind::Digit)),
                            Some(v) => value = v,
                        }
                   }
                }

                Ok((i.next_slice(i.eof_offset()).0, value))
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
        /// **WARNING:** Deprecated, replaced with
        /// [`winnow::character::dec_uint`][crate::character::dec_uint]
        #[deprecated(
          since = "0.1.0",
          note = "Replaced with `winnow::character::dec_uint`"
        )]
        pub fn $t<T, E: ParseError<T>>(input: T) -> IResult<T, $t, E>
            where
              T: Stream,
              <T as Stream>::Token: AsChar,
            {
                let i = input;

                if i.eof_offset() == 0 {
                    return Err(ErrMode::from_error_kind(i, ErrorKind::Digit));
                }

                let mut value: $t = 0;
                for (offset, c) in i.iter_offsets() {
                    match c.as_char().to_digit(10) {
                        None => {
                            if offset == 0 {
                                return Err(ErrMode::from_error_kind(i, ErrorKind::Digit));
                            } else {
                                return Ok((i.next_slice(offset).0, value));
                            }
                        },
                        Some(d) => match value.checked_mul(10).and_then(|v| v.checked_add(d as $t)) {
                            None => return Err(ErrMode::from_error_kind(i, ErrorKind::Digit)),
                            Some(v) => value = v,
                        }
                    }
                }

                Ok((i.next_slice(i.eof_offset()).0, value))
            }
        )+
    }
}

uints! { u8 u16 u32 u64 u128 }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::alt;
    use crate::error::ErrMode;
    use crate::error::Error;
    use crate::stream::ParseSlice;
    use proptest::prelude::*;

    macro_rules! assert_parse(
    ($left: expr, $right: expr) => {
      let res: $crate::IResult<_, _> = $left;
      assert_eq!(res, $right);
    };
  );

    #[test]
    fn character() {
        let empty: &[u8] = b"";
        let a: &[u8] = b"abcd";
        let b: &[u8] = b"1234";
        let c: &[u8] = b"a123";
        let d: &[u8] = "azé12".as_bstr();
        let e: &[u8] = b" ";
        let f: &[u8] = b" ;";
        //assert_eq!(alpha1::<_, Error<_>>(a), Err(ErrMode::Incomplete(Needed::Size(1))));
        assert_parse!(alpha1(a), Ok((empty, a)));
        assert_eq!(
            alpha1(b),
            Err(ErrMode::Backtrack(Error {
                input: b,
                kind: ErrorKind::Alpha
            }))
        );
        assert_eq!(alpha1::<_, Error<_>>(c), Ok((&c[1..], &b"a"[..])));
        assert_eq!(alpha1::<_, Error<_>>(d), Ok(("é12".as_bstr(), &b"az"[..])));
        assert_eq!(
            digit1(a),
            Err(ErrMode::Backtrack(Error {
                input: a,
                kind: ErrorKind::Digit
            }))
        );
        assert_eq!(digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(
            digit1(c),
            Err(ErrMode::Backtrack(Error {
                input: c,
                kind: ErrorKind::Digit
            }))
        );
        assert_eq!(
            digit1(d),
            Err(ErrMode::Backtrack(Error {
                input: d,
                kind: ErrorKind::Digit
            }))
        );
        assert_eq!(hex_digit1::<_, Error<_>>(a), Ok((empty, a)));
        assert_eq!(hex_digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(hex_digit1::<_, Error<_>>(c), Ok((empty, c)));
        assert_eq!(
            hex_digit1::<_, Error<_>>(d),
            Ok(("zé12".as_bstr(), &b"a"[..]))
        );
        assert_eq!(
            hex_digit1(e),
            Err(ErrMode::Backtrack(Error {
                input: e,
                kind: ErrorKind::HexDigit
            }))
        );
        assert_eq!(
            oct_digit1(a),
            Err(ErrMode::Backtrack(Error {
                input: a,
                kind: ErrorKind::OctDigit
            }))
        );
        assert_eq!(oct_digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(
            oct_digit1(c),
            Err(ErrMode::Backtrack(Error {
                input: c,
                kind: ErrorKind::OctDigit
            }))
        );
        assert_eq!(
            oct_digit1(d),
            Err(ErrMode::Backtrack(Error {
                input: d,
                kind: ErrorKind::OctDigit
            }))
        );
        assert_eq!(alphanumeric1::<_, Error<_>>(a), Ok((empty, a)));
        //assert_eq!(fix_error!(b,(), alphanumeric), Ok((empty, b)));
        assert_eq!(alphanumeric1::<_, Error<_>>(c), Ok((empty, c)));
        assert_eq!(
            alphanumeric1::<_, Error<_>>(d),
            Ok(("é12".as_bstr(), &b"az"[..]))
        );
        assert_eq!(space1::<_, Error<_>>(e), Ok((empty, e)));
        assert_eq!(space1::<_, Error<_>>(f), Ok((&b";"[..], &b" "[..])));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn character_s() {
        let empty = "";
        let a = "abcd";
        let b = "1234";
        let c = "a123";
        let d = "azé12";
        let e = " ";
        assert_eq!(alpha1::<_, Error<_>>(a), Ok((empty, a)));
        assert_eq!(
            alpha1(b),
            Err(ErrMode::Backtrack(Error {
                input: b,
                kind: ErrorKind::Alpha
            }))
        );
        assert_eq!(alpha1::<_, Error<_>>(c), Ok((&c[1..], "a")));
        assert_eq!(alpha1::<_, Error<_>>(d), Ok(("é12", "az")));
        assert_eq!(
            digit1(a),
            Err(ErrMode::Backtrack(Error {
                input: a,
                kind: ErrorKind::Digit
            }))
        );
        assert_eq!(digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(
            digit1(c),
            Err(ErrMode::Backtrack(Error {
                input: c,
                kind: ErrorKind::Digit
            }))
        );
        assert_eq!(
            digit1(d),
            Err(ErrMode::Backtrack(Error {
                input: d,
                kind: ErrorKind::Digit
            }))
        );
        assert_eq!(hex_digit1::<_, Error<_>>(a), Ok((empty, a)));
        assert_eq!(hex_digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(hex_digit1::<_, Error<_>>(c), Ok((empty, c)));
        assert_eq!(hex_digit1::<_, Error<_>>(d), Ok(("zé12", "a")));
        assert_eq!(
            hex_digit1(e),
            Err(ErrMode::Backtrack(Error {
                input: e,
                kind: ErrorKind::HexDigit
            }))
        );
        assert_eq!(
            oct_digit1(a),
            Err(ErrMode::Backtrack(Error {
                input: a,
                kind: ErrorKind::OctDigit
            }))
        );
        assert_eq!(oct_digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(
            oct_digit1(c),
            Err(ErrMode::Backtrack(Error {
                input: c,
                kind: ErrorKind::OctDigit
            }))
        );
        assert_eq!(
            oct_digit1(d),
            Err(ErrMode::Backtrack(Error {
                input: d,
                kind: ErrorKind::OctDigit
            }))
        );
        assert_eq!(alphanumeric1::<_, Error<_>>(a), Ok((empty, a)));
        //assert_eq!(fix_error!(b,(), alphanumeric), Ok((empty, b)));
        assert_eq!(alphanumeric1::<_, Error<_>>(c), Ok((empty, c)));
        assert_eq!(alphanumeric1::<_, Error<_>>(d), Ok(("é12", "az")));
        assert_eq!(space1::<_, Error<_>>(e), Ok((empty, e)));
    }

    use crate::stream::Offset;
    #[test]
    fn offset() {
        let a = &b"abcd;"[..];
        let b = &b"1234;"[..];
        let c = &b"a123;"[..];
        let d = &b" \t;"[..];
        let e = &b" \t\r\n;"[..];
        let f = &b"123abcDEF;"[..];

        match alpha1::<_, Error<_>>(a) {
            Ok((i, _)) => {
                assert_eq!(a.offset_to(i) + i.len(), a.len());
            }
            _ => panic!("wrong return type in offset test for alpha"),
        }
        match digit1::<_, Error<_>>(b) {
            Ok((i, _)) => {
                assert_eq!(b.offset_to(i) + i.len(), b.len());
            }
            _ => panic!("wrong return type in offset test for digit"),
        }
        match alphanumeric1::<_, Error<_>>(c) {
            Ok((i, _)) => {
                assert_eq!(c.offset_to(i) + i.len(), c.len());
            }
            _ => panic!("wrong return type in offset test for alphanumeric"),
        }
        match space1::<_, Error<_>>(d) {
            Ok((i, _)) => {
                assert_eq!(d.offset_to(i) + i.len(), d.len());
            }
            _ => panic!("wrong return type in offset test for space"),
        }
        match multispace1::<_, Error<_>>(e) {
            Ok((i, _)) => {
                assert_eq!(e.offset_to(i) + i.len(), e.len());
            }
            _ => panic!("wrong return type in offset test for multispace"),
        }
        match hex_digit1::<_, Error<_>>(f) {
            Ok((i, _)) => {
                assert_eq!(f.offset_to(i) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for hex_digit"),
        }
        match oct_digit1::<_, Error<_>>(f) {
            Ok((i, _)) => {
                assert_eq!(f.offset_to(i) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for oct_digit"),
        }
    }

    #[test]
    fn is_not_line_ending_bytes() {
        let a: &[u8] = b"ab12cd\nefgh";
        assert_eq!(
            not_line_ending::<_, Error<_>>(a),
            Ok((&b"\nefgh"[..], &b"ab12cd"[..]))
        );

        let b: &[u8] = b"ab12cd\nefgh\nijkl";
        assert_eq!(
            not_line_ending::<_, Error<_>>(b),
            Ok((&b"\nefgh\nijkl"[..], &b"ab12cd"[..]))
        );

        let c: &[u8] = b"ab12cd\r\nefgh\nijkl";
        assert_eq!(
            not_line_ending::<_, Error<_>>(c),
            Ok((&b"\r\nefgh\nijkl"[..], &b"ab12cd"[..]))
        );

        let d: &[u8] = b"ab12cd";
        assert_eq!(not_line_ending::<_, Error<_>>(d), Ok((&[][..], d)));
    }

    #[test]
    fn is_not_line_ending_str() {
        /*
        let a: &str = "ab12cd\nefgh";
        assert_eq!(not_line_ending(a), Ok((&"\nefgh"[..], &"ab12cd"[..])));

        let b: &str = "ab12cd\nefgh\nijkl";
        assert_eq!(not_line_ending(b), Ok((&"\nefgh\nijkl"[..], &"ab12cd"[..])));

        let c: &str = "ab12cd\r\nefgh\nijkl";
        assert_eq!(not_line_ending(c), Ok((&"\r\nefgh\nijkl"[..], &"ab12cd"[..])));

        let d = "βèƒôřè\nÂßÇáƒƭèř";
        assert_eq!(not_line_ending(d), Ok((&"\nÂßÇáƒƭèř"[..], &"βèƒôřè"[..])));

        let e = "βèƒôřè\r\nÂßÇáƒƭèř";
        assert_eq!(not_line_ending(e), Ok((&"\r\nÂßÇáƒƭèř"[..], &"βèƒôřè"[..])));
        */

        let f = "βèƒôřè\rÂßÇáƒƭèř";
        assert_eq!(
            not_line_ending(f),
            Err(ErrMode::Backtrack(Error {
                input: f,
                kind: ErrorKind::Tag
            }))
        );

        let g2: &str = "ab12cd";
        assert_eq!(not_line_ending::<_, Error<_>>(g2), Ok(("", g2)));
    }

    #[test]
    fn hex_digit_test() {
        let i = &b"0123456789abcdefABCDEF;"[..];
        assert_parse!(hex_digit1(i), Ok((&b";"[..], &i[..i.len() - 1])));

        let i = &b"g"[..];
        assert_parse!(
            hex_digit1(i),
            Err(ErrMode::Backtrack(error_position!(i, ErrorKind::HexDigit)))
        );

        let i = &b"G"[..];
        assert_parse!(
            hex_digit1(i),
            Err(ErrMode::Backtrack(error_position!(i, ErrorKind::HexDigit)))
        );

        assert!(AsChar::is_hex_digit(b'0'));
        assert!(AsChar::is_hex_digit(b'9'));
        assert!(AsChar::is_hex_digit(b'a'));
        assert!(AsChar::is_hex_digit(b'f'));
        assert!(AsChar::is_hex_digit(b'A'));
        assert!(AsChar::is_hex_digit(b'F'));
        assert!(!AsChar::is_hex_digit(b'g'));
        assert!(!AsChar::is_hex_digit(b'G'));
        assert!(!AsChar::is_hex_digit(b'/'));
        assert!(!AsChar::is_hex_digit(b':'));
        assert!(!AsChar::is_hex_digit(b'@'));
        assert!(!AsChar::is_hex_digit(b'\x60'));
    }

    #[test]
    fn oct_digit_test() {
        let i = &b"01234567;"[..];
        assert_parse!(oct_digit1(i), Ok((&b";"[..], &i[..i.len() - 1])));

        let i = &b"8"[..];
        assert_parse!(
            oct_digit1(i),
            Err(ErrMode::Backtrack(error_position!(i, ErrorKind::OctDigit)))
        );

        assert!(AsChar::is_oct_digit(b'0'));
        assert!(AsChar::is_oct_digit(b'7'));
        assert!(!AsChar::is_oct_digit(b'8'));
        assert!(!AsChar::is_oct_digit(b'9'));
        assert!(!AsChar::is_oct_digit(b'a'));
        assert!(!AsChar::is_oct_digit(b'A'));
        assert!(!AsChar::is_oct_digit(b'/'));
        assert!(!AsChar::is_oct_digit(b':'));
        assert!(!AsChar::is_oct_digit(b'@'));
        assert!(!AsChar::is_oct_digit(b'\x60'));
    }

    #[test]
    fn full_line_windows() {
        use crate::sequence::pair;
        fn take_full_line(i: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
            pair(not_line_ending, line_ending)(i)
        }
        let input = b"abc\r\n";
        let output = take_full_line(input);
        assert_eq!(output, Ok((&b""[..], (&b"abc"[..], &b"\r\n"[..]))));
    }

    #[test]
    fn full_line_unix() {
        use crate::sequence::pair;
        fn take_full_line(i: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
            pair(not_line_ending, line_ending)(i)
        }
        let input = b"abc\n";
        let output = take_full_line(input);
        assert_eq!(output, Ok((&b""[..], (&b"abc"[..], &b"\n"[..]))));
    }

    #[test]
    fn check_windows_lineending() {
        let input = b"\r\n";
        let output = line_ending(&input[..]);
        assert_parse!(output, Ok((&b""[..], &b"\r\n"[..])));
    }

    #[test]
    fn check_unix_lineending() {
        let input = b"\n";
        let output = line_ending(&input[..]);
        assert_parse!(output, Ok((&b""[..], &b"\n"[..])));
    }

    #[test]
    fn cr_lf() {
        assert_parse!(crlf(&b"\r\na"[..]), Ok((&b"a"[..], &b"\r\n"[..])));
        assert_parse!(
            crlf(&b"\r"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"\r"[..],
                ErrorKind::CrLf
            )))
        );
        assert_parse!(
            crlf(&b"\ra"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"\ra"[..],
                ErrorKind::CrLf
            )))
        );

        assert_parse!(crlf("\r\na"), Ok(("a", "\r\n")));
        assert_parse!(
            crlf("\r"),
            Err(ErrMode::Backtrack(error_position!("\r", ErrorKind::CrLf)))
        );
        assert_parse!(
            crlf("\ra"),
            Err(ErrMode::Backtrack(error_position!("\ra", ErrorKind::CrLf)))
        );
    }

    #[test]
    fn end_of_line() {
        assert_parse!(line_ending(&b"\na"[..]), Ok((&b"a"[..], &b"\n"[..])));
        assert_parse!(line_ending(&b"\r\na"[..]), Ok((&b"a"[..], &b"\r\n"[..])));
        assert_parse!(
            line_ending(&b"\r"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"\r"[..],
                ErrorKind::CrLf
            )))
        );
        assert_parse!(
            line_ending(&b"\ra"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"\ra"[..],
                ErrorKind::CrLf
            )))
        );

        assert_parse!(line_ending("\na"), Ok(("a", "\n")));
        assert_parse!(line_ending("\r\na"), Ok(("a", "\r\n")));
        assert_parse!(
            line_ending("\r"),
            Err(ErrMode::Backtrack(error_position!("\r", ErrorKind::CrLf)))
        );
        assert_parse!(
            line_ending("\ra"),
            Err(ErrMode::Backtrack(error_position!("\ra", ErrorKind::CrLf)))
        );
    }

    fn digit_to_i16(input: &str) -> IResult<&str, i16> {
        let i = input;
        let (i, opt_sign) = opt(alt((char('+'), char('-'))))(i)?;
        let sign = match opt_sign {
            Some('+') | None => true,
            Some('-') => false,
            _ => unreachable!(),
        };

        let (i, s) = match digit1::<_, crate::error::Error<_>>(i) {
            Ok((i, s)) => (i, s),
            Err(_) => return Err(ErrMode::from_error_kind(input, ErrorKind::Digit)),
        };

        match s.parse_slice() {
            Some(n) => {
                if sign {
                    Ok((i, n))
                } else {
                    Ok((i, -n))
                }
            }
            None => Err(ErrMode::from_error_kind(i, ErrorKind::Digit)),
        }
    }

    fn digit_to_u32(i: &str) -> IResult<&str, u32> {
        let (i, s) = digit1(i)?;
        match s.parse_slice() {
            Some(n) => Ok((i, n)),
            None => Err(ErrMode::from_error_kind(i, ErrorKind::Digit)),
        }
    }

    proptest! {
        #[test]
    #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn ints(s in "\\PC*") {
            let res1 = digit_to_i16(&s);
            let res2 = i16(s.as_str());
            assert_eq!(res1, res2);
        }

        #[test]
    #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn uints(s in "\\PC*") {
            let res1 = digit_to_u32(&s);
            let res2 = u32(s.as_str());
            assert_eq!(res1, res2);
        }
      }
}
