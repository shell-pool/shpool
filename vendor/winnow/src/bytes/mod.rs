//! Parsers recognizing bytes streams

#[cfg(test)]
mod tests;

use crate::error::ErrMode;
use crate::error::ErrorKind;
use crate::error::Needed;
use crate::error::ParseError;
use crate::lib::std::result::Result::Ok;
use crate::stream::{
    split_at_offset1_complete, split_at_offset1_partial, split_at_offset_complete,
    split_at_offset_partial, Compare, CompareResult, ContainsToken, FindSlice, SliceLen, Stream,
};
use crate::stream::{StreamIsPartial, ToUsize};
use crate::trace::trace;
use crate::IResult;
use crate::Parser;

/// Matches one token
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```rust
/// # use winnow::{bytes::any, error::ErrMode, error::{Error, ErrorKind}};
/// # use winnow::prelude::*;
/// fn parser(input: &str) -> IResult<&str, char> {
///     any.parse_next(input)
/// }
///
/// assert_eq!(parser("abc"), Ok(("bc",'a')));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Token))));
/// ```
///
/// ```rust
/// # use winnow::{bytes::any, error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// assert_eq!(any::<_, Error<_>>.parse_next(Partial::new("abc")), Ok((Partial::new("bc"),'a')));
/// assert_eq!(any::<_, Error<_>>.parse_next(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
#[doc(alias = "token")]
pub fn any<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Token, E>
where
    I: StreamIsPartial,
    I: Stream,
{
    trace("any", move |input: I| {
        if input.is_partial() {
            streaming_any(input)
        } else {
            complete_any(input)
        }
    })
    .parse_next(input)
}

pub(crate) fn streaming_any<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Token, E>
where
    I: Stream,
{
    input
        .next_token()
        .ok_or_else(|| ErrMode::Incomplete(Needed::new(1)))
}

pub(crate) fn complete_any<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Token, E>
where
    I: Stream,
{
    input
        .next_token()
        .ok_or_else(|| ErrMode::from_error_kind(input, ErrorKind::Token))
}

/// Recognizes a literal
///
/// The input data will be compared to the tag combinator's argument and will return the part of
/// the input that matches the argument
///
/// It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::Tag)))` if the input doesn't match the pattern
///
/// **Note:** [`Parser`][crate::Parser] is implemented for strings and byte strings as a convenience (complete
/// only)
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, &str> {
///   "Hello".parse_next(s)
/// }
///
/// assert_eq!(parser("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser("Something"), Err(ErrMode::Backtrack(Error::new("Something", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::Partial;
/// use winnow::bytes::tag;
///
/// fn parser(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   "Hello".parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new("Hello, World!")), Ok((Partial::new(", World!"), "Hello")));
/// assert_eq!(parser(Partial::new("Something")), Err(ErrMode::Backtrack(Error::new(Partial::new("Something"), ErrorKind::Tag))));
/// assert_eq!(parser(Partial::new("S")), Err(ErrMode::Backtrack(Error::new(Partial::new("S"), ErrorKind::Tag))));
/// assert_eq!(parser(Partial::new("H")), Err(ErrMode::Incomplete(Needed::new(4))));
/// ```
#[inline(always)]
#[doc(alias = "literal")]
#[doc(alias = "bytes")]
#[doc(alias = "just")]
pub fn tag<T, I, Error: ParseError<I>>(tag: T) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + Compare<T>,
    T: SliceLen + Clone,
{
    trace("tag", move |i: I| {
        let t = tag.clone();
        if i.is_partial() {
            streaming_tag_internal(i, t)
        } else {
            complete_tag_internal(i, t)
        }
    })
}

pub(crate) fn streaming_tag_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + Compare<T>,
    T: SliceLen,
{
    let tag_len = t.slice_len();
    match i.compare(t) {
        CompareResult::Ok => Ok(i.next_slice(tag_len)),
        CompareResult::Incomplete => {
            Err(ErrMode::Incomplete(Needed::new(tag_len - i.eof_offset())))
        }
        CompareResult::Error => {
            let e: ErrorKind = ErrorKind::Tag;
            Err(ErrMode::from_error_kind(i, e))
        }
    }
}

pub(crate) fn complete_tag_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + Compare<T>,
    T: SliceLen,
{
    let tag_len = t.slice_len();
    match i.compare(t) {
        CompareResult::Ok => Ok(i.next_slice(tag_len)),
        CompareResult::Incomplete | CompareResult::Error => {
            let e: ErrorKind = ErrorKind::Tag;
            Err(ErrMode::from_error_kind(i, e))
        }
    }
}

/// Recognizes a case insensitive literal.
///
/// The input data will be compared to the tag combinator's argument and will return the part of
/// the input that matches the argument with no regard to case.
///
/// It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::Tag)))` if the input doesn't match the pattern.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::tag_no_case;
///
/// fn parser(s: &str) -> IResult<&str, &str> {
///   tag_no_case("hello").parse_next(s)
/// }
///
/// assert_eq!(parser("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser("hello, World!"), Ok((", World!", "hello")));
/// assert_eq!(parser("HeLlO, World!"), Ok((", World!", "HeLlO")));
/// assert_eq!(parser("Something"), Err(ErrMode::Backtrack(Error::new("Something", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::bytes::tag_no_case;
///
/// fn parser(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   tag_no_case("hello").parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new("Hello, World!")), Ok((Partial::new(", World!"), "Hello")));
/// assert_eq!(parser(Partial::new("hello, World!")), Ok((Partial::new(", World!"), "hello")));
/// assert_eq!(parser(Partial::new("HeLlO, World!")), Ok((Partial::new(", World!"), "HeLlO")));
/// assert_eq!(parser(Partial::new("Something")), Err(ErrMode::Backtrack(Error::new(Partial::new("Something"), ErrorKind::Tag))));
/// assert_eq!(parser(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(5))));
/// ```
#[inline(always)]
#[doc(alias = "literal")]
#[doc(alias = "bytes")]
#[doc(alias = "just")]
pub fn tag_no_case<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + Compare<T>,
    T: SliceLen + Clone,
{
    trace("tag_no_case", move |i: I| {
        let t = tag.clone();
        if i.is_partial() {
            streaming_tag_no_case_internal(i, t)
        } else {
            complete_tag_no_case_internal(i, t)
        }
    })
}

pub(crate) fn streaming_tag_no_case_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + Compare<T>,
    T: SliceLen,
{
    let tag_len = t.slice_len();

    match (i).compare_no_case(t) {
        CompareResult::Ok => Ok(i.next_slice(tag_len)),
        CompareResult::Incomplete => {
            Err(ErrMode::Incomplete(Needed::new(tag_len - i.eof_offset())))
        }
        CompareResult::Error => {
            let e: ErrorKind = ErrorKind::Tag;
            Err(ErrMode::from_error_kind(i, e))
        }
    }
}

pub(crate) fn complete_tag_no_case_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + Compare<T>,
    T: SliceLen,
{
    let tag_len = t.slice_len();

    match (i).compare_no_case(t) {
        CompareResult::Ok => Ok(i.next_slice(tag_len)),
        CompareResult::Incomplete | CompareResult::Error => {
            let e: ErrorKind = ErrorKind::Tag;
            Err(ErrMode::from_error_kind(i, e))
        }
    }
}

/// Recognize a token that matches the [pattern][ContainsToken]
///
/// **Note:** [`Parser`][crate::Parser] is implemented as a convenience (complete
/// only) for
/// - `u8`
/// - `char`
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::bytes::one_of;
/// assert_eq!(one_of::<_, _, Error<_>>("abc").parse_next("b"), Ok(("", 'b')));
/// assert_eq!(one_of::<_, _, Error<_>>("a").parse_next("bc"), Err(ErrMode::Backtrack(Error::new("bc", ErrorKind::Verify))));
/// assert_eq!(one_of::<_, _, Error<_>>("a").parse_next(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Token))));
///
/// fn parser_fn(i: &str) -> IResult<&str, char> {
///     one_of(|c| c == 'a' || c == 'b').parse_next(i)
/// }
/// assert_eq!(parser_fn("abc"), Ok(("bc", 'a')));
/// assert_eq!(parser_fn("cd"), Err(ErrMode::Backtrack(Error::new("cd", ErrorKind::Verify))));
/// assert_eq!(parser_fn(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Token))));
/// ```
///
/// ```
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// # use winnow::bytes::one_of;
/// assert_eq!(one_of::<_, _, Error<_>>("abc").parse_next(Partial::new("b")), Ok((Partial::new(""), 'b')));
/// assert_eq!(one_of::<_, _, Error<_>>("a").parse_next(Partial::new("bc")), Err(ErrMode::Backtrack(Error::new(Partial::new("bc"), ErrorKind::Verify))));
/// assert_eq!(one_of::<_, _, Error<_>>("a").parse_next(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// fn parser_fn(i: Partial<&str>) -> IResult<Partial<&str>, char> {
///     one_of(|c| c == 'a' || c == 'b').parse_next(i)
/// }
/// assert_eq!(parser_fn(Partial::new("abc")), Ok((Partial::new("bc"), 'a')));
/// assert_eq!(parser_fn(Partial::new("cd")), Err(ErrMode::Backtrack(Error::new(Partial::new("cd"), ErrorKind::Verify))));
/// assert_eq!(parser_fn(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
#[doc(alias = "char")]
#[doc(alias = "token")]
#[doc(alias = "satisfy")]
pub fn one_of<I, T, Error: ParseError<I>>(list: T) -> impl Parser<I, <I as Stream>::Token, Error>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: Copy,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace(
        "one_of",
        any.verify(move |t: &<I as Stream>::Token| list.contains_token(*t)),
    )
}

/// Recognize a token that does not match the [pattern][ContainsToken]
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::prelude::*;
/// # use winnow::bytes::none_of;
/// assert_eq!(none_of::<_, _, Error<_>>("abc").parse_next("z"), Ok(("", 'z')));
/// assert_eq!(none_of::<_, _, Error<_>>("ab").parse_next("a"), Err(ErrMode::Backtrack(Error::new("a", ErrorKind::Verify))));
/// assert_eq!(none_of::<_, _, Error<_>>("a").parse_next(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Token))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// # use winnow::bytes::none_of;
/// assert_eq!(none_of::<_, _, Error<_>>("abc").parse_next(Partial::new("z")), Ok((Partial::new(""), 'z')));
/// assert_eq!(none_of::<_, _, Error<_>>("ab").parse_next(Partial::new("a")), Err(ErrMode::Backtrack(Error::new(Partial::new("a"), ErrorKind::Verify))));
/// assert_eq!(none_of::<_, _, Error<_>>("a").parse_next(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn none_of<I, T, Error: ParseError<I>>(list: T) -> impl Parser<I, <I as Stream>::Token, Error>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: Copy,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace(
        "none_of",
        any.verify(move |t: &<I as Stream>::Token| !list.contains_token(*t)),
    )
}

/// Recognize the longest input slice (if any) that matches the [pattern][ContainsToken]
///
/// *Partial version*: will return a `ErrMode::Incomplete(Needed::new(1))` if the pattern reaches the end of the input.
///
/// To recognize a series of tokens, use [`many0`][crate::multi::many0] to [`Accumulate`][crate::stream::Accumulate] into a `()` and then [`Parser::recognize`][crate::Parser::recognize].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::take_while0;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while0(AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha(b"12345"), Ok((&b"12345"[..], &b""[..])));
/// assert_eq!(alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(alpha(b""), Ok((&b""[..], &b""[..])));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::bytes::take_while0;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
///   take_while0(AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(alpha(Partial::new(b"latin123")), Ok((Partial::new(&b"123"[..]), &b"latin"[..])));
/// assert_eq!(alpha(Partial::new(b"12345")), Ok((Partial::new(&b"12345"[..]), &b""[..])));
/// assert_eq!(alpha(Partial::new(b"latin")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(alpha(Partial::new(b"")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn take_while0<T, I, Error: ParseError<I>>(
    list: T,
) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_while0", move |i: I| {
        if i.is_partial() {
            streaming_take_while_internal(i, &list)
        } else {
            complete_take_while_internal(i, &list)
        }
    })
}

pub(crate) fn streaming_take_while_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    split_at_offset_partial(&i, |c| !list.contains_token(c))
}

pub(crate) fn complete_take_while_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    split_at_offset_complete(&i, |c| !list.contains_token(c))
}

/// Recognize the longest (at least 1) input slice that matches the [pattern][ContainsToken]
///
/// It will return an `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::Slice)))` if the pattern wasn't met.
///
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` or if the pattern reaches the end of the input.
///
/// To recognize a series of tokens, use [`many1`][crate::multi::many1] to [`Accumulate`][crate::stream::Accumulate] into a `()` and then [`Parser::recognize`][crate::Parser::recognize].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::take_while1;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while1(AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(alpha(b"12345"), Err(ErrMode::Backtrack(Error::new(&b"12345"[..], ErrorKind::Slice))));
///
/// fn hex(s: &str) -> IResult<&str, &str> {
///   take_while1("1234567890ABCDEF").parse_next(s)
/// }
///
/// assert_eq!(hex("123 and voila"), Ok((" and voila", "123")));
/// assert_eq!(hex("DEADBEEF and others"), Ok((" and others", "DEADBEEF")));
/// assert_eq!(hex("BADBABEsomething"), Ok(("something", "BADBABE")));
/// assert_eq!(hex("D15EA5E"), Ok(("", "D15EA5E")));
/// assert_eq!(hex(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::bytes::take_while1;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
///   take_while1(AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(alpha(Partial::new(b"latin123")), Ok((Partial::new(&b"123"[..]), &b"latin"[..])));
/// assert_eq!(alpha(Partial::new(b"latin")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(alpha(Partial::new(b"12345")), Err(ErrMode::Backtrack(Error::new(Partial::new(&b"12345"[..]), ErrorKind::Slice))));
///
/// fn hex(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_while1("1234567890ABCDEF").parse_next(s)
/// }
///
/// assert_eq!(hex(Partial::new("123 and voila")), Ok((Partial::new(" and voila"), "123")));
/// assert_eq!(hex(Partial::new("DEADBEEF and others")), Ok((Partial::new(" and others"), "DEADBEEF")));
/// assert_eq!(hex(Partial::new("BADBABEsomething")), Ok((Partial::new("something"), "BADBABE")));
/// assert_eq!(hex(Partial::new("D15EA5E")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(hex(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
#[doc(alias = "is_a")]
pub fn take_while1<T, I, Error: ParseError<I>>(
    list: T,
) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_while1", move |i: I| {
        if i.is_partial() {
            streaming_take_while1_internal(i, &list)
        } else {
            complete_take_while1_internal(i, &list)
        }
    })
}

pub(crate) fn streaming_take_while1_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    let e: ErrorKind = ErrorKind::Slice;
    split_at_offset1_partial(&i, |c| !list.contains_token(c), e)
}

pub(crate) fn complete_take_while1_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    let e: ErrorKind = ErrorKind::Slice;
    split_at_offset1_complete(&i, |c| !list.contains_token(c), e)
}

/// Recognize the longest (m <= len <= n) input slice that matches the [pattern][ContainsToken]
///
/// It will return an `ErrMode::Backtrack(Error::new(_, ErrorKind::Slice))` if the pattern wasn't met or is out
/// of range (m <= len <= n).
///
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))`  if the pattern reaches the end of the input or is too short.
///
/// To recognize a series of tokens, use [`many_m_n`][crate::multi::many_m_n] to [`Accumulate`][crate::stream::Accumulate] into a `()` and then [`Parser::recognize`][crate::Parser::recognize].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::take_while_m_n;
/// use winnow::stream::AsChar;
///
/// fn short_alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while_m_n(3, 6, AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(short_alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"lengthy"), Ok((&b"y"[..], &b"length"[..])));
/// assert_eq!(short_alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"ed"), Err(ErrMode::Backtrack(Error::new(&b"ed"[..], ErrorKind::Slice))));
/// assert_eq!(short_alpha(b"12345"), Err(ErrMode::Backtrack(Error::new(&b"12345"[..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::bytes::take_while_m_n;
/// use winnow::stream::AsChar;
///
/// fn short_alpha(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
///   take_while_m_n(3, 6, AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(short_alpha(Partial::new(b"latin123")), Ok((Partial::new(&b"123"[..]), &b"latin"[..])));
/// assert_eq!(short_alpha(Partial::new(b"lengthy")), Ok((Partial::new(&b"y"[..]), &b"length"[..])));
/// assert_eq!(short_alpha(Partial::new(b"latin")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(short_alpha(Partial::new(b"ed")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(short_alpha(Partial::new(b"12345")), Err(ErrMode::Backtrack(Error::new(Partial::new(&b"12345"[..]), ErrorKind::Slice))));
/// ```
#[inline(always)]
pub fn take_while_m_n<T, I, Error: ParseError<I>>(
    m: usize,
    n: usize,
    list: T,
) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_while_m_n", move |i: I| {
        if i.is_partial() {
            streaming_take_while_m_n_internal(i, m, n, &list)
        } else {
            complete_take_while_m_n_internal(i, m, n, &list)
        }
    })
}

pub(crate) fn streaming_take_while_m_n_internal<T, I, Error: ParseError<I>>(
    input: I,
    m: usize,
    n: usize,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    if n < m {
        return Err(ErrMode::assert(input, "`m` should be <= `n`"));
    }

    let mut final_count = 0;
    for (processed, (offset, token)) in input.iter_offsets().enumerate() {
        if !list.contains_token(token) {
            if processed < m {
                return Err(ErrMode::from_error_kind(input, ErrorKind::Slice));
            } else {
                return Ok(input.next_slice(offset));
            }
        } else {
            if processed == n {
                return Ok(input.next_slice(offset));
            }
            final_count = processed + 1;
        }
    }

    if final_count == n {
        Ok(input.next_slice(input.eof_offset()))
    } else {
        let needed = if m > input.eof_offset() {
            m - input.eof_offset()
        } else {
            1
        };
        Err(ErrMode::Incomplete(Needed::new(needed)))
    }
}

pub(crate) fn complete_take_while_m_n_internal<T, I, Error: ParseError<I>>(
    input: I,
    m: usize,
    n: usize,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    if n < m {
        return Err(ErrMode::assert(input, "`m` should be <= `n`"));
    }

    let mut final_count = 0;
    for (processed, (offset, token)) in input.iter_offsets().enumerate() {
        if !list.contains_token(token) {
            if processed < m {
                return Err(ErrMode::from_error_kind(input, ErrorKind::Slice));
            } else {
                return Ok(input.next_slice(offset));
            }
        } else {
            if processed == n {
                return Ok(input.next_slice(offset));
            }
            final_count = processed + 1;
        }
    }

    if m <= final_count {
        Ok(input.next_slice(input.eof_offset()))
    } else {
        Err(ErrMode::from_error_kind(input, ErrorKind::Slice))
    }
}

/// Recognize the longest input slice (if any) till a [pattern][ContainsToken] is met.
///
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` if the match reaches the
/// end of input or if there was not match.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::take_till0;
///
/// fn till_colon(s: &str) -> IResult<&str, &str> {
///   take_till0(|c| c == ':').parse_next(s)
/// }
///
/// assert_eq!(till_colon("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon(":empty matched"), Ok((":empty matched", ""))); //allowed
/// assert_eq!(till_colon("12345"), Ok(("", "12345")));
/// assert_eq!(till_colon(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::bytes::take_till0;
///
/// fn till_colon(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_till0(|c| c == ':').parse_next(s)
/// }
///
/// assert_eq!(till_colon(Partial::new("latin:123")), Ok((Partial::new(":123"), "latin")));
/// assert_eq!(till_colon(Partial::new(":empty matched")), Ok((Partial::new(":empty matched"), ""))); //allowed
/// assert_eq!(till_colon(Partial::new("12345")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(till_colon(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn take_till0<T, I, Error: ParseError<I>>(
    list: T,
) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_till0", move |i: I| {
        if i.is_partial() {
            streaming_take_till_internal(i, &list)
        } else {
            complete_take_till_internal(i, &list)
        }
    })
}

pub(crate) fn streaming_take_till_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    split_at_offset_partial(&i, |c| list.contains_token(c))
}

pub(crate) fn complete_take_till_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    split_at_offset_complete(&i, |c| list.contains_token(c))
}

/// Recognize the longest (at least 1) input slice till a [pattern][ContainsToken] is met.
///
/// It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::Slice)))` if the input is empty or the
/// predicate matches the first input.
///
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` if the match reaches the
/// end of input or if there was not match.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::take_till1;
///
/// fn till_colon(s: &str) -> IResult<&str, &str> {
///   take_till1(|c| c == ':').parse_next(s)
/// }
///
/// assert_eq!(till_colon("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon(":empty matched"), Err(ErrMode::Backtrack(Error::new(":empty matched", ErrorKind::Slice))));
/// assert_eq!(till_colon("12345"), Ok(("", "12345")));
/// assert_eq!(till_colon(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
///
/// fn not_space(s: &str) -> IResult<&str, &str> {
///   take_till1(" \t\r\n").parse_next(s)
/// }
///
/// assert_eq!(not_space("Hello, World!"), Ok((" World!", "Hello,")));
/// assert_eq!(not_space("Sometimes\t"), Ok(("\t", "Sometimes")));
/// assert_eq!(not_space("Nospace"), Ok(("", "Nospace")));
/// assert_eq!(not_space(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::bytes::take_till1;
///
/// fn till_colon(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_till1(|c| c == ':').parse_next(s)
/// }
///
/// assert_eq!(till_colon(Partial::new("latin:123")), Ok((Partial::new(":123"), "latin")));
/// assert_eq!(till_colon(Partial::new(":empty matched")), Err(ErrMode::Backtrack(Error::new(Partial::new(":empty matched"), ErrorKind::Slice))));
/// assert_eq!(till_colon(Partial::new("12345")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(till_colon(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// fn not_space(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_till1(" \t\r\n").parse_next(s)
/// }
///
/// assert_eq!(not_space(Partial::new("Hello, World!")), Ok((Partial::new(" World!"), "Hello,")));
/// assert_eq!(not_space(Partial::new("Sometimes\t")), Ok((Partial::new("\t"), "Sometimes")));
/// assert_eq!(not_space(Partial::new("Nospace")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(not_space(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
#[doc(alias = "is_not")]
pub fn take_till1<T, I, Error: ParseError<I>>(
    list: T,
) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_till1", move |i: I| {
        if i.is_partial() {
            streaming_take_till1_internal(i, &list)
        } else {
            complete_take_till1_internal(i, &list)
        }
    })
}

pub(crate) fn streaming_take_till1_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    let e: ErrorKind = ErrorKind::Slice;
    split_at_offset1_partial(&i, |c| list.contains_token(c), e)
}

pub(crate) fn complete_take_till1_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    let e: ErrorKind = ErrorKind::Slice;
    split_at_offset1_complete(&i, |c| list.contains_token(c), e)
}

/// Recognize an input slice containing the first N input elements (I[..N]).
///
/// *Complete version*: It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::Slice)))` if the input is shorter than the argument.
///
/// *Partial version*: if the input has less than N elements, `take` will
/// return a `ErrMode::Incomplete(Needed::new(M))` where M is the number of
/// additional bytes the parser would need to succeed.
/// It is well defined for `&[u8]` as the number of elements is the byte size,
/// but for types like `&str`, we cannot know how many bytes correspond for
/// the next few chars, so the result will be `ErrMode::Incomplete(Needed::Unknown)`
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::take;
///
/// fn take6(s: &str) -> IResult<&str, &str> {
///   take(6usize).parse_next(s)
/// }
///
/// assert_eq!(take6("1234567"), Ok(("7", "123456")));
/// assert_eq!(take6("things"), Ok(("", "things")));
/// assert_eq!(take6("short"), Err(ErrMode::Backtrack(Error::new("short", ErrorKind::Slice))));
/// assert_eq!(take6(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
/// ```
///
/// The units that are taken will depend on the input type. For example, for a
/// `&str` it will take a number of `char`'s, whereas for a `&[u8]` it will
/// take that many `u8`'s:
///
/// ```rust
/// # use winnow::prelude::*;
/// use winnow::error::Error;
/// use winnow::bytes::take;
///
/// assert_eq!(take::<_, _, Error<_>>(1usize).parse_next("💙"), Ok(("", "💙")));
/// assert_eq!(take::<_, _, Error<_>>(1usize).parse_next("💙".as_bytes()), Ok((b"\x9F\x92\x99".as_ref(), b"\xF0".as_ref())));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::bytes::take;
///
/// fn take6(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take(6usize).parse_next(s)
/// }
///
/// assert_eq!(take6(Partial::new("1234567")), Ok((Partial::new("7"), "123456")));
/// assert_eq!(take6(Partial::new("things")), Ok((Partial::new(""), "things")));
/// // `Unknown` as we don't know the number of bytes that `count` corresponds to
/// assert_eq!(take6(Partial::new("short")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
#[inline(always)]
pub fn take<C, I, Error: ParseError<I>>(count: C) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    C: ToUsize,
{
    let c = count.to_usize();
    trace("take", move |i: I| {
        if i.is_partial() {
            streaming_take_internal(i, c)
        } else {
            complete_take_internal(i, c)
        }
    })
}

pub(crate) fn streaming_take_internal<I, Error: ParseError<I>>(
    i: I,
    c: usize,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
{
    match i.offset_at(c) {
        Ok(offset) => Ok(i.next_slice(offset)),
        Err(i) => Err(ErrMode::Incomplete(i)),
    }
}

pub(crate) fn complete_take_internal<I, Error: ParseError<I>>(
    i: I,
    c: usize,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
{
    match i.offset_at(c) {
        Ok(offset) => Ok(i.next_slice(offset)),
        Err(_needed) => Err(ErrMode::from_error_kind(i, ErrorKind::Slice)),
    }
}

/// Recognize the input slice up to the first occurrence of the literal.
///
/// It doesn't consume the pattern.
///
/// *Complete version*: It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::Slice)))`
/// if the pattern wasn't met.
///
/// *Partial version*: will return a `ErrMode::Incomplete(Needed::new(N))` if the input doesn't
/// contain the pattern or if the input is smaller than the pattern.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::take_until0;
///
/// fn until_eof(s: &str) -> IResult<&str, &str> {
///   take_until0("eof").parse_next(s)
/// }
///
/// assert_eq!(until_eof("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert_eq!(until_eof("hello, world"), Err(ErrMode::Backtrack(Error::new("hello, world", ErrorKind::Slice))));
/// assert_eq!(until_eof(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
/// assert_eq!(until_eof("1eof2eof"), Ok(("eof2eof", "1")));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::bytes::take_until0;
///
/// fn until_eof(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_until0("eof").parse_next(s)
/// }
///
/// assert_eq!(until_eof(Partial::new("hello, worldeof")), Ok((Partial::new("eof"), "hello, world")));
/// assert_eq!(until_eof(Partial::new("hello, world")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof(Partial::new("hello, worldeo")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof(Partial::new("1eof2eof")), Ok((Partial::new("eof2eof"), "1")));
/// ```
#[inline(always)]
pub fn take_until0<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + FindSlice<T>,
    T: SliceLen + Clone,
{
    trace("take_until0", move |i: I| {
        if i.is_partial() {
            streaming_take_until_internal(i, tag.clone())
        } else {
            complete_take_until_internal(i, tag.clone())
        }
    })
}

pub(crate) fn streaming_take_until_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + FindSlice<T>,
    T: SliceLen,
{
    match i.find_slice(t) {
        Some(offset) => Ok(i.next_slice(offset)),
        None => Err(ErrMode::Incomplete(Needed::Unknown)),
    }
}

pub(crate) fn complete_take_until_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + FindSlice<T>,
    T: SliceLen,
{
    match i.find_slice(t) {
        Some(offset) => Ok(i.next_slice(offset)),
        None => Err(ErrMode::from_error_kind(i, ErrorKind::Slice)),
    }
}

/// Recognize the non empty input slice up to the first occurrence of the literal.
///
/// It doesn't consume the pattern.
///
/// *Complete version*: It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::Slice)))`
/// if the pattern wasn't met.
///
/// *Partial version*: will return a `ErrMode::Incomplete(Needed::new(N))` if the input doesn't
/// contain the pattern or if the input is smaller than the pattern.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::bytes::take_until1;
///
/// fn until_eof(s: &str) -> IResult<&str, &str> {
///   take_until1("eof").parse_next(s)
/// }
///
/// assert_eq!(until_eof("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert_eq!(until_eof("hello, world"), Err(ErrMode::Backtrack(Error::new("hello, world", ErrorKind::Slice))));
/// assert_eq!(until_eof(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
/// assert_eq!(until_eof("1eof2eof"), Ok(("eof2eof", "1")));
/// assert_eq!(until_eof("eof"), Err(ErrMode::Backtrack(Error::new("eof", ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::bytes::take_until1;
///
/// fn until_eof(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_until1("eof").parse_next(s)
/// }
///
/// assert_eq!(until_eof(Partial::new("hello, worldeof")), Ok((Partial::new("eof"), "hello, world")));
/// assert_eq!(until_eof(Partial::new("hello, world")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof(Partial::new("hello, worldeo")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof(Partial::new("1eof2eof")), Ok((Partial::new("eof2eof"), "1")));
/// assert_eq!(until_eof(Partial::new("eof")),  Err(ErrMode::Backtrack(Error::new(Partial::new("eof"), ErrorKind::Slice))));
/// ```
#[inline(always)]
pub fn take_until1<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl Parser<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + FindSlice<T>,
    T: SliceLen + Clone,
{
    trace("take_until1", move |i: I| {
        if i.is_partial() {
            streaming_take_until1_internal(i, tag.clone())
        } else {
            complete_take_until1_internal(i, tag.clone())
        }
    })
}

pub(crate) fn streaming_take_until1_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + FindSlice<T>,
    T: SliceLen,
{
    match i.find_slice(t) {
        None => Err(ErrMode::Incomplete(Needed::Unknown)),
        Some(0) => Err(ErrMode::from_error_kind(i, ErrorKind::Slice)),
        Some(offset) => Ok(i.next_slice(offset)),
    }
}

pub(crate) fn complete_take_until1_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + FindSlice<T>,
    T: SliceLen,
{
    match i.find_slice(t) {
        None | Some(0) => Err(ErrMode::from_error_kind(i, ErrorKind::Slice)),
        Some(offset) => Ok(i.next_slice(offset)),
    }
}
