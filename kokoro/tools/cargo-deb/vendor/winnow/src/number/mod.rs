//! Parsers recognizing numbers

#![allow(deprecated)] // will just become `pub(crate)` later

#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub mod complete;
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub mod streaming;
#[cfg(test)]
mod tests;

use crate::error::ParseError;
use crate::stream::{AsBytes, Stream, StreamIsPartial};
use crate::trace::trace;
use crate::IResult;

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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u8;
///
/// let parser = |s| {
///   be_u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_u8;
///
/// let parser = |s| {
///   be_u8::<_, Error<_>>(s)
/// };
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
    trace("be_u8", move |input: I| {
        if input.is_partial() {
            streaming::be_u8(input)
        } else {
            complete::be_u8(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u16;
///
/// let parser = |s| {
///   be_u16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_u16;
///
/// let parser = |s| {
///   be_u16::<_, Error<_>>(s)
/// };
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
    trace("be_u16", move |input: I| {
        if input.is_partial() {
            streaming::be_u16(input)
        } else {
            complete::be_u16(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u24;
///
/// let parser = |s| {
///   be_u24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_u24;
///
/// let parser = |s| {
///   be_u24::<_, Error<_>>(s)
/// };
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
    trace("be_u23", move |input: I| {
        if input.is_partial() {
            streaming::be_u24(input)
        } else {
            complete::be_u24(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u32;
///
/// let parser = |s| {
///   be_u32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_u32;
///
/// let parser = |s| {
///   be_u32::<_, Error<_>>(s)
/// };
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
    trace("be_u32", move |input: I| {
        if input.is_partial() {
            streaming::be_u32(input)
        } else {
            complete::be_u32(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u64;
///
/// let parser = |s| {
///   be_u64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_u64;
///
/// let parser = |s| {
///   be_u64::<_, Error<_>>(s)
/// };
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
    trace("be_u64", move |input: I| {
        if input.is_partial() {
            streaming::be_u64(input)
        } else {
            complete::be_u64(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_u128;
///
/// let parser = |s| {
///   be_u128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_u128;
///
/// let parser = |s| {
///   be_u128::<_, Error<_>>(s)
/// };
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
    trace("be_u128", move |input: I| {
        if input.is_partial() {
            streaming::be_u128(input)
        } else {
            complete::be_u128(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i8;
///
/// let parser = |s| {
///   be_i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_i8;
///
/// let parser = be_i8::<_, Error<_>>;
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
    trace("be_i8", move |input: I| {
        if input.is_partial() {
            streaming::be_i8(input)
        } else {
            complete::be_i8(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i16;
///
/// let parser = |s| {
///   be_i16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_i16;
///
/// let parser = be_i16::<_, Error<_>>;
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
        if input.is_partial() {
            streaming::be_i16(input)
        } else {
            complete::be_i16(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i24;
///
/// let parser = |s| {
///   be_i24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_i24;
///
/// let parser = be_i24::<_, Error<_>>;
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
        if input.is_partial() {
            streaming::be_i24(input)
        } else {
            complete::be_i24(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i32;
///
/// let parser = |s| {
///   be_i32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_i32;
///
/// let parser = be_i32::<_, Error<_>>;
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
        if input.is_partial() {
            streaming::be_i32(input)
        } else {
            complete::be_i32(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i64;
///
/// let parser = |s| {
///   be_i64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_i64;
///
/// let parser = be_i64::<_, Error<_>>;
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
        if input.is_partial() {
            streaming::be_i64(input)
        } else {
            complete::be_i64(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_i128;
///
/// let parser = |s| {
///   be_i128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_i128;
///
/// let parser = be_i128::<_, Error<_>>;
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
        if input.is_partial() {
            streaming::be_i128(input)
        } else {
            complete::be_i128(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u8;
///
/// let parser = |s| {
///   le_u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_u8;
///
/// let parser = le_u8::<_, Error<_>>;
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
    trace("le_u8", move |input: I| {
        if input.is_partial() {
            streaming::le_u8(input)
        } else {
            complete::le_u8(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u16;
///
/// let parser = |s| {
///   le_u16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_u16;
///
/// let parser = |s| {
///   le_u16::<_, Error<_>>(s)
/// };
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
    trace("le_u16", move |input: I| {
        if input.is_partial() {
            streaming::le_u16(input)
        } else {
            complete::le_u16(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u24;
///
/// let parser = |s| {
///   le_u24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_u24;
///
/// let parser = |s| {
///   le_u24::<_, Error<_>>(s)
/// };
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
    trace("le_u24", move |input: I| {
        if input.is_partial() {
            streaming::le_u24(input)
        } else {
            complete::le_u24(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u32;
///
/// let parser = |s| {
///   le_u32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_u32;
///
/// let parser = |s| {
///   le_u32::<_, Error<_>>(s)
/// };
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
    trace("le_u32", move |input: I| {
        if input.is_partial() {
            streaming::le_u32(input)
        } else {
            complete::le_u32(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u64;
///
/// let parser = |s| {
///   le_u64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_u64;
///
/// let parser = |s| {
///   le_u64::<_, Error<_>>(s)
/// };
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
    trace("le_u64", move |input: I| {
        if input.is_partial() {
            streaming::le_u64(input)
        } else {
            complete::le_u64(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_u128;
///
/// let parser = |s| {
///   le_u128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_u128;
///
/// let parser = |s| {
///   le_u128::<_, Error<_>>(s)
/// };
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
    trace("le_u128", move |input: I| {
        if input.is_partial() {
            streaming::le_u128(input)
        } else {
            complete::le_u128(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i8;
///
/// let parser = |s| {
///   le_i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_i8;
///
/// let parser = le_i8::<_, Error<_>>;
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
    trace("le_i8", move |input: I| {
        if input.is_partial() {
            streaming::le_i8(input)
        } else {
            complete::le_i8(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i16;
///
/// let parser = |s| {
///   le_i16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_i16;
///
/// let parser = |s| {
///   le_i16::<_, Error<_>>(s)
/// };
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
        if input.is_partial() {
            streaming::le_i16(input)
        } else {
            complete::le_i16(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i24;
///
/// let parser = |s| {
///   le_i24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_i24;
///
/// let parser = |s| {
///   le_i24::<_, Error<_>>(s)
/// };
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
        if input.is_partial() {
            streaming::le_i24(input)
        } else {
            complete::le_i24(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i32;
///
/// let parser = |s| {
///   le_i32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_i32;
///
/// let parser = |s| {
///   le_i32::<_, Error<_>>(s)
/// };
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
        if input.is_partial() {
            streaming::le_i32(input)
        } else {
            complete::le_i32(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i64;
///
/// let parser = |s| {
///   le_i64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_i64;
///
/// let parser = |s| {
///   le_i64::<_, Error<_>>(s)
/// };
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
        if input.is_partial() {
            streaming::le_i64(input)
        } else {
            complete::le_i64(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_i128;
///
/// let parser = |s| {
///   le_i128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(ErrMode::Backtrack(Error::new(&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_i128;
///
/// let parser = |s| {
///   le_i128::<_, Error<_>>(s)
/// };
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
        if input.is_partial() {
            streaming::le_i128(input)
        } else {
            complete::le_i128(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::u8;
///
/// let parser = |s| {
///   u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u8;
///
/// let parser = |s| {
///   u8::<_, Error<_>>(s)
/// };
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
            streaming::u8(input)
        } else {
            complete::u8(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::u16;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u16;
///
/// let be_u16 = |s| {
///   u16::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u16(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0003)));
/// assert_eq!(be_u16(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// let le_u16 = |s| {
///   u16::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u16(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0300)));
/// assert_eq!(le_u16(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn u16<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, u16, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("u16", move |input: I| {
        if input.is_partial() {
            streaming::u16(endian)
        } else {
            complete::u16(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::u24;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u24;
///
/// let be_u24 = |s| {
///   u24::<_,Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u24(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x000305)));
/// assert_eq!(be_u24(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
///
/// let le_u24 = |s| {
///   u24::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u24(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x050300)));
/// assert_eq!(le_u24(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn u24<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, u32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("u23", move |input: I| {
        if input.is_partial() {
            streaming::u24(endian)
        } else {
            complete::u24(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::u32;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u32;
///
/// let be_u32 = |s| {
///   u32::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u32(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00030507)));
/// assert_eq!(be_u32(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
///
/// let le_u32 = |s| {
///   u32::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u32(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07050300)));
/// assert_eq!(le_u32(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn u32<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, u32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("u32", move |input: I| {
        if input.is_partial() {
            streaming::u32(endian)
        } else {
            complete::u32(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::u64;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u64;
///
/// let be_u64 = |s| {
///   u64::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u64(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0001020304050607)));
/// assert_eq!(be_u64(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
///
/// let le_u64 = |s| {
///   u64::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u64(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0706050403020100)));
/// assert_eq!(le_u64(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn u64<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, u64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("u64", move |input: I| {
        if input.is_partial() {
            streaming::u64(endian)
        } else {
            complete::u64(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::u128;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::u128;
///
/// let be_u128 = |s| {
///   u128::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u128(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00010203040506070001020304050607)));
/// assert_eq!(be_u128(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
///
/// let le_u128 = |s| {
///   u128::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u128(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07060504030201000706050403020100)));
/// assert_eq!(le_u128(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn u128<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, u128, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("u128", move |input: I| {
        if input.is_partial() {
            streaming::u128(endian)
        } else {
            complete::u128(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::i8;
///
/// let parser = |s| {
///   i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i8;
///
/// let parser = |s| {
///   i8::<_, Error<_>>(s)
/// };
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
            streaming::i8(input)
        } else {
            complete::i8(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::i16;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i16;
///
/// let be_i16 = |s| {
///   i16::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i16(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0003)));
/// assert_eq!(be_i16(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// let le_i16 = |s| {
///   i16::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i16(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0300)));
/// assert_eq!(le_i16(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn i16<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, i16, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("i16", move |input: I| {
        if input.is_partial() {
            streaming::i16(endian)
        } else {
            complete::i16(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::i24;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i24;
///
/// let be_i24 = |s| {
///   i24::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i24(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x000305)));
/// assert_eq!(be_i24(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
///
/// let le_i24 = |s| {
///   i24::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i24(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x050300)));
/// assert_eq!(le_i24(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn i24<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, i32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("i24", move |input: I| {
        if input.is_partial() {
            streaming::i24(endian)
        } else {
            complete::i24(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::i32;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i32;
///
/// let be_i32 = |s| {
///   i32::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i32(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00030507)));
/// assert_eq!(be_i32(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
///
/// let le_i32 = |s| {
///   i32::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i32(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07050300)));
/// assert_eq!(le_i32(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn i32<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, i32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("i32", move |input: I| {
        if input.is_partial() {
            streaming::i32(endian)
        } else {
            complete::i32(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::i64;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i64;
///
/// let be_i64 = |s| {
///   i64::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i64(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0001020304050607)));
/// assert_eq!(be_i64(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
///
/// let le_i64 = |s| {
///   i64::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i64(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0706050403020100)));
/// assert_eq!(le_i64(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn i64<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, i64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("i64", move |input: I| {
        if input.is_partial() {
            streaming::i64(endian)
        } else {
            complete::i64(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::i128;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::i128;
///
/// let be_i128 = |s| {
///   i128::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i128(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00010203040506070001020304050607)));
/// assert_eq!(be_i128(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
///
/// let le_i128 = |s| {
///   i128::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i128(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07060504030201000706050403020100)));
/// assert_eq!(le_i128(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn i128<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, i128, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("i128", move |input: I| {
        if input.is_partial() {
            streaming::i128(endian)
        } else {
            complete::i128(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_f32;
///
/// let parser = |s| {
///   be_f32(s)
/// };
///
/// assert_eq!(parser(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_f32;
///
/// let parser = |s| {
///   be_f32::<_, Error<_>>(s)
/// };
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
        if input.is_partial() {
            streaming::be_f32(input)
        } else {
            complete::be_f32(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::be_f64;
///
/// let parser = |s| {
///   be_f64(s)
/// };
///
/// assert_eq!(parser(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::be_f64;
///
/// let parser = |s| {
///   be_f64::<_, Error<_>>(s)
/// };
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
        if input.is_partial() {
            streaming::be_f64(input)
        } else {
            complete::be_f64(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_f32;
///
/// let parser = |s| {
///   le_f32(s)
/// };
///
/// assert_eq!(parser(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_f32;
///
/// let parser = |s| {
///   le_f32::<_, Error<_>>(s)
/// };
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
        if input.is_partial() {
            streaming::le_f32(input)
        } else {
            complete::le_f32(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::le_f64;
///
/// let parser = |s| {
///   le_f64(s)
/// };
///
/// assert_eq!(parser(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(ErrMode::Backtrack(Error::new(&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::Partial;
/// use winnow::number::le_f64;
///
/// let parser = |s| {
///   le_f64::<_, Error<_>>(s)
/// };
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
    trace("le_f64", move |input: I| {
        if input.is_partial() {
            streaming::le_f64(input)
        } else {
            complete::le_f64(input)
        }
    })(input)
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::f32;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::f32;
///
/// let be_f32 = |s| {
///   f32::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_f32(Partial::new(&[0x41, 0x48, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(be_f32(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// let le_f32 = |s| {
///   f32::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_f32(Partial::new(&[0x00, 0x00, 0x48, 0x41][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(le_f32(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn f32<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, f32, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("f32", move |input: I| {
        if input.is_partial() {
            streaming::f32(endian)
        } else {
            complete::f32(endian)
        }
    }(input))
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
/// # use winnow::error::Needed::Size;
/// use winnow::number::f64;
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
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::number::f64;
///
/// let be_f64 = |s| {
///   f64::<_, Error<_>>(winnow::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_f64(Partial::new(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(be_f64(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(5))));
///
/// let le_f64 = |s| {
///   f64::<_, Error<_>>(winnow::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_f64(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(le_f64(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(5))));
/// ```
#[inline(always)]
pub fn f64<I, E: ParseError<I>>(
    endian: crate::number::Endianness,
) -> impl FnMut(I) -> IResult<I, f64, E>
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    <I as Stream>::Slice: AsBytes,
{
    trace("f64", move |input: I|{
        if input.is_partial() {
            streaming::f64(endian)
        } else {
            complete::f64(endian)
        }
    }(input))
}
