//#![feature(trace_macros)]
#![allow(dead_code)]
#![cfg_attr(feature = "cargo-clippy", allow(redundant_closure))]

use nom8::input::Streaming;
use nom8::prelude::*;
use nom8::{error::ErrorKind, Err, IResult, Needed};

#[allow(dead_code)]
struct Range {
  start: char,
  end: char,
}

pub fn take_char(input: &[u8]) -> IResult<&[u8], char> {
  if !input.is_empty() {
    Ok((&input[1..], input[0] as char))
  } else {
    Err(Err::Incomplete(Needed::new(1)))
  }
}

#[cfg(feature = "std")]
mod parse_int {
  use nom8::input::HexDisplay;
  use nom8::input::Streaming;
  use nom8::prelude::*;
  use nom8::{
    character::{digit1 as digit, space1 as space},
    combinator::opt,
    multi::many0,
    IResult,
  };
  use std::str;

  fn parse_ints(input: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, Vec<i32>> {
    many0(spaces_or_int)(input)
  }

  fn spaces_or_int(input: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, i32> {
    println!("{}", input.to_hex(8));
    let (i, _) = opt(space.complete())(input)?;
    let (i, res) = digit
      .complete()
      .map(|x| {
        println!("x: {:?}", x);
        let result = str::from_utf8(x).unwrap();
        println!("Result: {}", result);
        println!("int is empty?: {}", x.is_empty());
        match result.parse() {
          Ok(i) => i,
          Err(e) => panic!("UH OH! NOT A DIGIT! {:?}", e),
        }
      })
      .parse(i)?;

    Ok((i, res))
  }

  #[test]
  fn issue_142() {
    let subject = parse_ints(Streaming(&b"12 34 5689a"[..]));
    let expected = Ok((Streaming(&b"a"[..]), vec![12, 34, 5689]));
    assert_eq!(subject, expected);

    let subject = parse_ints(Streaming(&b"12 34 5689 "[..]));
    let expected = Ok((Streaming(&b" "[..]), vec![12, 34, 5689]));
    assert_eq!(subject, expected)
  }
}

#[test]
fn usize_length_bytes_issue() {
  use nom8::multi::length_data;
  use nom8::number::be_u16;
  let _: IResult<Streaming<&[u8]>, &[u8], (Streaming<&[u8]>, ErrorKind)> =
    length_data(be_u16)(Streaming(b"012346"));
}

#[test]
fn take_till_issue() {
  use nom8::bytes::take_till;

  fn nothing(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_till(|_| true)(i)
  }

  assert_eq!(
    nothing(Streaming(b"")),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    nothing(Streaming(b"abc")),
    Ok((Streaming(&b"abc"[..]), &b""[..]))
  );
}

#[test]
fn issue_655() {
  use nom8::character::{line_ending, not_line_ending};
  fn twolines(i: Streaming<&str>) -> IResult<Streaming<&str>, (&str, &str)> {
    let (i, l1) = not_line_ending(i)?;
    let (i, _) = line_ending(i)?;
    let (i, l2) = not_line_ending(i)?;
    let (i, _) = line_ending(i)?;

    Ok((i, (l1, l2)))
  }

  assert_eq!(
    twolines(Streaming("foo\nbar\n")),
    Ok((Streaming(""), ("foo", "bar")))
  );
  assert_eq!(
    twolines(Streaming("féo\nbar\n")),
    Ok((Streaming(""), ("féo", "bar")))
  );
  assert_eq!(
    twolines(Streaming("foé\nbar\n")),
    Ok((Streaming(""), ("foé", "bar")))
  );
  assert_eq!(
    twolines(Streaming("foé\r\nbar\n")),
    Ok((Streaming(""), ("foé", "bar")))
  );
}

#[cfg(feature = "alloc")]
fn issue_717(i: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
  use nom8::bytes::{tag, take_till1};
  use nom8::multi::separated_list0;

  separated_list0(tag([0x0]), take_till1([0x0u8]))(i)
}

mod issue_647 {
  use nom8::bytes::tag;
  use nom8::multi::separated_list0;
  use nom8::prelude::*;
  use nom8::{error::Error, number::be_f64, Err, IResult};
  pub type Input<'a> = nom8::input::Streaming<&'a [u8]>;

  #[derive(PartialEq, Debug, Clone)]
  struct Data {
    c: f64,
    v: Vec<f64>,
  }

  fn list<'a, 'b>(
    input: Input<'a>,
    _cs: &'b f64,
  ) -> Result<(Input<'a>, Vec<f64>), Err<Error<Input<'a>>>> {
    separated_list0(tag(",").complete(), be_f64.complete())(input)
  }

  fn data(input: Input<'_>) -> IResult<Input<'_>, Data> {
    let (i, c) = be_f64(input)?;
    let (i, _) = tag("\n")(i)?;
    let (i, v) = list(i, &c)?;
    Ok((i, Data { c, v }))
  }
}

#[test]
fn issue_848_overflow_incomplete_bits_to_bytes() {
  fn take(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    use nom8::bytes::take;
    take(0x2000000000000000_usize)(i)
  }
  fn parser(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    use nom8::bits::{bits, bytes};

    bits(bytes(take))(i)
  }
  assert_eq!(
    parser(Streaming(&b""[..])),
    Err(Err::Failure(nom8::error_position!(
      Streaming(&b""[..]),
      ErrorKind::TooLarge
    )))
  );
}

#[test]
fn issue_942() {
  use nom8::error::{ContextError, ParseError};
  pub fn parser<'a, E: ParseError<&'a str> + ContextError<&'a str, &'static str>>(
    i: &'a str,
  ) -> IResult<&'a str, usize, E> {
    use nom8::{bytes::one_of, multi::many0_count};
    many0_count(one_of('a').context("char_a"))(i)
  }
  assert_eq!(parser::<()>("aaa"), Ok(("", 3)));
}

#[test]
fn issue_many_m_n_with_zeros() {
  use nom8::multi::many_m_n;
  let mut parser = many_m_n::<_, _, (), _>(0, 0, 'a');
  assert_eq!(parser("aaa"), Ok(("aaa", vec![])));
}

#[test]
fn issue_1027_convert_error_panic_nonempty() {
  use nom8::error::{convert_error, VerboseError};

  let input = "a";

  let result: IResult<_, _, VerboseError<&str>> = ('a', 'b').parse(input);
  let err = match result.unwrap_err() {
    Err::Error(e) => e,
    _ => unreachable!(),
  };

  let msg = convert_error(input, err);
  assert_eq!(msg, "0: at line 1, in OneOf:\na\n ^\n\n",);
}

#[test]
fn issue_1231_bits_expect_fn_closure() {
  use nom8::bits::{bits, take};
  use nom8::error::Error;
  pub fn example(input: &[u8]) -> IResult<&[u8], (u8, u8)> {
    bits::<_, _, Error<_>, _, _>((take(1usize), take(1usize)))(input)
  }
  assert_eq!(example(&[0xff]), Ok((&b""[..], (1, 1))));
}

#[test]
fn issue_1282_findtoken_char() {
  use nom8::bytes::one_of;
  use nom8::error::Error;
  let parser = one_of::<_, _, Error<_>, false>(&['a', 'b', 'c'][..]);
  assert_eq!(parser("aaa"), Ok(("aa", 'a')));
}

#[test]
fn issue_1459_clamp_capacity() {
  // shouldn't panic
  use nom8::multi::many_m_n;
  let mut parser = many_m_n::<_, _, (), _>(usize::MAX, usize::MAX, 'a');
  assert_eq!(parser("a"), Err(nom8::Err::Error(())));

  // shouldn't panic
  use nom8::multi::count;
  let mut parser = count::<_, _, (), _>('a', usize::MAX);
  assert_eq!(parser("a"), Err(nom8::Err::Error(())));
}
