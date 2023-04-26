//! Parsers recognizing numbers

#![allow(clippy::match_same_arms)]

#[cfg(test)]
mod tests;

use crate::bytes::take;
use crate::error::ErrMode;
use crate::error::ErrorKind;
use crate::error::Needed;
use crate::error::ParseError;
use crate::lib::std::ops::{Add, Shl};
use crate::stream::{AsBytes, Stream, StreamIsPartial};
use crate::trace::trace;
use crate::IResult;
use crate::Parser;

/// Configurable endianness
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Endianness {
    /// Big endian
    Big,
    /// Little endian
    Little,
    /// Will match the host's endianness
    Native,
}

/// Recognizes an unsigned 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u8;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u8> {
///     be_u8.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Token))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_u8;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u8> {
///     be_u8.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_u8<I, E: ParseError<I>>(input: I) -> IResult<I, u8, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
{
    u8(input)
}

/// Recognizes a big endian unsigned 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u16;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u16> {
///     be_u16.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_u16;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u16> {
///     be_u16.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0001)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_u16<I, E: ParseError<I>>(input: I) -> IResult<I, u16, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_u16", move |input: I| be_uint(input, 2)).parse_next(input)
}

/// Recognizes a big endian unsigned 3 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u24;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u32> {
///     be_u24.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_u24;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u32> {
///     be_u24.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x000102)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn be_u24<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_u23", move |input: I| be_uint(input, 3)).parse_next(input)
}

/// Recognizes a big endian unsigned 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u32;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u32> {
///     be_u32.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_u32;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u32> {
///     be_u32.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x00010203)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_u32<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_u32", move |input: I| be_uint(input, 4)).parse_next(input)
}

/// Recognizes a big endian unsigned 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u64;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u64> {
///     be_u64.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_u64;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u64> {
///     be_u64.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0001020304050607)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_u64<I, E: ParseError<I>>(input: I) -> IResult<I, u64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_u64", move |input: I| be_uint(input, 8)).parse_next(input)
}

/// Recognizes a big endian unsigned 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u128;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u128> {
///     be_u128.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_u128;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u128> {
///     be_u128.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x00010203040506070809101112131415)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn be_u128<I, E: ParseError<I>>(input: I) -> IResult<I, u128, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_u128", move |input: I| be_uint(input, 16)).parse_next(input)
}

#[inline]
fn be_uint<I, Uint, E: ParseError<I>>(input: I, bound: usize) -> IResult<I, Uint, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
    Uint: Default + Shl<u8, Output = Uint> + Add<Uint, Output = Uint> + From<u8>,
{
    debug_assert_ne!(bound, 1, "to_be_uint needs extra work to avoid overflow");
    take(bound)
        .map(|n: <I as Stream>::Slice| to_be_uint(n.as_bytes()))
        .parse_next(input)
}

#[inline]
fn to_be_uint<Uint>(number: &[u8]) -> Uint
where
    Uint: Default + Shl<u8, Output = Uint> + Add<Uint, Output = Uint> + From<u8>,
{
    let mut res = Uint::default();
    for byte in number.iter().copied() {
        res = (res << 8) + byte.into();
    }

    res
}

/// Recognizes a signed 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i8;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i8> {
///     be_i8.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Token))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_i8;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i8> {
///       be_i8.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_i8<I, E: ParseError<I>>(input: I) -> IResult<I, i8, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
{
    i8(input)
}

/// Recognizes a big endian signed 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i16;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i16> {
///     be_i16.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_i16;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i16> {
///       be_i16.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0001)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn be_i16<I, E: ParseError<I>>(input: I) -> IResult<I, i16, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_i16", move |input: I| {
        be_uint::<_, u16, _>(input, 2).map(|(i, n)| (i, n as i16))
    })
    .parse_next(input)
}

/// Recognizes a big endian signed 3 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i24;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i32> {
///     be_i24.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_i24;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i32> {
///       be_i24.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x000102)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_i24<I, E: ParseError<I>>(input: I) -> IResult<I, i32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_i24", move |input: I| {
        be_uint::<_, u32, _>(input, 3).map(|(i, n)| {
            // Same as the unsigned version but we need to sign-extend manually here
            let n = if n & 0x80_00_00 != 0 {
                (n | 0xff_00_00_00) as i32
            } else {
                n as i32
            };
            (i, n)
        })
    })
    .parse_next(input)
}

/// Recognizes a big endian signed 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i32;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i32> {
///       be_i32.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_i32;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i32> {
///       be_i32.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x00010203)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(4))));
/// ```
#[inline(always)]
pub fn be_i32<I, E: ParseError<I>>(input: I) -> IResult<I, i32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_i32", move |input: I| {
        be_uint::<_, u32, _>(input, 4).map(|(i, n)| (i, n as i32))
    })
    .parse_next(input)
}

/// Recognizes a big endian signed 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i64;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i64> {
///       be_i64.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_i64;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i64> {
///       be_i64.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0001020304050607)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_i64<I, E: ParseError<I>>(input: I) -> IResult<I, i64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_i64", move |input: I| {
        be_uint::<_, u64, _>(input, 8).map(|(i, n)| (i, n as i64))
    })
    .parse_next(input)
}

/// Recognizes a big endian signed 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i128;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i128> {
///       be_i128.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_i128;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i128> {
///       be_i128.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x00010203040506070809101112131415)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn be_i128<I, E: ParseError<I>>(input: I) -> IResult<I, i128, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_i128", move |input: I| {
        be_uint::<_, u128, _>(input, 16).map(|(i, n)| (i, n as i128))
    })
    .parse_next(input)
}

/// Recognizes an unsigned 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u8;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u8> {
///       le_u8.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Token))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_u8;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u8> {
///       le_u8.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_u8<I, E: ParseError<I>>(input: I) -> IResult<I, u8, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
{
    u8(input)
}

/// Recognizes a little endian unsigned 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u16;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u16> {
///       le_u16.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_u16;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u16> {
///       le_u16::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_u16<I, E: ParseError<I>>(input: I) -> IResult<I, u16, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_u16", move |input: I| le_uint(input, 2)).parse_next(input)
}

/// Recognizes a little endian unsigned 3 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u24;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u32> {
///       le_u24.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_u24;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u32> {
///       le_u24::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x020100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn le_u24<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_u24", move |input: I| le_uint(input, 3)).parse_next(input)
}

/// Recognizes a little endian unsigned 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u32;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u32> {
///       le_u32.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_u32;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u32> {
///       le_u32::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x03020100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_u32<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_u32", move |input: I| le_uint(input, 4)).parse_next(input)
}

/// Recognizes a little endian unsigned 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u64;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u64> {
///       le_u64.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_u64;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u64> {
///       le_u64::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0706050403020100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_u64<I, E: ParseError<I>>(input: I) -> IResult<I, u64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_u64", move |input: I| le_uint(input, 8)).parse_next(input)
}

/// Recognizes a little endian unsigned 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u128;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u128> {
///       le_u128.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_u128;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u128> {
///       le_u128::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x15141312111009080706050403020100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn le_u128<I, E: ParseError<I>>(input: I) -> IResult<I, u128, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_u128", move |input: I| le_uint(input, 16)).parse_next(input)
}

#[inline]
fn le_uint<I, Uint, E: ParseError<I>>(input: I, bound: usize) -> IResult<I, Uint, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
    Uint: Default + Shl<u8, Output = Uint> + Add<Uint, Output = Uint> + From<u8>,
{
    take(bound)
        .map(|n: <I as Stream>::Slice| to_le_uint(n.as_bytes()))
        .parse_next(input)
}

#[inline]
fn to_le_uint<Uint>(number: &[u8]) -> Uint
where
    Uint: Default + Shl<u8, Output = Uint> + Add<Uint, Output = Uint> + From<u8>,
{
    let mut res = Uint::default();
    for (index, byte) in number.iter_offsets() {
        res = res + (Uint::from(byte) << (8 * index as u8));
    }

    res
}

/// Recognizes a signed 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i8;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i8> {
///       le_i8.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Token))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_i8;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i8> {
///       le_i8.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_i8<I, E: ParseError<I>>(input: I) -> IResult<I, i8, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
{
    i8(input)
}

/// Recognizes a little endian signed 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i16;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i16> {
///       le_i16.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_i16;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i16> {
///       le_i16::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_i16<I, E: ParseError<I>>(input: I) -> IResult<I, i16, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_i16", move |input: I| {
        le_uint::<_, u16, _>(input, 2).map(|(i, n)| (i, n as i16))
    })
    .parse_next(input)
}

/// Recognizes a little endian signed 3 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i24;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i32> {
///       le_i24.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_i24;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i32> {
///       le_i24::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x020100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn le_i24<I, E: ParseError<I>>(input: I) -> IResult<I, i32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_i24", move |input: I| {
        le_uint::<_, u32, _>(input, 3).map(|(i, n)| {
            // Same as the unsigned version but we need to sign-extend manually here
            let n = if n & 0x80_00_00 != 0 {
                (n | 0xff_00_00_00) as i32
            } else {
                n as i32
            };
            (i, n)
        })
    })
    .parse_next(input)
}

/// Recognizes a little endian signed 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i32;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i32> {
///       le_i32.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_i32;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i32> {
///       le_i32::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x03020100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_i32<I, E: ParseError<I>>(input: I) -> IResult<I, i32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_i32", move |input: I| {
        le_uint::<_, u32, _>(input, 4).map(|(i, n)| (i, n as i32))
    })
    .parse_next(input)
}

/// Recognizes a little endian signed 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i64;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i64> {
///       le_i64.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_i64;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i64> {
///       le_i64::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0706050403020100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_i64<I, E: ParseError<I>>(input: I) -> IResult<I, i64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_i64", move |input: I| {
        le_uint::<_, u64, _>(input, 8).map(|(i, n)| (i, n as i64))
    })
    .parse_next(input)
}

/// Recognizes a little endian signed 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i128;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i128> {
///       le_i128.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_i128;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i128> {
///       le_i128::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x15141312111009080706050403020100)));
/// assert_eq!(parser(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn le_i128<I, E: ParseError<I>>(input: I) -> IResult<I, i128, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_i128", move |input: I| {
        le_uint::<_, u128, _>(input, 16).map(|(i, n)| (i, n as i128))
    })
    .parse_next(input)
}

/// Recognizes an unsigned 1 byte integer
///
/// **Note:** that endianness does not apply to 1 byte numbers.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::u8;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], u8> {
///       u8.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Token))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u8;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u8> {
///       u8::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"\x03abcefg"[..]), 0x00)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn u8<I, E: ParseError<I>>(input: I) -> IResult<I, u8, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
{
    trace("u8", move |input: I| {
        if input.is_partial() {
            streaming_u8(input)
        } else {
            complete_u8(input)
        }
    })
    .parse_next(input)
}

#[inline]
pub(crate) fn streaming_u8<I, E: ParseError<I>>(input: I) -> IResult<I, u8, E>
where
    I: Stream<Token = u8>,
{
    input
        .next_token()
        .ok_or_else(|| ErrMode::Incomplete(Needed::new(1)))
}

pub(crate) fn complete_u8<I, E: ParseError<I>>(input: I) -> IResult<I, u8, E>
where
    I: Stream<Token = u8>,
{
    input
        .next_token()
        .ok_or_else(|| ErrMode::Backtrack(E::from_error_kind(input, ErrorKind::Token)))
}

/// Recognizes an unsigned 2 bytes integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u16 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u16 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::u16;
///
/// let be_u16 = |s| {
///     u16(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(be_u16(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_u16 = |s| {
///     u16(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(le_u16(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u16;
///
/// let be_u16 = |s| {
///     u16::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u16(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0003)));
/// assert_eq!(be_u16(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// let le_u16 = |s| {
///     u16::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u16(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0300)));
/// assert_eq!(le_u16(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn u16<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, u16, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_u16,
            crate::number::Endianness::Little => le_u16,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_u16,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_u16,
        }
    }(input)
}

/// Recognizes an unsigned 3 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u24 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u24 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::u24;
///
/// let be_u24 = |s| {
///     u24(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(be_u24(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_u24 = |s| {
///     u24(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(le_u24(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u24;
///
/// let be_u24 = |s| {
///     u24::<_,Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u24(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x000305)));
/// assert_eq!(be_u24(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
///
/// let le_u24 = |s| {
///     u24::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u24(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x050300)));
/// assert_eq!(le_u24(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn u24<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, u32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_u24,
            crate::number::Endianness::Little => le_u24,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_u24,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_u24,
        }
    }(input)
}

/// Recognizes an unsigned 4 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u32 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u32 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::u32;
///
/// let be_u32 = |s| {
///     u32(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(be_u32(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_u32 = |s| {
///     u32(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(le_u32(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u32;
///
/// let be_u32 = |s| {
///     u32::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u32(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00030507)));
/// assert_eq!(be_u32(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
///
/// let le_u32 = |s| {
///     u32::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u32(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07050300)));
/// assert_eq!(le_u32(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn u32<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, u32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_u32,
            crate::number::Endianness::Little => le_u32,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_u32,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_u32,
        }
    }(input)
}

/// Recognizes an unsigned 8 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u64 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u64 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::u64;
///
/// let be_u64 = |s| {
///     u64(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(be_u64(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_u64 = |s| {
///     u64(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(le_u64(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u64;
///
/// let be_u64 = |s| {
///     u64::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u64(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0001020304050607)));
/// assert_eq!(be_u64(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
///
/// let le_u64 = |s| {
///     u64::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u64(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0706050403020100)));
/// assert_eq!(le_u64(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn u64<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, u64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_u64,
            crate::number::Endianness::Little => le_u64,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_u64,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_u64,
        }
    }(input)
}

/// Recognizes an unsigned 16 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u128 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u128 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::u128;
///
/// let be_u128 = |s| {
///     u128(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(be_u128(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_u128 = |s| {
///     u128(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(le_u128(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u128;
///
/// let be_u128 = |s| {
///     u128::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_u128(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00010203040506070001020304050607)));
/// assert_eq!(be_u128(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
///
/// let le_u128 = |s| {
///     u128::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_u128(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07060504030201000706050403020100)));
/// assert_eq!(le_u128(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn u128<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, u128, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_u128,
            crate::number::Endianness::Little => le_u128,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_u128,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_u128,
        }
    }(input)
}

/// Recognizes a signed 1 byte integer
///
/// **Note:** that endianness does not apply to 1 byte numbers.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::i8;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], i8> {
///       i8.parse_next(s)
/// }
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Token))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i8;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, i8> {
///       i8.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"\x03abcefg"[..]), 0x00)));
/// assert_eq!(parser(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn i8<I, E: ParseError<I>>(input: I) -> IResult<I, i8, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
{
    trace("i8", move |input: I| {
        if input.is_partial() {
            streaming_u8(input)
        } else {
            complete_u8(input)
        }
        .map(|(i, n)| (i, n as i8))
    })
    .parse_next(input)
}

/// Recognizes a signed 2 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i16 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i16 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::i16;
///
/// let be_i16 = |s| {
///     i16(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(be_i16(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_i16 = |s| {
///     i16(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(le_i16(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i16;
///
/// let be_i16 = |s| {
///     i16::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i16(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0003)));
/// assert_eq!(be_i16(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// let le_i16 = |s| {
///     i16::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i16(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0300)));
/// assert_eq!(le_i16(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn i16<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, i16, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_i16,
            crate::number::Endianness::Little => le_i16,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_i16,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_i16,
        }
    }(input)
}

/// Recognizes a signed 3 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i24 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i24 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::i24;
///
/// let be_i24 = |s| {
///     i24(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(be_i24(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_i24 = |s| {
///     i24(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(le_i24(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i24;
///
/// let be_i24 = |s| {
///     i24::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i24(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x000305)));
/// assert_eq!(be_i24(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
///
/// let le_i24 = |s| {
///     i24::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i24(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x050300)));
/// assert_eq!(le_i24(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn i24<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, i32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_i24,
            crate::number::Endianness::Little => le_i24,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_i24,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_i24,
        }
    }(input)
}

/// Recognizes a signed 4 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i32 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i32 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::i32;
///
/// let be_i32 = |s| {
///     i32(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(be_i32(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_i32 = |s| {
///     i32(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(le_i32(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i32;
///
/// let be_i32 = |s| {
///     i32::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i32(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00030507)));
/// assert_eq!(be_i32(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
///
/// let le_i32 = |s| {
///     i32::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i32(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07050300)));
/// assert_eq!(le_i32(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn i32<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, i32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_i32,
            crate::number::Endianness::Little => le_i32,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_i32,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_i32,
        }
    }(input)
}

/// Recognizes a signed 8 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i64 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i64 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::i64;
///
/// let be_i64 = |s| {
///     i64(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(be_i64(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_i64 = |s| {
///     i64(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(le_i64(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i64;
///
/// let be_i64 = |s| {
///     i64::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i64(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0001020304050607)));
/// assert_eq!(be_i64(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
///
/// let le_i64 = |s| {
///     i64::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i64(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0706050403020100)));
/// assert_eq!(le_i64(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn i64<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, i64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_i64,
            crate::number::Endianness::Little => le_i64,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_i64,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_i64,
        }
    }(input)
}

/// Recognizes a signed 16 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i128 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i128 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::i128;
///
/// let be_i128 = |s| {
///     i128(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(be_i128(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
///
/// let le_i128 = |s| {
///     i128(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(le_i128(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i128;
///
/// let be_i128 = |s| {
///     i128::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_i128(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00010203040506070001020304050607)));
/// assert_eq!(be_i128(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
///
/// let le_i128 = |s| {
///     i128::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_i128(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07060504030201000706050403020100)));
/// assert_eq!(le_i128(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn i128<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, i128, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_i128,
            crate::number::Endianness::Little => le_i128,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_i128,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_i128,
        }
    }(input)
}

/// Recognizes a big endian 4 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_f32;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], f32> {
///       be_f32.parse_next(s)
/// }
///
/// assert_eq!(parser(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_f32;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, f32> {
///       be_f32.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&[0x40, 0x29, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 2.640625)));
/// assert_eq!(parser(Partial::new(&[0x01][..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_f32<I, E: ParseError<I>>(input: I) -> IResult<I, f32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_f32", move |input: I| {
        be_uint::<_, u32, _>(input, 4).map(|(i, n)| (i, f32::from_bits(n)))
    })
    .parse_next(input)
}

/// Recognizes a big endian 8 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_f64;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], f64> {
///       be_f64.parse_next(s)
/// }
///
/// assert_eq!(parser(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::be_f64;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, f64> {
///       be_f64::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(parser(Partial::new(&[0x01][..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_f64<I, E: ParseError<I>>(input: I) -> IResult<I, f64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_f64", move |input: I| {
        be_uint::<_, u64, _>(input, 8).map(|(i, n)| (i, f64::from_bits(n)))
    })
    .parse_next(input)
}

/// Recognizes a little endian 4 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_f32;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], f32> {
///       le_f32.parse_next(s)
/// }
///
/// assert_eq!(parser(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_f32;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, f32> {
///       le_f32::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&[0x00, 0x00, 0x48, 0x41][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(parser(Partial::new(&[0x01][..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_f32<I, E: ParseError<I>>(input: I) -> IResult<I, f32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("le_f32", move |input: I| {
        le_uint::<_, u32, _>(input, 4).map(|(i, n)| (i, f32::from_bits(n)))
    })
    .parse_next(input)
}

/// Recognizes a little endian 8 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_f64;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], f64> {
///       le_f64.parse_next(s)
/// }
///
/// assert_eq!(parser(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::number::le_f64;
///
/// fn parser(s: Partial<&[u8]>) -> IResult<Partial<&[u8]>, f64> {
///       le_f64::<_, Error<_>>.parse_next(s)
/// }
///
/// assert_eq!(parser(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x48, 0x41][..])), Ok((Partial::new(&b""[..]), 3145728.0)));
/// assert_eq!(parser(Partial::new(&[0x01][..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_f64<I, E: ParseError<I>>(input: I) -> IResult<I, f64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("be_f64", move |input: I| {
        le_uint::<_, u64, _>(input, 8).map(|(i, n)| (i, f64::from_bits(n)))
    })
    .parse_next(input)
}

/// Recognizes a 4 byte floating point number
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian f32 float,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian f32 float.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::f32;
///
/// let be_f32 = |s| {
///     f32(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_f32(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(be_f32(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
///
/// let le_f32 = |s| {
///     f32(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_f32(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(le_f32(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::f32;
///
/// let be_f32 = |s| {
///     f32::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_f32(Partial::new(&[0x41, 0x48, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(be_f32(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// let le_f32 = |s| {
///     f32::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_f32(Partial::new(&[0x00, 0x00, 0x48, 0x41][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(le_f32(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn f32<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, f32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_f32,
            crate::number::Endianness::Little => le_f32,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_f32,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_f32,
        }
    }(input)
}

/// Recognizes an 8 byte floating point number
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian f64 float,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian f64 float.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::number::f64;
///
/// let be_f64 = |s| {
///     f64(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_f64(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(be_f64(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
///
/// let le_f64 = |s| {
///     f64(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_f64(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(le_f64(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Slice))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::f64;
///
/// let be_f64 = |s| {
///     f64::<_, Error<_>>(winnow::number::Endianness::Big).parse_next(s)
/// };
///
/// assert_eq!(be_f64(Partial::new(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(be_f64(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(5))));
///
/// let le_f64 = |s| {
///     f64::<_, Error<_>>(winnow::number::Endianness::Little).parse_next(s)
/// };
///
/// assert_eq!(le_f64(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(le_f64(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(5))));
/// ```
#[inline(always)]
pub fn f64<I, E: ParseError<I>>(endian: crate::number::Endianness) -> impl Parser<I, f64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    move |input: I| {
        match endian {
            crate::number::Endianness::Big => be_f64,
            crate::number::Endianness::Little => le_f64,
            #[cfg(target_endian = "big")]
            crate::number::Endianness::Native => be_f64,
            #[cfg(target_endian = "little")]
            crate::number::Endianness::Native => le_f64,
        }
    }(input)
}
