//! Bit level parsers
//!

#![allow(deprecated)]

use crate::error::{ErrMode, ErrorKind, Needed, ParseError};
use crate::lib::std::ops::{AddAssign, Div, Shl, Shr};
use crate::stream::{AsBytes, Stream, ToUsize};
use crate::IResult;

/// Generates a parser taking `count` bits
///
/// **WARNING:** Deprecated, replaced with [`winnow::bits::take`][crate::bits::take] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bits::take` with input wrapped in `winnow::Partial`"
)]
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
            Err(ErrMode::Incomplete(Needed::new(count)))
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
/// **WARNING:** Deprecated, replaced with [`winnow::bits::tag`][crate::bits::tag] with input wrapped in [`winnow::Partial`][crate::Partial]
#[deprecated(
    since = "0.1.0",
    note = "Replaced with `winnow::bits::tag` with input wrapped in `winnow::Partial`"
)]
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

    take(count)(input).and_then(|(i, o)| {
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
        let input = [].as_ref();
        let count = 0usize;
        assert_eq!(count, 0usize);
        let offset = 0usize;

        let result: crate::IResult<(&[u8], usize), usize> = take(count)((input, offset));

        assert_eq!(result, Ok(((input, offset), 0)));
    }

    #[test]
    fn test_tag_ok() {
        let input = [0b00011111].as_ref();
        let offset = 0usize;
        let bits_to_take = 4usize;
        let value_to_tag = 0b0001;

        let result: crate::IResult<(&[u8], usize), usize> =
            tag(value_to_tag, bits_to_take)((input, offset));

        assert_eq!(result, Ok(((input, bits_to_take), value_to_tag)));
    }

    #[test]
    fn test_tag_err() {
        let input = [0b00011111].as_ref();
        let offset = 0usize;
        let bits_to_take = 4usize;
        let value_to_tag = 0b1111;

        let result: crate::IResult<(&[u8], usize), usize> =
            tag(value_to_tag, bits_to_take)((input, offset));

        assert_eq!(
            result,
            Err(crate::error::ErrMode::Backtrack(crate::error::Error {
                input: (input, offset),
                kind: ErrorKind::TagBits
            }))
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
            Err(crate::error::ErrMode::Incomplete(Needed::new(1)))
        );
    }
}
