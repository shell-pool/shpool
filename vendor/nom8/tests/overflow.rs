#![cfg_attr(feature = "cargo-clippy", allow(unreadable_literal))]
#![cfg(target_pointer_width = "64")]

use nom8::bytes::take;
use nom8::input::Streaming;
#[cfg(feature = "alloc")]
use nom8::multi::{length_data, many0};
#[cfg(feature = "alloc")]
use nom8::number::be_u64;
use nom8::prelude::*;
use nom8::{Err, Needed};

// Parser definition

// We request a length that would trigger an overflow if computing consumed + requested
fn parser02(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, (&[u8], &[u8])> {
  (take(1_usize), take(18446744073709551615_usize)).parse(i)
}

#[test]
fn overflow_incomplete_tuple() {
  assert_eq!(
    parser02(Streaming(&b"3"[..])),
    Err(Err::Incomplete(Needed::new(18446744073709551615)))
  );
}

#[test]
#[cfg(feature = "alloc")]
fn overflow_incomplete_length_bytes() {
  fn multi(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Vec<&[u8]>> {
    many0(length_data(be_u64))(i)
  }

  // Trigger an overflow in length_data
  assert_eq!(
    multi(Streaming(
      &b"\x00\x00\x00\x00\x00\x00\x00\x01\xaa\xff\xff\xff\xff\xff\xff\xff\xff"[..]
    )),
    Err(Err::Incomplete(Needed::new(18446744073709551615)))
  );
}

#[test]
#[cfg(feature = "alloc")]
fn overflow_incomplete_many0() {
  fn multi(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Vec<&[u8]>> {
    many0(length_data(be_u64))(i)
  }

  // Trigger an overflow in many0
  assert_eq!(
    multi(Streaming(
      &b"\x00\x00\x00\x00\x00\x00\x00\x01\xaa\xff\xff\xff\xff\xff\xff\xff\xef"[..]
    )),
    Err(Err::Incomplete(Needed::new(18446744073709551599)))
  );
}

#[test]
#[cfg(feature = "alloc")]
fn overflow_incomplete_many1() {
  use nom8::multi::many1;

  fn multi(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Vec<&[u8]>> {
    many1(length_data(be_u64))(i)
  }

  // Trigger an overflow in many1
  assert_eq!(
    multi(Streaming(
      &b"\x00\x00\x00\x00\x00\x00\x00\x01\xaa\xff\xff\xff\xff\xff\xff\xff\xef"[..]
    )),
    Err(Err::Incomplete(Needed::new(18446744073709551599)))
  );
}

#[test]
#[cfg(feature = "alloc")]
fn overflow_incomplete_many_till() {
  use nom8::{bytes::tag, multi::many_till};

  fn multi(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, (Vec<&[u8]>, &[u8])> {
    many_till(length_data(be_u64), tag("abc"))(i)
  }

  // Trigger an overflow in many_till
  assert_eq!(
    multi(Streaming(
      &b"\x00\x00\x00\x00\x00\x00\x00\x01\xaa\xff\xff\xff\xff\xff\xff\xff\xef"[..]
    )),
    Err(Err::Incomplete(Needed::new(18446744073709551599)))
  );
}

#[test]
#[cfg(feature = "alloc")]
fn overflow_incomplete_many_m_n() {
  use nom8::multi::many_m_n;

  fn multi(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Vec<&[u8]>> {
    many_m_n(2, 4, length_data(be_u64))(i)
  }

  // Trigger an overflow in many_m_n
  assert_eq!(
    multi(Streaming(
      &b"\x00\x00\x00\x00\x00\x00\x00\x01\xaa\xff\xff\xff\xff\xff\xff\xff\xef"[..]
    )),
    Err(Err::Incomplete(Needed::new(18446744073709551599)))
  );
}

#[test]
#[cfg(feature = "alloc")]
fn overflow_incomplete_count() {
  use nom8::multi::count;

  fn counter(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Vec<&[u8]>> {
    count(length_data(be_u64), 2)(i)
  }

  assert_eq!(
    counter(Streaming(
      &b"\x00\x00\x00\x00\x00\x00\x00\x01\xaa\xff\xff\xff\xff\xff\xff\xff\xef"[..]
    )),
    Err(Err::Incomplete(Needed::new(18446744073709551599)))
  );
}

#[test]
#[cfg(feature = "alloc")]
fn overflow_incomplete_length_count() {
  use nom8::multi::length_count;
  use nom8::number::be_u8;

  fn multi(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Vec<&[u8]>> {
    length_count(be_u8, length_data(be_u64))(i)
  }

  assert_eq!(
    multi(Streaming(
      &b"\x04\x00\x00\x00\x00\x00\x00\x00\x01\xaa\xff\xff\xff\xff\xff\xff\xff\xee"[..]
    )),
    Err(Err::Incomplete(Needed::new(18446744073709551598)))
  );
}

#[test]
#[cfg(feature = "alloc")]
fn overflow_incomplete_length_data() {
  fn multi(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Vec<&[u8]>> {
    many0(length_data(be_u64))(i)
  }

  assert_eq!(
    multi(Streaming(
      &b"\x00\x00\x00\x00\x00\x00\x00\x01\xaa\xff\xff\xff\xff\xff\xff\xff\xff"[..]
    )),
    Err(Err::Incomplete(Needed::new(18446744073709551615)))
  );
}
