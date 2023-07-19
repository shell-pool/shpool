use super::*;
use crate::error::Error;
use crate::input::Streaming;

#[test]
/// Take the `bits` function and assert that remaining bytes are correctly returned, if the
/// previous bytes are fully consumed
fn test_complete_byte_consumption_bits() {
  let input = &[0x12, 0x34, 0x56, 0x78][..];

  // Take 3 bit slices with sizes [4, 8, 4].
  let result: IResult<&[u8], (u8, u8, u8)> =
    bits::<_, _, Error<(&[u8], usize)>, _, _>((take(4usize), take(8usize), take(4usize)))(input);

  let output = result.expect("We take 2 bytes and the input is longer than 2 bytes");

  let remaining = output.0;
  assert_eq!(remaining, [0x56, 0x78]);

  let parsed = output.1;
  assert_eq!(parsed.0, 0x01);
  assert_eq!(parsed.1, 0x23);
  assert_eq!(parsed.2, 0x04);
}

#[test]
/// Take the `bits` function and assert that remaining bytes are correctly returned, if the
/// previous bytes are NOT fully consumed. Partially consumed bytes are supposed to be dropped.
/// I.e. if we consume 1.5 bytes of 4 bytes, 2 bytes will be returned, bits 13-16 will be
/// dropped.
fn test_partial_byte_consumption_bits() {
  let input = &[0x12, 0x34, 0x56, 0x78][..];

  // Take bit slices with sizes [4, 8].
  let result: IResult<&[u8], (u8, u8)> =
    bits::<_, _, Error<(&[u8], usize)>, _, _>((take(4usize), take(8usize)))(input);

  let output = result.expect("We take 1.5 bytes and the input is longer than 2 bytes");

  let remaining = output.0;
  assert_eq!(remaining, [0x56, 0x78]);

  let parsed = output.1;
  assert_eq!(parsed.0, 0x01);
  assert_eq!(parsed.1, 0x23);
}

#[test]
#[cfg(feature = "std")]
/// Ensure that in Incomplete error is thrown, if too few bytes are passed for a given parser.
fn test_incomplete_bits() {
  let input = Streaming(&[0x12][..]);

  // Take bit slices with sizes [4, 8].
  let result: IResult<_, (u8, u8)> =
    bits::<_, _, Error<(_, usize)>, _, _>((take(4usize), take(8usize)))(input);

  assert!(result.is_err());
  let error = result.err().unwrap();
  assert_eq!("Parsing requires 2 bytes/chars", error.to_string());
}

#[test]
fn test_take_complete_0() {
  let input = &[0b00010010][..];
  let count = 0usize;
  assert_eq!(count, 0usize);
  let offset = 0usize;

  let result: crate::IResult<(&[u8], usize), usize> = take(count)((input, offset));

  assert_eq!(result, Ok(((input, offset), 0)));
}

#[test]
fn test_take_complete_eof() {
  let input = &[0b00010010][..];

  let result: crate::IResult<(&[u8], usize), usize> = take(1usize)((input, 8));

  assert_eq!(
    result,
    Err(crate::Err::Error(crate::error::Error {
      input: (input, 8),
      code: ErrorKind::Eof
    }))
  )
}

#[test]
fn test_take_complete_span_over_multiple_bytes() {
  let input = &[0b00010010, 0b00110100, 0b11111111, 0b11111111][..];

  let result: crate::IResult<(&[u8], usize), usize> = take(24usize)((input, 4));

  assert_eq!(
    result,
    Ok((([0b11111111].as_ref(), 4), 0b1000110100111111111111))
  );
}

#[test]
fn test_take_streaming_0() {
  let input = Streaming(&[][..]);
  let count = 0usize;
  assert_eq!(count, 0usize);
  let offset = 0usize;

  let result: crate::IResult<(_, usize), usize> = take(count)((input, offset));

  assert_eq!(result, Ok(((input, offset), 0)));
}

#[test]
fn test_tag_streaming_ok() {
  let input = Streaming(&[0b00011111][..]);
  let offset = 0usize;
  let bits_to_take = 4usize;
  let value_to_tag = 0b0001;

  let result: crate::IResult<(_, usize), usize> = tag(value_to_tag, bits_to_take)((input, offset));

  assert_eq!(result, Ok(((input, bits_to_take), value_to_tag)));
}

#[test]
fn test_tag_streaming_err() {
  let input = Streaming(&[0b00011111][..]);
  let offset = 0usize;
  let bits_to_take = 4usize;
  let value_to_tag = 0b1111;

  let result: crate::IResult<(_, usize), usize> = tag(value_to_tag, bits_to_take)((input, offset));

  assert_eq!(
    result,
    Err(crate::Err::Error(crate::error::Error {
      input: (input, offset),
      code: ErrorKind::TagBits
    }))
  );
}
