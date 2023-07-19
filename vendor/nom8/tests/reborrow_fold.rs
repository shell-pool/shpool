#![allow(dead_code)]
// #![allow(unused_variables)]

use std::str;

use nom8::bytes::take_till1;
use nom8::multi::fold_many0;
use nom8::prelude::*;
use nom8::sequence::delimited;
use nom8::IResult;

fn atom<'a>(_tomb: &'a mut ()) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], String> {
  move |input| {
    take_till1(" \t\r\n")
      .map_res(str::from_utf8)
      .map(ToString::to_string)
      .parse(input)
  }
}

// FIXME: should we support the use case of borrowing data mutably in a parser?
fn list<'a>(i: &'a [u8], tomb: &'a mut ()) -> IResult<&'a [u8], String> {
  delimited(
    '(',
    fold_many0(atom(tomb), String::new, |acc: String, next: String| {
      acc + next.as_str()
    }),
    ')',
  )(i)
}
