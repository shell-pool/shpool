//! Bit level parsers
//!

#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub mod complete;
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub mod streaming;
#[cfg(test)]
mod tests;

use crate::error::{ErrMode, ErrorConvert, ErrorKind, Needed, ParseError};
use crate::lib::std::ops::{AddAssign, Shl, Shr};
use crate::stream::{AsBytes, Stream, StreamIsPartial, ToUsize};
use crate::trace::trace;
use crate::{IResult, Parser};

/// Converts a byte-level input to a bit-level input
///
/// See [`bytes`] to convert it back.
///
/// # Example
/// ```
/// use winnow::prelude::*;
/// use winnow::Bytes;
/// use winnow::bits::{bits, take};
/// use winnow::error::Error;
///
/// type Stream<'i> = &'i Bytes;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Bytes::new(b)
/// }
///
/// fn parse(input: Stream<'_>) -> IResult<Stream<'_>, (u8, u8)> {
///     bits::<_, _, Error<(_, usize)>, _, _>((take(4usize), take(8usize)))(input)
/// }
///
/// let input = stream(&[0x12, 0x34, 0xff, 0xff]);
///
/// let output = parse(input).expect("We take 1.5 bytes and the input is longer than 2 bytes");
///
/// // The first byte is consumed, the second byte is partially consumed and dropped.
/// let remaining = output.0;
/// assert_eq!(remaining, stream(&[0xff, 0xff]));
///
/// let parsed = output.1;
/// assert_eq!(parsed.0, 0x01);
/// assert_eq!(parsed.1, 0x23);
/// ```
pub fn bits<I, O, E1, E2, P>(mut parser: P) -> impl FnMut(I) -> IResult<I, O, E2>
where
    E1: ParseError<(I, usize)> + ErrorConvert<E2>,
    E2: ParseError<I>,
    I: Stream,
    P: Parser<(I, usize), O, E1>,
{
    trace("bits", move |input: I| {
        match parser.parse_next((input, 0)) {
            Ok(((rest, offset), result)) => {
                // If the next byte has been partially read, it will be sliced away as well.
                // The parser functions might already slice away all fully read bytes.
                // That's why `offset / 8` isn't necessarily needed at all times.
                let remaining_bytes_index = offset / 8 + if offset % 8 == 0 { 0 } else { 1 };
                let (input, _) = rest.next_slice(remaining_bytes_index);
                Ok((input, result))
            }
            Err(ErrMode::Incomplete(n)) => Err(ErrMode::Incomplete(n.map(|u| u.get() / 8 + 1))),
            Err(e) => Err(e.convert()),
        }
    })
}

/// Convert a [`bits`] stream back into a byte stream
///
/// **Warning:** A partial byte remaining in the input will be ignored and the given parser will
/// start parsing at the next full byte.
///
/// ```
/// use winnow::prelude::*;
/// use winnow::Bytes;
/// use winnow::bits::{bits, bytes, take};
/// use winnow::combinator::rest;
/// use winnow::error::Error;
///
/// type Stream<'i> = &'i Bytes;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Bytes::new(b)
/// }
///
/// fn parse(input: Stream<'_>) -> IResult<Stream<'_>, (u8, u8, &[u8])> {
///   bits::<_, _, Error<(_, usize)>, _, _>((
///     take(4usize),
///     take(8usize),
///     bytes::<_, _, Error<_>, _, _>(rest)
///   ))(input)
/// }
///
/// let input = stream(&[0x12, 0x34, 0xff, 0xff]);
///
/// assert_eq!(parse(input), Ok(( stream(&[]), (0x01, 0x23, &[0xff, 0xff][..]) )));
/// ```
pub fn bytes<I, O, E1, E2, P>(mut parser: P) -> impl FnMut((I, usize)) -> IResult<(I, usize), O, E2>
where
    E1: ParseError<I> + ErrorConvert<E2>,
    E2: ParseError<(I, usize)>,
    I: Stream<Token = u8>,
    P: Parser<I, O, E1>,
{
    trace("bytes", move |(input, offset): (I, usize)| {
        let (inner, _) = if offset % 8 != 0 {
            input.next_slice(1 + offset / 8)
        } else {
            input.next_slice(offset / 8)
        };
        let i = (input, offset);
        match parser.parse_next(inner) {
            Ok((rest, res)) => Ok(((rest, 0), res)),
            Err(ErrMode::Incomplete(Needed::Unknown)) => Err(ErrMode::Incomplete(Needed::Unknown)),
            Err(ErrMode::Incomplete(Needed::Size(sz))) => Err(match sz.get().checked_mul(8) {
                Some(v) => ErrMode::Incomplete(Needed::new(v)),
                None => ErrMode::Cut(E2::from_error_kind(i, ErrorKind::TooLarge)),
            }),
            Err(e) => Err(e.convert()),
        }
    })
}

/// Parse taking `count` bits
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::Bytes;
/// # use winnow::error::{Error, ErrorKind};
/// use winnow::bits::take;
///
/// type Stream<'i> = &'i Bytes;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Bytes::new(b)
/// }
///
/// fn parser(input: (Stream<'_>, usize), count: usize)-> IResult<(Stream<'_>, usize), u8> {
///  take(count)(input)
/// }
///
/// // Consumes 0 bits, returns 0
/// assert_eq!(parser((stream(&[0b00010010]), 0), 0), Ok(((stream(&[0b00010010]), 0), 0)));
///
/// // Consumes 4 bits, returns their values and increase offset to 4
/// assert_eq!(parser((stream(&[0b00010010]), 0), 4), Ok(((stream(&[0b00010010]), 4), 0b00000001)));
///
/// // Consumes 4 bits, offset is 4, returns their values and increase offset to 0 of next byte
/// assert_eq!(parser((stream(&[0b00010010]), 4), 4), Ok(((stream(&[]), 0), 0b00000010)));
///
/// // Tries to consume 12 bits but only 8 are available
/// assert_eq!(parser((stream(&[0b00010010]), 0), 12), Err(winnow::error::ErrMode::Backtrack(Error{input: (stream(&[0b00010010]), 0), kind: ErrorKind::Eof })));
/// ```
#[inline(always)]
pub fn take<I, O, C, E: ParseError<(I, usize)>>(
    count: C,
) -> impl FnMut((I, usize)) -> IResult<(I, usize), O, E>
where
    I: Stream<Token = u8> + AsBytes + StreamIsPartial,
    C: ToUsize,
    O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O>,
{
    let count = count.to_usize();
    trace("take", move |input: (I, usize)| {
        if input.is_partial() {
            streaming::take_internal(input, count)
        } else {
            complete::take_internal(input, count)
        }
    })
}

/// Parse taking `count` bits and comparing them to `pattern`
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::Bytes;
/// # use winnow::error::{Error, ErrorKind};
/// use winnow::bits::tag;
///
/// type Stream<'i> = &'i Bytes;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Bytes::new(b)
/// }
///
/// /// Compare the lowest `count` bits of `input` against the lowest `count` bits of `pattern`.
/// /// Return Ok and the matching section of `input` if there's a match.
/// /// Return Err if there's no match.
/// fn parser(pattern: u8, count: u8, input: (Stream<'_>, usize)) -> IResult<(Stream<'_>, usize), u8> {
///     tag(pattern, count)(input)
/// }
///
/// // The lowest 4 bits of 0b00001111 match the lowest 4 bits of 0b11111111.
/// assert_eq!(
///     parser(0b0000_1111, 4, (stream(&[0b1111_1111]), 0)),
///     Ok(((stream(&[0b1111_1111]), 4), 0b0000_1111))
/// );
///
/// // The lowest bit of 0b00001111 matches the lowest bit of 0b11111111 (both are 1).
/// assert_eq!(
///     parser(0b00000001, 1, (stream(&[0b11111111]), 0)),
///     Ok(((stream(&[0b11111111]), 1), 0b00000001))
/// );
///
/// // The lowest 2 bits of 0b11111111 and 0b00000001 are different.
/// assert_eq!(
///     parser(0b000000_01, 2, (stream(&[0b111111_11]), 0)),
///     Err(winnow::error::ErrMode::Backtrack(Error {
///         input: (stream(&[0b11111111]), 0),
///         kind: ErrorKind::TagBits
///     }))
/// );
///
/// // The lowest 8 bits of 0b11111111 and 0b11111110 are different.
/// assert_eq!(
///     parser(0b11111110, 8, (stream(&[0b11111111]), 0)),
///     Err(winnow::error::ErrMode::Backtrack(Error {
///         input: (stream(&[0b11111111]), 0),
///         kind: ErrorKind::TagBits
///     }))
/// );
/// ```
#[inline(always)]
#[doc(alias = "literal")]
#[doc(alias = "just")]
pub fn tag<I, O, C, E: ParseError<(I, usize)>>(
    pattern: O,
    count: C,
) -> impl FnMut((I, usize)) -> IResult<(I, usize), O, E>
where
    I: Stream<Token = u8> + AsBytes + StreamIsPartial,
    C: ToUsize,
    O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O> + PartialEq,
{
    let count = count.to_usize();
    trace("tag", move |input: (I, usize)| {
        if input.is_partial() {
            streaming::tag_internal(input, &pattern, count)
        } else {
            complete::tag_internal(input, &pattern, count)
        }
    })
}

/// Parses one specific bit as a bool.
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::Bytes;
/// # use winnow::error::{Error, ErrorKind};
/// use winnow::bits::bool;
///
/// type Stream<'i> = &'i Bytes;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Bytes::new(b)
/// }
///
/// fn parse(input: (Stream<'_>, usize)) -> IResult<(Stream<'_>, usize), bool> {
///     bool(input)
/// }
///
/// assert_eq!(parse((stream(&[0b10000000]), 0)), Ok(((stream(&[0b10000000]), 1), true)));
/// assert_eq!(parse((stream(&[0b10000000]), 1)), Ok(((stream(&[0b10000000]), 2), false)));
/// ```
#[doc(alias = "any")]
pub fn bool<I, E: ParseError<(I, usize)>>(input: (I, usize)) -> IResult<(I, usize), bool, E>
where
    I: Stream<Token = u8> + AsBytes + StreamIsPartial,
{
    #![allow(deprecated)]
    trace("bool", |input: (I, usize)| {
        if input.is_partial() {
            streaming::bool(input)
        } else {
            complete::bool(input)
        }
    })(input)
}
