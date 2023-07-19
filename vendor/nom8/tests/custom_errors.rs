#![allow(dead_code)]

use nom8::bytes::tag;
use nom8::character::digit1 as digit;
use nom8::error::{ErrorKind, ParseError};
use nom8::input::Streaming;
#[cfg(feature = "alloc")]
use nom8::multi::count;
use nom8::prelude::*;
use nom8::sequence::terminated;
use nom8::IResult;

#[derive(Debug)]
pub struct CustomError(String);

impl<'a> From<(&'a str, ErrorKind)> for CustomError {
  fn from(error: (&'a str, ErrorKind)) -> Self {
    CustomError(format!("error code was: {:?}", error))
  }
}

impl<'a> ParseError<Streaming<&'a str>> for CustomError {
  fn from_error_kind(_: Streaming<&'a str>, kind: ErrorKind) -> Self {
    CustomError(format!("error code was: {:?}", kind))
  }

  fn append(_: Streaming<&'a str>, kind: ErrorKind, other: CustomError) -> Self {
    CustomError(format!("{:?}\nerror code was: {:?}", other, kind))
  }
}

fn test1(input: Streaming<&str>) -> IResult<Streaming<&str>, &str, CustomError> {
  //fix_error!(input, CustomError, tag!("abcd"))
  tag("abcd")(input)
}

fn test2(input: Streaming<&str>) -> IResult<Streaming<&str>, &str, CustomError> {
  //terminated!(input, test1, fix_error!(CustomError, digit))
  terminated(test1, digit)(input)
}

fn test3(input: Streaming<&str>) -> IResult<Streaming<&str>, &str, CustomError> {
  test1.verify(|s: &str| s.starts_with("abcd")).parse(input)
}

#[cfg(feature = "alloc")]
fn test4(input: Streaming<&str>) -> IResult<Streaming<&str>, Vec<&str>, CustomError> {
  count(test1, 4)(input)
}
