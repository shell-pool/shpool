use super::*;
use crate::bytes::tag;
use crate::bytes::take;
use crate::error::ErrorKind;
use crate::error::ParseError;
use crate::input::Streaming;
#[cfg(feature = "alloc")]
use crate::lib::std::boxed::Box;
use crate::number::u8;
use crate::Parser;
use crate::{Err, IResult, Needed};

macro_rules! assert_parse(
  ($left: expr, $right: expr) => {
    let res: $crate::IResult<_, _, (_, ErrorKind)> = $left;
    assert_eq!(res, $right);
  };
);

#[test]
fn eof_on_slices() {
  let not_over: &[u8] = &b"Hello, world!"[..];
  let is_over: &[u8] = &b""[..];

  let res_not_over = eof(not_over);
  assert_parse!(
    res_not_over,
    Err(Err::Error(error_position!(not_over, ErrorKind::Eof)))
  );

  let res_over = eof(is_over);
  assert_parse!(res_over, Ok((is_over, is_over)));
}

#[test]
fn eof_on_strs() {
  let not_over: &str = "Hello, world!";
  let is_over: &str = "";

  let res_not_over = eof(not_over);
  assert_parse!(
    res_not_over,
    Err(Err::Error(error_position!(not_over, ErrorKind::Eof)))
  );

  let res_over = eof(is_over);
  assert_parse!(res_over, Ok((is_over, is_over)));
}

#[test]
fn rest_on_slices() {
  let input: &[u8] = &b"Hello, world!"[..];
  let empty: &[u8] = &b""[..];
  assert_parse!(rest(input), Ok((empty, input)));
}

#[test]
fn rest_on_strs() {
  let input: &str = "Hello, world!";
  let empty: &str = "";
  assert_parse!(rest(input), Ok((empty, input)));
}

#[test]
fn rest_len_on_slices() {
  let input: &[u8] = &b"Hello, world!"[..];
  assert_parse!(rest_len(input), Ok((input, input.len())));
}

use crate::lib::std::convert::From;
impl From<u32> for CustomError {
  fn from(_: u32) -> Self {
    CustomError
  }
}

impl<I> ParseError<I> for CustomError {
  fn from_error_kind(_: I, _: ErrorKind) -> Self {
    CustomError
  }

  fn append(_: I, _: ErrorKind, _: CustomError) -> Self {
    CustomError
  }
}

struct CustomError;
#[allow(dead_code)]
fn custom_error(input: &[u8]) -> IResult<&[u8], &[u8], CustomError> {
  //fix_error!(input, CustomError, alphanumeric)
  crate::character::alphanumeric1(input)
}

#[test]
fn test_flat_map() {
  #![allow(deprecated)]
  let input: &[u8] = &[3, 100, 101, 102, 103, 104][..];
  assert_parse!(
    flat_map(u8, take)(input),
    Ok((&[103, 104][..], &[100, 101, 102][..]))
  );
}

#[test]
fn test_parser_flat_map() {
  let input: &[u8] = &[3, 100, 101, 102, 103, 104][..];
  assert_parse!(
    u8.flat_map(take).parse(input),
    Ok((&[103, 104][..], &[100, 101, 102][..]))
  );
}

#[test]
fn test_map_opt() {
  #![allow(deprecated)]
  let input: &[u8] = &[50][..];
  assert_parse!(
    map_opt(u8, |u| if u < 20 { Some(u) } else { None })(input),
    Err(Err::Error((&[50][..], ErrorKind::MapOpt)))
  );
  assert_parse!(
    map_opt(u8, |u| if u > 20 { Some(u) } else { None })(input),
    Ok((&[][..], 50))
  );
}

#[test]
fn test_parser_map_opt() {
  let input: &[u8] = &[50][..];
  assert_parse!(
    u8.map_opt(|u| if u < 20 { Some(u) } else { None })
      .parse(input),
    Err(Err::Error((&[50][..], ErrorKind::MapOpt)))
  );
  assert_parse!(
    u8.map_opt(|u| if u > 20 { Some(u) } else { None })
      .parse(input),
    Ok((&[][..], 50))
  );
}

#[test]
fn test_map_parser() {
  #![allow(deprecated)]
  let input: &[u8] = &[100, 101, 102, 103, 104][..];
  assert_parse!(
    map_parser(take(4usize), take(2usize))(input),
    Ok((&[104][..], &[100, 101][..]))
  );
}

#[test]
fn test_parser_map_parser() {
  let input: &[u8] = &[100, 101, 102, 103, 104][..];
  assert_parse!(
    take(4usize).and_then(take(2usize)).parse(input),
    Ok((&[104][..], &[100, 101][..]))
  );
}

#[test]
fn test_all_consuming() {
  let input: &[u8] = &[100, 101, 102][..];
  assert_parse!(
    all_consuming(take(2usize))(input),
    Err(Err::Error((&[102][..], ErrorKind::Eof)))
  );
  assert_parse!(
    all_consuming(take(3usize))(input),
    Ok((&[][..], &[100, 101, 102][..]))
  );
}

#[test]
#[allow(unused)]
fn test_verify_ref() {
  #![allow(deprecated)]
  use crate::bytes::take;

  let mut parser1 = verify(take(3u8), |s: &[u8]| s == &b"abc"[..]);

  assert_eq!(parser1(&b"abcd"[..]), Ok((&b"d"[..], &b"abc"[..])));
  assert_eq!(
    parser1(&b"defg"[..]),
    Err(Err::Error((&b"defg"[..], ErrorKind::Verify)))
  );

  fn parser2(i: &[u8]) -> IResult<&[u8], u32> {
    verify(crate::number::be_u32, |val: &u32| *val < 3)(i)
  }
}

#[test]
#[cfg(feature = "alloc")]
fn test_verify_alloc() {
  #![allow(deprecated)]
  use crate::bytes::take;
  let mut parser1 = verify(take(3u8).map(|s: &[u8]| s.to_vec()), |s: &[u8]| {
    s == &b"abc"[..]
  });

  assert_eq!(parser1(&b"abcd"[..]), Ok((&b"d"[..], (&b"abc").to_vec())));
  assert_eq!(
    parser1(&b"defg"[..]),
    Err(Err::Error((&b"defg"[..], ErrorKind::Verify)))
  );
}

#[test]
#[cfg(feature = "std")]
fn test_into() {
  #![allow(deprecated)]
  use crate::bytes::take;
  use crate::{
    error::{Error, ParseError},
    Err,
  };

  let mut parser = into(take::<_, _, Error<_>, false>(3u8));
  let result: IResult<&[u8], Vec<u8>> = parser(&b"abcdefg"[..]);

  assert_eq!(result, Ok((&b"defg"[..], vec![97, 98, 99])));
}

#[test]
#[cfg(feature = "std")]
fn test_parser_into() {
  use crate::bytes::take;
  use crate::{
    error::{Error, ParseError},
    Err,
  };

  let mut parser = take::<_, _, Error<_>, false>(3u8).output_into();
  let result: IResult<&[u8], Vec<u8>> = parser.parse(&b"abcdefg"[..]);

  assert_eq!(result, Ok((&b"defg"[..], vec![97, 98, 99])));
}

#[test]
fn opt_test() {
  fn opt_abcd(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Option<&[u8]>> {
    opt(tag("abcd"))(i)
  }

  let a = &b"abcdef"[..];
  let b = &b"bcdefg"[..];
  let c = &b"ab"[..];
  assert_eq!(
    opt_abcd(Streaming(a)),
    Ok((Streaming(&b"ef"[..]), Some(&b"abcd"[..])))
  );
  assert_eq!(
    opt_abcd(Streaming(b)),
    Ok((Streaming(&b"bcdefg"[..]), None))
  );
  assert_eq!(opt_abcd(Streaming(c)), Err(Err::Incomplete(Needed::new(2))));
}

#[test]
fn peek_test() {
  fn peek_tag(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    peek(tag("abcd"))(i)
  }

  assert_eq!(
    peek_tag(Streaming(&b"abcdef"[..])),
    Ok((Streaming(&b"abcdef"[..]), &b"abcd"[..]))
  );
  assert_eq!(
    peek_tag(Streaming(&b"ab"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    peek_tag(Streaming(&b"xxx"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"xxx"[..]),
      ErrorKind::Tag
    )))
  );
}

#[test]
fn not_test() {
  fn not_aaa(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, ()> {
    not(tag("aaa"))(i)
  }

  assert_eq!(
    not_aaa(Streaming(&b"aaa"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"aaa"[..]),
      ErrorKind::Not
    )))
  );
  assert_eq!(
    not_aaa(Streaming(&b"aa"[..])),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    not_aaa(Streaming(&b"abcd"[..])),
    Ok((Streaming(&b"abcd"[..]), ()))
  );
}

#[test]
fn verify_test() {
  #![allow(deprecated)]
  use crate::bytes::take;

  fn test(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    verify(take(5u8), |slice: &[u8]| slice[0] == b'a')(i)
  }
  assert_eq!(
    test(Streaming(&b"bcd"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    test(Streaming(&b"bcdefg"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"bcdefg"[..]),
      ErrorKind::Verify
    )))
  );
  assert_eq!(
    test(Streaming(&b"abcdefg"[..])),
    Ok((Streaming(&b"fg"[..]), &b"abcde"[..]))
  );
}

#[test]
fn test_parser_verify() {
  use crate::bytes::take;

  fn test(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take(5u8).verify(|slice: &[u8]| slice[0] == b'a').parse(i)
  }
  assert_eq!(
    test(Streaming(&b"bcd"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    test(Streaming(&b"bcdefg"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"bcdefg"[..]),
      ErrorKind::Verify
    )))
  );
  assert_eq!(
    test(Streaming(&b"abcdefg"[..])),
    Ok((Streaming(&b"fg"[..]), &b"abcde"[..]))
  );
}

#[test]
#[allow(unused)]
fn test_parser_verify_ref() {
  use crate::bytes::take;

  let mut parser1 = take(3u8).verify(|s: &[u8]| s == &b"abc"[..]);

  assert_eq!(parser1.parse(&b"abcd"[..]), Ok((&b"d"[..], &b"abc"[..])));
  assert_eq!(
    parser1.parse(&b"defg"[..]),
    Err(Err::Error((&b"defg"[..], ErrorKind::Verify)))
  );

  fn parser2(i: &[u8]) -> IResult<&[u8], u32> {
    crate::number::be_u32.verify(|val: &u32| *val < 3).parse(i)
  }
}

#[test]
#[cfg(feature = "alloc")]
fn test_parser_verify_alloc() {
  use crate::bytes::take;
  let mut parser1 = take(3u8)
    .map(|s: &[u8]| s.to_vec())
    .verify(|s: &[u8]| s == &b"abc"[..]);

  assert_eq!(
    parser1.parse(&b"abcd"[..]),
    Ok((&b"d"[..], (&b"abc").to_vec()))
  );
  assert_eq!(
    parser1.parse(&b"defg"[..]),
    Err(Err::Error((&b"defg"[..], ErrorKind::Verify)))
  );
}

#[test]
fn fail_test() {
  let a = "string";
  let b = "another string";

  assert_eq!(fail::<_, &str, _>(a), Err(Err::Error((a, ErrorKind::Fail))));
  assert_eq!(fail::<_, &str, _>(b), Err(Err::Error((b, ErrorKind::Fail))));
}
