use super::*;
use crate::bytes::{tag, take};
use crate::error::{Error, ErrorKind};
use crate::input::Streaming;
use crate::number::be_u16;
use crate::{Err, IResult, Needed};

#[test]
fn single_element_tuples() {
  #![allow(deprecated)]
  use crate::character::alpha1;
  use crate::{error::ErrorKind, Err};

  let mut parser = tuple((alpha1,));
  assert_eq!(parser("abc123def"), Ok(("123def", ("abc",))));
  assert_eq!(
    parser("123def"),
    Err(Err::Error(("123def", ErrorKind::Alpha)))
  );
}

#[derive(PartialEq, Eq, Debug)]
struct B {
  a: u8,
  b: u8,
}

#[derive(PartialEq, Eq, Debug)]
struct C {
  a: u8,
  b: Option<u8>,
}

#[test]
fn complete() {
  use crate::bytes::tag;
  fn err_test(i: &[u8]) -> IResult<&[u8], &[u8]> {
    let (i, _) = tag("ijkl")(i)?;
    tag("mnop")(i)
  }
  let a = &b"ijklmn"[..];

  let res_a = err_test(a);
  assert_eq!(
    res_a,
    Err(Err::Error(error_position!(&b"mn"[..], ErrorKind::Tag)))
  );
}

#[test]
fn pair_test() {
  #![allow(deprecated)]
  fn pair_abc_def(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, (&[u8], &[u8])> {
    pair(tag("abc"), tag("def"))(i)
  }

  assert_eq!(
    pair_abc_def(Streaming(&b"abcdefghijkl"[..])),
    Ok((Streaming(&b"ghijkl"[..]), (&b"abc"[..], &b"def"[..])))
  );
  assert_eq!(
    pair_abc_def(Streaming(&b"ab"[..])),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    pair_abc_def(Streaming(&b"abcd"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    pair_abc_def(Streaming(&b"xxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    pair_abc_def(Streaming(&b"xxxdef"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxxdef"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    pair_abc_def(Streaming(&b"abcxxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
}

#[test]
fn separated_pair_test() {
  fn sep_pair_abc_def(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, (&[u8], &[u8])> {
    separated_pair(tag("abc"), tag(","), tag("def"))(i)
  }

  assert_eq!(
    sep_pair_abc_def(Streaming(&b"abc,defghijkl"[..])),
    Ok((Streaming(&b"ghijkl"[..]), (&b"abc"[..], &b"def"[..])))
  );
  assert_eq!(
    sep_pair_abc_def(Streaming(&b"ab"[..])),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    sep_pair_abc_def(Streaming(&b"abc,d"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    sep_pair_abc_def(Streaming(&b"xxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    sep_pair_abc_def(Streaming(&b"xxx,def"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx,def"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    sep_pair_abc_def(Streaming(&b"abc,xxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
}

#[test]
fn preceded_test() {
  fn preceded_abcd_efgh(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    preceded(tag("abcd"), tag("efgh"))(i)
  }

  assert_eq!(
    preceded_abcd_efgh(Streaming(&b"abcdefghijkl"[..])),
    Ok((Streaming(&b"ijkl"[..]), &b"efgh"[..]))
  );
  assert_eq!(
    preceded_abcd_efgh(Streaming(&b"ab"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    preceded_abcd_efgh(Streaming(&b"abcde"[..])),
    Err(Err::Incomplete(Needed::new(3)))
  );
  assert_eq!(
    preceded_abcd_efgh(Streaming(&b"xxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    preceded_abcd_efgh(Streaming(&b"xxxxdef"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxxxdef"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    preceded_abcd_efgh(Streaming(&b"abcdxxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
}

#[test]
fn terminated_test() {
  fn terminated_abcd_efgh(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    terminated(tag("abcd"), tag("efgh"))(i)
  }

  assert_eq!(
    terminated_abcd_efgh(Streaming(&b"abcdefghijkl"[..])),
    Ok((Streaming(&b"ijkl"[..]), &b"abcd"[..]))
  );
  assert_eq!(
    terminated_abcd_efgh(Streaming(&b"ab"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    terminated_abcd_efgh(Streaming(&b"abcde"[..])),
    Err(Err::Incomplete(Needed::new(3)))
  );
  assert_eq!(
    terminated_abcd_efgh(Streaming(&b"xxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    terminated_abcd_efgh(Streaming(&b"xxxxdef"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxxxdef"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    terminated_abcd_efgh(Streaming(&b"abcdxxxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxxx"[..]),
      ErrorKind::Tag
    )))
  );
}

#[test]
fn delimited_test() {
  fn delimited_abc_def_ghi(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    delimited(tag("abc"), tag("def"), tag("ghi"))(i)
  }

  assert_eq!(
    delimited_abc_def_ghi(Streaming(&b"abcdefghijkl"[..])),
    Ok((Streaming(&b"jkl"[..]), &b"def"[..]))
  );
  assert_eq!(
    delimited_abc_def_ghi(Streaming(&b"ab"[..])),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    delimited_abc_def_ghi(Streaming(&b"abcde"[..])),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    delimited_abc_def_ghi(Streaming(&b"abcdefgh"[..])),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    delimited_abc_def_ghi(Streaming(&b"xxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    delimited_abc_def_ghi(Streaming(&b"xxxdefghi"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxxdefghi"[..]),
      ErrorKind::Tag
    ),))
  );
  assert_eq!(
    delimited_abc_def_ghi(Streaming(&b"abcxxxghi"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxxghi"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    delimited_abc_def_ghi(Streaming(&b"abcdefxxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
}

#[test]
fn tuple_test() {
  #![allow(deprecated)]
  fn tuple_3(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, (u16, &[u8], &[u8])> {
    tuple((be_u16, take(3u8), tag("fg")))(i)
  }

  assert_eq!(
    tuple_3(Streaming(&b"abcdefgh"[..])),
    Ok((Streaming(&b"h"[..]), (0x6162u16, &b"cde"[..], &b"fg"[..])))
  );
  assert_eq!(
    tuple_3(Streaming(&b"abcd"[..])),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    tuple_3(Streaming(&b"abcde"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    tuple_3(Streaming(&b"abcdejk"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"jk"[..]),
      ErrorKind::Tag
    )))
  );
}

#[test]
fn unit_type() {
  #![allow(deprecated)]
  assert_eq!(
    tuple::<&'static str, (), Error<&'static str>, ()>(())("abxsbsh"),
    Ok(("abxsbsh", ()))
  );
  assert_eq!(
    tuple::<&'static str, (), Error<&'static str>, ()>(())("sdfjakdsas"),
    Ok(("sdfjakdsas", ()))
  );
  assert_eq!(
    tuple::<&'static str, (), Error<&'static str>, ()>(())(""),
    Ok(("", ()))
  );
}
