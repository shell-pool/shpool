//! Bit level parsers
//!

#![allow(deprecated)]

use crate::error::{ErrMode, ErrorKind, ParseError};
use crate::lib::std::ops::{AddAssign, Div, Shl, Shr};
use crate::stream::{AsBytes, Stream, ToUsize};
use crate::IResult;

/// Generates a parser taking `count` bits
///
/// # Example
/// ```rust
/// # use winnow::bits::complete::take;
/// # use winnow::IResult;
/// # use winnow::error::{Error, ErrorKind};
/// // Stream is a tuple of (input: I, bit_offset: usize)
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
/// assert_eq!(parser(([0b00010010].as_ref(), 0), 12), Err(winnow::error::ErrMode::Backtrack(Error{input: ([0b00010010].as_ref(), 0), kind: ErrorKind::Eof })));
/// ```
///
/// **WARNING:** Deprecated, replaced with [`winnow::bits::take`][crate::bits::take]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::bits::take`")]
pub fn take<I, O, C, E: ParseError<(I, usize)>>(
    count: C,
) -> impl Fn((I, usize)) -> IResult<(I, usize), O, E>
where
    I: Stream<Token = u8> + AsBytes,
    C: ToUsize,
    O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O>,
{
    let count = count.to_usize();
    move |input: (I, usize)| take_internal(input, count)
}

pub(crate) fn take_internal<I, O, E: ParseError<(I, usize)>>(
    (input, bit_offset): (I, usize),
    count: usize,
) -> IResult<(I, usize), O, E>
where
    I: Stream<Token = u8> + AsBytes,
    O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O>,
{
    if count == 0 {
        Ok(((input, bit_offset), 0u8.into()))
    } else {
        let cnt = (count + bit_offset).div(8);
        if input.eof_offset() * 8 < count + bit_offset {
            Err(ErrMode::from_error_kind(
                (input, bit_offset),
                ErrorKind::Eof,
            ))
        } else {
            let mut acc: O = 0_u8.into();
            let mut offset: usize = bit_offset;
            let mut remaining: usize = count;
            let mut end_offset: usize = 0;

            for byte in input.as_bytes().iter().copied().take(cnt + 1) {
                if remaining == 0 {
                    break;
                }
                let val: O = if offset == 0 {
                    byte.into()
                } else {
                    (byte << offset >> offset).into()
                };

                if remaining < 8 - offset {
                    acc += val >> (8 - offset - remaining);
                    end_offset = remaining + offset;
                    break;
                } else {
                    acc += val << (remaining - (8 - offset));
                    remaining -= 8 - offset;
                    offset = 0;
                }
            }
            let (input, _) = input.next_slice(cnt);
            Ok(((input, end_offset), acc))
        }
    }
}

/// Generates a parser taking `count` bits and comparing them to `pattern`
///
/// **WARNING:** Deprecated, replaced with [`winnow::bits::tag`][crate::bits::tag]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::bits::tag`")]
pub fn tag<I, O, C, E: ParseError<(I, usize)>>(
    pattern: O,
    count: C,
) -> impl Fn((I, usize)) -> IResult<(I, usize), O, E>
where
    I: Stream<Token = u8> + AsBytes,
    C: ToUsize,
    O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O> + PartialEq,
{
    let count = count.to_usize();
    move |input: (I, usize)| tag_internal(input, &pattern, count)
}

pub(crate) fn tag_internal<I, O, E: ParseError<(I, usize)>>(
    input: (I, usize),
    pattern: &O,
    count: usize,
) -> IResult<(I, usize), O, E>
where
    I: Stream<Token = u8> + AsBytes,
    O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O> + PartialEq,
{
    let inp = input.clone();

    take_internal(input, count).and_then(|(i, o)| {
        if *pattern == o {
            Ok((i, o))
        } else {
            Err(ErrMode::Backtrack(error_position!(inp, ErrorKind::TagBits)))
        }
    })
}

/// Parses one specific bit as a bool.
///
/// # Example
/// ```rust
/// # use winnow::bits::complete::bool;
/// # use winnow::IResult;
/// # use winnow::error::{Error, ErrorKind};
///
/// fn parse(input: (&[u8], usize)) -> IResult<(&[u8], usize), bool> {
///     bool(input)
/// }
///
/// assert_eq!(parse(([0b10000000].as_ref(), 0)), Ok((([0b10000000].as_ref(), 1), true)));
/// assert_eq!(parse(([0b10000000].as_ref(), 1)), Ok((([0b10000000].as_ref(), 2), false)));
/// ```
/// **WARNING:** Deprecated, replaced with [`winnow::bits::bool`][crate::bits::bool]
#[deprecated(since = "0.1.0", note = "Replaced with `winnow::bits::bool`")]
pub fn bool<I, E: ParseError<(I, usize)>>(input: (I, usize)) -> IResult<(I, usize), bool, E>
where
    I: Stream<Token = u8> + AsBytes,
{
    let (res, bit): (_, u32) = take(1usize)(input)?;
    Ok((res, bit != 0))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_take_0() {
        let input = [0b00010010].as_ref();
        let count = 0usize;
        assert_eq!(count, 0usize);
        let offset = 0usize;

        let result: crate::IResult<(&[u8], usize), usize> = take(count)((input, offset));

        assert_eq!(result, Ok(((input, offset), 0)));
    }

    #[test]
    fn test_take_eof() {
        let input = [0b00010010].as_ref();

        let result: crate::IResult<(&[u8], usize), usize> = take(1usize)((input, 8));

        assert_eq!(
            result,
            Err(crate::error::ErrMode::Backtrack(crate::error::Error {
                input: (input, 8),
                kind: ErrorKind::Eof
            }))
        );
    }

    #[test]
    fn test_take_span_over_multiple_bytes() {
        let input = [0b00010010, 0b00110100, 0b11111111, 0b11111111].as_ref();

        let result: crate::IResult<(&[u8], usize), usize> = take(24usize)((input, 4));

        assert_eq!(
            result,
            Ok((([0b11111111].as_ref(), 4), 0b1000110100111111111111))
        );
    }

    #[test]
    fn test_bool_0() {
        let input = [0b10000000].as_ref();

        let result: crate::IResult<(&[u8], usize), bool> = bool((input, 0));

        assert_eq!(result, Ok(((input, 1), true)));
    }

    #[test]
    fn test_bool_eof() {
        let input = [0b10000000].as_ref();

        let result: crate::IResult<(&[u8], usize), bool> = bool((input, 8));

        assert_eq!(
            result,
            Err(crate::error::ErrMode::Backtrack(crate::error::Error {
                input: (input, 8),
                kind: ErrorKind::Eof
            }))
        );
    }
}
