//! Parsers recognizing bytes streams

#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub mod complete;
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub mod streaming;
#[cfg(test)]
mod tests;

use crate::error::ParseError;
use crate::stream::{
    Compare, ContainsToken, FindSlice, SliceLen, Stream, StreamIsPartial, ToUsize,
};
use crate::trace::trace;
use crate::IResult;

/// Matches one token
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use winnow::{bytes::any, error::ErrMode, error::{Error, ErrorKind}, IResult};
/// fn parser(input: &str) -> IResult<&str, char> {
///     any(input)
/// }
///
/// assert_eq!(parser("abc"), Ok(("bc",'a')));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Eof))));
/// ```
///
/// ```
/// # use winnow::{bytes::any, error::ErrMode, error::ErrorKind, error::Error, IResult, error::Needed};
/// # use winnow::Partial;
/// assert_eq!(any::<_, Error<_>>(Partial::new("abc")), Ok((Partial::new("bc"),'a')));
/// assert_eq!(any::<_, Error<_>>(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
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
            streaming::any(input)
        } else {
            complete::any(input)
        }
    })(input)
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
///   tag("Hello")(s)
/// }
///
/// assert_eq!(parser("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser("Something"), Err(ErrMode::Backtrack(Error::new("Something", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::tag;
///
/// fn parser(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   tag("Hello")(s)
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
pub fn tag<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + Compare<T>,
    T: SliceLen + Clone,
{
    trace("tag", move |i: I| {
        let t = tag.clone();
        if i.is_partial() {
            streaming::tag_internal(i, t)
        } else {
            complete::tag_internal(i, t)
        }
    })
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
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::tag_no_case;
///
/// fn parser(s: &str) -> IResult<&str, &str> {
///   tag_no_case("hello")(s)
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
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::tag_no_case;
///
/// fn parser(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   tag_no_case("hello")(s)
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
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + Compare<T>,
    T: SliceLen + Clone,
{
    trace("tag_no_case", move |i: I| {
        let t = tag.clone();
        if i.is_partial() {
            streaming::tag_no_case_internal(i, t)
        } else {
            complete::tag_no_case_internal(i, t)
        }
    })
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
/// ```
/// # use winnow::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::bytes::one_of;
/// assert_eq!(one_of::<_, _, Error<_>>("abc")("b"), Ok(("", 'b')));
/// assert_eq!(one_of::<_, _, Error<_>>("a")("bc"), Err(ErrMode::Backtrack(Error::new("bc", ErrorKind::OneOf))));
/// assert_eq!(one_of::<_, _, Error<_>>("a")(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::OneOf))));
///
/// fn parser_fn(i: &str) -> IResult<&str, char> {
///     one_of(|c| c == 'a' || c == 'b')(i)
/// }
/// assert_eq!(parser_fn("abc"), Ok(("bc", 'a')));
/// assert_eq!(parser_fn("cd"), Err(ErrMode::Backtrack(Error::new("cd", ErrorKind::OneOf))));
/// assert_eq!(parser_fn(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::OneOf))));
/// ```
///
/// ```
/// # use winnow::*;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// # use winnow::bytes::one_of;
/// assert_eq!(one_of::<_, _, Error<_>>("abc")(Partial::new("b")), Ok((Partial::new(""), 'b')));
/// assert_eq!(one_of::<_, _, Error<_>>("a")(Partial::new("bc")), Err(ErrMode::Backtrack(Error::new(Partial::new("bc"), ErrorKind::OneOf))));
/// assert_eq!(one_of::<_, _, Error<_>>("a")(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// fn parser_fn(i: Partial<&str>) -> IResult<Partial<&str>, char> {
///     one_of(|c| c == 'a' || c == 'b')(i)
/// }
/// assert_eq!(parser_fn(Partial::new("abc")), Ok((Partial::new("bc"), 'a')));
/// assert_eq!(parser_fn(Partial::new("cd")), Err(ErrMode::Backtrack(Error::new(Partial::new("cd"), ErrorKind::OneOf))));
/// assert_eq!(parser_fn(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
#[doc(alias = "char")]
#[doc(alias = "token")]
#[doc(alias = "satisfy")]
pub fn one_of<I, T, Error: ParseError<I>>(
    list: T,
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Token, Error>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: Copy,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("one_of", move |i: I| {
        if i.is_partial() {
            streaming::one_of_internal(i, &list)
        } else {
            complete::one_of_internal(i, &list)
        }
    })
}

/// Recognize a token that does not match the [pattern][ContainsToken]
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::bytes::none_of;
/// assert_eq!(none_of::<_, _, Error<_>>("abc")("z"), Ok(("", 'z')));
/// assert_eq!(none_of::<_, _, Error<_>>("ab")("a"), Err(ErrMode::Backtrack(Error::new("a", ErrorKind::NoneOf))));
/// assert_eq!(none_of::<_, _, Error<_>>("a")(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::NoneOf))));
/// ```
///
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// # use winnow::bytes::none_of;
/// assert_eq!(none_of::<_, _, Error<_>>("abc")(Partial::new("z")), Ok((Partial::new(""), 'z')));
/// assert_eq!(none_of::<_, _, Error<_>>("ab")(Partial::new("a")), Err(ErrMode::Backtrack(Error::new(Partial::new("a"), ErrorKind::NoneOf))));
/// assert_eq!(none_of::<_, _, Error<_>>("a")(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn none_of<I, T, Error: ParseError<I>>(
    list: T,
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Token, Error>
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: Copy,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("none_of", move |i: I| {
        if i.is_partial() {
            streaming::none_of_internal(i, &list)
        } else {
            complete::none_of_internal(i, &list)
        }
    })
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
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed, IResult};
/// use winnow::bytes::take_while0;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while0(AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha(b"12345"), Ok((&b"12345"[..], &b""[..])));
/// assert_eq!(alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(alpha(b""), Ok((&b""[..], &b""[..])));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::take_while0;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
///   take_while0(AsChar::is_alpha)(s)
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
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_while0", move |i: I| {
        if i.is_partial() {
            streaming::take_while_internal(i, &list)
        } else {
            complete::take_while_internal(i, &list)
        }
    })
}

/// Recognize the longest (at least 1) input slice that matches the [pattern][ContainsToken]
///
/// It will return an `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::TakeWhile1)))` if the pattern wasn't met.
///
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` or if the pattern reaches the end of the input.
///
/// To recognize a series of tokens, use [`many1`][crate::multi::many1] to [`Accumulate`][crate::stream::Accumulate] into a `()` and then [`Parser::recognize`][crate::Parser::recognize].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::take_while1;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while1(AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(alpha(b"12345"), Err(ErrMode::Backtrack(Error::new(&b"12345"[..], ErrorKind::TakeWhile1))));
///
/// fn hex(s: &str) -> IResult<&str, &str> {
///   take_while1("1234567890ABCDEF")(s)
/// }
///
/// assert_eq!(hex("123 and voila"), Ok((" and voila", "123")));
/// assert_eq!(hex("DEADBEEF and others"), Ok((" and others", "DEADBEEF")));
/// assert_eq!(hex("BADBABEsomething"), Ok(("something", "BADBABE")));
/// assert_eq!(hex("D15EA5E"), Ok(("", "D15EA5E")));
/// assert_eq!(hex(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::TakeWhile1))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::take_while1;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
///   take_while1(AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(alpha(Partial::new(b"latin123")), Ok((Partial::new(&b"123"[..]), &b"latin"[..])));
/// assert_eq!(alpha(Partial::new(b"latin")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(alpha(Partial::new(b"12345")), Err(ErrMode::Backtrack(Error::new(Partial::new(&b"12345"[..]), ErrorKind::TakeWhile1))));
///
/// fn hex(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_while1("1234567890ABCDEF")(s)
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
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_while1", move |i: I| {
        if i.is_partial() {
            streaming::take_while1_internal(i, &list)
        } else {
            complete::take_while1_internal(i, &list)
        }
    })
}

/// Recognize the longest (m <= len <= n) input slice that matches the [pattern][ContainsToken]
///
/// It will return an `ErrMode::Backtrack(Error::new(_, ErrorKind::TakeWhileMN))` if the pattern wasn't met or is out
/// of range (m <= len <= n).
///
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))`  if the pattern reaches the end of the input or is too short.
///
/// To recognize a series of tokens, use [`many_m_n`][crate::multi::many_m_n] to [`Accumulate`][crate::stream::Accumulate] into a `()` and then [`Parser::recognize`][crate::Parser::recognize].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::take_while_m_n;
/// use winnow::stream::AsChar;
///
/// fn short_alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while_m_n(3, 6, AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(short_alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"lengthy"), Ok((&b"y"[..], &b"length"[..])));
/// assert_eq!(short_alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"ed"), Err(ErrMode::Backtrack(Error::new(&b"ed"[..], ErrorKind::TakeWhileMN))));
/// assert_eq!(short_alpha(b"12345"), Err(ErrMode::Backtrack(Error::new(&b"12345"[..], ErrorKind::TakeWhileMN))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::take_while_m_n;
/// use winnow::stream::AsChar;
///
/// fn short_alpha(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
///   take_while_m_n(3, 6, AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(short_alpha(Partial::new(b"latin123")), Ok((Partial::new(&b"123"[..]), &b"latin"[..])));
/// assert_eq!(short_alpha(Partial::new(b"lengthy")), Ok((Partial::new(&b"y"[..]), &b"length"[..])));
/// assert_eq!(short_alpha(Partial::new(b"latin")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(short_alpha(Partial::new(b"ed")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(short_alpha(Partial::new(b"12345")), Err(ErrMode::Backtrack(Error::new(Partial::new(&b"12345"[..]), ErrorKind::TakeWhileMN))));
/// ```
#[inline(always)]
pub fn take_while_m_n<T, I, Error: ParseError<I>>(
    m: usize,
    n: usize,
    list: T,
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_while_m_n", move |i: I| {
        if i.is_partial() {
            streaming::take_while_m_n_internal(i, m, n, &list)
        } else {
            complete::take_while_m_n_internal(i, m, n, &list)
        }
    })
}

/// Recognize the longest input slice (if any) till a [pattern][ContainsToken] is met.
///
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` if the match reaches the
/// end of input or if there was not match.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed, IResult};
/// use winnow::bytes::take_till0;
///
/// fn till_colon(s: &str) -> IResult<&str, &str> {
///   take_till0(|c| c == ':')(s)
/// }
///
/// assert_eq!(till_colon("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon(":empty matched"), Ok((":empty matched", ""))); //allowed
/// assert_eq!(till_colon("12345"), Ok(("", "12345")));
/// assert_eq!(till_colon(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::take_till0;
///
/// fn till_colon(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_till0(|c| c == ':')(s)
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
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_till0", move |i: I| {
        if i.is_partial() {
            streaming::take_till_internal(i, &list)
        } else {
            complete::take_till_internal(i, &list)
        }
    })
}

/// Recognize the longest (at least 1) input slice till a [pattern][ContainsToken] is met.
///
/// It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::TakeTill1)))` if the input is empty or the
/// predicate matches the first input.
///
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` if the match reaches the
/// end of input or if there was not match.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::take_till1;
///
/// fn till_colon(s: &str) -> IResult<&str, &str> {
///   take_till1(|c| c == ':')(s)
/// }
///
/// assert_eq!(till_colon("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon(":empty matched"), Err(ErrMode::Backtrack(Error::new(":empty matched", ErrorKind::TakeTill1))));
/// assert_eq!(till_colon("12345"), Ok(("", "12345")));
/// assert_eq!(till_colon(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::TakeTill1))));
///
/// fn not_space(s: &str) -> IResult<&str, &str> {
///   take_till1(" \t\r\n")(s)
/// }
///
/// assert_eq!(not_space("Hello, World!"), Ok((" World!", "Hello,")));
/// assert_eq!(not_space("Sometimes\t"), Ok(("\t", "Sometimes")));
/// assert_eq!(not_space("Nospace"), Ok(("", "Nospace")));
/// assert_eq!(not_space(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::TakeTill1))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::take_till1;
///
/// fn till_colon(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_till1(|c| c == ':')(s)
/// }
///
/// assert_eq!(till_colon(Partial::new("latin:123")), Ok((Partial::new(":123"), "latin")));
/// assert_eq!(till_colon(Partial::new(":empty matched")), Err(ErrMode::Backtrack(Error::new(Partial::new(":empty matched"), ErrorKind::TakeTill1))));
/// assert_eq!(till_colon(Partial::new("12345")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(till_colon(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// fn not_space(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_till1(" \t\r\n")(s)
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
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    trace("take_till1", move |i: I| {
        if i.is_partial() {
            streaming::take_till1_internal(i, &list)
        } else {
            complete::take_till1_internal(i, &list)
        }
    })
}

/// Recognize an input slice containing the first N input elements (I[..N]).
///
/// *Complete version*: It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::Eof)))` if the input is shorter than the argument.
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
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::take;
///
/// fn take6(s: &str) -> IResult<&str, &str> {
///   take(6usize)(s)
/// }
///
/// assert_eq!(take6("1234567"), Ok(("7", "123456")));
/// assert_eq!(take6("things"), Ok(("", "things")));
/// assert_eq!(take6("short"), Err(ErrMode::Backtrack(Error::new("short", ErrorKind::Eof))));
/// assert_eq!(take6(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Eof))));
/// ```
///
/// The units that are taken will depend on the input type. For example, for a
/// `&str` it will take a number of `char`'s, whereas for a `&[u8]` it will
/// take that many `u8`'s:
///
/// ```rust
/// use winnow::error::Error;
/// use winnow::bytes::take;
///
/// assert_eq!(take::<_, _, Error<_>>(1usize)("💙"), Ok(("", "💙")));
/// assert_eq!(take::<_, _, Error<_>>(1usize)("💙".as_bytes()), Ok((b"\x9F\x92\x99".as_ref(), b"\xF0".as_ref())));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::take;
///
/// fn take6(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take(6usize)(s)
/// }
///
/// assert_eq!(take6(Partial::new("1234567")), Ok((Partial::new("7"), "123456")));
/// assert_eq!(take6(Partial::new("things")), Ok((Partial::new(""), "things")));
/// // `Unknown` as we don't know the number of bytes that `count` corresponds to
/// assert_eq!(take6(Partial::new("short")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
#[inline(always)]
pub fn take<C, I, Error: ParseError<I>>(
    count: C,
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    C: ToUsize,
{
    let c = count.to_usize();
    trace("take", move |i: I| {
        if i.is_partial() {
            streaming::take_internal(i, c)
        } else {
            complete::take_internal(i, c)
        }
    })
}

/// Recognize the input slice up to the first occurrence of the literal.
///
/// It doesn't consume the pattern.
///
/// *Complete version*: It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::TakeUntil)))`
/// if the pattern wasn't met.
///
/// *Partial version*: will return a `ErrMode::Incomplete(Needed::new(N))` if the input doesn't
/// contain the pattern or if the input is smaller than the pattern.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::take_until0;
///
/// fn until_eof(s: &str) -> IResult<&str, &str> {
///   take_until0("eof")(s)
/// }
///
/// assert_eq!(until_eof("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert_eq!(until_eof("hello, world"), Err(ErrMode::Backtrack(Error::new("hello, world", ErrorKind::TakeUntil))));
/// assert_eq!(until_eof(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::TakeUntil))));
/// assert_eq!(until_eof("1eof2eof"), Ok(("eof2eof", "1")));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::take_until0;
///
/// fn until_eof(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_until0("eof")(s)
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
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + FindSlice<T>,
    T: SliceLen + Clone,
{
    trace("take_until0", move |i: I| {
        if i.is_partial() {
            streaming::take_until_internal(i, tag.clone())
        } else {
            complete::take_until_internal(i, tag.clone())
        }
    })
}

/// Recognize the non empty input slice up to the first occurrence of the literal.
///
/// It doesn't consume the pattern.
///
/// *Complete version*: It will return `Err(ErrMode::Backtrack(Error::new(_, ErrorKind::TakeUntil)))`
/// if the pattern wasn't met.
///
/// *Partial version*: will return a `ErrMode::Incomplete(Needed::new(N))` if the input doesn't
/// contain the pattern or if the input is smaller than the pattern.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::take_until1;
///
/// fn until_eof(s: &str) -> IResult<&str, &str> {
///   take_until1("eof")(s)
/// }
///
/// assert_eq!(until_eof("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert_eq!(until_eof("hello, world"), Err(ErrMode::Backtrack(Error::new("hello, world", ErrorKind::TakeUntil))));
/// assert_eq!(until_eof(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::TakeUntil))));
/// assert_eq!(until_eof("1eof2eof"), Ok(("eof2eof", "1")));
/// assert_eq!(until_eof("eof"), Err(ErrMode::Backtrack(Error::new("eof", ErrorKind::TakeUntil))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// # use winnow::Partial;
/// use winnow::bytes::take_until1;
///
/// fn until_eof(s: Partial<&str>) -> IResult<Partial<&str>, &str> {
///   take_until1("eof")(s)
/// }
///
/// assert_eq!(until_eof(Partial::new("hello, worldeof")), Ok((Partial::new("eof"), "hello, world")));
/// assert_eq!(until_eof(Partial::new("hello, world")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof(Partial::new("hello, worldeo")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof(Partial::new("1eof2eof")), Ok((Partial::new("eof2eof"), "1")));
/// assert_eq!(until_eof(Partial::new("eof")),  Err(ErrMode::Backtrack(Error::new(Partial::new("eof"), ErrorKind::TakeUntil))));
/// ```
#[inline(always)]
pub fn take_until1<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + FindSlice<T>,
    T: SliceLen + Clone,
{
    trace("take_until1", move |i: I| {
        if i.is_partial() {
            streaming::take_until1_internal(i, tag.clone())
        } else {
            complete::take_until1_internal(i, tag.clone())
        }
    })
}
