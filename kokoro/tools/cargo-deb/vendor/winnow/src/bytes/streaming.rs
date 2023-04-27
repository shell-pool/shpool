//! Parsers recognizing bytes streams, streaming version

#![allow(deprecated)]

use crate::error::ErrMode;
use crate::error::ErrorKind;
use crate::error::Needed;
use crate::error::ParseError;
use crate::lib::std::result::Result::Ok;
use crate::stream::{
    split_at_offset1_partial, split_at_offset_partial, Compare, CompareResult, ContainsToken,
    FindSlice, Offset, SliceLen, Stream, ToUsize,
};
use crate::{IResult, Parser};

pub(crate) fn any<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Token, E>
where
    I: Stream,
{
    input
        .next_token()
        .ok_or_else(|| ErrMode::Incomplete(Needed::new(1)))
}

/// Recognizes a pattern.
///
/// The input data will be compared to the tag combinator's argument and will return the part of
/// the input that matches the argument.
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::streaming::tag;
///
/// fn parser(s: &str) -> IResult<&str, &str> {
///   tag("Hello")(s)
/// }
///
/// assert_eq!(parser("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser("Something"), Err(ErrMode::Backtrack(Error::new("Something", ErrorKind::Tag))));
/// assert_eq!(parser("S"), Err(ErrMode::Backtrack(Error::new("S", ErrorKind::Tag))));
/// assert_eq!(parser("H"), Err(ErrMode::Incomplete(Needed::new(4))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::tag`][crate::bytes::tag] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::tag` with input wrapped in `winnow::Partial`"
)]
pub fn tag<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + Compare<T>,
    T: SliceLen + Clone,
{
    move |i: I| tag_internal(i, tag.clone())
}

pub(crate) fn tag_internal<T, I, Error: ParseError<I>>(
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

/// Recognizes a case insensitive pattern.
///
/// The input data will be compared to the tag combinator's argument and will return the part of
/// the input that matches the argument with no regard to case.
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::streaming::tag_no_case;
///
/// fn parser(s: &str) -> IResult<&str, &str> {
///   tag_no_case("hello")(s)
/// }
///
/// assert_eq!(parser("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser("hello, World!"), Ok((", World!", "hello")));
/// assert_eq!(parser("HeLlO, World!"), Ok((", World!", "HeLlO")));
/// assert_eq!(parser("Something"), Err(ErrMode::Backtrack(Error::new("Something", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(ErrMode::Incomplete(Needed::new(5))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::tag_no_case`][crate::bytes::tag_no_case] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::tag_no_case` with input wrapped in `winnow::Partial`"
)]
pub fn tag_no_case<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + Compare<T>,
    T: SliceLen + Clone,
{
    move |i: I| tag_no_case_internal(i, tag.clone())
}

pub(crate) fn tag_no_case_internal<T, I, Error: ParseError<I>>(
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

pub(crate) fn one_of_internal<I, T, E: ParseError<I>>(
    input: I,
    list: &T,
) -> IResult<I, <I as Stream>::Token, E>
where
    I: Stream,
    <I as Stream>::Token: Copy,
    T: ContainsToken<<I as Stream>::Token>,
{
    let (new_input, token) = input
        .next_token()
        .ok_or_else(|| ErrMode::Incomplete(Needed::new(1)))?;
    if list.contains_token(token) {
        Ok((new_input, token))
    } else {
        Err(ErrMode::from_error_kind(input, ErrorKind::OneOf))
    }
}

pub(crate) fn none_of_internal<I, T, E: ParseError<I>>(
    input: I,
    list: &T,
) -> IResult<I, <I as Stream>::Token, E>
where
    I: Stream,
    <I as Stream>::Token: Copy,
    T: ContainsToken<<I as Stream>::Token>,
{
    let (new_input, token) = input
        .next_token()
        .ok_or_else(|| ErrMode::Incomplete(Needed::new(1)))?;
    if !list.contains_token(token) {
        Ok((new_input, token))
    } else {
        Err(ErrMode::from_error_kind(input, ErrorKind::NoneOf))
    }
}

/// Parse till certain characters are met.
///
/// The parser will return the longest slice till one of the characters of the combinator's argument are met.
///
/// It doesn't consume the matched character.
///
/// It will return a `ErrMode::Incomplete(Needed::new(1))` if the pattern wasn't met.
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, IResult};
/// use winnow::bytes::streaming::is_not;
///
/// fn not_space(s: &str) -> IResult<&str, &str> {
///   is_not(" \t\r\n")(s)
/// }
///
/// assert_eq!(not_space("Hello, World!"), Ok((" World!", "Hello,")));
/// assert_eq!(not_space("Sometimes\t"), Ok(("\t", "Sometimes")));
/// assert_eq!(not_space("Nospace"), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(not_space(""), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_till1`][crate::bytes::take_till1] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_till1` with input wrapped in `winnow::Partial`"
)]
pub fn is_not<T, I, Error: ParseError<I>>(
    arr: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| is_not_internal(i, &arr)
}

pub(crate) fn is_not_internal<T, I, Error: ParseError<I>>(
    i: I,
    arr: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    let e: ErrorKind = ErrorKind::IsNot;
    split_at_offset1_partial(&i, |c| arr.contains_token(c), e)
}

/// Returns the longest slice of the matches the pattern.
///
/// The parser will return the longest slice consisting of the characters in provided in the
/// combinator's argument.
///
/// # Partial specific
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` if the pattern wasn't met
/// or if the pattern reaches the end of the input.
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, IResult};
/// use winnow::bytes::streaming::is_a;
///
/// fn hex(s: &str) -> IResult<&str, &str> {
///   is_a("1234567890ABCDEF")(s)
/// }
///
/// assert_eq!(hex("123 and voila"), Ok((" and voila", "123")));
/// assert_eq!(hex("DEADBEEF and others"), Ok((" and others", "DEADBEEF")));
/// assert_eq!(hex("BADBABEsomething"), Ok(("something", "BADBABE")));
/// assert_eq!(hex("D15EA5E"), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(hex(""), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_while1`][crate::bytes::take_while1] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_while1` with input wrapped in `winnow::Partial`"
)]
pub fn is_a<T, I, Error: ParseError<I>>(
    arr: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| is_a_internal(i, &arr)
}

pub(crate) fn is_a_internal<T, I, Error: ParseError<I>>(
    i: I,
    arr: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    let e: ErrorKind = ErrorKind::IsA;
    split_at_offset1_partial(&i, |c| !arr.contains_token(c), e)
}

/// Returns the longest input slice (if any) that matches the predicate.
///
/// The parser will return the longest slice that matches the given predicate *(a function that
/// takes the input and returns a bool)*.
///
/// # Partial Specific
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` if the pattern reaches the end of the input.
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, IResult};
/// use winnow::bytes::streaming::take_while;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while(AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha(b"12345"), Ok((&b"12345"[..], &b""[..])));
/// assert_eq!(alpha(b"latin"), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(alpha(b""), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_while0`][crate::bytes::take_while0] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_while0` with input wrapped in `winnow::Partial`"
)]
pub fn take_while<T, I, Error: ParseError<I>>(
    list: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| take_while_internal(i, &list)
}

pub(crate) fn take_while_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    split_at_offset_partial(&i, |c| !list.contains_token(c))
}

/// Returns the longest (at least 1) input slice that matches the predicate.
///
/// The parser will return the longest slice that matches the given predicate *(a function that
/// takes the input and returns a bool)*.
///
/// It will return an `Err(ErrMode::Backtrack((_, ErrorKind::TakeWhile1)))` if the pattern wasn't met.
///
/// # Partial Specific
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` or if the pattern reaches the end of the input.
///
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::streaming::take_while1;
/// use winnow::stream::AsChar;
///
/// fn alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while1(AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha(b"latin"), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(alpha(b"12345"), Err(ErrMode::Backtrack(Error::new(&b"12345"[..], ErrorKind::TakeWhile1))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_while1`][crate::bytes::take_while1] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_while1` with input wrapped in `winnow::Partial`"
)]
pub fn take_while1<T, I, Error: ParseError<I>>(
    list: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| take_while1_internal(i, &list)
}

pub(crate) fn take_while1_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    let e: ErrorKind = ErrorKind::TakeWhile1;
    split_at_offset1_partial(&i, |c| !list.contains_token(c), e)
}

/// Returns the longest (m <= len <= n) input slice  that matches the predicate.
///
/// The parser will return the longest slice that matches the given predicate *(a function that
/// takes the input and returns a bool)*.
///
/// It will return an `ErrMode::Backtrack((_, ErrorKind::TakeWhileMN))` if the pattern wasn't met.
/// # Partial Specific
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))`  if the pattern reaches the end of the input or is too short.
///
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::streaming::take_while_m_n;
/// use winnow::stream::AsChar;
///
/// fn short_alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   take_while_m_n(3, 6, AsChar::is_alpha)(s)
/// }
///
/// assert_eq!(short_alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"lengthy"), Ok((&b"y"[..], &b"length"[..])));
/// assert_eq!(short_alpha(b"latin"), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(short_alpha(b"ed"), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(short_alpha(b"12345"), Err(ErrMode::Backtrack(Error::new(&b"12345"[..], ErrorKind::TakeWhileMN))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_while_m_n`][crate::bytes::take_while_m_n] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_while_m_n` with input wrapped in `winnow::Partial`"
)]
pub fn take_while_m_n<T, I, Error: ParseError<I>>(
    m: usize,
    n: usize,
    list: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| take_while_m_n_internal(i, m, n, &list)
}

pub(crate) fn take_while_m_n_internal<T, I, Error: ParseError<I>>(
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
        return Err(ErrMode::from_error_kind(input, ErrorKind::TakeWhileMN));
    }

    let mut final_count = 0;
    for (processed, (offset, token)) in input.iter_offsets().enumerate() {
        if !list.contains_token(token) {
            if processed < m {
                return Err(ErrMode::from_error_kind(input, ErrorKind::TakeWhileMN));
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

/// Returns the longest input slice (if any) till a predicate is met.
///
/// The parser will return the longest slice till the given predicate *(a function that
/// takes the input and returns a bool)*.
///
/// # Partial Specific
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` if the match reaches the
/// end of input or if there was not match.
///
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, IResult};
/// use winnow::bytes::streaming::take_till;
///
/// fn till_colon(s: &str) -> IResult<&str, &str> {
///   take_till(|c| c == ':')(s)
/// }
///
/// assert_eq!(till_colon("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon(":empty matched"), Ok((":empty matched", ""))); //allowed
/// assert_eq!(till_colon("12345"), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(till_colon(""), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_till0`][crate::bytes::take_till0] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_till0` with input wrapped in `winnow::Partial`"
)]
#[allow(clippy::redundant_closure)]
pub fn take_till<T, I, Error: ParseError<I>>(
    list: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| take_till_internal(i, &list)
}

pub(crate) fn take_till_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    split_at_offset_partial(&i, |c| list.contains_token(c))
}

/// Returns the longest (at least 1) input slice till a predicate is met.
///
/// The parser will return the longest slice till the given predicate *(a function that
/// takes the input and returns a bool)*.
///
/// # Partial Specific
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(1))` if the match reaches the
/// end of input or if there was not match.
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::streaming::take_till1;
///
/// fn till_colon(s: &str) -> IResult<&str, &str> {
///   take_till1(|c| c == ':')(s)
/// }
///
/// assert_eq!(till_colon("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon(":empty matched"), Err(ErrMode::Backtrack(Error::new(":empty matched", ErrorKind::TakeTill1))));
/// assert_eq!(till_colon("12345"), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(till_colon(""), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_till1`][crate::bytes::take_till1] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_till1` with input wrapped in `winnow::Partial`"
)]
#[allow(clippy::redundant_closure)]
pub fn take_till1<T, I, Error: ParseError<I>>(
    list: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    move |i: I| take_till1_internal(i, &list)
}

pub(crate) fn take_till1_internal<T, I, Error: ParseError<I>>(
    i: I,
    list: &T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    T: ContainsToken<<I as Stream>::Token>,
{
    let e: ErrorKind = ErrorKind::TakeTill1;
    split_at_offset1_partial(&i, |c| list.contains_token(c), e)
}

/// Returns an input slice containing the first N input elements (I[..N]).
///
/// # Partial Specific
/// *Partial version* if the input has less than N elements, `take` will
/// return a `ErrMode::Incomplete(Needed::new(M))` where M is the number of
/// additional bytes the parser would need to succeed.
/// It is well defined for `&[u8]` as the number of elements is the byte size,
/// but for types like `&str`, we cannot know how many bytes correspond for
/// the next few chars, so the result will be `ErrMode::Incomplete(Needed::Unknown)`
///
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, IResult};
/// use winnow::bytes::streaming::take;
///
/// fn take6(s: &str) -> IResult<&str, &str> {
///   take(6usize)(s)
/// }
///
/// assert_eq!(take6("1234567"), Ok(("7", "123456")));
/// assert_eq!(take6("things"), Ok(("", "things")));
/// assert_eq!(take6("short"), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take`][crate::bytes::take] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take` with input wrapped in `winnow::Partial`"
)]
pub fn take<C, I, Error: ParseError<I>>(
    count: C,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream,
    C: ToUsize,
{
    let c = count.to_usize();
    move |i: I| take_internal(i, c)
}

pub(crate) fn take_internal<I, Error: ParseError<I>>(
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

/// Returns the input slice up to the first occurrence of the pattern.
///
/// It doesn't consume the pattern.
///
/// # Partial Specific
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(N))` if the input doesn't
/// contain the pattern or if the input is smaller than the pattern.
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, IResult};
/// use winnow::bytes::streaming::take_until;
///
/// fn until_eof(s: &str) -> IResult<&str, &str> {
///   take_until("eof")(s)
/// }
///
/// assert_eq!(until_eof("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert_eq!(until_eof("hello, world"), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof("hello, worldeo"), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof("1eof2eof"), Ok(("eof2eof", "1")));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_until0`][crate::bytes::take_until0] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_until0` with input wrapped in `winnow::Partial`"
)]
pub fn take_until<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + FindSlice<T>,
    T: SliceLen + Clone,
{
    move |i: I| take_until_internal(i, tag.clone())
}

pub(crate) fn take_until_internal<T, I, Error: ParseError<I>>(
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

/// Returns the non empty input slice up to the first occurrence of the pattern.
///
/// It doesn't consume the pattern.
///
/// # Partial Specific
/// *Partial version* will return a `ErrMode::Incomplete(Needed::new(N))` if the input doesn't
/// contain the pattern or if the input is smaller than the pattern.
/// # Example
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::bytes::streaming::take_until1;
///
/// fn until_eof(s: &str) -> IResult<&str, &str> {
///   take_until1("eof")(s)
/// }
///
/// assert_eq!(until_eof("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert_eq!(until_eof("hello, world"), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof("hello, worldeo"), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof("1eof2eof"), Ok(("eof2eof", "1")));
/// assert_eq!(until_eof("eof"),  Err(ErrMode::Backtrack(Error::new("eof", ErrorKind::TakeUntil))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bytes::take_until1`][crate::bytes::take_until1] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bytes::take_until1` with input wrapped in `winnow::Partial`"
)]
pub fn take_until1<T, I, Error: ParseError<I>>(
    tag: T,
) -> impl Fn(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + FindSlice<T>,
    T: SliceLen + Clone,
{
    move |i: I| take_until1_internal(i, tag.clone())
}

pub(crate) fn take_until1_internal<T, I, Error: ParseError<I>>(
    i: I,
    t: T,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + FindSlice<T>,
    T: SliceLen,
{
    match i.find_slice(t) {
        None => Err(ErrMode::Incomplete(Needed::Unknown)),
        Some(0) => Err(ErrMode::from_error_kind(i, ErrorKind::TakeUntil)),
        Some(offset) => Ok(i.next_slice(offset)),
    }
}

/// Matches a byte string with escaped characters.
///
/// * The first argument matches the normal characters (it must not accept the control character)
/// * The second argument is the control character (like `\` in most languages)
/// * The third argument matches the escaped characters
/// # Example
/// ```
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, IResult};
/// # use winnow::character::streaming::digit1;
/// use winnow::bytes::streaming::escaped;
/// use winnow::character::streaming::one_of;
///
/// fn esc(s: &str) -> IResult<&str, &str> {
///   escaped(digit1, '\\', one_of("\"n\\"))(s)
/// }
///
/// assert_eq!(esc("123;"), Ok((";", "123")));
/// assert_eq!(esc("12\\\"34;"), Ok((";", "12\\\"34")));
/// ```
///
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::escaped`][crate::character::escaped] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::escaped` with input wrapped in `winnow::Partial`"
)]
pub fn escaped<I, Error, F, G, O1, O2>(
    mut normal: F,
    control_char: char,
    mut escapable: G,
) -> impl FnMut(I) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + Offset,
    <I as Stream>::Token: crate::stream::AsChar,
    F: Parser<I, O1, Error>,
    G: Parser<I, O2, Error>,
    Error: ParseError<I>,
{
    move |input: I| escaped_internal(input, &mut normal, control_char, &mut escapable)
}

pub(crate) fn escaped_internal<I, Error, F, G, O1, O2>(
    input: I,
    normal: &mut F,
    control_char: char,
    escapable: &mut G,
) -> IResult<I, <I as Stream>::Slice, Error>
where
    I: Stream + Offset,
    <I as Stream>::Token: crate::stream::AsChar,
    F: Parser<I, O1, Error>,
    G: Parser<I, O2, Error>,
    Error: ParseError<I>,
{
    use crate::stream::AsChar;

    let mut i = input.clone();

    while i.eof_offset() > 0 {
        let current_len = i.eof_offset();

        match normal.parse_next(i.clone()) {
            Ok((i2, _)) => {
                if i2.eof_offset() == 0 {
                    return Err(ErrMode::Incomplete(Needed::Unknown));
                } else if i2.eof_offset() == current_len {
                    let offset = input.offset_to(&i2);
                    return Ok(input.next_slice(offset));
                } else {
                    i = i2;
                }
            }
            Err(ErrMode::Backtrack(_)) => {
                if i.next_token().expect("eof_offset > 0").1.as_char() == control_char {
                    let next = control_char.len_utf8();
                    if next >= i.eof_offset() {
                        return Err(ErrMode::Incomplete(Needed::new(1)));
                    } else {
                        match escapable.parse_next(i.next_slice(next).0) {
                            Ok((i2, _)) => {
                                if i2.eof_offset() == 0 {
                                    return Err(ErrMode::Incomplete(Needed::Unknown));
                                } else {
                                    i = i2;
                                }
                            }
                            Err(e) => return Err(e),
                        }
                    }
                } else {
                    let offset = input.offset_to(&i);
                    return Ok(input.next_slice(offset));
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    Err(ErrMode::Incomplete(Needed::Unknown))
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
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, IResult};
/// # use std::str::from_utf8;
/// use winnow::bytes::streaming::{escaped_transform, tag};
/// use winnow::character::streaming::alpha1;
/// use winnow::branch::alt;
/// use winnow::combinator::value;
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
/// assert_eq!(parser("ab\\\"cd\""), Ok(("\"", String::from("ab\"cd"))));
/// ```
#[cfg(feature = "alloc")]
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::escaped_transform`][crate::character::escaped_transform] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::character::escaped_transform` with input wrapped in `winnow::Partial`"
)]
pub fn escaped_transform<I, Error, F, G, Output>(
    mut normal: F,
    control_char: char,
    mut transform: G,
) -> impl FnMut(I) -> IResult<I, Output, Error>
where
    I: Stream + Offset,
    <I as Stream>::Token: crate::stream::AsChar,
    Output: crate::stream::Accumulate<<I as Stream>::Slice>,
    F: Parser<I, <I as Stream>::Slice, Error>,
    G: Parser<I, <I as Stream>::Slice, Error>,
    Error: ParseError<I>,
{
    move |input: I| escaped_transform_internal(input, &mut normal, control_char, &mut transform)
}

#[cfg(feature = "alloc")]
pub(crate) fn escaped_transform_internal<I, Error, F, G, Output>(
    input: I,
    normal: &mut F,
    control_char: char,
    transform: &mut G,
) -> IResult<I, Output, Error>
where
    I: Stream + Offset,
    <I as Stream>::Token: crate::stream::AsChar,
    Output: crate::stream::Accumulate<<I as Stream>::Slice>,
    F: Parser<I, <I as Stream>::Slice, Error>,
    G: Parser<I, <I as Stream>::Slice, Error>,
    Error: ParseError<I>,
{
    use crate::stream::AsChar;

    let mut offset = 0;
    let mut res = Output::initial(Some(input.eof_offset()));

    let i = input.clone();

    while offset < i.eof_offset() {
        let current_len = i.eof_offset();
        let remainder = i.next_slice(offset).0;
        match normal.parse_next(remainder.clone()) {
            Ok((i2, o)) => {
                res.accumulate(o);
                if i2.eof_offset() == 0 {
                    return Err(ErrMode::Incomplete(Needed::Unknown));
                } else if i2.eof_offset() == current_len {
                    return Ok((remainder, res));
                } else {
                    offset = input.offset_to(&i2);
                }
            }
            Err(ErrMode::Backtrack(_)) => {
                if remainder.next_token().expect("eof_offset > 0").1.as_char() == control_char {
                    let next = offset + control_char.len_utf8();
                    let eof_offset = input.eof_offset();

                    if next >= eof_offset {
                        return Err(ErrMode::Incomplete(Needed::Unknown));
                    } else {
                        match transform.parse_next(i.next_slice(next).0) {
                            Ok((i2, o)) => {
                                res.accumulate(o);
                                if i2.eof_offset() == 0 {
                                    return Err(ErrMode::Incomplete(Needed::Unknown));
                                } else {
                                    offset = input.offset_to(&i2);
                                }
                            }
                            Err(e) => return Err(e),
                        }
                    }
                } else {
                    return Ok((remainder, res));
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(ErrMode::Incomplete(Needed::Unknown))
}

#[cfg(test)]
mod tests {
    use crate::character::streaming::{
        alpha1 as alpha, alphanumeric1 as alphanumeric, digit1 as digit, hex_digit1 as hex_digit,
        multispace1 as multispace, oct_digit1 as oct_digit, space1 as space,
    };
    use crate::error::ErrorKind;
    use crate::error::{ErrMode, Needed};
    use crate::stream::AsChar;
    use crate::IResult;

    #[test]
    fn is_a() {
        use crate::bytes::streaming::is_a;

        fn a_or_b(i: &[u8]) -> IResult<&[u8], &[u8]> {
            is_a("ab")(i)
        }

        let a = &b"abcd"[..];
        assert_eq!(a_or_b(a), Ok((&b"cd"[..], &b"ab"[..])));

        let b = &b"bcde"[..];
        assert_eq!(a_or_b(b), Ok((&b"cde"[..], &b"b"[..])));

        let c = &b"cdef"[..];
        assert_eq!(
            a_or_b(c),
            Err(ErrMode::Backtrack(error_position!(c, ErrorKind::IsA)))
        );

        let d = &b"bacdef"[..];
        assert_eq!(a_or_b(d), Ok((&b"cdef"[..], &b"ba"[..])));
    }

    #[test]
    fn is_not() {
        use crate::bytes::streaming::is_not;

        fn a_or_b(i: &[u8]) -> IResult<&[u8], &[u8]> {
            is_not("ab")(i)
        }

        let a = &b"cdab"[..];
        assert_eq!(a_or_b(a), Ok((&b"ab"[..], &b"cd"[..])));

        let b = &b"cbde"[..];
        assert_eq!(a_or_b(b), Ok((&b"bde"[..], &b"c"[..])));

        let c = &b"abab"[..];
        assert_eq!(
            a_or_b(c),
            Err(ErrMode::Backtrack(error_position!(c, ErrorKind::IsNot)))
        );

        let d = &b"cdefba"[..];
        assert_eq!(a_or_b(d), Ok((&b"ba"[..], &b"cdef"[..])));

        let e = &b"e"[..];
        assert_eq!(a_or_b(e), Err(ErrMode::Incomplete(Needed::new(1))));
    }

    #[test]
    fn take_until_incomplete() {
        use crate::bytes::streaming::take_until;
        fn y(i: &[u8]) -> IResult<&[u8], &[u8]> {
            take_until("end")(i)
        }
        assert_eq!(y(&b"nd"[..]), Err(ErrMode::Incomplete(Needed::Unknown)));
        assert_eq!(y(&b"123"[..]), Err(ErrMode::Incomplete(Needed::Unknown)));
        assert_eq!(y(&b"123en"[..]), Err(ErrMode::Incomplete(Needed::Unknown)));
    }

    #[test]
    fn take_until_incomplete_s() {
        use crate::bytes::streaming::take_until;
        fn ys(i: &str) -> IResult<&str, &str> {
            take_until("end")(i)
        }
        assert_eq!(ys("123en"), Err(ErrMode::Incomplete(Needed::Unknown)));
    }

    #[test]
    fn recognize() {
        use crate::bytes::streaming::{tag, take};
        use crate::combinator::recognize;
        use crate::sequence::delimited;

        fn x(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(delimited(tag("<!--"), take(5_usize), tag("-->")))(i)
        }
        let r = x(&b"<!-- abc --> aaa"[..]);
        assert_eq!(r, Ok((&b" aaa"[..], &b"<!-- abc -->"[..])));

        let semicolon = &b";"[..];

        fn ya(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(alpha)(i)
        }
        let ra = ya(&b"abc;"[..]);
        assert_eq!(ra, Ok((semicolon, &b"abc"[..])));

        fn yd(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(digit)(i)
        }
        let rd = yd(&b"123;"[..]);
        assert_eq!(rd, Ok((semicolon, &b"123"[..])));

        fn yhd(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(hex_digit)(i)
        }
        let rhd = yhd(&b"123abcDEF;"[..]);
        assert_eq!(rhd, Ok((semicolon, &b"123abcDEF"[..])));

        fn yod(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(oct_digit)(i)
        }
        let rod = yod(&b"1234567;"[..]);
        assert_eq!(rod, Ok((semicolon, &b"1234567"[..])));

        fn yan(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(alphanumeric)(i)
        }
        let ran = yan(&b"123abc;"[..]);
        assert_eq!(ran, Ok((semicolon, &b"123abc"[..])));

        fn ys(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(space)(i)
        }
        let rs = ys(&b" \t;"[..]);
        assert_eq!(rs, Ok((semicolon, &b" \t"[..])));

        fn yms(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(multispace)(i)
        }
        let rms = yms(&b" \t\r\n;"[..]);
        assert_eq!(rms, Ok((semicolon, &b" \t\r\n"[..])));
    }

    #[test]
    fn take_while() {
        use crate::bytes::streaming::take_while;

        fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
            take_while(AsChar::is_alpha)(i)
        }
        let a = b"";
        let b = b"abcd";
        let c = b"abcd123";
        let d = b"123";

        assert_eq!(f(&a[..]), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f(&b[..]), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f(&c[..]), Ok((&d[..], &b[..])));
        assert_eq!(f(&d[..]), Ok((&d[..], &a[..])));
    }

    #[test]
    fn take_while1() {
        use crate::bytes::streaming::take_while1;

        fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
            take_while1(AsChar::is_alpha)(i)
        }
        let a = b"";
        let b = b"abcd";
        let c = b"abcd123";
        let d = b"123";

        assert_eq!(f(&a[..]), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f(&b[..]), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f(&c[..]), Ok((&b"123"[..], &b[..])));
        assert_eq!(
            f(&d[..]),
            Err(ErrMode::Backtrack(error_position!(
                &d[..],
                ErrorKind::TakeWhile1
            )))
        );
    }

    #[test]
    fn take_while_m_n() {
        use crate::bytes::streaming::take_while_m_n;

        fn x(i: &[u8]) -> IResult<&[u8], &[u8]> {
            take_while_m_n(2, 4, AsChar::is_alpha)(i)
        }
        let a = b"";
        let b = b"a";
        let c = b"abc";
        let d = b"abc123";
        let e = b"abcde";
        let f = b"123";

        assert_eq!(x(&a[..]), Err(ErrMode::Incomplete(Needed::new(2))));
        assert_eq!(x(&b[..]), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(x(&c[..]), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(x(&d[..]), Ok((&b"123"[..], &c[..])));
        assert_eq!(x(&e[..]), Ok((&b"e"[..], &b"abcd"[..])));
        assert_eq!(
            x(&f[..]),
            Err(ErrMode::Backtrack(error_position!(
                &f[..],
                ErrorKind::TakeWhileMN
            )))
        );
    }

    #[test]
    fn take_till() {
        use crate::bytes::streaming::take_till;

        fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
            take_till(AsChar::is_alpha)(i)
        }
        let a = b"";
        let b = b"abcd";
        let c = b"123abcd";
        let d = b"123";

        assert_eq!(f(&a[..]), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f(&b[..]), Ok((&b"abcd"[..], &b""[..])));
        assert_eq!(f(&c[..]), Ok((&b"abcd"[..], &b"123"[..])));
        assert_eq!(f(&d[..]), Err(ErrMode::Incomplete(Needed::new(1))));
    }

    #[test]
    fn take_till1() {
        use crate::bytes::streaming::take_till1;

        fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
            take_till1(AsChar::is_alpha)(i)
        }
        let a = b"";
        let b = b"abcd";
        let c = b"123abcd";
        let d = b"123";

        assert_eq!(f(&a[..]), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(
            f(&b[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b[..],
                ErrorKind::TakeTill1
            )))
        );
        assert_eq!(f(&c[..]), Ok((&b"abcd"[..], &b"123"[..])));
        assert_eq!(f(&d[..]), Err(ErrMode::Incomplete(Needed::new(1))));
    }

    #[test]
    fn take_while_utf8() {
        use crate::bytes::streaming::take_while;

        fn f(i: &str) -> IResult<&str, &str> {
            take_while(|c| c != '點')(i)
        }

        assert_eq!(f(""), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f("abcd"), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f("abcd點"), Ok(("點", "abcd")));
        assert_eq!(f("abcd點a"), Ok(("點a", "abcd")));

        fn g(i: &str) -> IResult<&str, &str> {
            take_while(|c| c == '點')(i)
        }

        assert_eq!(g(""), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(g("點abcd"), Ok(("abcd", "點")));
        assert_eq!(g("點點點a"), Ok(("a", "點點點")));
    }

    #[test]
    fn take_till_utf8() {
        use crate::bytes::streaming::take_till;

        fn f(i: &str) -> IResult<&str, &str> {
            take_till(|c| c == '點')(i)
        }

        assert_eq!(f(""), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f("abcd"), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(f("abcd點"), Ok(("點", "abcd")));
        assert_eq!(f("abcd點a"), Ok(("點a", "abcd")));

        fn g(i: &str) -> IResult<&str, &str> {
            take_till(|c| c != '點')(i)
        }

        assert_eq!(g(""), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(g("點abcd"), Ok(("abcd", "點")));
        assert_eq!(g("點點點a"), Ok(("a", "點點點")));
    }

    #[test]
    fn take_utf8() {
        use crate::bytes::streaming::{take, take_while};

        fn f(i: &str) -> IResult<&str, &str> {
            take(3_usize)(i)
        }

        assert_eq!(f(""), Err(ErrMode::Incomplete(Needed::Unknown)));
        assert_eq!(f("ab"), Err(ErrMode::Incomplete(Needed::Unknown)));
        assert_eq!(f("點"), Err(ErrMode::Incomplete(Needed::Unknown)));
        assert_eq!(f("ab點cd"), Ok(("cd", "ab點")));
        assert_eq!(f("a點bcd"), Ok(("cd", "a點b")));
        assert_eq!(f("a點b"), Ok(("", "a點b")));

        fn g(i: &str) -> IResult<&str, &str> {
            take_while(|c| c == '點')(i)
        }

        assert_eq!(g(""), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_eq!(g("點abcd"), Ok(("abcd", "點")));
        assert_eq!(g("點點點a"), Ok(("a", "點點點")));
    }

    #[test]
    fn take_while_m_n_utf8() {
        use crate::bytes::streaming::take_while_m_n;

        fn parser(i: &str) -> IResult<&str, &str> {
            take_while_m_n(1, 1, |c| c == 'A' || c == '😃')(i)
        }
        assert_eq!(parser("A!"), Ok(("!", "A")));
        assert_eq!(parser("😃!"), Ok(("!", "😃")));
    }

    #[test]
    fn take_while_m_n_utf8_full_match() {
        use crate::bytes::streaming::take_while_m_n;

        fn parser(i: &str) -> IResult<&str, &str> {
            take_while_m_n(1, 1, |c: char| c.is_alphabetic())(i)
        }
        assert_eq!(parser("øn"), Ok(("n", "ø")));
    }

    #[test]
    #[cfg(feature = "std")]
    fn recognize_take_while() {
        use crate::bytes::streaming::take_while;
        use crate::combinator::recognize;

        fn x(i: &[u8]) -> IResult<&[u8], &[u8]> {
            take_while(AsChar::is_alphanum)(i)
        }
        fn y(i: &[u8]) -> IResult<&[u8], &[u8]> {
            recognize(x)(i)
        }
        assert_eq!(x(&b"ab."[..]), Ok((&b"."[..], &b"ab"[..])));
        println!("X: {:?}", x(&b"ab"[..]));
        assert_eq!(y(&b"ab."[..]), Ok((&b"."[..], &b"ab"[..])));
    }

    #[test]
    fn length_bytes() {
        use crate::Partial;
        use crate::{bytes::streaming::tag, multi::length_data, number::streaming::le_u8};

        fn x(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
            length_data(le_u8)(i)
        }
        assert_eq!(
            x(Partial::new(b"\x02..>>")),
            Ok((Partial::new(&b">>"[..]), &b".."[..]))
        );
        assert_eq!(
            x(Partial::new(b"\x02..")),
            Ok((Partial::new(&[][..]), &b".."[..]))
        );
        assert_eq!(
            x(Partial::new(b"\x02.")),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            x(Partial::new(b"\x02")),
            Err(ErrMode::Incomplete(Needed::new(2)))
        );

        fn y(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
            let (i, _) = tag("magic")(i)?;
            length_data(le_u8)(i)
        }
        assert_eq!(
            y(Partial::new(b"magic\x02..>>")),
            Ok((Partial::new(&b">>"[..]), &b".."[..]))
        );
        assert_eq!(
            y(Partial::new(b"magic\x02..")),
            Ok((Partial::new(&[][..]), &b".."[..]))
        );
        assert_eq!(
            y(Partial::new(b"magic\x02.")),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            y(Partial::new(b"magic\x02")),
            Err(ErrMode::Incomplete(Needed::new(2)))
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn case_insensitive() {
        use crate::bytes::streaming::tag_no_case;

        fn test(i: &[u8]) -> IResult<&[u8], &[u8]> {
            tag_no_case("ABcd")(i)
        }
        assert_eq!(test(&b"aBCdefgh"[..]), Ok((&b"efgh"[..], &b"aBCd"[..])));
        assert_eq!(test(&b"abcdefgh"[..]), Ok((&b"efgh"[..], &b"abcd"[..])));
        assert_eq!(test(&b"ABCDefgh"[..]), Ok((&b"efgh"[..], &b"ABCD"[..])));
        assert_eq!(test(&b"ab"[..]), Err(ErrMode::Incomplete(Needed::new(2))));
        assert_eq!(
            test(&b"Hello"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"Hello"[..],
                ErrorKind::Tag
            )))
        );
        assert_eq!(
            test(&b"Hel"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"Hel"[..],
                ErrorKind::Tag
            )))
        );

        fn test2(i: &str) -> IResult<&str, &str> {
            tag_no_case("ABcd")(i)
        }
        assert_eq!(test2("aBCdefgh"), Ok(("efgh", "aBCd")));
        assert_eq!(test2("abcdefgh"), Ok(("efgh", "abcd")));
        assert_eq!(test2("ABCDefgh"), Ok(("efgh", "ABCD")));
        assert_eq!(test2("ab"), Err(ErrMode::Incomplete(Needed::new(2))));
        assert_eq!(
            test2("Hello"),
            Err(ErrMode::Backtrack(error_position!("Hello", ErrorKind::Tag)))
        );
        assert_eq!(
            test2("Hel"),
            Err(ErrMode::Backtrack(error_position!("Hel", ErrorKind::Tag)))
        );
    }

    #[test]
    fn tag_fixed_size_array() {
        use crate::bytes::streaming::tag;

        fn test(i: &[u8]) -> IResult<&[u8], &[u8]> {
            tag([0x42])(i)
        }
        fn test2(i: &[u8]) -> IResult<&[u8], &[u8]> {
            tag(&[0x42])(i)
        }
        let input = [0x42, 0x00];
        assert_eq!(test(&input), Ok((&b"\x00"[..], &b"\x42"[..])));
        assert_eq!(test2(&input), Ok((&b"\x00"[..], &b"\x42"[..])));
    }
}
