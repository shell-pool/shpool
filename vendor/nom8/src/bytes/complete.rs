//! Parsers recognizing bytes streams, complete input version

#![allow(deprecated)]

use crate::error::ErrorKind;
use crate::error::ParseError;
use crate::input::{
  Compare, CompareResult, FindSubstring, FindToken, InputIter, InputLength, InputTake,
  InputTakeAtPosition, IntoOutput, Slice, ToUsize,
};
use crate::lib::std::ops::RangeFrom;
use crate::lib::std::result::Result::*;
use crate::IntoOutputIResult;
use crate::{Err, IResult, Parser};

pub(crate) fn any<I, E: ParseError<I>>(input: I) -> IResult<I, <I as InputIter>::Item, E>
where
  I: InputIter + InputLength + Slice<RangeFrom<usize>>,
{
  let mut it = input.iter_indices();
  match it.next() {
    None => Err(Err::Error(E::from_error_kind(input, ErrorKind::Eof))),
    Some((_, c)) => match it.next() {
      None => Ok((input.slice(input.input_len()..), c)),
      Some((idx, _)) => Ok((input.slice(idx..), c)),
    },
  }
}

/// Recognizes a pattern
///
/// The input data will be compared to the tag combinator's argument and will return the part of
/// the input that matches the argument
///
/// It will return `Err(Err::Error((_, ErrorKind::Tag)))` if the input doesn't match the pattern
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, &str> {
///   tag("Hello")(s)
/// }
///
/// assert_eq!(parser("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser("Something"), Err(Err::Error(Error::new("Something", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Tag))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::tag`][crate::bytes::tag]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::tag`")]
pub fn tag<T, Input, Error: ParseError<Input>>(
  tag: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + Compare<T>,
  Input: IntoOutput,
  T: InputLength + Clone,
{
  move |i: Input| tag_internal(i, tag.clone())
}

pub(crate) fn tag_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  t: T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + Compare<T>,
  Input: IntoOutput,
  T: InputLength,
{
  let tag_len = t.input_len();
  let res: IResult<_, _, Error> = match i.compare(t) {
    CompareResult::Ok => Ok(i.take_split(tag_len)),
    _ => {
      let e: ErrorKind = ErrorKind::Tag;
      Err(Err::Error(Error::from_error_kind(i, e)))
    }
  };
  res.into_output()
}

/// Recognizes a case insensitive pattern.
///
/// The input data will be compared to the tag combinator's argument and will return the part of
/// the input that matches the argument with no regard to case.
///
/// It will return `Err(Err::Error((_, ErrorKind::Tag)))` if the input doesn't match the pattern.
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::tag_no_case;
///
/// fn parser(s: &str) -> IResult<&str, &str> {
///   tag_no_case("hello")(s)
/// }
///
/// assert_eq!(parser("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser("hello, World!"), Ok((", World!", "hello")));
/// assert_eq!(parser("HeLlO, World!"), Ok((", World!", "HeLlO")));
/// assert_eq!(parser("Something"), Err(Err::Error(Error::new("Something", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Tag))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::tag_no_case`][crate::bytes::tag_no_case]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::tag_no_case`")]
pub fn tag_no_case<T, Input, Error: ParseError<Input>>(
  tag: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + Compare<T>,
  Input: IntoOutput,
  T: InputLength + Clone,
{
  move |i: Input| tag_no_case_internal(i, tag.clone())
}

pub(crate) fn tag_no_case_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  t: T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + Compare<T>,
  Input: IntoOutput,
  T: InputLength,
{
  let tag_len = t.input_len();

  let res: IResult<_, _, Error> = match (i).compare_no_case(t) {
    CompareResult::Ok => Ok(i.take_split(tag_len)),
    _ => {
      let e: ErrorKind = ErrorKind::Tag;
      Err(Err::Error(Error::from_error_kind(i, e)))
    }
  };
  res.into_output()
}

pub(crate) fn one_of_internal<I, T, E: ParseError<I>>(
  input: I,
  list: &T,
) -> IResult<I, <I as InputIter>::Item, E>
where
  I: Slice<RangeFrom<usize>> + InputIter + InputLength,
  <I as InputIter>::Item: Copy,
  T: FindToken<<I as InputIter>::Item>,
{
  let mut it = input.iter_indices();
  match it.next() {
    Some((_, c)) if list.find_token(c) => match it.next() {
      None => Ok((input.slice(input.input_len()..), c)),
      Some((idx, _)) => Ok((input.slice(idx..), c)),
    },
    Some(_) => Err(Err::Error(E::from_error_kind(input, ErrorKind::OneOf))),
    None => Err(Err::Error(E::from_error_kind(input, ErrorKind::OneOf))),
  }
}

pub(crate) fn none_of_internal<I, T, E: ParseError<I>>(
  input: I,
  list: &T,
) -> IResult<I, <I as InputIter>::Item, E>
where
  I: Slice<RangeFrom<usize>> + InputIter + InputLength,
  <I as InputIter>::Item: Copy,
  T: FindToken<<I as InputIter>::Item>,
{
  let mut it = input.iter_indices();
  match it.next() {
    Some((_, c)) if !list.find_token(c) => match it.next() {
      None => Ok((input.slice(input.input_len()..), c)),
      Some((idx, _)) => Ok((input.slice(idx..), c)),
    },
    Some(_) => Err(Err::Error(E::from_error_kind(input, ErrorKind::NoneOf))),
    None => Err(Err::Error(E::from_error_kind(input, ErrorKind::NoneOf))),
  }
}

/// Parse till certain characters are met.
///
/// The parser will return the longest slice till one of the characters of the combinator's argument are met.
///
/// It doesn't consume the matched character.
///
/// It will return a `Err::Error(("", ErrorKind::IsNot))` if the pattern wasn't met.
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::is_not;
///
/// fn not_space(s: &str) -> IResult<&str, &str> {
///   is_not(" \t\r\n")(s)
/// }
///
/// assert_eq!(not_space("Hello, World!"), Ok((" World!", "Hello,")));
/// assert_eq!(not_space("Sometimes\t"), Ok(("\t", "Sometimes")));
/// assert_eq!(not_space("Nospace"), Ok(("", "Nospace")));
/// assert_eq!(not_space(""), Err(Err::Error(Error::new("", ErrorKind::IsNot))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_till1`][crate::bytes::take_till1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_till1`")]
pub fn is_not<T, Input, Error: ParseError<Input>>(
  arr: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  move |i: Input| is_not_internal(i, &arr)
}

pub(crate) fn is_not_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  arr: &T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  let e: ErrorKind = ErrorKind::IsNot;
  i.split_at_position1_complete(|c| arr.find_token(c), e)
    .into_output()
}

/// Returns the longest slice of the matches the pattern.
///
/// The parser will return the longest slice consisting of the characters in provided in the
/// combinator's argument.
///
/// It will return a `Err(Err::Error((_, ErrorKind::IsA)))` if the pattern wasn't met.
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::is_a;
///
/// fn hex(s: &str) -> IResult<&str, &str> {
///   is_a("1234567890ABCDEF")(s)
/// }
///
/// assert_eq!(hex("123 and voila"), Ok((" and voila", "123")));
/// assert_eq!(hex("DEADBEEF and others"), Ok((" and others", "DEADBEEF")));
/// assert_eq!(hex("BADBABEsomething"), Ok(("something", "BADBABE")));
/// assert_eq!(hex("D15EA5E"), Ok(("", "D15EA5E")));
/// assert_eq!(hex(""), Err(Err::Error(Error::new("", ErrorKind::IsA))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_while1`][crate::bytes::take_while1`]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_while1`")]
pub fn is_a<T, Input, Error: ParseError<Input>>(
  arr: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  move |i: Input| is_a_internal(i, &arr)
}

pub(crate) fn is_a_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  arr: &T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  let e: ErrorKind = ErrorKind::IsA;
  i.split_at_position1_complete(|c| !arr.find_token(c), e)
    .into_output()
}

/// Returns the longest input slice (if any) that matches the predicate.
///
/// The parser will return the longest slice that matches the given predicate *(a function that
/// takes the input and returns a bool)*.
/// # Example
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed, IResult};
/// use nom8::bytes::complete::take_while;
/// use nom8::input::AsChar;
///
/// fn alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while(AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha(b"12345"), Ok((&b"12345"[..], &b""[..])));
/// assert_eq!(alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(alpha(b""), Ok((&b""[..], &b""[..])));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_while`][crate::bytes::take_while]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_while`")]
pub fn take_while<T, Input, Error: ParseError<Input>>(
  list: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  move |i: Input| take_while_internal(i, &list)
}

pub(crate) fn take_while_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  list: &T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  i.split_at_position_complete(|c| !list.find_token(c))
    .into_output()
}

/// Returns the longest (at least 1) input slice that matches the predicate.
///
/// The parser will return the longest slice that matches the given predicate *(a function that
/// takes the input and returns a bool)*.
///
/// It will return an `Err(Err::Error((_, ErrorKind::TakeWhile1)))` if the pattern wasn't met.
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::take_while1;
/// use nom8::input::AsChar;
///
/// fn alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while1(AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(alpha(b"12345"), Err(Err::Error(Error::new(&b"12345"[..], ErrorKind::TakeWhile1))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_while1`][crate::bytes::take_while1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_while1`")]
pub fn take_while1<T, Input, Error: ParseError<Input>>(
  list: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  move |i: Input| take_while1_internal(i, &list)
}

pub(crate) fn take_while1_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  list: &T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  let e: ErrorKind = ErrorKind::TakeWhile1;
  i.split_at_position1_complete(|c| !list.find_token(c), e)
    .into_output()
}

/// Returns the longest (m <= len <= n) input slice  that matches the predicate.
///
/// The parser will return the longest slice that matches the given predicate *(a function that
/// takes the input and returns a bool)*.
///
/// It will return an `Err::Error((_, ErrorKind::TakeWhileMN))` if the pattern wasn't met or is out
/// of range (m <= len <= n).
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::take_while_m_n;
/// use nom8::input::AsChar;
///
/// fn short_alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while_m_n(3, 6, AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(short_alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"lengthy"), Ok((&b"y"[..], &b"length"[..])));
/// assert_eq!(short_alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"ed"), Err(Err::Error(Error::new(&b"ed"[..], ErrorKind::TakeWhileMN))));
/// assert_eq!(short_alpha(b"12345"), Err(Err::Error(Error::new(&b"12345"[..], ErrorKind::TakeWhileMN))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_while_m_n`][crate::bytes::take_while_m_n]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_while_m_n`")]
pub fn take_while_m_n<T, Input, Error: ParseError<Input>>(
  m: usize,
  n: usize,
  list: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + InputIter + InputLength + Slice<RangeFrom<usize>>,
  Input: IntoOutput,
  T: FindToken<<Input as InputIter>::Item>,
{
  move |i: Input| take_while_m_n_internal(i, m, n, &list)
}

pub(crate) fn take_while_m_n_internal<T, Input, Error: ParseError<Input>>(
  input: Input,
  m: usize,
  n: usize,
  list: &T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + InputIter + InputLength + Slice<RangeFrom<usize>>,
  Input: IntoOutput,
  T: FindToken<<Input as InputIter>::Item>,
{
  match input.position(|c| !list.find_token(c)) {
    Some(idx) => {
      if idx >= m {
        if idx <= n {
          let res: IResult<_, _, Error> = if let Ok(index) = input.slice_index(idx) {
            Ok(input.take_split(index)).into_output()
          } else {
            Err(Err::Error(Error::from_error_kind(
              input,
              ErrorKind::TakeWhileMN,
            )))
          };
          res
        } else {
          let res: IResult<_, _, Error> = if let Ok(index) = input.slice_index(n) {
            Ok(input.take_split(index)).into_output()
          } else {
            Err(Err::Error(Error::from_error_kind(
              input,
              ErrorKind::TakeWhileMN,
            )))
          };
          res
        }
      } else {
        let e = ErrorKind::TakeWhileMN;
        Err(Err::Error(Error::from_error_kind(input, e)))
      }
    }
    None => {
      let len = input.input_len();
      if len >= n {
        match input.slice_index(n) {
          Ok(index) => Ok(input.take_split(index)).into_output(),
          Err(_needed) => Err(Err::Error(Error::from_error_kind(
            input,
            ErrorKind::TakeWhileMN,
          ))),
        }
      } else if len >= m && len <= n {
        let res: IResult<_, _, Error> = Ok((input.slice(len..), input));
        res.into_output()
      } else {
        let e = ErrorKind::TakeWhileMN;
        Err(Err::Error(Error::from_error_kind(input, e)))
      }
    }
  }
}

/// Returns the longest input slice (if any) till a predicate is met.
///
/// The parser will return the longest slice till the given predicate *(a function that
/// takes the input and returns a bool)*.
/// # Example
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed, IResult};
/// use nom8::bytes::complete::take_till;
///
/// fn till_colon(s: &str) -> IResult<&str, &str> {
///   take_till(|c| c == ':')(s)
/// }
///
/// assert_eq!(till_colon("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon(":empty matched"), Ok((":empty matched", ""))); //allowed
/// assert_eq!(till_colon("12345"), Ok(("", "12345")));
/// assert_eq!(till_colon(""), Ok(("", "")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_till`][crate::bytes::take_till]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_till`")]
pub fn take_till<T, Input, Error: ParseError<Input>>(
  list: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  move |i: Input| take_till_internal(i, &list)
}

pub(crate) fn take_till_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  list: &T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  i.split_at_position_complete(|c| list.find_token(c))
    .into_output()
}

/// Returns the longest (at least 1) input slice till a predicate is met.
///
/// The parser will return the longest slice till the given predicate *(a function that
/// takes the input and returns a bool)*.
///
/// It will return `Err(Err::Error((_, ErrorKind::TakeTill1)))` if the input is empty or the
/// predicate matches the first input.
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::take_till1;
///
/// fn till_colon(s: &str) -> IResult<&str, &str> {
///   take_till1(|c| c == ':')(s)
/// }
///
/// assert_eq!(till_colon("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon(":empty matched"), Err(Err::Error(Error::new(":empty matched", ErrorKind::TakeTill1))));
/// assert_eq!(till_colon("12345"), Ok(("", "12345")));
/// assert_eq!(till_colon(""), Err(Err::Error(Error::new("", ErrorKind::TakeTill1))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_till1`][crate::bytes::take_till1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_till1`")]
pub fn take_till1<T, Input, Error: ParseError<Input>>(
  list: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  move |i: Input| take_till1_internal(i, &list)
}

pub(crate) fn take_till1_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  list: &T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTakeAtPosition,
  Input: IntoOutput,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  let e: ErrorKind = ErrorKind::TakeTill1;
  i.split_at_position1_complete(|c| list.find_token(c), e)
    .into_output()
}

/// Returns an input slice containing the first N input elements (Input[..N]).
///
/// It will return `Err(Err::Error((_, ErrorKind::Eof)))` if the input is shorter than the argument.
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::take;
///
/// fn take6(s: &str) -> IResult<&str, &str> {
///   take(6usize)(s)
/// }
///
/// assert_eq!(take6("1234567"), Ok(("7", "123456")));
/// assert_eq!(take6("things"), Ok(("", "things")));
/// assert_eq!(take6("short"), Err(Err::Error(Error::new("short", ErrorKind::Eof))));
/// assert_eq!(take6(""), Err(Err::Error(Error::new("", ErrorKind::Eof))));
/// ```
///
/// The units that are taken will depend on the input type. For example, for a
/// `&str` it will take a number of `char`'s, whereas for a `&[u8]` it will
/// take that many `u8`'s:
///
/// ```rust
/// use nom8::error::Error;
/// use nom8::bytes::complete::take;
///
/// assert_eq!(take::<_, _, Error<_>>(1usize)("💙"), Ok(("", "💙")));
/// assert_eq!(take::<_, _, Error<_>>(1usize)("💙".as_bytes()), Ok((b"\x9F\x92\x99".as_ref(), b"\xF0".as_ref())));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take`][crate::bytes::take]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take`")]
pub fn take<C, Input, Error: ParseError<Input>>(
  count: C,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputIter + InputTake,
  Input: IntoOutput,
  C: ToUsize,
{
  let c = count.to_usize();
  move |i: Input| take_internal(i, c)
}

pub(crate) fn take_internal<Input, Error: ParseError<Input>>(
  i: Input,
  c: usize,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputIter + InputTake,
  Input: IntoOutput,
{
  match i.slice_index(c) {
    Err(_needed) => Err(Err::Error(Error::from_error_kind(i, ErrorKind::Eof))),
    Ok(index) => Ok(i.take_split(index)).into_output(),
  }
}

/// Returns the input slice up to the first occurrence of the pattern.
///
/// It doesn't consume the pattern. It will return `Err(Err::Error((_, ErrorKind::TakeUntil)))`
/// if the pattern wasn't met.
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::take_until;
///
/// fn until_eof(s: &str) -> IResult<&str, &str> {
///   take_until("eof")(s)
/// }
///
/// assert_eq!(until_eof("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert_eq!(until_eof("hello, world"), Err(Err::Error(Error::new("hello, world", ErrorKind::TakeUntil))));
/// assert_eq!(until_eof(""), Err(Err::Error(Error::new("", ErrorKind::TakeUntil))));
/// assert_eq!(until_eof("1eof2eof"), Ok(("eof2eof", "1")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_until`][crate::bytes::take_until]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_until`")]
pub fn take_until<T, Input, Error: ParseError<Input>>(
  tag: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + FindSubstring<T>,
  Input: IntoOutput,
  T: InputLength + Clone,
{
  move |i: Input| take_until_internal(i, tag.clone())
}

pub(crate) fn take_until_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  t: T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + FindSubstring<T>,
  Input: IntoOutput,
  T: InputLength,
{
  let res: IResult<_, _, Error> = match i.find_substring(t) {
    None => Err(Err::Error(Error::from_error_kind(i, ErrorKind::TakeUntil))),
    Some(index) => Ok(i.take_split(index)),
  };
  res.into_output()
}

/// Returns the non empty input slice up to the first occurrence of the pattern.
///
/// It doesn't consume the pattern. It will return `Err(Err::Error((_, ErrorKind::TakeUntil)))`
/// if the pattern wasn't met.
/// # Example
/// ```rust
/// # use nom8::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom8::bytes::complete::take_until1;
///
/// fn until_eof(s: &str) -> IResult<&str, &str> {
///   take_until1("eof")(s)
/// }
///
/// assert_eq!(until_eof("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert_eq!(until_eof("hello, world"), Err(Err::Error(Error::new("hello, world", ErrorKind::TakeUntil))));
/// assert_eq!(until_eof(""), Err(Err::Error(Error::new("", ErrorKind::TakeUntil))));
/// assert_eq!(until_eof("1eof2eof"), Ok(("eof2eof", "1")));
/// assert_eq!(until_eof("eof"), Err(Err::Error(Error::new("eof", ErrorKind::TakeUntil))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::take_until1`][crate::bytes::take_until1]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::take_until1`")]
pub fn take_until1<T, Input, Error: ParseError<Input>>(
  tag: T,
) -> impl Fn(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + FindSubstring<T>,
  Input: IntoOutput,
  T: InputLength + Clone,
{
  move |i: Input| take_until1_internal(i, tag.clone())
}

pub(crate) fn take_until1_internal<T, Input, Error: ParseError<Input>>(
  i: Input,
  t: T,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: InputTake + FindSubstring<T>,
  Input: IntoOutput,
  T: InputLength,
{
  let res: IResult<_, _, Error> = match i.find_substring(t) {
    None => Err(Err::Error(Error::from_error_kind(i, ErrorKind::TakeUntil))),
    Some(0) => Err(Err::Error(Error::from_error_kind(i, ErrorKind::TakeUntil))),
    Some(index) => Ok(i.take_split(index)),
  };
  res.into_output()
}

/// Matches a byte string with escaped characters.
///
/// * The first argument matches the normal characters (it must not accept the control character)
/// * The second argument is the control character (like `\` in most languages)
/// * The third argument matches the escaped characters
/// # Example
/// ```
/// # use nom8::{Err, error::ErrorKind, Needed, IResult};
/// # use nom8::character::complete::digit1;
/// use nom8::bytes::complete::escaped;
/// use nom8::character::complete::one_of;
///
/// fn esc(s: &str) -> IResult<&str, &str> {
///   escaped(digit1, '\\', one_of(r#""n\"#))(s)
/// }
///
/// assert_eq!(esc("123;"), Ok((";", "123")));
/// assert_eq!(esc(r#"12\"34;"#), Ok((";", r#"12\"34"#)));
/// ```
///
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::escaped`][crate::bytes::escaped]
#[deprecated(since = "8.0.0", note = "Replaced with `nom8::bytes::escaped`")]
pub fn escaped<'a, Input: 'a, Error, F, G, O1, O2>(
  mut normal: F,
  control_char: char,
  mut escapable: G,
) -> impl FnMut(Input) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: Clone
    + crate::input::Offset
    + InputLength
    + InputTake
    + InputTakeAtPosition
    + Slice<RangeFrom<usize>>
    + InputIter,
  Input: IntoOutput,
  <Input as InputIter>::Item: crate::input::AsChar,
  F: Parser<Input, O1, Error>,
  G: Parser<Input, O2, Error>,
  Error: ParseError<Input>,
{
  move |input: Input| escaped_internal(input, &mut normal, control_char, &mut escapable)
}

pub(crate) fn escaped_internal<'a, Input: 'a, Error, F, G, O1, O2>(
  input: Input,
  normal: &mut F,
  control_char: char,
  escapable: &mut G,
) -> IResult<Input, <Input as IntoOutput>::Output, Error>
where
  Input: Clone
    + crate::input::Offset
    + InputLength
    + InputTake
    + InputTakeAtPosition
    + Slice<RangeFrom<usize>>
    + InputIter,
  Input: IntoOutput,
  <Input as InputIter>::Item: crate::input::AsChar,
  F: Parser<Input, O1, Error>,
  G: Parser<Input, O2, Error>,
  Error: ParseError<Input>,
{
  use crate::input::AsChar;

  let mut i = input.clone();

  while i.input_len() > 0 {
    let current_len = i.input_len();

    match normal.parse(i.clone()) {
      Ok((i2, _)) => {
        // return if we consumed everything or if the normal parser
        // does not consume anything
        if i2.input_len() == 0 {
          return Ok((input.slice(input.input_len()..), input)).into_output();
        } else if i2.input_len() == current_len {
          let index = input.offset(&i2);
          return Ok(input.take_split(index)).into_output();
        } else {
          i = i2;
        }
      }
      Err(Err::Error(_)) => {
        // unwrap() should be safe here since index < $i.input_len()
        if i.iter_elements().next().unwrap().as_char() == control_char {
          let next = control_char.len_utf8();
          if next >= i.input_len() {
            return Err(Err::Error(Error::from_error_kind(
              input,
              ErrorKind::Escaped,
            )));
          } else {
            match escapable.parse(i.slice(next..)) {
              Ok((i2, _)) => {
                if i2.input_len() == 0 {
                  return Ok((input.slice(input.input_len()..), input)).into_output();
                } else {
                  i = i2;
                }
              }
              Err(e) => return Err(e),
            }
          }
        } else {
          let index = input.offset(&i);
          if index == 0 {
            return Err(Err::Error(Error::from_error_kind(
              input,
              ErrorKind::Escaped,
            )));
          }
          return Ok(input.take_split(index)).into_output();
        }
      }
      Err(e) => {
        return Err(e);
      }
    }
  }

  Ok((input.slice(input.input_len()..), input)).into_output()
}

/// Matches a byte string with escaped characters.
///
/// * The first argument matches the normal characters (it must not match the control character)
/// * The second argument is the control character (like `\` in most languages)
/// * The third argument matches the escaped characters and transforms them
///
/// As an example, the chain `abc\tdef` could be `abc    def` (it also consumes the control character)
///
/// ```
/// # use nom8::{Err, error::ErrorKind, Needed, IResult};
/// # use std::str::from_utf8;
/// use nom8::bytes::complete::{escaped_transform, tag};
/// use nom8::character::complete::alpha1;
/// use nom8::branch::alt;
/// use nom8::combinator::value;
///
/// fn parser(input: &str) -> IResult<&str, String> {
///   escaped_transform(
///     alpha1,
///     '\\',
///     alt((
///       value("\\", tag("\\")),
///       value("\"", tag("\"")),
///       value("\n", tag("n")),
///     ))
///   )(input)
/// }
///
/// assert_eq!(parser("ab\\\"cd"), Ok(("", String::from("ab\"cd"))));
/// assert_eq!(parser("ab\\ncd"), Ok(("", String::from("ab\ncd"))));
/// ```
#[cfg(feature = "alloc")]
///
/// **WARNING:** Deprecated, replaced with [`nom8::bytes::escaped_transform`][crate::bytes::escaped_transform]
#[deprecated(
  since = "8.0.0",
  note = "Replaced with `nom8::bytes::escaped_transform`"
)]
pub fn escaped_transform<Input, Error, F, G, O1, O2, ExtendItem, Output>(
  mut normal: F,
  control_char: char,
  mut transform: G,
) -> impl FnMut(Input) -> IResult<Input, Output, Error>
where
  Input: Clone
    + crate::input::Offset
    + InputLength
    + InputTake
    + InputTakeAtPosition
    + Slice<RangeFrom<usize>>
    + InputIter,
  Input: crate::input::ExtendInto<Item = ExtendItem, Extender = Output>,
  O1: crate::input::ExtendInto<Item = ExtendItem, Extender = Output>,
  O2: crate::input::ExtendInto<Item = ExtendItem, Extender = Output>,
  <Input as InputIter>::Item: crate::input::AsChar,
  F: Parser<Input, O1, Error>,
  G: Parser<Input, O2, Error>,
  Error: ParseError<Input>,
{
  move |input: Input| escaped_transform_internal(input, &mut normal, control_char, &mut transform)
}

#[cfg(feature = "alloc")]
pub(crate) fn escaped_transform_internal<Input, Error, F, G, O1, O2, ExtendItem, Output>(
  input: Input,
  normal: &mut F,
  control_char: char,
  transform: &mut G,
) -> IResult<Input, Output, Error>
where
  Input: Clone
    + crate::input::Offset
    + InputLength
    + InputTake
    + InputTakeAtPosition
    + Slice<RangeFrom<usize>>
    + InputIter,
  Input: crate::input::ExtendInto<Item = ExtendItem, Extender = Output>,
  O1: crate::input::ExtendInto<Item = ExtendItem, Extender = Output>,
  O2: crate::input::ExtendInto<Item = ExtendItem, Extender = Output>,
  <Input as InputIter>::Item: crate::input::AsChar,
  F: Parser<Input, O1, Error>,
  G: Parser<Input, O2, Error>,
  Error: ParseError<Input>,
{
  use crate::input::AsChar;

  let mut index = 0;
  let mut res = input.new_builder();

  let i = input.clone();

  while index < i.input_len() {
    let current_len = i.input_len();
    let remainder = i.slice(index..);
    match normal.parse(remainder.clone()) {
      Ok((i2, o)) => {
        o.extend_into(&mut res);
        if i2.input_len() == 0 {
          return Ok((i.slice(i.input_len()..), res));
        } else if i2.input_len() == current_len {
          return Ok((remainder, res));
        } else {
          index = input.offset(&i2);
        }
      }
      Err(Err::Error(_)) => {
        // unwrap() should be safe here since index < $i.input_len()
        if remainder.iter_elements().next().unwrap().as_char() == control_char {
          let next = index + control_char.len_utf8();
          let input_len = input.input_len();

          if next >= input_len {
            return Err(Err::Error(Error::from_error_kind(
              remainder,
              ErrorKind::EscapedTransform,
            )));
          } else {
            match transform.parse(i.slice(next..)) {
              Ok((i2, o)) => {
                o.extend_into(&mut res);
                if i2.input_len() == 0 {
                  return Ok((i.slice(i.input_len()..), res));
                } else {
                  index = input.offset(&i2);
                }
              }
              Err(e) => return Err(e),
            }
          }
        } else {
          if index == 0 {
            return Err(Err::Error(Error::from_error_kind(
              remainder,
              ErrorKind::EscapedTransform,
            )));
          }
          return Ok((remainder, res));
        }
      }
      Err(e) => return Err(e),
    }
  }
  Ok((input.slice(index..), res))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::character::complete::{alpha1 as alpha, digit1 as digit};
  #[cfg(feature = "alloc")]
  use crate::{
    branch::alt,
    combinator::{map, value},
    lib::std::string::String,
    lib::std::vec::Vec,
  };

  #[test]
  fn complete_take_while_m_n_utf8_all_matching() {
    let result: IResult<&str, &str> =
      super::take_while_m_n(1, 4, |c: char| c.is_alphabetic())("øn");
    assert_eq!(result, Ok(("", "øn")));
  }

  #[test]
  fn complete_take_while_m_n_utf8_all_matching_substring() {
    let result: IResult<&str, &str> =
      super::take_while_m_n(1, 1, |c: char| c.is_alphabetic())("øn");
    assert_eq!(result, Ok(("n", "ø")));
  }

  // issue #1336 "escaped hangs if normal parser accepts empty"
  fn escaped_string(input: &str) -> IResult<&str, &str> {
    use crate::character::complete::{alpha0, one_of};
    escaped(alpha0, '\\', one_of("n"))(input)
  }

  // issue #1336 "escaped hangs if normal parser accepts empty"
  #[test]
  fn escaped_hang() {
    escaped_string("7").unwrap();
    escaped_string("a7").unwrap();
  }

  // issue ##1118 escaped does not work with empty string
  fn unquote<'a>(input: &'a str) -> IResult<&'a str, &'a str> {
    use crate::bytes::complete::*;
    use crate::character::complete::*;
    use crate::combinator::opt;
    use crate::sequence::delimited;

    delimited(
      char('"'),
      escaped(opt(none_of(r#"\""#)), '\\', one_of(r#"\"rnt"#)),
      char('"'),
    )(input)
  }

  #[test]
  fn escaped_hang_1118() {
    assert_eq!(unquote(r#""""#), Ok(("", "")));
  }

  #[cfg(feature = "alloc")]
  #[allow(unused_variables)]
  #[test]
  fn escaping() {
    use crate::character::complete::one_of;

    fn esc(i: &[u8]) -> IResult<&[u8], &[u8]> {
      escaped(alpha, '\\', one_of("\"n\\"))(i)
    }
    assert_eq!(esc(&b"abcd;"[..]), Ok((&b";"[..], &b"abcd"[..])));
    assert_eq!(esc(&b"ab\\\"cd;"[..]), Ok((&b";"[..], &b"ab\\\"cd"[..])));
    assert_eq!(esc(&b"\\\"abcd;"[..]), Ok((&b";"[..], &b"\\\"abcd"[..])));
    assert_eq!(esc(&b"\\n;"[..]), Ok((&b";"[..], &b"\\n"[..])));
    assert_eq!(esc(&b"ab\\\"12"[..]), Ok((&b"12"[..], &b"ab\\\""[..])));
    assert_eq!(
      esc(&b"AB\\"[..]),
      Err(Err::Error(error_position!(
        &b"AB\\"[..],
        ErrorKind::Escaped
      )))
    );
    assert_eq!(
      esc(&b"AB\\A"[..]),
      Err(Err::Error(error_node_position!(
        &b"AB\\A"[..],
        ErrorKind::Escaped,
        error_position!(&b"A"[..], ErrorKind::OneOf)
      )))
    );

    fn esc2(i: &[u8]) -> IResult<&[u8], &[u8]> {
      escaped(digit, '\\', one_of("\"n\\"))(i)
    }
    assert_eq!(esc2(&b"12\\nnn34"[..]), Ok((&b"nn34"[..], &b"12\\n"[..])));
  }

  #[cfg(feature = "alloc")]
  #[test]
  fn escaping_str() {
    use crate::character::complete::one_of;

    fn esc(i: &str) -> IResult<&str, &str> {
      escaped(alpha, '\\', one_of("\"n\\"))(i)
    }
    assert_eq!(esc("abcd;"), Ok((";", "abcd")));
    assert_eq!(esc("ab\\\"cd;"), Ok((";", "ab\\\"cd")));
    assert_eq!(esc("\\\"abcd;"), Ok((";", "\\\"abcd")));
    assert_eq!(esc("\\n;"), Ok((";", "\\n")));
    assert_eq!(esc("ab\\\"12"), Ok(("12", "ab\\\"")));
    assert_eq!(
      esc("AB\\"),
      Err(Err::Error(error_position!("AB\\", ErrorKind::Escaped)))
    );
    assert_eq!(
      esc("AB\\A"),
      Err(Err::Error(error_node_position!(
        "AB\\A",
        ErrorKind::Escaped,
        error_position!("A", ErrorKind::OneOf)
      )))
    );

    fn esc2(i: &str) -> IResult<&str, &str> {
      escaped(digit, '\\', one_of("\"n\\"))(i)
    }
    assert_eq!(esc2("12\\nnn34"), Ok(("nn34", "12\\n")));

    fn esc3(i: &str) -> IResult<&str, &str> {
      escaped(alpha, '\u{241b}', one_of("\"n"))(i)
    }
    assert_eq!(esc3("ab␛ncd;"), Ok((";", "ab␛ncd")));
  }

  #[cfg(feature = "alloc")]
  fn to_s(i: Vec<u8>) -> String {
    String::from_utf8_lossy(&i).into_owned()
  }

  #[cfg(feature = "alloc")]
  #[test]
  fn escape_transform() {
    fn esc(i: &[u8]) -> IResult<&[u8], String> {
      map(
        escaped_transform(
          alpha,
          '\\',
          alt((
            value(&b"\\"[..], tag("\\")),
            value(&b"\""[..], tag("\"")),
            value(&b"\n"[..], tag("n")),
          )),
        ),
        to_s,
      )(i)
    }

    assert_eq!(esc(&b"abcd;"[..]), Ok((&b";"[..], String::from("abcd"))));
    assert_eq!(
      esc(&b"ab\\\"cd;"[..]),
      Ok((&b";"[..], String::from("ab\"cd")))
    );
    assert_eq!(
      esc(&b"\\\"abcd;"[..]),
      Ok((&b";"[..], String::from("\"abcd")))
    );
    assert_eq!(esc(&b"\\n;"[..]), Ok((&b";"[..], String::from("\n"))));
    assert_eq!(
      esc(&b"ab\\\"12"[..]),
      Ok((&b"12"[..], String::from("ab\"")))
    );
    assert_eq!(
      esc(&b"AB\\"[..]),
      Err(Err::Error(error_position!(
        &b"\\"[..],
        ErrorKind::EscapedTransform
      )))
    );
    assert_eq!(
      esc(&b"AB\\A"[..]),
      Err(Err::Error(error_node_position!(
        &b"AB\\A"[..],
        ErrorKind::EscapedTransform,
        error_position!(&b"A"[..], ErrorKind::Tag)
      )))
    );

    fn esc2(i: &[u8]) -> IResult<&[u8], String> {
      map(
        escaped_transform(
          alpha,
          '&',
          alt((
            value("è".as_bytes(), tag("egrave;")),
            value("à".as_bytes(), tag("agrave;")),
          )),
        ),
        to_s,
      )(i)
    }
    assert_eq!(
      esc2(&b"ab&egrave;DEF;"[..]),
      Ok((&b";"[..], String::from("abèDEF")))
    );
    assert_eq!(
      esc2(&b"ab&egrave;D&agrave;EF;"[..]),
      Ok((&b";"[..], String::from("abèDàEF")))
    );
  }

  #[cfg(feature = "std")]
  #[test]
  fn escape_transform_str() {
    fn esc(i: &str) -> IResult<&str, String> {
      escaped_transform(
        alpha,
        '\\',
        alt((
          value("\\", tag("\\")),
          value("\"", tag("\"")),
          value("\n", tag("n")),
        )),
      )(i)
    }

    assert_eq!(esc("abcd;"), Ok((";", String::from("abcd"))));
    assert_eq!(esc("ab\\\"cd;"), Ok((";", String::from("ab\"cd"))));
    assert_eq!(esc("\\\"abcd;"), Ok((";", String::from("\"abcd"))));
    assert_eq!(esc("\\n;"), Ok((";", String::from("\n"))));
    assert_eq!(esc("ab\\\"12"), Ok(("12", String::from("ab\""))));
    assert_eq!(
      esc("AB\\"),
      Err(Err::Error(error_position!(
        "\\",
        ErrorKind::EscapedTransform
      )))
    );
    assert_eq!(
      esc("AB\\A"),
      Err(Err::Error(error_node_position!(
        "AB\\A",
        ErrorKind::EscapedTransform,
        error_position!("A", ErrorKind::Tag)
      )))
    );

    fn esc2(i: &str) -> IResult<&str, String> {
      escaped_transform(
        alpha,
        '&',
        alt((value("è", tag("egrave;")), value("à", tag("agrave;")))),
      )(i)
    }
    assert_eq!(esc2("ab&egrave;DEF;"), Ok((";", String::from("abèDEF"))));
    assert_eq!(
      esc2("ab&egrave;D&agrave;EF;"),
      Ok((";", String::from("abèDàEF")))
    );

    fn esc3(i: &str) -> IResult<&str, String> {
      escaped_transform(
        alpha,
        '␛',
        alt((value("\0", tag("0")), value("\n", tag("n")))),
      )(i)
    }
    assert_eq!(esc3("a␛0bc␛n"), Ok(("", String::from("a\0bc\n"))));
  }
}
