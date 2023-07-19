//! Parsers recognizing numbers

#![allow(deprecated)] // will just become `pub(crate)` later

pub mod complete;
pub mod streaming;
#[cfg(test)]
mod tests;

use crate::error::ParseError;
use crate::input::{
  AsBytes, AsChar, InputIsStreaming, InputIter, InputLength, InputTakeAtPosition, Slice,
};
use crate::lib::std::ops::{RangeFrom, RangeTo};
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
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_u8;
///
/// let parser = |s| {
///   be_u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(Err::Error((&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_u8;
///
/// let parser = |s| {
///   be_u8::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01abcd"[..])), Ok((Streaming(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_u8<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u8, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_u8(input)
  } else {
    complete::be_u8(input)
  }
}

/// Recognizes a big endian unsigned 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_u16;
///
/// let parser = |s| {
///   be_u16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_u16;
///
/// let parser = |s| {
///   be_u16::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x0001)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_u16<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u16, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_u16(input)
  } else {
    complete::be_u16(input)
  }
}

/// Recognizes a big endian unsigned 3 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_u24;
///
/// let parser = |s| {
///   be_u24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_u24;
///
/// let parser = |s| {
///   be_u24::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x000102)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn be_u24<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_u24(input)
  } else {
    complete::be_u24(input)
  }
}

/// Recognizes a big endian unsigned 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_u32;
///
/// let parser = |s| {
///   be_u32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_u32;
///
/// let parser = |s| {
///   be_u32::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x00010203)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_u32<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_u32(input)
  } else {
    complete::be_u32(input)
  }
}

/// Recognizes a big endian unsigned 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_u64;
///
/// let parser = |s| {
///   be_u64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_u64;
///
/// let parser = |s| {
///   be_u64::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x0001020304050607)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_u64<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_u64(input)
  } else {
    complete::be_u64(input)
  }
}

/// Recognizes a big endian unsigned 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_u128;
///
/// let parser = |s| {
///   be_u128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_u128;
///
/// let parser = |s| {
///   be_u128::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x00010203040506070809101112131415)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn be_u128<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u128, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_u128(input)
  } else {
    complete::be_u128(input)
  }
}

/// Recognizes a signed 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_i8;
///
/// let parser = |s| {
///   be_i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(Err::Error((&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_i8;
///
/// let parser = be_i8::<_, (_, ErrorKind), true>;
///
/// assert_eq!(parser(Streaming(&b"\x00\x01abcd"[..])), Ok((Streaming(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_i8<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i8, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_i8(input)
  } else {
    complete::be_i8(input)
  }
}

/// Recognizes a big endian signed 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_i16;
///
/// let parser = |s| {
///   be_i16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_i16;
///
/// let parser = be_i16::<_, (_, ErrorKind), true>;
///
/// assert_eq!(parser(Streaming(&b"\x00\x01abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x0001)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn be_i16<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i16, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_i16(input)
  } else {
    complete::be_i16(input)
  }
}

/// Recognizes a big endian signed 3 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_i24;
///
/// let parser = |s| {
///   be_i24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_i24;
///
/// let parser = be_i24::<_, (_, ErrorKind), true>;
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x000102)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_i24<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_i24(input)
  } else {
    complete::be_i24(input)
  }
}

/// Recognizes a big endian signed 4 bytes integer.
///
/// *Complete version*: Teturns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_i32;
///
/// let parser = |s| {
///   be_i32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_i32;
///
/// let parser = be_i32::<_, (_, ErrorKind), true>;
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x00010203)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(4))));
/// ```
#[inline(always)]
pub fn be_i32<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_i32(input)
  } else {
    complete::be_i32(input)
  }
}

/// Recognizes a big endian signed 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_i64;
///
/// let parser = |s| {
///   be_i64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_i64;
///
/// let parser = be_i64::<_, (_, ErrorKind), true>;
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x0001020304050607)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_i64<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_i64(input)
  } else {
    complete::be_i64(input)
  }
}

/// Recognizes a big endian signed 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_i128;
///
/// let parser = |s| {
///   be_i128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_i128;
///
/// let parser = be_i128::<_, (_, ErrorKind), true>;
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x00010203040506070809101112131415)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn be_i128<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i128, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_i128(input)
  } else {
    complete::be_i128(input)
  }
}

/// Recognizes an unsigned 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_u8;
///
/// let parser = |s| {
///   le_u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(Err::Error((&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_u8;
///
/// let parser = le_u8::<_, (_, ErrorKind), true>;
///
/// assert_eq!(parser(Streaming(&b"\x00\x01abcd"[..])), Ok((Streaming(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_u8<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u8, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_u8(input)
  } else {
    complete::le_u8(input)
  }
}

/// Recognizes a little endian unsigned 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_u16;
///
/// let parser = |s| {
///   le_u16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_u16;
///
/// let parser = |s| {
///   le_u16::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x0100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_u16<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u16, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_u16(input)
  } else {
    complete::le_u16(input)
  }
}

/// Recognizes a little endian unsigned 3 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_u24;
///
/// let parser = |s| {
///   le_u24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_u24;
///
/// let parser = |s| {
///   le_u24::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x020100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn le_u24<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_u24(input)
  } else {
    complete::le_u24(input)
  }
}

/// Recognizes a little endian unsigned 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_u32;
///
/// let parser = |s| {
///   le_u32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_u32;
///
/// let parser = |s| {
///   le_u32::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x03020100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_u32<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_u32(input)
  } else {
    complete::le_u32(input)
  }
}

/// Recognizes a little endian unsigned 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_u64;
///
/// let parser = |s| {
///   le_u64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_u64;
///
/// let parser = |s| {
///   le_u64::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x0706050403020100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_u64<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_u64(input)
  } else {
    complete::le_u64(input)
  }
}

/// Recognizes a little endian unsigned 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_u128;
///
/// let parser = |s| {
///   le_u128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_u128;
///
/// let parser = |s| {
///   le_u128::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x15141312111009080706050403020100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn le_u128<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u128, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_u128(input)
  } else {
    complete::le_u128(input)
  }
}

/// Recognizes a signed 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_i8;
///
/// let parser = |s| {
///   le_i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(Err::Error((&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_i8;
///
/// let parser = le_i8::<_, (_, ErrorKind), true>;
///
/// assert_eq!(parser(Streaming(&b"\x00\x01abcd"[..])), Ok((Streaming(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_i8<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i8, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_i8(input)
  } else {
    complete::le_i8(input)
  }
}

/// Recognizes a little endian signed 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_i16;
///
/// let parser = |s| {
///   le_i16(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_i16;
///
/// let parser = |s| {
///   le_i16::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x0100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_i16<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i16, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_i16(input)
  } else {
    complete::le_i16(input)
  }
}

/// Recognizes a little endian signed 3 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_i24;
///
/// let parser = |s| {
///   le_i24(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_i24;
///
/// let parser = |s| {
///   le_i24::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x020100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn le_i24<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_i24(input)
  } else {
    complete::le_i24(input)
  }
}

/// Recognizes a little endian signed 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_i32;
///
/// let parser = |s| {
///   le_i32(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_i32;
///
/// let parser = |s| {
///   le_i32::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x03020100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_i32<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_i32(input)
  } else {
    complete::le_i32(input)
  }
}

/// Recognizes a little endian signed 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_i64;
///
/// let parser = |s| {
///   le_i64(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_i64;
///
/// let parser = |s| {
///   le_i64::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x0706050403020100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_i64<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_i64(input)
  } else {
    complete::le_i64(input)
  }
}

/// Recognizes a little endian signed 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_i128;
///
/// let parser = |s| {
///   le_i128(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(parser(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_i128;
///
/// let parser = |s| {
///   le_i128::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Streaming(&b"abcd"[..]), 0x15141312111009080706050403020100)));
/// assert_eq!(parser(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn le_i128<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i128, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_i128(input)
  } else {
    complete::le_i128(input)
  }
}

/// Recognizes an unsigned 1 byte integer
///
/// **Note:** that endianness does not apply to 1 byte numbers.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::u8;
///
/// let parser = |s| {
///   u8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(Err::Error((&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::u8;
///
/// let parser = |s| {
///   u8::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x03abcefg"[..])), Ok((Streaming(&b"\x03abcefg"[..]), 0x00)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn u8<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u8, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::u8(input)
  } else {
    complete::u8(input)
  }
}

/// Recognizes an unsigned 2 bytes integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian u16 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian u16 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::u16;
///
/// let be_u16 = |s| {
///   u16(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(be_u16(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_u16 = |s| {
///   u16(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(le_u16(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::u16;
///
/// let be_u16 = |s| {
///   u16::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u16(Streaming(&b"\x00\x03abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x0003)));
/// assert_eq!(be_u16(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(1))));
///
/// let le_u16 = |s| {
///   u16::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u16(Streaming(&b"\x00\x03abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x0300)));
/// assert_eq!(le_u16(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn u16<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, u16, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::u16(endian)
  } else {
    complete::u16(endian)
  }
}

/// Recognizes an unsigned 3 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian u24 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian u24 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::u24;
///
/// let be_u24 = |s| {
///   u24(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(be_u24(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_u24 = |s| {
///   u24(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(le_u24(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::u24;
///
/// let be_u24 = |s| {
///   u24::<_,(_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u24(Streaming(&b"\x00\x03\x05abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x000305)));
/// assert_eq!(be_u24(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(2))));
///
/// let le_u24 = |s| {
///   u24::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u24(Streaming(&b"\x00\x03\x05abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x050300)));
/// assert_eq!(le_u24(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn u24<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, u32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::u24(endian)
  } else {
    complete::u24(endian)
  }
}

/// Recognizes an unsigned 4 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian u32 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian u32 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::u32;
///
/// let be_u32 = |s| {
///   u32(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(be_u32(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_u32 = |s| {
///   u32(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(le_u32(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::u32;
///
/// let be_u32 = |s| {
///   u32::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u32(Streaming(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x00030507)));
/// assert_eq!(be_u32(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(3))));
///
/// let le_u32 = |s| {
///   u32::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u32(Streaming(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x07050300)));
/// assert_eq!(le_u32(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn u32<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, u32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::u32(endian)
  } else {
    complete::u32(endian)
  }
}

/// Recognizes an unsigned 8 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian u64 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian u64 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::u64;
///
/// let be_u64 = |s| {
///   u64(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(be_u64(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_u64 = |s| {
///   u64(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(le_u64(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::u64;
///
/// let be_u64 = |s| {
///   u64::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u64(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x0001020304050607)));
/// assert_eq!(be_u64(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(7))));
///
/// let le_u64 = |s| {
///   u64::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u64(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x0706050403020100)));
/// assert_eq!(le_u64(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn u64<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, u64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::u64(endian)
  } else {
    complete::u64(endian)
  }
}

/// Recognizes an unsigned 16 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian u128 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian u128 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::u128;
///
/// let be_u128 = |s| {
///   u128(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(be_u128(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_u128 = |s| {
///   u128(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(le_u128(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::u128;
///
/// let be_u128 = |s| {
///   u128::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_u128(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x00010203040506070001020304050607)));
/// assert_eq!(be_u128(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(15))));
///
/// let le_u128 = |s| {
///   u128::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_u128(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x07060504030201000706050403020100)));
/// assert_eq!(le_u128(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn u128<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, u128, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::u128(endian)
  } else {
    complete::u128(endian)
  }
}

/// Recognizes a signed 1 byte integer
///
/// **Note:** that endianness does not apply to 1 byte numbers.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::i8;
///
/// let parser = |s| {
///   i8(s)
/// };
///
/// assert_eq!(parser(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert_eq!(parser(&b""[..]), Err(Err::Error((&[][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::i8;
///
/// let parser = |s| {
///   i8::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"\x00\x03abcefg"[..])), Ok((Streaming(&b"\x03abcefg"[..]), 0x00)));
/// assert_eq!(parser(Streaming(&b""[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn i8<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, i8, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::i8(input)
  } else {
    complete::i8(input)
  }
}

/// Recognizes a signed 2 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian i16 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian i16 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::i16;
///
/// let be_i16 = |s| {
///   i16(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert_eq!(be_i16(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_i16 = |s| {
///   i16(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i16(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert_eq!(le_i16(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::i16;
///
/// let be_i16 = |s| {
///   i16::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i16(Streaming(&b"\x00\x03abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x0003)));
/// assert_eq!(be_i16(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(1))));
///
/// let le_i16 = |s| {
///   i16::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i16(Streaming(&b"\x00\x03abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x0300)));
/// assert_eq!(le_i16(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn i16<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, i16, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::i16(endian)
  } else {
    complete::i16(endian)
  }
}

/// Recognizes a signed 3 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian i24 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian i24 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::i24;
///
/// let be_i24 = |s| {
///   i24(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert_eq!(be_i24(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_i24 = |s| {
///   i24(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i24(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert_eq!(le_i24(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::i24;
///
/// let be_i24 = |s| {
///   i24::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i24(Streaming(&b"\x00\x03\x05abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x000305)));
/// assert_eq!(be_i24(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(2))));
///
/// let le_i24 = |s| {
///   i24::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i24(Streaming(&b"\x00\x03\x05abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x050300)));
/// assert_eq!(le_i24(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn i24<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, i32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::i24(endian)
  } else {
    complete::i24(endian)
  }
}

/// Recognizes a signed 4 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian i32 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian i32 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::i32;
///
/// let be_i32 = |s| {
///   i32(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert_eq!(be_i32(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_i32 = |s| {
///   i32(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i32(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert_eq!(le_i32(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::i32;
///
/// let be_i32 = |s| {
///   i32::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i32(Streaming(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x00030507)));
/// assert_eq!(be_i32(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(3))));
///
/// let le_i32 = |s| {
///   i32::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i32(Streaming(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x07050300)));
/// assert_eq!(le_i32(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn i32<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, i32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::i32(endian)
  } else {
    complete::i32(endian)
  }
}

/// Recognizes a signed 8 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian i64 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian i64 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::i64;
///
/// let be_i64 = |s| {
///   i64(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert_eq!(be_i64(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_i64 = |s| {
///   i64(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i64(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert_eq!(le_i64(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::i64;
///
/// let be_i64 = |s| {
///   i64::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i64(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x0001020304050607)));
/// assert_eq!(be_i64(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(7))));
///
/// let le_i64 = |s| {
///   i64::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i64(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x0706050403020100)));
/// assert_eq!(le_i64(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn i64<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, i64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::i64(endian)
  } else {
    complete::i64(endian)
  }
}

/// Recognizes a signed 16 byte integer
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian i128 integer,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian i128 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::i128;
///
/// let be_i128 = |s| {
///   i128(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert_eq!(be_i128(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
///
/// let le_i128 = |s| {
///   i128(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i128(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert_eq!(le_i128(&b"\x01"[..]), Err(Err::Error((&[0x01][..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::i128;
///
/// let be_i128 = |s| {
///   i128::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_i128(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x00010203040506070001020304050607)));
/// assert_eq!(be_i128(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(15))));
///
/// let le_i128 = |s| {
///   i128::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_i128(Streaming(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Streaming(&b"abcefg"[..]), 0x07060504030201000706050403020100)));
/// assert_eq!(le_i128(Streaming(&b"\x01"[..])), Err(Err::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn i128<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, i128, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::i128(endian)
  } else {
    complete::i128(endian)
  }
}

/// Recognizes a big endian 4 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_f32;
///
/// let parser = |s| {
///   be_f32(s)
/// };
///
/// assert_eq!(parser(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(Err::Error((&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_f32;
///
/// let parser = |s| {
///   be_f32::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&[0x40, 0x29, 0x00, 0x00][..])), Ok((Streaming(&b""[..]), 2.640625)));
/// assert_eq!(parser(Streaming(&[0x01][..])), Err(Err::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_f32<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, f32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_f32(input)
  } else {
    complete::be_f32(input)
  }
}

/// Recognizes a big endian 8 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::be_f64;
///
/// let parser = |s| {
///   be_f64(s)
/// };
///
/// assert_eq!(parser(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(Err::Error((&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::be_f64;
///
/// let parser = |s| {
///   be_f64::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])), Ok((Streaming(&b""[..]), 12.5)));
/// assert_eq!(parser(Streaming(&[0x01][..])), Err(Err::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_f64<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, f64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::be_f64(input)
  } else {
    complete::be_f64(input)
  }
}

/// Recognizes a little endian 4 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_f32;
///
/// let parser = |s| {
///   le_f32(s)
/// };
///
/// assert_eq!(parser(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(Err::Error((&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_f32;
///
/// let parser = |s| {
///   le_f32::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&[0x00, 0x00, 0x48, 0x41][..])), Ok((Streaming(&b""[..]), 12.5)));
/// assert_eq!(parser(Streaming(&[0x01][..])), Err(Err::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_f32<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, f32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_f32(input)
  } else {
    complete::le_f32(input)
  }
}

/// Recognizes a little endian 8 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::le_f64;
///
/// let parser = |s| {
///   le_f64(s)
/// };
///
/// assert_eq!(parser(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(parser(&b"abc"[..]), Err(Err::Error((&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::le_f64;
///
/// let parser = |s| {
///   le_f64::<_, (_, ErrorKind), true>(s)
/// };
///
/// assert_eq!(parser(Streaming(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x48, 0x41][..])), Ok((Streaming(&b""[..]), 3145728.0)));
/// assert_eq!(parser(Streaming(&[0x01][..])), Err(Err::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_f64<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, f64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::le_f64(input)
  } else {
    complete::le_f64(input)
  }
}

/// Recognizes a 4 byte floating point number
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian f32 float,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian f32 float.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::f32;
///
/// let be_f32 = |s| {
///   f32(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_f32(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(be_f32(&b"abc"[..]), Err(Err::Error((&b"abc"[..], ErrorKind::Eof))));
///
/// let le_f32 = |s| {
///   f32(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_f32(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(le_f32(&b"abc"[..]), Err(Err::Error((&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::f32;
///
/// let be_f32 = |s| {
///   f32::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_f32(Streaming(&[0x41, 0x48, 0x00, 0x00][..])), Ok((Streaming(&b""[..]), 12.5)));
/// assert_eq!(be_f32(Streaming(&b"abc"[..])), Err(Err::Incomplete(Needed::new(1))));
///
/// let le_f32 = |s| {
///   f32::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_f32(Streaming(&[0x00, 0x00, 0x48, 0x41][..])), Ok((Streaming(&b""[..]), 12.5)));
/// assert_eq!(le_f32(Streaming(&b"abc"[..])), Err(Err::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn f32<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, f32, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::f32(endian)
  } else {
    complete::f32(endian)
  }
}

/// Recognizes an 8 byte floating point number
///
/// If the parameter is `nom8::number::Endianness::Big`, parse a big endian f64 float,
/// otherwise if `nom8::number::Endianness::Little` parse a little endian f64 float.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::f64;
///
/// let be_f64 = |s| {
///   f64(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_f64(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(be_f64(&b"abc"[..]), Err(Err::Error((&b"abc"[..], ErrorKind::Eof))));
///
/// let le_f64 = |s| {
///   f64(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_f64(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert_eq!(le_f64(&b"abc"[..]), Err(Err::Error((&b"abc"[..], ErrorKind::Eof))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// # use nom8::input::Streaming;
/// use nom8::number::f64;
///
/// let be_f64 = |s| {
///   f64::<_, (_, ErrorKind), true>(nom8::number::Endianness::Big)(s)
/// };
///
/// assert_eq!(be_f64(Streaming(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])), Ok((Streaming(&b""[..]), 12.5)));
/// assert_eq!(be_f64(Streaming(&b"abc"[..])), Err(Err::Incomplete(Needed::new(5))));
///
/// let le_f64 = |s| {
///   f64::<_, (_, ErrorKind), true>(nom8::number::Endianness::Little)(s)
/// };
///
/// assert_eq!(le_f64(Streaming(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..])), Ok((Streaming(&b""[..]), 12.5)));
/// assert_eq!(le_f64(Streaming(&b"abc"[..])), Err(Err::Incomplete(Needed::new(5))));
/// ```
#[inline(always)]
pub fn f64<I, E: ParseError<I>, const STREAMING: bool>(
  endian: crate::number::Endianness,
) -> fn(I) -> IResult<I, f64, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
{
  if STREAMING {
    streaming::f64(endian)
  } else {
    complete::f64(endian)
  }
}

/// Recognizes a hex-encoded integer.
///
/// *Complete version*: Will parse until the end of input if it has less than 8 bytes.
///
/// *Streaming version*: Will return `Err(nom8::Err::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::Needed::Size;
/// use nom8::number::hex_u32;
///
/// let parser = |s| {
///   hex_u32(s)
/// };
///
/// assert_eq!(parser(&b"01AE"[..]), Ok((&b""[..], 0x01AE)));
/// assert_eq!(parser(&b"abc"[..]), Ok((&b""[..], 0x0ABC)));
/// assert_eq!(parser(&b"ggg"[..]), Err(Err::Error((&b"ggg"[..], ErrorKind::IsA))));
/// ```
///
/// ```rust
/// # use nom8::{Err, error::ErrorKind, Needed};
/// # use nom8::input::Streaming;
/// use nom8::number::hex_u32;
///
/// let parser = |s| {
///   hex_u32(s)
/// };
///
/// assert_eq!(parser(Streaming(&b"01AE;"[..])), Ok((Streaming(&b";"[..]), 0x01AE)));
/// assert_eq!(parser(Streaming(&b"abc"[..])), Err(Err::Incomplete(Needed::new(1))));
/// assert_eq!(parser(Streaming(&b"ggg"[..])), Err(Err::Error((Streaming(&b"ggg"[..]), ErrorKind::IsA))));
/// ```
#[inline(always)]
pub fn hex_u32<I, E: ParseError<I>, const STREAMING: bool>(input: I) -> IResult<I, u32, E>
where
  I: InputTakeAtPosition + InputIsStreaming<STREAMING>,
  I: Slice<RangeFrom<usize>> + Slice<RangeTo<usize>>,
  <I as InputTakeAtPosition>::Item: AsChar,
  I: AsBytes,
  I: InputLength,
{
  if STREAMING {
    streaming::hex_u32(input)
  } else {
    complete::hex_u32(input)
  }
}
