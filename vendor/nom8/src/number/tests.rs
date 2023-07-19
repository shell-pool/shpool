use super::*;

mod complete {
  use super::*;
  use crate::error::ErrorKind;
  use crate::Err;

  macro_rules! assert_parse(
    ($left: expr, $right: expr) => {
      let res: $crate::IResult<_, _, (_, ErrorKind)> = $left;
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
          0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          0x00
        ][..]
      ),
      Ok((&b""[..], 0))
    );
    assert_parse!(
      be_i128(
        &[
          0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
          0xff
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
          0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
          0xff
        ][..]
      ),
      Ok((&b""[..], -1))
    );
    assert_parse!(
      be_i128(
        &[
          0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          0x00
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
          0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          0x00
        ][..]
      ),
      Ok((&b""[..], 0))
    );
    assert_parse!(
      le_i128(
        &[
          0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
          0x7f
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
          0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
          0xff
        ][..]
      ),
      Ok((&b""[..], -1))
    );
    assert_parse!(
      le_i128(
        &[
          0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          0x80
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
      Ok((&b""[..], 185_728_392_f32))
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
      Ok((&b""[..], 185_728_392_f32))
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
      Err(Err::Error(error_position!(&b";"[..], ErrorKind::IsA)))
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
}

mod streaming {
  use super::*;
  use crate::error::ErrorKind;
  use crate::input::Streaming;
  use crate::{Err, Needed};

  macro_rules! assert_parse(
    ($left: expr, $right: expr) => {
      let res: $crate::IResult<_, _, (_, ErrorKind)> = $left;
      assert_eq!(res, $right);
    };
  );

  #[test]
  fn i8_tests() {
    assert_parse!(be_i8(Streaming(&[0x00][..])), Ok((Streaming(&b""[..]), 0)));
    assert_parse!(
      be_i8(Streaming(&[0x7f][..])),
      Ok((Streaming(&b""[..]), 127))
    );
    assert_parse!(be_i8(Streaming(&[0xff][..])), Ok((Streaming(&b""[..]), -1)));
    assert_parse!(
      be_i8(Streaming(&[0x80][..])),
      Ok((Streaming(&b""[..]), -128))
    );
    assert_parse!(
      be_i8(Streaming(&[][..])),
      Err(Err::Incomplete(Needed::new(1)))
    );
  }

  #[test]
  fn i16_tests() {
    assert_parse!(
      be_i16(Streaming(&[0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      be_i16(Streaming(&[0x7f, 0xff][..])),
      Ok((Streaming(&b""[..]), 32_767_i16))
    );
    assert_parse!(
      be_i16(Streaming(&[0xff, 0xff][..])),
      Ok((Streaming(&b""[..]), -1))
    );
    assert_parse!(
      be_i16(Streaming(&[0x80, 0x00][..])),
      Ok((Streaming(&b""[..]), -32_768_i16))
    );
    assert_parse!(
      be_i16(Streaming(&[][..])),
      Err(Err::Incomplete(Needed::new(2)))
    );
    assert_parse!(
      be_i16(Streaming(&[0x00][..])),
      Err(Err::Incomplete(Needed::new(1)))
    );
  }

  #[test]
  fn u24_tests() {
    assert_parse!(
      be_u24(Streaming(&[0x00, 0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      be_u24(Streaming(&[0x00, 0xFF, 0xFF][..])),
      Ok((Streaming(&b""[..]), 65_535_u32))
    );
    assert_parse!(
      be_u24(Streaming(&[0x12, 0x34, 0x56][..])),
      Ok((Streaming(&b""[..]), 1_193_046_u32))
    );
    assert_parse!(
      be_u24(Streaming(&[][..])),
      Err(Err::Incomplete(Needed::new(3)))
    );
    assert_parse!(
      be_u24(Streaming(&[0x00][..])),
      Err(Err::Incomplete(Needed::new(2)))
    );
    assert_parse!(
      be_u24(Streaming(&[0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(1)))
    );
  }

  #[test]
  fn i24_tests() {
    assert_parse!(
      be_i24(Streaming(&[0xFF, 0xFF, 0xFF][..])),
      Ok((Streaming(&b""[..]), -1_i32))
    );
    assert_parse!(
      be_i24(Streaming(&[0xFF, 0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), -65_536_i32))
    );
    assert_parse!(
      be_i24(Streaming(&[0xED, 0xCB, 0xAA][..])),
      Ok((Streaming(&b""[..]), -1_193_046_i32))
    );
    assert_parse!(
      be_i24(Streaming(&[][..])),
      Err(Err::Incomplete(Needed::new(3)))
    );
    assert_parse!(
      be_i24(Streaming(&[0x00][..])),
      Err(Err::Incomplete(Needed::new(2)))
    );
    assert_parse!(
      be_i24(Streaming(&[0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(1)))
    );
  }

  #[test]
  fn i32_tests() {
    assert_parse!(
      be_i32(Streaming(&[0x00, 0x00, 0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      be_i32(Streaming(&[0x7f, 0xff, 0xff, 0xff][..])),
      Ok((Streaming(&b""[..]), 2_147_483_647_i32))
    );
    assert_parse!(
      be_i32(Streaming(&[0xff, 0xff, 0xff, 0xff][..])),
      Ok((Streaming(&b""[..]), -1))
    );
    assert_parse!(
      be_i32(Streaming(&[0x80, 0x00, 0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), -2_147_483_648_i32))
    );
    assert_parse!(
      be_i32(Streaming(&[][..])),
      Err(Err::Incomplete(Needed::new(4)))
    );
    assert_parse!(
      be_i32(Streaming(&[0x00][..])),
      Err(Err::Incomplete(Needed::new(3)))
    );
    assert_parse!(
      be_i32(Streaming(&[0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(2)))
    );
    assert_parse!(
      be_i32(Streaming(&[0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(1)))
    );
  }

  #[test]
  fn i64_tests() {
    assert_parse!(
      be_i64(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      be_i64(Streaming(
        &[0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]
      )),
      Ok((Streaming(&b""[..]), 9_223_372_036_854_775_807_i64))
    );
    assert_parse!(
      be_i64(Streaming(
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]
      )),
      Ok((Streaming(&b""[..]), -1))
    );
    assert_parse!(
      be_i64(Streaming(
        &[0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Ok((Streaming(&b""[..]), -9_223_372_036_854_775_808_i64))
    );
    assert_parse!(
      be_i64(Streaming(&[][..])),
      Err(Err::Incomplete(Needed::new(8)))
    );
    assert_parse!(
      be_i64(Streaming(&[0x00][..])),
      Err(Err::Incomplete(Needed::new(7)))
    );
    assert_parse!(
      be_i64(Streaming(&[0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(6)))
    );
    assert_parse!(
      be_i64(Streaming(&[0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(5)))
    );
    assert_parse!(
      be_i64(Streaming(&[0x00, 0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(4)))
    );
    assert_parse!(
      be_i64(Streaming(&[0x00, 0x00, 0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(3)))
    );
    assert_parse!(
      be_i64(Streaming(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(2)))
    );
    assert_parse!(
      be_i64(Streaming(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(1)))
    );
  }

  #[test]
  fn i128_tests() {
    assert_parse!(
      be_i128(Streaming(
        &[
          0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          0x00
        ][..]
      )),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      be_i128(Streaming(
        &[
          0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
          0xff
        ][..]
      )),
      Ok((
        Streaming(&b""[..]),
        170_141_183_460_469_231_731_687_303_715_884_105_727_i128
      ))
    );
    assert_parse!(
      be_i128(Streaming(
        &[
          0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
          0xff
        ][..]
      )),
      Ok((Streaming(&b""[..]), -1))
    );
    assert_parse!(
      be_i128(Streaming(
        &[
          0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          0x00
        ][..]
      )),
      Ok((
        Streaming(&b""[..]),
        -170_141_183_460_469_231_731_687_303_715_884_105_728_i128
      ))
    );
    assert_parse!(
      be_i128(Streaming(&[][..])),
      Err(Err::Incomplete(Needed::new(16)))
    );
    assert_parse!(
      be_i128(Streaming(&[0x00][..])),
      Err(Err::Incomplete(Needed::new(15)))
    );
    assert_parse!(
      be_i128(Streaming(&[0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(14)))
    );
    assert_parse!(
      be_i128(Streaming(&[0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(13)))
    );
    assert_parse!(
      be_i128(Streaming(&[0x00, 0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(12)))
    );
    assert_parse!(
      be_i128(Streaming(&[0x00, 0x00, 0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(11)))
    );
    assert_parse!(
      be_i128(Streaming(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(10)))
    );
    assert_parse!(
      be_i128(Streaming(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])),
      Err(Err::Incomplete(Needed::new(9)))
    );
    assert_parse!(
      be_i128(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Err(Err::Incomplete(Needed::new(8)))
    );
    assert_parse!(
      be_i128(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Err(Err::Incomplete(Needed::new(7)))
    );
    assert_parse!(
      be_i128(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Err(Err::Incomplete(Needed::new(6)))
    );
    assert_parse!(
      be_i128(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Err(Err::Incomplete(Needed::new(5)))
    );
    assert_parse!(
      be_i128(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Err(Err::Incomplete(Needed::new(4)))
    );
    assert_parse!(
      be_i128(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Err(Err::Incomplete(Needed::new(3)))
    );
    assert_parse!(
      be_i128(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Err(Err::Incomplete(Needed::new(2)))
    );
    assert_parse!(
      be_i128(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
          [..]
      )),
      Err(Err::Incomplete(Needed::new(1)))
    );
  }

  #[test]
  fn le_i8_tests() {
    assert_parse!(le_i8(Streaming(&[0x00][..])), Ok((Streaming(&b""[..]), 0)));
    assert_parse!(
      le_i8(Streaming(&[0x7f][..])),
      Ok((Streaming(&b""[..]), 127))
    );
    assert_parse!(le_i8(Streaming(&[0xff][..])), Ok((Streaming(&b""[..]), -1)));
    assert_parse!(
      le_i8(Streaming(&[0x80][..])),
      Ok((Streaming(&b""[..]), -128))
    );
  }

  #[test]
  fn le_i16_tests() {
    assert_parse!(
      le_i16(Streaming(&[0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      le_i16(Streaming(&[0xff, 0x7f][..])),
      Ok((Streaming(&b""[..]), 32_767_i16))
    );
    assert_parse!(
      le_i16(Streaming(&[0xff, 0xff][..])),
      Ok((Streaming(&b""[..]), -1))
    );
    assert_parse!(
      le_i16(Streaming(&[0x00, 0x80][..])),
      Ok((Streaming(&b""[..]), -32_768_i16))
    );
  }

  #[test]
  fn le_u24_tests() {
    assert_parse!(
      le_u24(Streaming(&[0x00, 0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      le_u24(Streaming(&[0xFF, 0xFF, 0x00][..])),
      Ok((Streaming(&b""[..]), 65_535_u32))
    );
    assert_parse!(
      le_u24(Streaming(&[0x56, 0x34, 0x12][..])),
      Ok((Streaming(&b""[..]), 1_193_046_u32))
    );
  }

  #[test]
  fn le_i24_tests() {
    assert_parse!(
      le_i24(Streaming(&[0xFF, 0xFF, 0xFF][..])),
      Ok((Streaming(&b""[..]), -1_i32))
    );
    assert_parse!(
      le_i24(Streaming(&[0x00, 0x00, 0xFF][..])),
      Ok((Streaming(&b""[..]), -65_536_i32))
    );
    assert_parse!(
      le_i24(Streaming(&[0xAA, 0xCB, 0xED][..])),
      Ok((Streaming(&b""[..]), -1_193_046_i32))
    );
  }

  #[test]
  fn le_i32_tests() {
    assert_parse!(
      le_i32(Streaming(&[0x00, 0x00, 0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      le_i32(Streaming(&[0xff, 0xff, 0xff, 0x7f][..])),
      Ok((Streaming(&b""[..]), 2_147_483_647_i32))
    );
    assert_parse!(
      le_i32(Streaming(&[0xff, 0xff, 0xff, 0xff][..])),
      Ok((Streaming(&b""[..]), -1))
    );
    assert_parse!(
      le_i32(Streaming(&[0x00, 0x00, 0x00, 0x80][..])),
      Ok((Streaming(&b""[..]), -2_147_483_648_i32))
    );
  }

  #[test]
  fn le_i64_tests() {
    assert_parse!(
      le_i64(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      le_i64(Streaming(
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f][..]
      )),
      Ok((Streaming(&b""[..]), 9_223_372_036_854_775_807_i64))
    );
    assert_parse!(
      le_i64(Streaming(
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]
      )),
      Ok((Streaming(&b""[..]), -1))
    );
    assert_parse!(
      le_i64(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80][..]
      )),
      Ok((Streaming(&b""[..]), -9_223_372_036_854_775_808_i64))
    );
  }

  #[test]
  fn le_i128_tests() {
    assert_parse!(
      le_i128(Streaming(
        &[
          0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          0x00
        ][..]
      )),
      Ok((Streaming(&b""[..]), 0))
    );
    assert_parse!(
      le_i128(Streaming(
        &[
          0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
          0x7f
        ][..]
      )),
      Ok((
        Streaming(&b""[..]),
        170_141_183_460_469_231_731_687_303_715_884_105_727_i128
      ))
    );
    assert_parse!(
      le_i128(Streaming(
        &[
          0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
          0xff
        ][..]
      )),
      Ok((Streaming(&b""[..]), -1))
    );
    assert_parse!(
      le_i128(Streaming(
        &[
          0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          0x80
        ][..]
      )),
      Ok((
        Streaming(&b""[..]),
        -170_141_183_460_469_231_731_687_303_715_884_105_728_i128
      ))
    );
  }

  #[test]
  fn be_f32_tests() {
    assert_parse!(
      be_f32(Streaming(&[0x00, 0x00, 0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), 0_f32))
    );
    assert_parse!(
      be_f32(Streaming(&[0x4d, 0x31, 0x1f, 0xd8][..])),
      Ok((Streaming(&b""[..]), 185_728_392_f32))
    );
  }

  #[test]
  fn be_f64_tests() {
    assert_parse!(
      be_f64(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Ok((Streaming(&b""[..]), 0_f64))
    );
    assert_parse!(
      be_f64(Streaming(
        &[0x41, 0xa6, 0x23, 0xfb, 0x10, 0x00, 0x00, 0x00][..]
      )),
      Ok((Streaming(&b""[..]), 185_728_392_f64))
    );
  }

  #[test]
  fn le_f32_tests() {
    assert_parse!(
      le_f32(Streaming(&[0x00, 0x00, 0x00, 0x00][..])),
      Ok((Streaming(&b""[..]), 0_f32))
    );
    assert_parse!(
      le_f32(Streaming(&[0xd8, 0x1f, 0x31, 0x4d][..])),
      Ok((Streaming(&b""[..]), 185_728_392_f32))
    );
  }

  #[test]
  fn le_f64_tests() {
    assert_parse!(
      le_f64(Streaming(
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
      )),
      Ok((Streaming(&b""[..]), 0_f64))
    );
    assert_parse!(
      le_f64(Streaming(
        &[0x00, 0x00, 0x00, 0x10, 0xfb, 0x23, 0xa6, 0x41][..]
      )),
      Ok((Streaming(&b""[..]), 185_728_392_f64))
    );
  }

  #[test]
  fn hex_u32_tests() {
    assert_parse!(
      hex_u32(Streaming(&b";"[..])),
      Err(Err::Error(error_position!(
        Streaming(&b";"[..]),
        ErrorKind::IsA
      )))
    );
    assert_parse!(
      hex_u32(Streaming(&b"ff;"[..])),
      Ok((Streaming(&b";"[..]), 255))
    );
    assert_parse!(
      hex_u32(Streaming(&b"1be2;"[..])),
      Ok((Streaming(&b";"[..]), 7138))
    );
    assert_parse!(
      hex_u32(Streaming(&b"c5a31be2;"[..])),
      Ok((Streaming(&b";"[..]), 3_315_801_058))
    );
    assert_parse!(
      hex_u32(Streaming(&b"C5A31be2;"[..])),
      Ok((Streaming(&b";"[..]), 3_315_801_058))
    );
    assert_parse!(
      hex_u32(Streaming(&b"00c5a31be2;"[..])),
      Ok((Streaming(&b"e2;"[..]), 12_952_347))
    );
    assert_parse!(
      hex_u32(Streaming(&b"c5a31be201;"[..])),
      Ok((Streaming(&b"01;"[..]), 3_315_801_058))
    );
    assert_parse!(
      hex_u32(Streaming(&b"ffffffff;"[..])),
      Ok((Streaming(&b";"[..]), 4_294_967_295))
    );
    assert_parse!(
      hex_u32(Streaming(&b"0x1be2;"[..])),
      Ok((Streaming(&b"x1be2;"[..]), 0))
    );
    assert_parse!(
      hex_u32(Streaming(&b"12af"[..])),
      Err(Err::Incomplete(Needed::new(1)))
    );
  }

  #[test]
  fn configurable_endianness() {
    use crate::number::Endianness;

    fn be_tst16(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u16> {
      u16(Endianness::Big)(i)
    }
    fn le_tst16(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u16> {
      u16(Endianness::Little)(i)
    }
    assert_eq!(
      be_tst16(Streaming(&[0x80, 0x00])),
      Ok((Streaming(&b""[..]), 32_768_u16))
    );
    assert_eq!(
      le_tst16(Streaming(&[0x80, 0x00])),
      Ok((Streaming(&b""[..]), 128_u16))
    );

    fn be_tst32(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u32> {
      u32(Endianness::Big)(i)
    }
    fn le_tst32(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u32> {
      u32(Endianness::Little)(i)
    }
    assert_eq!(
      be_tst32(Streaming(&[0x12, 0x00, 0x60, 0x00])),
      Ok((Streaming(&b""[..]), 302_014_464_u32))
    );
    assert_eq!(
      le_tst32(Streaming(&[0x12, 0x00, 0x60, 0x00])),
      Ok((Streaming(&b""[..]), 6_291_474_u32))
    );

    fn be_tst64(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u64> {
      u64(Endianness::Big)(i)
    }
    fn le_tst64(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u64> {
      u64(Endianness::Little)(i)
    }
    assert_eq!(
      be_tst64(Streaming(&[0x12, 0x00, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00])),
      Ok((Streaming(&b""[..]), 1_297_142_246_100_992_000_u64))
    );
    assert_eq!(
      le_tst64(Streaming(&[0x12, 0x00, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00])),
      Ok((Streaming(&b""[..]), 36_028_874_334_666_770_u64))
    );

    fn be_tsti16(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, i16> {
      i16(Endianness::Big)(i)
    }
    fn le_tsti16(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, i16> {
      i16(Endianness::Little)(i)
    }
    assert_eq!(
      be_tsti16(Streaming(&[0x00, 0x80])),
      Ok((Streaming(&b""[..]), 128_i16))
    );
    assert_eq!(
      le_tsti16(Streaming(&[0x00, 0x80])),
      Ok((Streaming(&b""[..]), -32_768_i16))
    );

    fn be_tsti32(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, i32> {
      i32(Endianness::Big)(i)
    }
    fn le_tsti32(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, i32> {
      i32(Endianness::Little)(i)
    }
    assert_eq!(
      be_tsti32(Streaming(&[0x00, 0x12, 0x60, 0x00])),
      Ok((Streaming(&b""[..]), 1_204_224_i32))
    );
    assert_eq!(
      le_tsti32(Streaming(&[0x00, 0x12, 0x60, 0x00])),
      Ok((Streaming(&b""[..]), 6_296_064_i32))
    );

    fn be_tsti64(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, i64> {
      i64(Endianness::Big)(i)
    }
    fn le_tsti64(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, i64> {
      i64(Endianness::Little)(i)
    }
    assert_eq!(
      be_tsti64(Streaming(&[0x00, 0xFF, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00])),
      Ok((Streaming(&b""[..]), 71_881_672_479_506_432_i64))
    );
    assert_eq!(
      le_tsti64(Streaming(&[0x00, 0xFF, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00])),
      Ok((Streaming(&b""[..]), 36_028_874_334_732_032_i64))
    );
  }
}
