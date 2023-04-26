//! # Chapter 5: Repetition
//!
//! In [`chapter_3`], we covered how to sequence different parsers into a tuple but sometimes you need to run a
//! single parser many times into a [`Vec`].
//!
//! Let's take our `parse_digits` and collect a list of them with [`many0`]:
//! ```rust
//! # use winnow::IResult;
//! # use winnow::Parser;
//! # use winnow::bytes::take_while1;
//! # use winnow::branch::dispatch;
//! # use winnow::bytes::take;
//! # use winnow::combinator::fail;
//! use winnow::combinator::opt;
//! use winnow::multi::many0;
//! use winnow::sequence::terminated;
//!
//! fn parse_list(input: &str) -> IResult<&str, Vec<usize>> {
//!     many0(terminated(parse_digits, opt(','))).parse_next(input)
//! }
//!
//! // ...
//! # fn parse_digits(input: &str) -> IResult<&str, usize> {
//! #     dispatch!(take(2usize);
//! #          "0b" => parse_bin_digits.map_res(|s| usize::from_str_radix(s, 2)),
//! #          "0o" => parse_oct_digits.map_res(|s| usize::from_str_radix(s, 8)),
//! #          "0d" => parse_dec_digits.map_res(|s| usize::from_str_radix(s, 10)),
//! #          "0x" => parse_hex_digits.map_res(|s| usize::from_str_radix(s, 16)),
//! #          _ => fail,
//! #      ).parse_next(input)
//! # }
//! #
//! # fn parse_bin_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//!
//! fn main() {
//!     let input = "0x1a2b,0x3c4d,0x5e6f Hello";
//!
//!     let (remainder, digits) = parse_list.parse_next(input).unwrap();
//!
//!     assert_eq!(remainder, " Hello");
//!     assert_eq!(digits, vec![0x1a2b, 0x3c4d, 0x5e6f]);
//!
//!     assert!(parse_digits("ghiWorld").is_err());
//! }
//! ```
//!
//! You'll notice that the above allows trailing `,` when we intended to not support that.  We can
//! easily fix this by using [`separated0`]:
//! ```rust
//! # use winnow::IResult;
//! # use winnow::Parser;
//! # use winnow::bytes::take_while1;
//! # use winnow::branch::dispatch;
//! # use winnow::bytes::take;
//! # use winnow::combinator::fail;
//! use winnow::multi::separated0;
//!
//! fn parse_list(input: &str) -> IResult<&str, Vec<usize>> {
//!     separated0(parse_digits, ",").parse_next(input)
//! }
//!
//! // ...
//! # fn parse_digits(input: &str) -> IResult<&str, usize> {
//! #     dispatch!(take(2usize);
//! #          "0b" => parse_bin_digits.map_res(|s| usize::from_str_radix(s, 2)),
//! #          "0o" => parse_oct_digits.map_res(|s| usize::from_str_radix(s, 8)),
//! #          "0d" => parse_dec_digits.map_res(|s| usize::from_str_radix(s, 10)),
//! #          "0x" => parse_hex_digits.map_res(|s| usize::from_str_radix(s, 16)),
//! #          _ => fail,
//! #      ).parse_next(input)
//! # }
//! #
//! # fn parse_bin_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//!
//! fn main() {
//!     let input = "0x1a2b,0x3c4d,0x5e6f Hello";
//!
//!     let (remainder, digits) = parse_list.parse_next(input).unwrap();
//!
//!     assert_eq!(remainder, " Hello");
//!     assert_eq!(digits, vec![0x1a2b, 0x3c4d, 0x5e6f]);
//!
//!     assert!(parse_digits("ghiWorld").is_err());
//! }
//! ```
//!
//! If you look closely at [`many0`], it isn't collecting directly into a [`Vec`] but
//! [`Accumulate`] to gather the results.  This let's us make more complex parsers than we did in
//! [`chapter_2`] by accumulating the results into a `()` and [`recognize`][Parser::recognize]-ing the captured input:
//! ```rust
//! # use winnow::IResult;
//! # use winnow::Parser;
//! # use winnow::bytes::take_while1;
//! # use winnow::branch::dispatch;
//! # use winnow::bytes::take;
//! # use winnow::combinator::fail;
//! # use winnow::multi::separated0;
//! #
//! fn recognize_list(input: &str) -> IResult<&str, &str> {
//!     parse_list.recognize().parse_next(input)
//! }
//!
//! fn parse_list(input: &str) -> IResult<&str, ()> {
//!     separated0(parse_digits, ",").parse_next(input)
//! }
//!
//! // ...
//! # fn parse_digits(input: &str) -> IResult<&str, usize> {
//! #     dispatch!(take(2usize);
//! #          "0b" => parse_bin_digits.map_res(|s| usize::from_str_radix(s, 2)),
//! #          "0o" => parse_oct_digits.map_res(|s| usize::from_str_radix(s, 8)),
//! #          "0d" => parse_dec_digits.map_res(|s| usize::from_str_radix(s, 10)),
//! #          "0x" => parse_hex_digits.map_res(|s| usize::from_str_radix(s, 16)),
//! #          _ => fail,
//! #      ).parse_next(input)
//! # }
//! #
//! # fn parse_bin_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//!
//! fn main() {
//!     let input = "0x1a2b,0x3c4d,0x5e6f Hello";
//!
//!     let (remainder, digits) = recognize_list.parse_next(input).unwrap();
//!
//!     assert_eq!(remainder, " Hello");
//!     assert_eq!(digits, "0x1a2b,0x3c4d,0x5e6f");
//!
//!     assert!(parse_digits("ghiWorld").is_err());
//! }
//! ```

#![allow(unused_imports)]
use super::chapter_2;
use super::chapter_3;
use crate::multi::many0;
use crate::multi::separated0;
use crate::stream::Accumulate;
use crate::Parser;
use std::vec::Vec;

pub use super::chapter_4 as previous;
pub use super::chapter_6 as next;
