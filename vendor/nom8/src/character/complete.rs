//! Character specific parsers and combinators, complete input version.
//!
//! Functions recognizing specific characters.

#![allow(deprecated)]

use crate::branch::alt;
use crate::combinator::opt;
use crate::error::ErrorKind;
use crate::error::ParseError;
use crate::input::{
  AsChar, FindToken, InputIter, InputLength, InputTake, InputTakeAtPosition, IntoOutput, Slice,
};
use crate::input::{Compare, CompareResult};
use crate::lib::std::ops::{Range, RangeFrom, RangeTo};
use crate::IntoOutputIResult as _;
use crate::{Err, IResult};

/// Recognizes one character.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{Err, error::{ErrorKind, Error}, IResult};
/// # use nom8::character::complete::char;
/// fn parser(i: &str) -> IResult<&str, char> {
///     char('a')(i)
/// }
/// assert_eq!(parser("abc"), Ok(("bc", 'a')));
/// assert_eq!(parser(" abc"), Err(Err::Error(Error::new(" abc", ErrorKind::Char))));
/// assert_eq!(parser("bc"), Err(Err::Error(Error::new("bc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Char))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::one_of`][crate::bytes::one_of]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::one_of`")]
pub fn char<I, Error: ParseError<I>>(c: char) -> impl Fn(I) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter,
  <I as InputIter>::Item: AsChar,
{
  move |i: I| char_internal(i, c)
}

pub(crate) fn char_internal<I, Error: ParseError<I>>(i: I, c: char) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter,
  <I as InputIter>::Item: AsChar,
{
  match (i).iter_elements().next().map(|t| {
    let b = t.as_char() == c;
    (&c, b)
  }) {
    Some((c, true)) => Ok((i.slice(c.len()..), c.as_char())),
    _ => Err(Err::Error(Error::from_char(i, c))),
  }
}

/// Recognizes one character and checks that it satisfies a predicate
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{Err, error::{ErrorKind, Error}, Needed, IResult};
/// # use nom8::character::complete::satisfy;
/// fn parser(i: &str) -> IResult<&str, char> {
///     satisfy(|c| c == 'a' || c == 'b')(i)
/// }
/// assert_eq!(parser("abc"), Ok(("bc", 'a')));
/// assert_eq!(parser("cd"), Err(Err::Error(Error::new("cd", ErrorKind::Satisfy))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Satisfy))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::one_of`][crate::bytes::one_of]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::one_of`")]
pub fn satisfy<F, I, Error: ParseError<I>>(cond: F) -> impl Fn(I) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter,
  <I as InputIter>::Item: AsChar,
  F: Fn(char) -> bool,
{
  move |i: I| satisfy_internal(i, &cond)
}

pub(crate) fn satisfy_internal<F, I, Error: ParseError<I>>(
  i: I,
  cond: &F,
) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter,
  <I as InputIter>::Item: AsChar,
  F: Fn(char) -> bool,
{
  match (i).iter_elements().next().map(|t| {
    let c = t.as_char();
    let b = cond(c);
    (c, b)
  }) {
    Some((c, true)) => Ok((i.slice(c.len()..), c)),
    _ => Err(Err::Error(Error::from_error_kind(i, ErrorKind::Satisfy))),
  }
}

/// Recognizes one of the provided characters.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind};
/// # use nom8::character::complete::one_of;
/// assert_eq!(one_of::<_, _, (&str, ErrorKind)>("abc")("b"), Ok(("", 'b')));
/// assert_eq!(one_of::<_, _, (&str, ErrorKind)>("a")("bc"), Err(Err::Error(("bc", ErrorKind::OneOf))));
/// assert_eq!(one_of::<_, _, (&str, ErrorKind)>("a")(""), Err(Err::Error(("", ErrorKind::OneOf))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::one_of`][crate::bytes::one_of]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::one_of`")]
pub fn one_of<I, T, Error: ParseError<I>>(list: T) -> impl Fn(I) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter + InputLength,
  <I as InputIter>::Item: AsChar + Copy,
  T: FindToken<<I as InputIter>::Item>,
{
  move |i: I| crate::bytes::complete::one_of_internal(i, &list).map(|(i, c)| (i, c.as_char()))
}

/// Recognizes a character that is not in the provided characters.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind};
/// # use nom8::character::complete::none_of;
/// assert_eq!(none_of::<_, _, (&str, ErrorKind)>("abc")("z"), Ok(("", 'z')));
/// assert_eq!(none_of::<_, _, (&str, ErrorKind)>("ab")("a"), Err(Err::Error(("a", ErrorKind::NoneOf))));
/// assert_eq!(none_of::<_, _, (&str, ErrorKind)>("a")(""), Err(Err::Error(("", ErrorKind::NoneOf))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::none_of`][crate::bytes::none_of]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::none_of`")]
pub fn none_of<I, T, Error: ParseError<I>>(list: T) -> impl Fn(I) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputLength + InputIter,
  <I as InputIter>::Item: AsChar + Copy,
  T: FindToken<<I as InputIter>::Item>,
{
  move |i: I| crate::bytes::complete::none_of_internal(i, &list).map(|(i, c)| (i, c.as_char()))
}

/// Recognizes the string "\r\n".
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult};
/// # use nom8::character::complete::crlf;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     crlf(input)
/// }
///
/// assert_eq!(parser("\r\nc"), Ok(("c", "\r\n")));
/// assert_eq!(parser("ab\r\nc"), Err(Err::Error(Error::new("ab\r\nc", ErrorKind::CrLf))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::CrLf))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::crlf`][crate::character::crlf]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::crlf`")]
pub fn crlf<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: Slice<Range<usize>> + Slice<RangeFrom<usize>>,
  T: InputIter,
  T: IntoOutput,
  T: Compare<&'static str>,
{
  match input.compare("\r\n") {
    //FIXME: is this the right index?
    CompareResult::Ok => Ok((input.slice(2..), input.slice(0..2))).into_output(),
    _ => {
      let e: ErrorKind = ErrorKind::CrLf;
      Err(Err::Error(E::from_error_kind(input, e)))
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
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::not_line_ending;
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
/// **WARNING:** Deprecated, replaced with [`nom8::character::not_line_ending`][crate::character::not_line_ending]
#[deprecated(
  since = "8.0.0",
  note = "Replaced with `nom8::character::not_line_ending`"
)]
pub fn not_line_ending<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: Slice<Range<usize>> + Slice<RangeFrom<usize>> + Slice<RangeTo<usize>>,
  T: InputIter + InputLength,
  T: IntoOutput,
  T: Compare<&'static str>,
  <T as InputIter>::Item: AsChar,
  <T as InputIter>::Item: AsChar,
{
  match input.position(|item| {
    let c = item.as_char();
    c == '\r' || c == '\n'
  }) {
    None => Ok((input.slice(input.input_len()..), input)).into_output(),
    Some(index) => {
      let mut it = input.slice(index..).iter_elements();
      let nth = it.next().unwrap().as_char();
      if nth == '\r' {
        let sliced = input.slice(index..);
        let comp = sliced.compare("\r\n");
        match comp {
          //FIXME: calculate the right index
          CompareResult::Ok => Ok((input.slice(index..), input.slice(..index))).into_output(),
          _ => {
            let e: ErrorKind = ErrorKind::Tag;
            Err(Err::Error(E::from_error_kind(input, e)))
          }
        }
      } else {
        Ok((input.slice(index..), input.slice(..index))).into_output()
      }
    }
  }
}

/// Recognizes an end of line (both '\n' and '\r\n').
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::line_ending;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     line_ending(input)
/// }
///
/// assert_eq!(parser("\r\nc"), Ok(("c", "\r\n")));
/// assert_eq!(parser("ab\r\nc"), Err(Err::Error(Error::new("ab\r\nc", ErrorKind::CrLf))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::CrLf))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::line_ending`][crate::character::line_ending]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::line_ending`")]
pub fn line_ending<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: Slice<Range<usize>> + Slice<RangeFrom<usize>> + Slice<RangeTo<usize>>,
  T: InputIter + InputLength,
  T: IntoOutput,
  T: Compare<&'static str>,
{
  match input.compare("\n") {
    CompareResult::Ok => Ok((input.slice(1..), input.slice(0..1))).into_output(),
    CompareResult::Incomplete => Err(Err::Error(E::from_error_kind(input, ErrorKind::CrLf))),
    CompareResult::Error => {
      match input.compare("\r\n") {
        //FIXME: is this the right index?
        CompareResult::Ok => Ok((input.slice(2..), input.slice(0..2))).into_output(),
        _ => Err(Err::Error(E::from_error_kind(input, ErrorKind::CrLf))),
      }
    }
  }
}

/// Matches a newline character '\n'.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::newline;
/// fn parser(input: &str) -> IResult<&str, char> {
///     newline(input)
/// }
///
/// assert_eq!(parser("\nc"), Ok(("c", '\n')));
/// assert_eq!(parser("\r\nc"), Err(Err::Error(Error::new("\r\nc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Char))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::newline`][crate::character::newline]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::newline`")]
pub fn newline<I, Error: ParseError<I>>(input: I) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter,
  <I as InputIter>::Item: AsChar,
{
  char('\n')(input)
}

/// Matches a tab character '\t'.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::tab;
/// fn parser(input: &str) -> IResult<&str, char> {
///     tab(input)
/// }
///
/// assert_eq!(parser("\tc"), Ok(("c", '\t')));
/// assert_eq!(parser("\r\nc"), Err(Err::Error(Error::new("\r\nc", ErrorKind::Char))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Char))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::tab`][crate::character::tab]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::tab`")]
pub fn tab<I, Error: ParseError<I>>(input: I) -> IResult<I, char, Error>
where
  I: Slice<RangeFrom<usize>> + InputIter,
  <I as InputIter>::Item: AsChar,
{
  char('\t')(input)
}

/// Matches one byte as a character. Note that the input type will
/// accept a `str`, but not a `&[u8]`, unlike many other nom parsers.
///
/// *Complete version*: Will return an error if there's not enough input data.
/// # Example
///
/// ```
/// # use nom8::{character::complete::anychar, Err, error::{Error, ErrorKind}, IResult};
/// fn parser(input: &str) -> IResult<&str, char> {
///     anychar(input)
/// }
///
/// assert_eq!(parser("abc"), Ok(("bc",'a')));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Eof))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::any`][crate::bytes::any]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::any`")]
pub fn anychar<T, E: ParseError<T>>(input: T) -> IResult<T, char, E>
where
  T: InputIter + InputLength + Slice<RangeFrom<usize>>,
  <T as InputIter>::Item: AsChar,
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
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::complete::alpha0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alpha0(input)
/// }
///
/// assert_eq!(parser("ab1c"), Ok(("1c", "ab")));
/// assert_eq!(parser("1c"), Ok(("1c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::alpha0`][crate::character::alpha0]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::alpha0`")]
pub fn alpha0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position_complete(|item| !item.is_alpha())
    .into_output()
}

/// Recognizes one or more lowercase and uppercase ASCII alphabetic characters: a-z, A-Z
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found  (a non alphabetic character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::alpha1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alpha1(input)
/// }
///
/// assert_eq!(parser("aB1c"), Ok(("1c", "aB")));
/// assert_eq!(parser("1c"), Err(Err::Error(Error::new("1c", ErrorKind::Alpha))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Alpha))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::alpha1`][crate::character::alpha1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::alpha1`")]
pub fn alpha1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position1_complete(|item| !item.is_alpha(), ErrorKind::Alpha)
    .into_output()
}

/// Recognizes zero or more ASCII numerical characters: 0-9
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::complete::digit0;
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
/// **WARNING:** Deprecated, replaced with [`nom8::character::digit0`][crate::character::digit0]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::digit0`")]
pub fn digit0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position_complete(|item| !item.is_dec_digit())
    .into_output()
}

/// Recognizes one or more ASCII numerical characters: 0-9
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     digit1(input)
/// }
///
/// assert_eq!(parser("21c"), Ok(("c", "21")));
/// assert_eq!(parser("c1"), Err(Err::Error(Error::new("c1", ErrorKind::Digit))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Digit))));
/// ```
///
/// ## Parsing an integer
/// You can use `digit1` in combination with [`map_res`] to parse an integer:
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::combinator::map_res;
/// # use nom8::character::complete::digit1;
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
/// **WARNING:** Deprecated, replaced with [`nom8::character::digit1`][crate::character::digit1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::digit1`")]
pub fn digit1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position1_complete(|item| !item.is_dec_digit(), ErrorKind::Digit)
    .into_output()
}

/// Recognizes zero or more ASCII hexadecimal numerical characters: 0-9, A-F, a-f
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non hexadecimal digit character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::complete::hex_digit0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     hex_digit0(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::hex_digit0`][crate::character::hex_digit0]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::hex_digit0`")]
pub fn hex_digit0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position_complete(|item| !item.is_hex_digit())
    .into_output()
}
/// Recognizes one or more ASCII hexadecimal numerical characters: 0-9, A-F, a-f
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non hexadecimal digit character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::hex_digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     hex_digit1(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(parser("H2"), Err(Err::Error(Error::new("H2", ErrorKind::HexDigit))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::HexDigit))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::hex_digit1`][crate::character::hex_digit1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::hex_digit1`")]
pub fn hex_digit1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position1_complete(|item| !item.is_hex_digit(), ErrorKind::HexDigit)
    .into_output()
}

/// Recognizes zero or more octal characters: 0-7
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non octal
/// digit character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::complete::oct_digit0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     oct_digit0(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("cZ", "21")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::oct_digit0`][crate::character::oct_digit0]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::oct_digit0`")]
pub fn oct_digit0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position_complete(|item| !item.is_oct_digit())
    .into_output()
}

/// Recognizes one or more octal characters: 0-7
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non octal digit character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::oct_digit1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     oct_digit1(input)
/// }
///
/// assert_eq!(parser("21cZ"), Ok(("cZ", "21")));
/// assert_eq!(parser("H2"), Err(Err::Error(Error::new("H2", ErrorKind::OctDigit))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::OctDigit))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::oct_digit1`][crate::character::oct_digit1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::oct_digit1`")]
pub fn oct_digit1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position1_complete(|item| !item.is_oct_digit(), ErrorKind::OctDigit)
    .into_output()
}

/// Recognizes zero or more ASCII numerical and alphabetic characters: 0-9, a-z, A-Z
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphanumerical character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::complete::alphanumeric0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alphanumeric0(input)
/// }
///
/// assert_eq!(parser("21cZ%1"), Ok(("%1", "21cZ")));
/// assert_eq!(parser("&Z21c"), Ok(("&Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::alphanumeric0`][crate::character::alphanumeric0]
#[deprecated(
  since = "8.0.0",
  note = "Replaced with `nom8::character::alphanumeric0`"
)]
pub fn alphanumeric0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position_complete(|item| !item.is_alphanum())
    .into_output()
}

/// Recognizes one or more ASCII numerical and alphabetic characters: 0-9, a-z, A-Z
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non alphanumerical character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::alphanumeric1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     alphanumeric1(input)
/// }
///
/// assert_eq!(parser("21cZ%1"), Ok(("%1", "21cZ")));
/// assert_eq!(parser("&H2"), Err(Err::Error(Error::new("&H2", ErrorKind::AlphaNumeric))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::AlphaNumeric))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::alphanumeric1`][crate::character::alphanumeric1]
#[deprecated(
  since = "8.0.0",
  note = "Replaced with `nom8::character::alphanumeric1`"
)]
pub fn alphanumeric1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar,
{
  input
    .split_at_position1_complete(|item| !item.is_alphanum(), ErrorKind::AlphaNumeric)
    .into_output()
}

/// Recognizes zero or more spaces and tabs.
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non space
/// character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::complete::space0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     space0(input)
/// }
///
/// assert_eq!(parser(" \t21c"), Ok(("21c", " \t")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::space0`][crate::character::space0]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::space0`")]
pub fn space0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar + Clone,
{
  input
    .split_at_position_complete(|item| {
      let c = item.as_char();
      !(c == ' ' || c == '\t')
    })
    .into_output()
}

/// Recognizes one or more spaces and tabs.
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::space1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     space1(input)
/// }
///
/// assert_eq!(parser(" \t21c"), Ok(("21c", " \t")));
/// assert_eq!(parser("H2"), Err(Err::Error(Error::new("H2", ErrorKind::Space))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Space))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::space1`][crate::character::space1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::space1`")]
pub fn space1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar + Clone,
{
  input
    .split_at_position1_complete(
      |item| {
        let c = item.as_char();
        !(c == ' ' || c == '\t')
      },
      ErrorKind::Space,
    )
    .into_output()
}

/// Recognizes zero or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return the whole input if no terminating token is found (a non space
/// character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::ErrorKind, IResult, Needed};
/// # use nom8::character::complete::multispace0;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     multispace0(input)
/// }
///
/// assert_eq!(parser(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert_eq!(parser("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::multispace0`][crate::character::multispace0]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::multispace0`")]
pub fn multispace0<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar + Clone,
{
  input
    .split_at_position_complete(|item| {
      let c = item.as_char();
      !(c == ' ' || c == '\t' || c == '\r' || c == '\n')
    })
    .into_output()
}

/// Recognizes one or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
/// # Example
///
/// ```
/// # use nom8::{Err, error::{Error, ErrorKind}, IResult, Needed};
/// # use nom8::character::complete::multispace1;
/// fn parser(input: &str) -> IResult<&str, &str> {
///     multispace1(input)
/// }
///
/// assert_eq!(parser(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert_eq!(parser("H2"), Err(Err::Error(Error::new("H2", ErrorKind::MultiSpace))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::MultiSpace))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::character::multispace1`][crate::character::multispace1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::character::multispace1`")]
pub fn multispace1<T, E: ParseError<T>>(input: T) -> IResult<T, <T as IntoOutput>::Output, E>
where
  T: InputTakeAtPosition,
  T: IntoOutput,
  <T as InputTakeAtPosition>::Item: AsChar + Clone,
{
  input
    .split_at_position1_complete(
      |item| {
        let c = item.as_char();
        !(c == ' ' || c == '\t' || c == '\r' || c == '\n')
      },
      ErrorKind::MultiSpace,
    )
    .into_output()
}

pub(crate) fn sign<T, E: ParseError<T>>(input: T) -> IResult<T, bool, E>
where
  T: Clone + InputTake,
  T: IntoOutput,
  T: for<'a> Compare<&'a [u8]>,
{
  use crate::bytes::complete::tag;
  use crate::combinator::value;

  let (i, opt_sign) = opt(alt((
    value(false, tag(&b"-"[..])),
    value(true, tag(&b"+"[..])),
  )))(input)?;
  let sign = opt_sign.unwrap_or(true);

  Ok((i, sign))
}

#[doc(hidden)]
macro_rules! ints {
    ($($t:tt)+) => {
        $(
        /// will parse a number in text form to a number
        ///
        /// *Complete version*: can parse until the end of input.
        pub fn $t<T, E: ParseError<T>>(input: T) -> IResult<T, $t, E>
            where
            T: InputIter + Slice<RangeFrom<usize>> + InputLength + InputTake + Clone,
            T: IntoOutput,
            <T as InputIter>::Item: AsChar,
            T: for <'a> Compare<&'a[u8]>,
            {
                let (i, sign) = sign(input.clone())?;

                if i.input_len() == 0 {
                    return Err(Err::Error(E::from_error_kind(input, ErrorKind::Digit)));
                }

                let mut value: $t = 0;
                if sign {
                    for (pos, c) in i.iter_indices() {
                        match c.as_char().to_digit(10) {
                            None => {
                                if pos == 0 {
                                    return Err(Err::Error(E::from_error_kind(input, ErrorKind::Digit)));
                                } else {
                                    return Ok((i.slice(pos..), value));
                                }
                            },
                            Some(d) => match value.checked_mul(10).and_then(|v| v.checked_add(d as $t)) {
                                None => return Err(Err::Error(E::from_error_kind(input, ErrorKind::Digit))),
                                Some(v) => value = v,
                            }
                        }
                    }
                } else {
                    for (pos, c) in i.iter_indices() {
                        match c.as_char().to_digit(10) {
                            None => {
                                if pos == 0 {
                                    return Err(Err::Error(E::from_error_kind(input, ErrorKind::Digit)));
                                } else {
                                    return Ok((i.slice(pos..), value));
                                }
                            },
                            Some(d) => match value.checked_mul(10).and_then(|v| v.checked_sub(d as $t)) {
                                None => return Err(Err::Error(E::from_error_kind(input, ErrorKind::Digit))),
                                Some(v) => value = v,
                            }
                        }
                    }
                }

                Ok((i.slice(i.input_len()..), value))
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
        pub fn $t<T, E: ParseError<T>>(input: T) -> IResult<T, $t, E>
            where
            T: InputIter + Slice<RangeFrom<usize>> + InputLength,
            T: IntoOutput,
            <T as InputIter>::Item: AsChar,
            {
                let i = input;

                if i.input_len() == 0 {
                    return Err(Err::Error(E::from_error_kind(i, ErrorKind::Digit)));
                }

                let mut value: $t = 0;
                for (pos, c) in i.iter_indices() {
                    match c.as_char().to_digit(10) {
                        None => {
                            if pos == 0 {
                                return Err(Err::Error(E::from_error_kind(i, ErrorKind::Digit)));
                            } else {
                                return Ok((i.slice(pos..), value));
                            }
                        },
                        Some(d) => match value.checked_mul(10).and_then(|v| v.checked_add(d as $t)) {
                            None => return Err(Err::Error(E::from_error_kind(i, ErrorKind::Digit))),
                            Some(v) => value = v,
                        }
                    }
                }

                Ok((i.slice(i.input_len()..), value))
            }
        )+
    }
}

uints! { u8 u16 u32 u64 u128 }

#[cfg(test)]
mod tests {
  use super::*;
  use crate::input::ParseTo;
  use crate::Err;
  use proptest::prelude::*;

  macro_rules! assert_parse(
    ($left: expr, $right: expr) => {
      let res: $crate::IResult<_, _, (_, ErrorKind)> = $left;
      assert_eq!(res, $right);
    };
  );

  #[test]
  fn character() {
    let empty: &[u8] = b"";
    let a: &[u8] = b"abcd";
    let b: &[u8] = b"1234";
    let c: &[u8] = b"a123";
    let d: &[u8] = "azé12".as_bytes();
    let e: &[u8] = b" ";
    let f: &[u8] = b" ;";
    //assert_eq!(alpha1::<_, (_, ErrorKind)>(a), Err(Err::Incomplete(Needed::Size(1))));
    assert_parse!(alpha1(a), Ok((empty, a)));
    assert_eq!(alpha1(b), Err(Err::Error((b, ErrorKind::Alpha))));
    assert_eq!(alpha1::<_, (_, ErrorKind)>(c), Ok((&c[1..], &b"a"[..])));
    assert_eq!(
      alpha1::<_, (_, ErrorKind)>(d),
      Ok(("é12".as_bytes(), &b"az"[..]))
    );
    assert_eq!(digit1(a), Err(Err::Error((a, ErrorKind::Digit))));
    assert_eq!(digit1::<_, (_, ErrorKind)>(b), Ok((empty, b)));
    assert_eq!(digit1(c), Err(Err::Error((c, ErrorKind::Digit))));
    assert_eq!(digit1(d), Err(Err::Error((d, ErrorKind::Digit))));
    assert_eq!(hex_digit1::<_, (_, ErrorKind)>(a), Ok((empty, a)));
    assert_eq!(hex_digit1::<_, (_, ErrorKind)>(b), Ok((empty, b)));
    assert_eq!(hex_digit1::<_, (_, ErrorKind)>(c), Ok((empty, c)));
    assert_eq!(
      hex_digit1::<_, (_, ErrorKind)>(d),
      Ok(("zé12".as_bytes(), &b"a"[..]))
    );
    assert_eq!(hex_digit1(e), Err(Err::Error((e, ErrorKind::HexDigit))));
    assert_eq!(oct_digit1(a), Err(Err::Error((a, ErrorKind::OctDigit))));
    assert_eq!(oct_digit1::<_, (_, ErrorKind)>(b), Ok((empty, b)));
    assert_eq!(oct_digit1(c), Err(Err::Error((c, ErrorKind::OctDigit))));
    assert_eq!(oct_digit1(d), Err(Err::Error((d, ErrorKind::OctDigit))));
    assert_eq!(alphanumeric1::<_, (_, ErrorKind)>(a), Ok((empty, a)));
    //assert_eq!(fix_error!(b,(), alphanumeric), Ok((empty, b)));
    assert_eq!(alphanumeric1::<_, (_, ErrorKind)>(c), Ok((empty, c)));
    assert_eq!(
      alphanumeric1::<_, (_, ErrorKind)>(d),
      Ok(("é12".as_bytes(), &b"az"[..]))
    );
    assert_eq!(space1::<_, (_, ErrorKind)>(e), Ok((empty, e)));
    assert_eq!(space1::<_, (_, ErrorKind)>(f), Ok((&b";"[..], &b" "[..])));
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
    assert_eq!(alpha1::<_, (_, ErrorKind)>(a), Ok((empty, a)));
    assert_eq!(alpha1(b), Err(Err::Error((b, ErrorKind::Alpha))));
    assert_eq!(alpha1::<_, (_, ErrorKind)>(c), Ok((&c[1..], &"a"[..])));
    assert_eq!(alpha1::<_, (_, ErrorKind)>(d), Ok(("é12", &"az"[..])));
    assert_eq!(digit1(a), Err(Err::Error((a, ErrorKind::Digit))));
    assert_eq!(digit1::<_, (_, ErrorKind)>(b), Ok((empty, b)));
    assert_eq!(digit1(c), Err(Err::Error((c, ErrorKind::Digit))));
    assert_eq!(digit1(d), Err(Err::Error((d, ErrorKind::Digit))));
    assert_eq!(hex_digit1::<_, (_, ErrorKind)>(a), Ok((empty, a)));
    assert_eq!(hex_digit1::<_, (_, ErrorKind)>(b), Ok((empty, b)));
    assert_eq!(hex_digit1::<_, (_, ErrorKind)>(c), Ok((empty, c)));
    assert_eq!(hex_digit1::<_, (_, ErrorKind)>(d), Ok(("zé12", &"a"[..])));
    assert_eq!(hex_digit1(e), Err(Err::Error((e, ErrorKind::HexDigit))));
    assert_eq!(oct_digit1(a), Err(Err::Error((a, ErrorKind::OctDigit))));
    assert_eq!(oct_digit1::<_, (_, ErrorKind)>(b), Ok((empty, b)));
    assert_eq!(oct_digit1(c), Err(Err::Error((c, ErrorKind::OctDigit))));
    assert_eq!(oct_digit1(d), Err(Err::Error((d, ErrorKind::OctDigit))));
    assert_eq!(alphanumeric1::<_, (_, ErrorKind)>(a), Ok((empty, a)));
    //assert_eq!(fix_error!(b,(), alphanumeric), Ok((empty, b)));
    assert_eq!(alphanumeric1::<_, (_, ErrorKind)>(c), Ok((empty, c)));
    assert_eq!(alphanumeric1::<_, (_, ErrorKind)>(d), Ok(("é12", "az")));
    assert_eq!(space1::<_, (_, ErrorKind)>(e), Ok((empty, e)));
  }

  use crate::input::Offset;
  #[test]
  fn offset() {
    let a = &b"abcd;"[..];
    let b = &b"1234;"[..];
    let c = &b"a123;"[..];
    let d = &b" \t;"[..];
    let e = &b" \t\r\n;"[..];
    let f = &b"123abcDEF;"[..];

    match alpha1::<_, (_, ErrorKind)>(a) {
      Ok((i, _)) => {
        assert_eq!(a.offset(i) + i.len(), a.len());
      }
      _ => panic!("wrong return type in offset test for alpha"),
    }
    match digit1::<_, (_, ErrorKind)>(b) {
      Ok((i, _)) => {
        assert_eq!(b.offset(i) + i.len(), b.len());
      }
      _ => panic!("wrong return type in offset test for digit"),
    }
    match alphanumeric1::<_, (_, ErrorKind)>(c) {
      Ok((i, _)) => {
        assert_eq!(c.offset(i) + i.len(), c.len());
      }
      _ => panic!("wrong return type in offset test for alphanumeric"),
    }
    match space1::<_, (_, ErrorKind)>(d) {
      Ok((i, _)) => {
        assert_eq!(d.offset(i) + i.len(), d.len());
      }
      _ => panic!("wrong return type in offset test for space"),
    }
    match multispace1::<_, (_, ErrorKind)>(e) {
      Ok((i, _)) => {
        assert_eq!(e.offset(i) + i.len(), e.len());
      }
      _ => panic!("wrong return type in offset test for multispace"),
    }
    match hex_digit1::<_, (_, ErrorKind)>(f) {
      Ok((i, _)) => {
        assert_eq!(f.offset(i) + i.len(), f.len());
      }
      _ => panic!("wrong return type in offset test for hex_digit"),
    }
    match oct_digit1::<_, (_, ErrorKind)>(f) {
      Ok((i, _)) => {
        assert_eq!(f.offset(i) + i.len(), f.len());
      }
      _ => panic!("wrong return type in offset test for oct_digit"),
    }
  }

  #[test]
  fn is_not_line_ending_bytes() {
    let a: &[u8] = b"ab12cd\nefgh";
    assert_eq!(
      not_line_ending::<_, (_, ErrorKind)>(a),
      Ok((&b"\nefgh"[..], &b"ab12cd"[..]))
    );

    let b: &[u8] = b"ab12cd\nefgh\nijkl";
    assert_eq!(
      not_line_ending::<_, (_, ErrorKind)>(b),
      Ok((&b"\nefgh\nijkl"[..], &b"ab12cd"[..]))
    );

    let c: &[u8] = b"ab12cd\r\nefgh\nijkl";
    assert_eq!(
      not_line_ending::<_, (_, ErrorKind)>(c),
      Ok((&b"\r\nefgh\nijkl"[..], &b"ab12cd"[..]))
    );

    let d: &[u8] = b"ab12cd";
    assert_eq!(
      not_line_ending::<_, (_, ErrorKind)>(d),
      Ok((&[][..], &d[..]))
    );
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
    assert_eq!(not_line_ending(f), Err(Err::Error((f, ErrorKind::Tag))));

    let g2: &str = "ab12cd";
    assert_eq!(not_line_ending::<_, (_, ErrorKind)>(g2), Ok(("", g2)));
  }

  #[test]
  fn hex_digit_test() {
    let i = &b"0123456789abcdefABCDEF;"[..];
    assert_parse!(hex_digit1(i), Ok((&b";"[..], &i[..i.len() - 1])));

    let i = &b"g"[..];
    assert_parse!(
      hex_digit1(i),
      Err(Err::Error(error_position!(i, ErrorKind::HexDigit)))
    );

    let i = &b"G"[..];
    assert_parse!(
      hex_digit1(i),
      Err(Err::Error(error_position!(i, ErrorKind::HexDigit)))
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
      Err(Err::Error(error_position!(i, ErrorKind::OctDigit)))
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
      Err(Err::Error(error_position!(&b"\r"[..], ErrorKind::CrLf)))
    );
    assert_parse!(
      crlf(&b"\ra"[..]),
      Err(Err::Error(error_position!(&b"\ra"[..], ErrorKind::CrLf)))
    );

    assert_parse!(crlf("\r\na"), Ok(("a", "\r\n")));
    assert_parse!(
      crlf("\r"),
      Err(Err::Error(error_position!(&"\r"[..], ErrorKind::CrLf)))
    );
    assert_parse!(
      crlf("\ra"),
      Err(Err::Error(error_position!("\ra", ErrorKind::CrLf)))
    );
  }

  #[test]
  fn end_of_line() {
    assert_parse!(line_ending(&b"\na"[..]), Ok((&b"a"[..], &b"\n"[..])));
    assert_parse!(line_ending(&b"\r\na"[..]), Ok((&b"a"[..], &b"\r\n"[..])));
    assert_parse!(
      line_ending(&b"\r"[..]),
      Err(Err::Error(error_position!(&b"\r"[..], ErrorKind::CrLf)))
    );
    assert_parse!(
      line_ending(&b"\ra"[..]),
      Err(Err::Error(error_position!(&b"\ra"[..], ErrorKind::CrLf)))
    );

    assert_parse!(line_ending("\na"), Ok(("a", "\n")));
    assert_parse!(line_ending("\r\na"), Ok(("a", "\r\n")));
    assert_parse!(
      line_ending("\r"),
      Err(Err::Error(error_position!(&"\r"[..], ErrorKind::CrLf)))
    );
    assert_parse!(
      line_ending("\ra"),
      Err(Err::Error(error_position!("\ra", ErrorKind::CrLf)))
    );
  }

  fn digit_to_i16(input: &str) -> IResult<&str, i16> {
    let i = input;
    let (i, opt_sign) = opt(alt((char('+'), char('-'))))(i)?;
    let sign = match opt_sign {
      Some('+') => true,
      Some('-') => false,
      _ => true,
    };

    let (i, s) = match digit1::<_, crate::error::Error<_>>(i) {
      Ok((i, s)) => (i, s),
      Err(_) => {
        return Err(Err::Error(crate::error::Error::from_error_kind(
          input,
          ErrorKind::Digit,
        )))
      }
    };

    match s.parse_to() {
      Some(n) => {
        if sign {
          Ok((i, n))
        } else {
          Ok((i, -n))
        }
      }
      None => Err(Err::Error(crate::error::Error::from_error_kind(
        i,
        ErrorKind::Digit,
      ))),
    }
  }

  fn digit_to_u32(i: &str) -> IResult<&str, u32> {
    let (i, s) = digit1(i)?;
    match s.parse_to() {
      Some(n) => Ok((i, n)),
      None => Err(Err::Error(crate::error::Error::from_error_kind(
        i,
        ErrorKind::Digit,
      ))),
    }
  }

  proptest! {
    #[test]
    fn ints(s in "\\PC*") {
        let res1 = digit_to_i16(&s);
        let res2 = i16(s.as_str());
        assert_eq!(res1, res2);
    }

    #[test]
    fn uints(s in "\\PC*") {
        let res1 = digit_to_u32(&s);
        let res2 = u32(s.as_str());
        assert_eq!(res1, res2);
    }
  }
}
