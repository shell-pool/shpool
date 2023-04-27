//! # Chapter 4: Parsers With Custom Return Types
//!
//! So far, we have seen mostly functions that take an `&str`, and return a
//! `IResult<&str, &str>`. Splitting strings into smaller strings and characters is certainly
//! useful, but it's not the only thing winnow is capable of!
//!
//! A useful operation when parsing is to convert between types; for example
//! parsing from `&str` to another primitive, like [`usize`].
//!
//! All we need to do for our parser to return a different type is to change
//! the second type parameter of [`IResult`] to the desired return type.
//! For example, to return a `usize`, return a `IResult<&str, usize>`.
//! Recall that the first type parameter of the `IResult` is the input
//! type, so even if you're returning something different, if your input
//! is a `&str`, the first type argument of `IResult` should be also.
//!
//! One winnow-native way of doing a type conversion is to use the
//! [`Parser::parse_to`] combinator
//! to convert from a successful parse to a particular type using [`FromStr`].
//!
//! The following code converts from a string containing a number to `usize`:
//! ```rust
//! # use winnow::Parser;
//! # use winnow::IResult;
//! # use winnow::character::digit1;
//! #
//! fn parse_digits(input: &str) -> IResult<&str, usize> {
//!     digit1
//!         .parse_to()
//!         .parse_next(input)
//! }
//!
//! fn main() {
//!     let input = "1024 Hello";
//!
//!     let (remainder, output) = parse_digits.parse_next(input).unwrap();
//!     assert_eq!(remainder, " Hello");
//!     assert_eq!(output, 1024);
//!
//!     assert!(parse_digits("Z").is_err());
//! }
//! ```
//!
//! `Parser::parse_to` is just a convenient form of [`Parser::map_res`] which we can use to handle
//! all radices of numbers:
//! ```rust
//! # use winnow::IResult;
//! # use winnow::Parser;
//! # use winnow::bytes::take_while1;
//! use winnow::branch::dispatch;
//! use winnow::bytes::take;
//! use winnow::combinator::fail;
//!
//! fn parse_digits(input: &str) -> IResult<&str, usize> {
//!     dispatch!(take(2usize);
//!         "0b" => parse_bin_digits.map_res(|s| usize::from_str_radix(s, 2)),
//!         "0o" => parse_oct_digits.map_res(|s| usize::from_str_radix(s, 8)),
//!         "0d" => parse_dec_digits.map_res(|s| usize::from_str_radix(s, 10)),
//!         "0x" => parse_hex_digits.map_res(|s| usize::from_str_radix(s, 16)),
//!         _ => fail,
//!     ).parse_next(input)
//! }
//!
//! // ...
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
//!     let input = "0x1a2b Hello";
//!
//!     let (remainder, digits) = parse_digits.parse_next(input).unwrap();
//!
//!     assert_eq!(remainder, " Hello");
//!     assert_eq!(digits, 0x1a2b);
//!
//!     assert!(parse_digits("ghiWorld").is_err());
//! }
//! ```

#![allow(unused_imports)]
use crate::IResult;
use crate::Parser;
use std::str::FromStr;

pub use super::chapter_3 as previous;
pub use super::chapter_5 as next;
