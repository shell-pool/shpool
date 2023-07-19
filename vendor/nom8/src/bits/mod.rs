//! Bit level parsers
//!

pub mod complete;
pub mod streaming;
#[cfg(test)]
mod tests;

use crate::error::{ErrorKind, ParseError};
use crate::input::{ErrorConvert, InputIsStreaming, InputIter, InputLength, Slice, ToUsize};
use crate::lib::std::ops::{AddAssign, RangeFrom, Shl, Shr};
use crate::{Err, IResult, Needed, Parser};

/// Converts a byte-level input to a bit-level input, for consumption by a parser that uses bits.
///
/// Afterwards, the input is converted back to a byte-level parser, with any remaining bits thrown
/// away.
///
/// # Example
/// ```
/// use nom8::bits::{bits, take};
/// use nom8::error::Error;
/// use nom8::IResult;
///
/// fn parse(input: &[u8]) -> IResult<&[u8], (u8, u8)> {
///     bits::<_, _, Error<(&[u8], usize)>, _, _>((take(4usize), take(8usize)))(input)
/// }
///
/// let input = &[0x12, 0x34, 0xff, 0xff];
///
/// let output = parse(input).expect("We take 1.5 bytes and the input is longer than 2 bytes");
///
/// // The first byte is consumed, the second byte is partially consumed and dropped.
/// let remaining = output.0;
/// assert_eq!(remaining, [0xff, 0xff]);
///
/// let parsed = output.1;
/// assert_eq!(parsed.0, 0x01);
/// assert_eq!(parsed.1, 0x23);
/// ```
pub fn bits<I, O, E1, E2, P>(mut parser: P) -> impl FnMut(I) -> IResult<I, O, E2>
where
  E1: ParseError<(I, usize)> + ErrorConvert<E2>,
  E2: ParseError<I>,
  I: Slice<RangeFrom<usize>>,
  P: Parser<(I, usize), O, E1>,
{
  move |input: I| match parser.parse((input, 0)) {
    Ok(((rest, offset), result)) => {
      // If the next byte has been partially read, it will be sliced away as well.
      // The parser functions might already slice away all fully read bytes.
      // That's why `offset / 8` isn't necessarily needed at all times.
      let remaining_bytes_index = offset / 8 + if offset % 8 == 0 { 0 } else { 1 };
      Ok((rest.slice(remaining_bytes_index..), result))
    }
    Err(Err::Incomplete(n)) => Err(Err::Incomplete(n.map(|u| u.get() / 8 + 1))),
    Err(Err::Error(e)) => Err(Err::Error(e.convert())),
    Err(Err::Failure(e)) => Err(Err::Failure(e.convert())),
  }
}

/// Counterpart to `bits`, `bytes` transforms its bit stream input into a byte slice for the underlying
/// parser, allowing byte-slice parsers to work on bit streams.
///
/// A partial byte remaining in the input will be ignored and the given parser will start parsing
/// at the next full byte.
///
/// ```
/// use nom8::bits::{bits, bytes, take};
/// use nom8::combinator::rest;
/// use nom8::error::Error;
/// use nom8::IResult;
///
/// fn parse(input: &[u8]) -> IResult<&[u8], (u8, u8, &[u8])> {
///   bits::<_, _, Error<(&[u8], usize)>, _, _>((
///     take(4usize),
///     take(8usize),
///     bytes::<_, _, Error<&[u8]>, _, _>(rest)
///   ))(input)
/// }
///
/// let input = &[0x12, 0x34, 0xff, 0xff];
///
/// assert_eq!(parse( input ), Ok(( &[][..], (0x01, 0x23, &[0xff, 0xff][..]) )));
/// ```
pub fn bytes<I, O, E1, E2, P>(mut parser: P) -> impl FnMut((I, usize)) -> IResult<(I, usize), O, E2>
where
  E1: ParseError<I> + ErrorConvert<E2>,
  E2: ParseError<(I, usize)>,
  I: Slice<RangeFrom<usize>> + Clone,
  P: Parser<I, O, E1>,
{
  move |(input, offset): (I, usize)| {
    let inner = if offset % 8 != 0 {
      input.slice((1 + offset / 8)..)
    } else {
      input.slice((offset / 8)..)
    };
    let i = (input, offset);
    match parser.parse(inner) {
      Ok((rest, res)) => Ok(((rest, 0), res)),
      Err(Err::Incomplete(Needed::Unknown)) => Err(Err::Incomplete(Needed::Unknown)),
      Err(Err::Incomplete(Needed::Size(sz))) => Err(match sz.get().checked_mul(8) {
        Some(v) => Err::Incomplete(Needed::new(v)),
        None => Err::Failure(E2::from_error_kind(i, ErrorKind::TooLarge)),
      }),
      Err(Err::Error(e)) => Err(Err::Error(e.convert())),
      Err(Err::Failure(e)) => Err(Err::Failure(e.convert())),
    }
  }
}

/// Generates a parser taking `count` bits
///
/// # Example
/// ```rust
/// # use nom8::bits::take;
/// # use nom8::IResult;
/// # use nom8::error::{Error, ErrorKind};
/// // Input is a tuple of (input: I, bit_offset: usize)
/// fn parser(input: (&[u8], usize), count: usize)-> IResult<(&[u8], usize), u8> {
///  take(count)(input)
/// }
///
/// // Consumes 0 bits, returns 0
/// assert_eq!(parser(([0b00010010].as_ref(), 0), 0), Ok((([0b00010010].as_ref(), 0), 0)));
///
/// // Consumes 4 bits, returns their values and increase offset to 4
/// assert_eq!(parser(([0b00010010].as_ref(), 0), 4), Ok((([0b00010010].as_ref(), 4), 0b00000001)));
///
/// // Consumes 4 bits, offset is 4, returns their values and increase offset to 0 of next byte
/// assert_eq!(parser(([0b00010010].as_ref(), 4), 4), Ok((([].as_ref(), 0), 0b00000010)));
///
/// // Tries to consume 12 bits but only 8 are available
/// assert_eq!(parser(([0b00010010].as_ref(), 0), 12), Err(nom8::Err::Error(Error{input: ([0b00010010].as_ref(), 0), code: ErrorKind::Eof })));
/// ```
#[inline(always)]
pub fn take<I, O, C, E: ParseError<(I, usize)>, const STREAMING: bool>(
  count: C,
) -> impl Fn((I, usize)) -> IResult<(I, usize), O, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + InputIsStreaming<STREAMING>,
  C: ToUsize,
  O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O>,
{
  let count = count.to_usize();
  move |input: (I, usize)| {
    if STREAMING {
      streaming::take_internal(input, count)
    } else {
      complete::take_internal(input, count)
    }
  }
}

/// Generates a parser taking `count` bits and comparing them to `pattern`
#[inline(always)]
pub fn tag<I, O, C, E: ParseError<(I, usize)>, const STREAMING: bool>(
  pattern: O,
  count: C,
) -> impl Fn((I, usize)) -> IResult<(I, usize), O, E>
where
  I: Slice<RangeFrom<usize>>
    + InputIter<Item = u8>
    + InputLength
    + InputIsStreaming<STREAMING>
    + Clone,
  C: ToUsize,
  O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O> + PartialEq,
{
  let count = count.to_usize();
  move |input: (I, usize)| {
    if STREAMING {
      streaming::tag_internal(input, &pattern, count)
    } else {
      complete::tag_internal(input, &pattern, count)
    }
  }
}
