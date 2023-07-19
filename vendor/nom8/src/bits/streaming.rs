//! Bit level parsers
//!

#![allow(deprecated)]

use crate::error::{ErrorKind, ParseError};
use crate::input::{InputIter, InputLength, Slice, ToUsize};
use crate::lib::std::ops::{AddAssign, Div, RangeFrom, Shl, Shr};
use crate::{Err, IResult, Needed};

/// Generates a parser taking `count` bits
///
/// **WARNING:** Deprecated, replaced with [`nom8::bits::take`][crate::bits::take] with input wrapped in [`nom8::input::Streaming`][crate::input::Streaming]
#[deprecated(
  since = "8.0.0",
  note = "Replaced with `nom8::bits::take` with input wrapped in `nom8::input::Streaming`"
)]
pub fn take<I, O, C, E: ParseError<(I, usize)>>(
  count: C,
) -> impl Fn((I, usize)) -> IResult<(I, usize), O, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength,
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
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength,
  O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O>,
{
  if count == 0 {
    Ok(((input, bit_offset), 0u8.into()))
  } else {
    let cnt = (count + bit_offset).div(8);
    if input.input_len() * 8 < count + bit_offset {
      Err(Err::Incomplete(Needed::new(count as usize)))
    } else {
      let mut acc: O = 0_u8.into();
      let mut offset: usize = bit_offset;
      let mut remaining: usize = count;
      let mut end_offset: usize = 0;

      for byte in input.iter_elements().take(cnt + 1) {
        if remaining == 0 {
          break;
        }
        let val: O = if offset == 0 {
          byte.into()
        } else {
          ((byte << offset) as u8 >> offset).into()
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
      Ok(((input.slice(cnt..), end_offset), acc))
    }
  }
}

/// Generates a parser taking `count` bits and comparing them to `pattern`
///
/// **WARNING:** Deprecated, replaced with [`nom8::bits::tag`][crate::bits::tag] with input wrapped in [`nom8::input::Streaming`][crate::input::Streaming]
#[deprecated(
  since = "8.0.0",
  note = "Replaced with `nom8::bits::tag` with input wrapped in `nom8::input::Streaming`"
)]
pub fn tag<I, O, C, E: ParseError<(I, usize)>>(
  pattern: O,
  count: C,
) -> impl Fn((I, usize)) -> IResult<(I, usize), O, E>
where
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + Clone,
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
  I: Slice<RangeFrom<usize>> + InputIter<Item = u8> + InputLength + Clone,
  O: From<u8> + AddAssign + Shl<usize, Output = O> + Shr<usize, Output = O> + PartialEq,
{
  let inp = input.clone();

  take(count)(input).and_then(|(i, o)| {
    if *pattern == o {
      Ok((i, o))
    } else {
      Err(Err::Error(error_position!(inp, ErrorKind::TagBits)))
    }
  })
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
      Err(crate::Err::Error(crate::error::Error {
        input: (input, offset),
        code: ErrorKind::TagBits
      }))
    );
  }
}
