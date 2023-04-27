//! Parsers recognizing numbers, complete input version

#![allow(deprecated)]
#![allow(clippy::match_same_arms)]

use crate::branch::alt;
use crate::bytes::complete::tag;
use crate::character::complete::{char, digit1, sign};
use crate::combinator::{cut_err, map, opt};
use crate::error::ParseError;
use crate::error::{make_error, ErrMode, ErrorKind};
use crate::lib::std::ops::{Add, Shl};
use crate::sequence::{pair, tuple};
use crate::stream::{AsBStr, AsBytes, AsChar, Compare, Offset, SliceLen, Stream};
use crate::*;

/// Recognizes an unsigned 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_u8;
///
/// let parser = |s| {
///   be_u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_u8`][crate::number::be_u8]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_u8`")]
pub fn be_u8<I, E: ParseError<I>>(input: I) -> IResult<I, u8, E>
where
    I: Stream<Token = u8>,
{
    u8(input)
}

/// Recognizes a big endian unsigned 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_u16;
///
/// let parser = |s| {
///   be_u16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_u16`][crate::number::be_u16]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_u16`")]
pub fn be_u16<I, E: ParseError<I>>(input: I) -> IResult<I, u16, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_uint(input, 2)
}

/// Recognizes a big endian unsigned 3 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_u24;
///
/// let parser = |s| {
///   be_u24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_u24`][crate::number::be_u24]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_u24`")]
pub fn be_u24<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_uint(input, 3)
}

/// Recognizes a big endian unsigned 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_u32;
///
/// let parser = |s| {
///   be_u32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_u32`][crate::number::be_u32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_u32`")]
pub fn be_u32<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_uint(input, 4)
}

/// Recognizes a big endian unsigned 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_u64;
///
/// let parser = |s| {
///   be_u64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_u64`][crate::number::be_u64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_u64`")]
pub fn be_u64<I, E: ParseError<I>>(input: I) -> IResult<I, u64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_uint(input, 8)
}

/// Recognizes a big endian unsigned 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_u128;
///
/// let parser = |s| {
///   be_u128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_u128`][crate::number::be_u128]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_u128`")]
pub fn be_u128<I, E: ParseError<I>>(input: I) -> IResult<I, u128, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_uint(input, 16)
}

#[inline]
fn be_uint<I, Uint, E: ParseError<I>>(input: I, bound: usize) -> IResult<I, Uint, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
    Uint: Default + Shl<u8, Output = Uint> + Add<Uint, Output = Uint> + From<u8>,
{
    let offset = input
        .offset_at(bound)
        .map_err(|_err| ErrMode::Backtrack(make_error(input.clone(), ErrorKind::Eof)))?;
    let (input, number) = input.next_slice(offset);
    let number = number.as_bytes();

    let mut res = Uint::default();
    // special case to avoid shift a byte with overflow
    if bound > 1 {
        for byte in number.iter().copied().take(bound) {
            res = (res << 8) + byte.into();
        }
    } else {
        for byte in number.iter().copied().take(bound) {
            res = byte.into();
        }
    }

    Ok((input, res))
}

/// Recognizes a signed 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_i8;
///
/// let parser = |s| {
///   be_i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_i8`][crate::number::be_i8]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_i8`")]
pub fn be_i8<I, E: ParseError<I>>(input: I) -> IResult<I, i8, E>
where
    I: Stream<Token = u8>,
{
    be_u8.map(|x| x as i8).parse_next(input)
}

/// Recognizes a big endian signed 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_i16;
///
/// let parser = |s| {
///   be_i16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_i16`][crate::number::be_i16]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_i16`")]
pub fn be_i16<I, E: ParseError<I>>(input: I) -> IResult<I, i16, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_u16.map(|x| x as i16).parse_next(input)
}

/// Recognizes a big endian signed 3 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_i24;
///
/// let parser = |s| {
///   be_i24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_i24`][crate::number::be_i24]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_i24`")]
pub fn be_i24<I, E: ParseError<I>>(input: I) -> IResult<I, i32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    // Same as the unsigned version but we need to sign-extend manually here
    be_u24
        .map(|x| {
            if x & 0x80_00_00 != 0 {
                (x | 0xff_00_00_00) as i32
            } else {
                x as i32
            }
        })
        .parse_next(input)
}

/// Recognizes a big endian signed 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_i32;
///
/// let parser = |s| {
///   be_i32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_i32`][crate::number::be_i32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_i32`")]
pub fn be_i32<I, E: ParseError<I>>(input: I) -> IResult<I, i32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_u32.map(|x| x as i32).parse_next(input)
}

/// Recognizes a big endian signed 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_i64;
///
/// let parser = |s| {
///   be_i64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_i64`][crate::number::be_i64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_i64`")]
pub fn be_i64<I, E: ParseError<I>>(input: I) -> IResult<I, i64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_u64.map(|x| x as i64).parse_next(input)
}

/// Recognizes a big endian signed 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_i128;
///
/// let parser = |s| {
///   be_i128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_i128`][crate::number::be_i128]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_i128`")]
pub fn be_i128<I, E: ParseError<I>>(input: I) -> IResult<I, i128, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    be_u128.map(|x| x as i128).parse_next(input)
}

/// Recognizes an unsigned 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_u8;
///
/// let parser = |s| {
///   le_u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_u8`][crate::number::le_u8]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_u8`")]
pub fn le_u8<I, E: ParseError<I>>(input: I) -> IResult<I, u8, E>
where
    I: Stream<Token = u8>,
{
    u8(input)
}

/// Recognizes a little endian unsigned 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_u16;
///
/// let parser = |s| {
///   le_u16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_u16`][crate::number::le_u16]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_u16`")]
pub fn le_u16<I, E: ParseError<I>>(input: I) -> IResult<I, u16, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_uint(input, 2)
}

/// Recognizes a little endian unsigned 3 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_u24;
///
/// let parser = |s| {
///   le_u24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_u24`][crate::number::le_u24]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_u24`")]
pub fn le_u24<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_uint(input, 3)
}

/// Recognizes a little endian unsigned 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_u32;
///
/// let parser = |s| {
///   le_u32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_u32`][crate::number::le_u32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_u32`")]
pub fn le_u32<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_uint(input, 4)
}

/// Recognizes a little endian unsigned 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_u64;
///
/// let parser = |s| {
///   le_u64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_u64`][crate::number::le_u64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_u64`")]
pub fn le_u64<I, E: ParseError<I>>(input: I) -> IResult<I, u64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_uint(input, 8)
}

/// Recognizes a little endian unsigned 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_u128;
///
/// let parser = |s| {
///   le_u128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_u128`][crate::number::le_u128]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_u128`")]
pub fn le_u128<I, E: ParseError<I>>(input: I) -> IResult<I, u128, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_uint(input, 16)
}

#[inline]
fn le_uint<I, Uint, E: ParseError<I>>(input: I, bound: usize) -> IResult<I, Uint, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
    Uint: Default + Shl<u8, Output = Uint> + Add<Uint, Output = Uint> + From<u8>,
{
    let offset = input
        .offset_at(bound)
        .map_err(|_err| ErrMode::Backtrack(make_error(input.clone(), ErrorKind::Eof)))?;
    let (input, number) = input.next_slice(offset);
    let number = number.as_bytes();

    let mut res = Uint::default();
    for (index, byte) in number.iter_offsets().take(bound) {
        res = res + (Uint::from(byte) << (8 * index as u8));
    }

    Ok((input, res))
}

/// Recognizes a signed 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_i8;
///
/// let parser = |s| {
///   le_i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_i8`][crate::number::le_i8]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_i8`")]
pub fn le_i8<I, E: ParseError<I>>(input: I) -> IResult<I, i8, E>
where
    I: Stream<Token = u8>,
{
    be_u8.map(|x| x as i8).parse_next(input)
}

/// Recognizes a little endian signed 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_i16;
///
/// let parser = |s| {
///   le_i16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_i16`][crate::number::le_i16]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_i16`")]
pub fn le_i16<I, E: ParseError<I>>(input: I) -> IResult<I, i16, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_u16.map(|x| x as i16).parse_next(input)
}

/// Recognizes a little endian signed 3 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_i24;
///
/// let parser = |s| {
///   le_i24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_i24`][crate::number::le_i24]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_i24`")]
pub fn le_i24<I, E: ParseError<I>>(input: I) -> IResult<I, i32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    // Same as the unsigned version but we need to sign-extend manually here
    le_u24
        .map(|x| {
            if x & 0x80_00_00 != 0 {
                (x | 0xff_00_00_00) as i32
            } else {
                x as i32
            }
        })
        .parse_next(input)
}

/// Recognizes a little endian signed 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_i32;
///
/// let parser = |s| {
///   le_i32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_i32`][crate::number::le_i32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_i32`")]
pub fn le_i32<I, E: ParseError<I>>(input: I) -> IResult<I, i32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_u32.map(|x| x as i32).parse_next(input)
}

/// Recognizes a little endian signed 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_i64;
///
/// let parser = |s| {
///   le_i64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_i64`][crate::number::le_i64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_i64`")]
pub fn le_i64<I, E: ParseError<I>>(input: I) -> IResult<I, i64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_u64.map(|x| x as i64).parse_next(input)
}

/// Recognizes a little endian signed 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_i128;
///
/// let parser = |s| {
///   le_i128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_i128`][crate::number::le_i128]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_i128`")]
pub fn le_i128<I, E: ParseError<I>>(input: I) -> IResult<I, i128, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    le_u128.map(|x| x as i128).parse_next(input)
}

/// Recognizes an unsigned 1 byte integer
///
/// Note that endianness does not apply to 1 byte numbers.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::u8;
///
/// let parser = |s| {
///   u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::u8`][crate::number::u8]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::u8`")]
pub fn u8<I, E: ParseError<I>>(input: I) -> IResult<I, u8, E>
where
    I: Stream<Token = u8>,
{
    input
        .next_token()
        .ok_or_else(|| ErrMode::Backtrack(make_error(input, ErrorKind::Eof)))
}

/// Recognizes an unsigned 2 bytes integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u16 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u16 integer.
/// *complete version*: returns an error if there is not enough input data
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::u16;
///
/// let be_u16 = |s| {
///   u16(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(be_u16(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_u16 = |s| {
///   u16(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(le_u16(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::u16`][crate::number::u16]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::u16`")]
pub fn u16<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, u16, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_u16,
        crate::number::Endianness::Little => le_u16,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_u16,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_u16,
    }
}

/// Recognizes an unsigned 3 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u24 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u24 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::u24;
///
/// let be_u24 = |s| {
///   u24(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(be_u24(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_u24 = |s| {
///   u24(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(le_u24(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::u24`][crate::number::u24]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::u24`")]
pub fn u24<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, u32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_u24,
        crate::number::Endianness::Little => le_u24,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_u24,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_u24,
    }
}

/// Recognizes an unsigned 4 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u32 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u32 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::u32;
///
/// let be_u32 = |s| {
///   u32(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(be_u32(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_u32 = |s| {
///   u32(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(le_u32(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::u32`][crate::number::u32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::u32`")]
pub fn u32<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, u32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_u32,
        crate::number::Endianness::Little => le_u32,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_u32,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_u32,
    }
}

/// Recognizes an unsigned 8 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u64 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u64 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::u64;
///
/// let be_u64 = |s| {
///   u64(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(be_u64(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_u64 = |s| {
///   u64(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(le_u64(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::u64`][crate::number::u64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::u64`")]
pub fn u64<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, u64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_u64,
        crate::number::Endianness::Little => le_u64,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_u64,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_u64,
    }
}

/// Recognizes an unsigned 16 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian u128 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian u128 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::u128;
///
/// let be_u128 = |s| {
///   u128(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(be_u128(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_u128 = |s| {
///   u128(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(le_u128(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::u128`][crate::number::u128]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::u128`")]
pub fn u128<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, u128, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_u128,
        crate::number::Endianness::Little => le_u128,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_u128,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_u128,
    }
}

/// Recognizes a signed 1 byte integer
///
/// Note that endianness does not apply to 1 byte numbers.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::i8;
///
/// let parser = |s| {
///   i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::i8`][crate::number::i8]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::i8`")]
pub fn i8<I, E: ParseError<I>>(i: I) -> IResult<I, i8, E>
where
    I: Stream<Token = u8>,
{
    u8.map(|x| x as i8).parse_next(i)
}

/// Recognizes a signed 2 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i16 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i16 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::i16;
///
/// let be_i16 = |s| {
///   i16(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(be_i16(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_i16 = |s| {
///   i16(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(le_i16(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::i16`][crate::number::i16]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::i16`")]
pub fn i16<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, i16, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_i16,
        crate::number::Endianness::Little => le_i16,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_i16,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_i16,
    }
}

/// Recognizes a signed 3 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i24 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i24 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::i24;
///
/// let be_i24 = |s| {
///   i24(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(be_i24(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_i24 = |s| {
///   i24(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(le_i24(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::i24`][crate::number::i24]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::i24`")]
pub fn i24<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, i32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_i24,
        crate::number::Endianness::Little => le_i24,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_i24,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_i24,
    }
}

/// Recognizes a signed 4 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i32 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i32 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::i32;
///
/// let be_i32 = |s| {
///   i32(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(be_i32(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_i32 = |s| {
///   i32(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(le_i32(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::i32`][crate::number::i32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::i32`")]
pub fn i32<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, i32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_i32,
        crate::number::Endianness::Little => le_i32,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_i32,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_i32,
    }
}

/// Recognizes a signed 8 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i64 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i64 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::i64;
///
/// let be_i64 = |s| {
///   i64(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(be_i64(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_i64 = |s| {
///   i64(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(le_i64(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::i64`][crate::number::i64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::i64`")]
pub fn i64<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, i64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_i64,
        crate::number::Endianness::Little => le_i64,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_i64,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_i64,
    }
}

/// Recognizes a signed 16 byte integer
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian i128 integer,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian i128 integer.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::i128;
///
/// let be_i128 = |s| {
///   i128(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(be_i128(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
///
/// let le_i128 = |s| {
///   i128(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(le_i128(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::i128`][crate::number::i128]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::i128`")]
pub fn i128<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, i128, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_i128,
        crate::number::Endianness::Little => le_i128,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_i128,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_i128,
    }
}

/// Recognizes a big endian 4 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_f32;
///
/// let parser = |s| {
///   be_f32(s)
/// };
///
/// assert_eq!(parser(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_f32`][crate::number::be_f32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_f32`")]
pub fn be_f32<I, E: ParseError<I>>(input: I) -> IResult<I, f32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match be_u32(input) {
        Err(e) => Err(e),
        Ok((i, o)) => Ok((i, f32::from_bits(o))),
    }
}

/// Recognizes a big endian 8 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::be_f64;
///
/// let parser = |s| {
///   be_f64(s)
/// };
///
/// assert_eq!(parser(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::be_f64`][crate::number::be_f64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::be_f64`")]
pub fn be_f64<I, E: ParseError<I>>(input: I) -> IResult<I, f64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match be_u64(input) {
        Err(e) => Err(e),
        Ok((i, o)) => Ok((i, f64::from_bits(o))),
    }
}

/// Recognizes a little endian 4 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_f32;
///
/// let parser = |s| {
///   le_f32(s)
/// };
///
/// assert_eq!(parser(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_f32`][crate::number::le_f32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_f32`")]
pub fn le_f32<I, E: ParseError<I>>(input: I) -> IResult<I, f32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match le_u32(input) {
        Err(e) => Err(e),
        Ok((i, o)) => Ok((i, f32::from_bits(o))),
    }
}

/// Recognizes a little endian 8 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::le_f64;
///
/// let parser = |s| {
///   le_f64(s)
/// };
///
/// assert_eq!(parser(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::le_f64`][crate::number::le_f64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::le_f64`")]
pub fn le_f64<I, E: ParseError<I>>(input: I) -> IResult<I, f64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match le_u64(input) {
        Err(e) => Err(e),
        Ok((i, o)) => Ok((i, f64::from_bits(o))),
    }
}

/// Recognizes a 4 byte floating point number
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian f32 float,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian f32 float.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::f32;
///
/// let be_f32 = |s| {
///   f32(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_f32(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(be_f32(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
///
/// let le_f32 = |s| {
///   f32(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_f32(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(le_f32(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::f32`][crate::number::f32]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::f32`")]
pub fn f32<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, f32, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_f32,
        crate::number::Endianness::Little => le_f32,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_f32,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_f32,
    }
}

/// Recognizes an 8 byte floating point number
///
/// If the parameter is `winnow::number::Endianness::Big`, parse a big endian f64 float,
/// otherwise if `winnow::number::Endianness::Little` parse a little endian f64 float.
/// *complete version*: returns an error if there is not enough input data
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::f64;
///
/// let be_f64 = |s| {
///   f64(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_f64(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(be_f64(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
///
/// let le_f64 = |s| {
///   f64(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_f64(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(le_f64(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::number::f64`][crate::number::f64]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::number::f64`")]
pub fn f64<I, E: ParseError<I>>(endian: crate::number::Endianness) -> fn(I) -> IResult<I, f64, E>
where
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    match endian {
        crate::number::Endianness::Big => be_f64,
        crate::number::Endianness::Little => le_f64,
        #[cfg(target_endian = "big")]
        crate::number::Endianness::Native => be_f64,
        #[cfg(target_endian = "little")]
        crate::number::Endianness::Native => le_f64,
    }
}

/// Recognizes a hex-encoded integer.
///
/// *Complete version*: Will parse until the end of input if it has less than 8 bytes.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::hex_u32;
///
/// let parser = |s| {
///   hex_u32(s)
/// };
///
/// assert_eq!(parser(&b"01AE"[..]), Ok((&b""[..], 0x01AE)));
/// assert_eq!(parser(&b"abc"[..]), Ok((&b""[..], 0x0ABC)));
/// assert_eq!(parser(&b"ggg"[..]), Err(ErrMode::Backtrack(Error::new(&b"ggg"[..], ErrorKind::IsA))));
/// ```
#[inline]
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::hex_uint`][crate::character::hex_uint]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::hex_uint`")]
pub fn hex_u32<I, E: ParseError<I>>(input: I) -> IResult<I, u32, E>
where
    I: Stream,
    <I as Stream>::Token: AsChar,
    <I as Stream>::Slice: AsBStr,
{
    let invalid_offset = input
        .offset_for(|c| {
            let c = c.as_char();
            !"0123456789abcdefABCDEF".contains(c)
        })
        .unwrap_or_else(|| input.eof_offset());
    const MAX_DIGITS: usize = 8;
    let max_offset = input
        .offset_at(MAX_DIGITS)
        .unwrap_or_else(|_err| input.eof_offset());
    let offset = invalid_offset.min(max_offset);
    if offset == 0 {
        return Err(ErrMode::from_error_kind(input, ErrorKind::IsA));
    }
    let (remaining, parsed) = input.next_slice(offset);

    let res = parsed
        .as_bstr()
        .iter()
        .rev()
        .enumerate()
        .map(|(k, &v)| {
            let digit = v as char;
            digit.to_digit(16).unwrap_or(0) << (k * 4)
        })
        .sum();

    Ok((remaining, res))
}

/// **WARNING:** Deprecated, no longer supported
#[deprecated(since = "0.3.0", note = "No longer supported")]
pub fn recognize_float<T, E: ParseError<T>>(input: T) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    T: Offset + Compare<&'static str>,
    <T as Stream>::Token: AsChar + Copy,
    <T as Stream>::IterOffsets: Clone,
    T: AsBStr,
{
    tuple((
        opt(alt((char('+'), char('-')))),
        alt((
            map(tuple((digit1, opt(pair(char('.'), opt(digit1))))), |_| ()),
            map(tuple((char('.'), digit1)), |_| ()),
        )),
        opt(tuple((
            alt((char('e'), char('E'))),
            opt(alt((char('+'), char('-')))),
            cut_err(digit1),
        ))),
    ))
    .recognize()
    .parse_next(input)
}

/// **WARNING:** Deprecated, no longer supported
#[deprecated(since = "0.3.0", note = "No longer supported")]
pub fn recognize_float_or_exceptions<T, E: ParseError<T>>(
    input: T,
) -> IResult<T, <T as Stream>::Slice, E>
where
    T: Stream,
    T: Offset + Compare<&'static str>,
    <T as Stream>::Token: AsChar + Copy,
    <T as Stream>::IterOffsets: Clone,
    T: AsBStr,
{
    alt((
        |i: T| {
            recognize_float::<_, E>(i.clone()).map_err(|e| match e {
                crate::error::ErrMode::Backtrack(_) => {
                    crate::error::ErrMode::from_error_kind(i, ErrorKind::Float)
                }
                crate::error::ErrMode::Cut(_) => {
                    crate::error::ErrMode::Cut(E::from_error_kind(i, ErrorKind::Float))
                }
                crate::error::ErrMode::Incomplete(needed) => {
                    crate::error::ErrMode::Incomplete(needed)
                }
            })
        },
        |i: T| {
            crate::bytes::complete::tag_no_case::<_, _, E>("nan")(i.clone())
                .map_err(|_err| crate::error::ErrMode::from_error_kind(i, ErrorKind::Float))
        },
        |i: T| {
            crate::bytes::complete::tag_no_case::<_, _, E>("inf")(i.clone())
                .map_err(|_err| crate::error::ErrMode::from_error_kind(i, ErrorKind::Float))
        },
        |i: T| {
            crate::bytes::complete::tag_no_case::<_, _, E>("infinity")(i.clone())
                .map_err(|_err| crate::error::ErrMode::from_error_kind(i, ErrorKind::Float))
        },
    ))(input)
}

/// **WARNING:** Deprecated, no longer supported
#[allow(clippy::type_complexity)]
#[deprecated(since = "0.3.0", note = "No longer supported")]
pub fn recognize_float_parts<T, E: ParseError<T>>(
    input: T,
) -> IResult<T, (bool, <T as Stream>::Slice, <T as Stream>::Slice, i32), E>
where
    T: Stream + Compare<&'static [u8]> + AsBStr,
    <T as Stream>::Token: AsChar + Copy,
    <T as Stream>::Slice: SliceLen,
{
    let (i, sign) = sign(input.clone())?;

    let (i, integer) = match i.offset_for(|c| !c.is_dec_digit()) {
        Some(offset) => i.next_slice(offset),
        None => i.next_slice(i.eof_offset()),
    };

    let (i, opt_dot) = opt(tag(&b"."[..]))(i)?;
    let (i, fraction) = if opt_dot.is_none() {
        i.next_slice(0)
    } else {
        // match number
        let mut zero_count = 0usize;
        let mut offset = None;
        for (pos, c) in i.as_bstr().iter().enumerate() {
            if *c >= b'0' && *c <= b'9' {
                if *c == b'0' {
                    zero_count += 1;
                } else {
                    zero_count = 0;
                }
            } else {
                offset = Some(pos);
                break;
            }
        }
        let offset = offset.unwrap_or_else(|| i.eof_offset());

        // trim right zeroes
        let trimmed_offset = (offset - zero_count).max(1);

        let (_, frac) = i.next_slice(trimmed_offset);
        let (i, _) = i.next_slice(offset);
        (i, frac)
    };

    if integer.slice_len() == 0 && fraction.slice_len() == 0 {
        return Err(ErrMode::from_error_kind(input, ErrorKind::Float));
    }

    let i2 = i.clone();
    let (i, e) = i
        .next_token()
        .filter(|(_, t)| t.as_char() == 'e' || t.as_char() == 'E')
        .map(|(i, _)| (i, true))
        .unwrap_or((i, false));

    let (i, exp) = if e {
        cut_err(crate::character::complete::i32)(i)?
    } else {
        (i2, 0)
    };

    Ok((i, (sign, integer, fraction, exp)))
}

use crate::stream::ParseSlice;

/// Recognizes floating point number in text format and returns a f32.
///
/// *Complete version*: Can parse until the end of input.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::float;
///
/// let parser = |s| {
///   float(s)
/// };
///
/// assert_eq!(parser("11e-1"), Ok(("", 1.1)));
/// assert_eq!(parser("123E-02"), Ok(("", 1.23)));
/// assert_eq!(parser("123K-01"), Ok(("K-01", 123.0)));
/// assert_eq!(parser("abc"), Err(ErrMode::Backtrack(Error::new("abc", ErrorKind::Float))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::float`][crate::character::float]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::float`")]
pub fn float<T, E: ParseError<T>>(input: T) -> IResult<T, f32, E>
where
    T: Stream,
    T: Offset + Compare<&'static str>,
    <T as Stream>::Slice: ParseSlice<f32>,
    <T as Stream>::Token: AsChar + Copy,
    <T as Stream>::IterOffsets: Clone,
    T: AsBStr,
{
    let (i, s) = recognize_float_or_exceptions(input)?;
    match s.parse_slice() {
        Some(f) => Ok((i, f)),
        None => Err(crate::error::ErrMode::from_error_kind(
            i,
            crate::error::ErrorKind::Float,
        )),
    }
}

/// Recognizes floating point number in text format and returns a f64.
///
/// *Complete version*: Can parse until the end of input.
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::number::complete::double;
///
/// let parser = |s| {
///   double(s)
/// };
///
/// assert_eq!(parser("11e-1"), Ok(("", 1.1)));
/// assert_eq!(parser("123E-02"), Ok(("", 1.23)));
/// assert_eq!(parser("123K-01"), Ok(("K-01", 123.0)));
/// assert_eq!(parser("abc"), Err(ErrMode::Backtrack(Error::new("abc", ErrorKind::Float))));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::character::float`][crate::character::float]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::character::float`")]
pub fn double<T, E: ParseError<T>>(input: T) -> IResult<T, f64, E>
where
    T: Stream,
    T: Offset + Compare<&'static str>,
    <T as Stream>::Slice: ParseSlice<f64>,
    <T as Stream>::Token: AsChar + Copy,
    <T as Stream>::IterOffsets: Clone,
    T: AsBStr,
{
    let (i, s) = recognize_float_or_exceptions(input)?;
    match s.parse_slice() {
        Some(f) => Ok((i, f)),
        None => Err(crate::error::ErrMode::from_error_kind(
            i,
            crate::error::ErrorKind::Float,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrMode;
    use crate::error::ErrorKind;
    use proptest::prelude::*;

    macro_rules! assert_parse(
    ($left: expr, $right: expr) => {
      let res: $crate::IResult<_, _> = $left;
      assert_eq!(res, $right);
    };
  );

    #[test]
    fn i8_tests() {
        assert_parse!(i8(&[0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(i8(&[0x7f][..]), Ok((&b""[..], 127)));
        assert_parse!(i8(&[0xff][..]), Ok((&b""[..], -1)));
        assert_parse!(i8(&[0x80][..]), Ok((&b""[..], -128)));
    }

    #[test]
    fn be_i8_tests() {
        assert_parse!(be_i8(&[0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(be_i8(&[0x7f][..]), Ok((&b""[..], 127)));
        assert_parse!(be_i8(&[0xff][..]), Ok((&b""[..], -1)));
        assert_parse!(be_i8(&[0x80][..]), Ok((&b""[..], -128)));
    }

    #[test]
    fn be_i16_tests() {
        assert_parse!(be_i16(&[0x00, 0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(be_i16(&[0x7f, 0xff][..]), Ok((&b""[..], 32_767_i16)));
        assert_parse!(be_i16(&[0xff, 0xff][..]), Ok((&b""[..], -1)));
        assert_parse!(be_i16(&[0x80, 0x00][..]), Ok((&b""[..], -32_768_i16)));
    }

    #[test]
    fn be_u24_tests() {
        assert_parse!(be_u24(&[0x00, 0x00, 0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(be_u24(&[0x00, 0xFF, 0xFF][..]), Ok((&b""[..], 65_535_u32)));
        assert_parse!(
            be_u24(&[0x12, 0x34, 0x56][..]),
            Ok((&b""[..], 1_193_046_u32))
        );
    }

    #[test]
    fn be_i24_tests() {
        assert_parse!(be_i24(&[0xFF, 0xFF, 0xFF][..]), Ok((&b""[..], -1_i32)));
        assert_parse!(be_i24(&[0xFF, 0x00, 0x00][..]), Ok((&b""[..], -65_536_i32)));
        assert_parse!(
            be_i24(&[0xED, 0xCB, 0xAA][..]),
            Ok((&b""[..], -1_193_046_i32))
        );
    }

    #[test]
    fn be_i32_tests() {
        assert_parse!(be_i32(&[0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(
            be_i32(&[0x7f, 0xff, 0xff, 0xff][..]),
            Ok((&b""[..], 2_147_483_647_i32))
        );
        assert_parse!(be_i32(&[0xff, 0xff, 0xff, 0xff][..]), Ok((&b""[..], -1)));
        assert_parse!(
            be_i32(&[0x80, 0x00, 0x00, 0x00][..]),
            Ok((&b""[..], -2_147_483_648_i32))
        );
    }

    #[test]
    fn be_i64_tests() {
        assert_parse!(
            be_i64(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            Ok((&b""[..], 0))
        );
        assert_parse!(
            be_i64(&[0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]),
            Ok((&b""[..], 9_223_372_036_854_775_807_i64))
        );
        assert_parse!(
            be_i64(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]),
            Ok((&b""[..], -1))
        );
        assert_parse!(
            be_i64(&[0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            Ok((&b""[..], -9_223_372_036_854_775_808_i64))
        );
    }

    #[test]
    fn be_i128_tests() {
        assert_parse!(
            be_i128(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            ),
            Ok((&b""[..], 0))
        );
        assert_parse!(
            be_i128(
                &[
                    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            ),
            Ok((
                &b""[..],
                170_141_183_460_469_231_731_687_303_715_884_105_727_i128
            ))
        );
        assert_parse!(
            be_i128(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            ),
            Ok((&b""[..], -1))
        );
        assert_parse!(
            be_i128(
                &[
                    0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            ),
            Ok((
                &b""[..],
                -170_141_183_460_469_231_731_687_303_715_884_105_728_i128
            ))
        );
    }

    #[test]
    fn le_i8_tests() {
        assert_parse!(le_i8(&[0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(le_i8(&[0x7f][..]), Ok((&b""[..], 127)));
        assert_parse!(le_i8(&[0xff][..]), Ok((&b""[..], -1)));
        assert_parse!(le_i8(&[0x80][..]), Ok((&b""[..], -128)));
    }

    #[test]
    fn le_i16_tests() {
        assert_parse!(le_i16(&[0x00, 0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(le_i16(&[0xff, 0x7f][..]), Ok((&b""[..], 32_767_i16)));
        assert_parse!(le_i16(&[0xff, 0xff][..]), Ok((&b""[..], -1)));
        assert_parse!(le_i16(&[0x00, 0x80][..]), Ok((&b""[..], -32_768_i16)));
    }

    #[test]
    fn le_u24_tests() {
        assert_parse!(le_u24(&[0x00, 0x00, 0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(le_u24(&[0xFF, 0xFF, 0x00][..]), Ok((&b""[..], 65_535_u32)));
        assert_parse!(
            le_u24(&[0x56, 0x34, 0x12][..]),
            Ok((&b""[..], 1_193_046_u32))
        );
    }

    #[test]
    fn le_i24_tests() {
        assert_parse!(le_i24(&[0xFF, 0xFF, 0xFF][..]), Ok((&b""[..], -1_i32)));
        assert_parse!(le_i24(&[0x00, 0x00, 0xFF][..]), Ok((&b""[..], -65_536_i32)));
        assert_parse!(
            le_i24(&[0xAA, 0xCB, 0xED][..]),
            Ok((&b""[..], -1_193_046_i32))
        );
    }

    #[test]
    fn le_i32_tests() {
        assert_parse!(le_i32(&[0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 0)));
        assert_parse!(
            le_i32(&[0xff, 0xff, 0xff, 0x7f][..]),
            Ok((&b""[..], 2_147_483_647_i32))
        );
        assert_parse!(le_i32(&[0xff, 0xff, 0xff, 0xff][..]), Ok((&b""[..], -1)));
        assert_parse!(
            le_i32(&[0x00, 0x00, 0x00, 0x80][..]),
            Ok((&b""[..], -2_147_483_648_i32))
        );
    }

    #[test]
    fn le_i64_tests() {
        assert_parse!(
            le_i64(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            Ok((&b""[..], 0))
        );
        assert_parse!(
            le_i64(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f][..]),
            Ok((&b""[..], 9_223_372_036_854_775_807_i64))
        );
        assert_parse!(
            le_i64(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]),
            Ok((&b""[..], -1))
        );
        assert_parse!(
            le_i64(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80][..]),
            Ok((&b""[..], -9_223_372_036_854_775_808_i64))
        );
    }

    #[test]
    fn le_i128_tests() {
        assert_parse!(
            le_i128(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            ),
            Ok((&b""[..], 0))
        );
        assert_parse!(
            le_i128(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0x7f
                ][..]
            ),
            Ok((
                &b""[..],
                170_141_183_460_469_231_731_687_303_715_884_105_727_i128
            ))
        );
        assert_parse!(
            le_i128(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            ),
            Ok((&b""[..], -1))
        );
        assert_parse!(
            le_i128(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x80
                ][..]
            ),
            Ok((
                &b""[..],
                -170_141_183_460_469_231_731_687_303_715_884_105_728_i128
            ))
        );
    }

    #[test]
    fn be_f32_tests() {
        assert_parse!(be_f32(&[0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 0_f32)));
        assert_parse!(
            be_f32(&[0x4d, 0x31, 0x1f, 0xd8][..]),
            Ok((&b""[..], 185_728_380_f32))
        );
    }

    #[test]
    fn be_f64_tests() {
        assert_parse!(
            be_f64(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            Ok((&b""[..], 0_f64))
        );
        assert_parse!(
            be_f64(&[0x41, 0xa6, 0x23, 0xfb, 0x10, 0x00, 0x00, 0x00][..]),
            Ok((&b""[..], 185_728_392_f64))
        );
    }

    #[test]
    fn le_f32_tests() {
        assert_parse!(le_f32(&[0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 0_f32)));
        assert_parse!(
            le_f32(&[0xd8, 0x1f, 0x31, 0x4d][..]),
            Ok((&b""[..], 185_728_380_f32))
        );
    }

    #[test]
    fn le_f64_tests() {
        assert_parse!(
            le_f64(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            Ok((&b""[..], 0_f64))
        );
        assert_parse!(
            le_f64(&[0x00, 0x00, 0x00, 0x10, 0xfb, 0x23, 0xa6, 0x41][..]),
            Ok((&b""[..], 185_728_392_f64))
        );
    }

    #[test]
    fn hex_u32_tests() {
        assert_parse!(
            hex_u32(&b";"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b";"[..],
                ErrorKind::IsA
            )))
        );
        assert_parse!(hex_u32(&b"ff;"[..]), Ok((&b";"[..], 255)));
        assert_parse!(hex_u32(&b"1be2;"[..]), Ok((&b";"[..], 7138)));
        assert_parse!(hex_u32(&b"c5a31be2;"[..]), Ok((&b";"[..], 3_315_801_058)));
        assert_parse!(hex_u32(&b"C5A31be2;"[..]), Ok((&b";"[..], 3_315_801_058)));
        assert_parse!(hex_u32(&b"00c5a31be2;"[..]), Ok((&b"e2;"[..], 12_952_347)));
        assert_parse!(
            hex_u32(&b"c5a31be201;"[..]),
            Ok((&b"01;"[..], 3_315_801_058))
        );
        assert_parse!(hex_u32(&b"ffffffff;"[..]), Ok((&b";"[..], 4_294_967_295)));
        assert_parse!(hex_u32(&b"0x1be2;"[..]), Ok((&b"x1be2;"[..], 0)));
        assert_parse!(hex_u32(&b"12af"[..]), Ok((&b""[..], 0x12af)));
    }

    #[test]
    #[cfg(feature = "std")]
    fn float_test() {
        use crate::error::Error;

        let mut test_cases = vec![
            "+3.14",
            "3.14",
            "-3.14",
            "0",
            "0.0",
            "1.",
            ".789",
            "-.5",
            "1e7",
            "-1E-7",
            ".3e-2",
            "1.e4",
            "1.2e4",
            "12.34",
            "-1.234E-12",
            "-1.234e-12",
            "0.00000000000000000087",
        ];

        for test in test_cases.drain(..) {
            let expected32 = str::parse::<f32>(test).unwrap();
            let expected64 = str::parse::<f64>(test).unwrap();

            println!("now parsing: {} -> {}", test, expected32);

            assert_parse!(recognize_float(test), Ok(("", test)));

            assert_parse!(float(test.as_bytes()), Ok((&b""[..], expected32)));
            assert_parse!(float(test), Ok(("", expected32)));

            assert_parse!(double(test.as_bytes()), Ok((&b""[..], expected64)));
            assert_parse!(double(test), Ok(("", expected64)));
        }

        let remaining_exponent = "-1.234E-";
        assert_parse!(
            recognize_float(remaining_exponent),
            Err(ErrMode::Cut(Error::new("", ErrorKind::Digit)))
        );

        let (_i, nan) = float::<_, ()>("NaN").unwrap();
        assert!(nan.is_nan());

        let (_i, inf) = float::<_, ()>("inf").unwrap();
        assert!(inf.is_infinite());
        let (_i, inf) = float::<_, ()>("infinite").unwrap();
        assert!(inf.is_infinite());
    }

    #[test]
    fn configurable_endianness() {
        use crate::number::Endianness;

        fn be_tst16(i: &[u8]) -> IResult<&[u8], u16> {
            u16(Endianness::Big)(i)
        }
        fn le_tst16(i: &[u8]) -> IResult<&[u8], u16> {
            u16(Endianness::Little)(i)
        }
        assert_eq!(be_tst16(&[0x80, 0x00]), Ok((&b""[..], 32_768_u16)));
        assert_eq!(le_tst16(&[0x80, 0x00]), Ok((&b""[..], 128_u16)));

        fn be_tst32(i: &[u8]) -> IResult<&[u8], u32> {
            u32(Endianness::Big)(i)
        }
        fn le_tst32(i: &[u8]) -> IResult<&[u8], u32> {
            u32(Endianness::Little)(i)
        }
        assert_eq!(
            be_tst32(&[0x12, 0x00, 0x60, 0x00]),
            Ok((&b""[..], 302_014_464_u32))
        );
        assert_eq!(
            le_tst32(&[0x12, 0x00, 0x60, 0x00]),
            Ok((&b""[..], 6_291_474_u32))
        );

        fn be_tst64(i: &[u8]) -> IResult<&[u8], u64> {
            u64(Endianness::Big)(i)
        }
        fn le_tst64(i: &[u8]) -> IResult<&[u8], u64> {
            u64(Endianness::Little)(i)
        }
        assert_eq!(
            be_tst64(&[0x12, 0x00, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00]),
            Ok((&b""[..], 1_297_142_246_100_992_000_u64))
        );
        assert_eq!(
            le_tst64(&[0x12, 0x00, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00]),
            Ok((&b""[..], 36_028_874_334_666_770_u64))
        );

        fn be_tsti16(i: &[u8]) -> IResult<&[u8], i16> {
            i16(Endianness::Big)(i)
        }
        fn le_tsti16(i: &[u8]) -> IResult<&[u8], i16> {
            i16(Endianness::Little)(i)
        }
        assert_eq!(be_tsti16(&[0x00, 0x80]), Ok((&b""[..], 128_i16)));
        assert_eq!(le_tsti16(&[0x00, 0x80]), Ok((&b""[..], -32_768_i16)));

        fn be_tsti32(i: &[u8]) -> IResult<&[u8], i32> {
            i32(Endianness::Big)(i)
        }
        fn le_tsti32(i: &[u8]) -> IResult<&[u8], i32> {
            i32(Endianness::Little)(i)
        }
        assert_eq!(
            be_tsti32(&[0x00, 0x12, 0x60, 0x00]),
            Ok((&b""[..], 1_204_224_i32))
        );
        assert_eq!(
            le_tsti32(&[0x00, 0x12, 0x60, 0x00]),
            Ok((&b""[..], 6_296_064_i32))
        );

        fn be_tsti64(i: &[u8]) -> IResult<&[u8], i64> {
            i64(Endianness::Big)(i)
        }
        fn le_tsti64(i: &[u8]) -> IResult<&[u8], i64> {
            i64(Endianness::Little)(i)
        }
        assert_eq!(
            be_tsti64(&[0x00, 0xFF, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00]),
            Ok((&b""[..], 71_881_672_479_506_432_i64))
        );
        assert_eq!(
            le_tsti64(&[0x00, 0xFF, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00]),
            Ok((&b""[..], 36_028_874_334_732_032_i64))
        );
    }

    #[cfg(feature = "std")]
    fn parse_f64(i: &str) -> IResult<&str, f64, ()> {
        match recognize_float_or_exceptions(i) {
            Err(e) => Err(e),
            Ok((i, s)) => {
                if s.is_empty() {
                    return Err(ErrMode::Backtrack(()));
                }
                match s.parse_slice() {
                    Some(n) => Ok((i, n)),
                    None => Err(ErrMode::Backtrack(())),
                }
            }
        }
    }

    proptest! {
        #[test]
        #[cfg(feature = "std")]
    #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn floats(s in "\\PC*") {
            println!("testing {}", s);
            let res1 = parse_f64(&s);
            let res2 = double::<_, ()>(s.as_str());
            assert_eq!(res1, res2);
        }
      }
}
