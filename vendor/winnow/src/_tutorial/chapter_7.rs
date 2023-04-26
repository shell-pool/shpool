//! # Chapter 7: Integrating the Parser
//!
//! So far, we've highlighted how to incrementally parse, but how do we bring this all together
//! into our application?
//!
//! The type we've been working with looks like:
//! ```rust
//! # use winnow::error::VerboseError;
//! # use winnow::error::ErrMode;
//! type IResult<'i, O> = Result<
//!     (&'i str, O),
//!     ErrMode<
//!         VerboseError<&'i str>
//!     >
//! >;
//! ```
//! 1. We have to decide what to do about the `remainder` of the input.  
//! 2. The error type is not compatible with the rest of the Rust ecosystem
//!
//! Normally, Rust applications want errors that are `std::error::Error + Send + Sync + 'static`
//! meaning:
//! - They implement the [`std::error::Error`] trait
//! - They can be sent across threads
//! - They are safe to be referenced across threads
//! - They do not borrow
//!
//! winnow provides some helpers for this:
//! ```rust
//! # use winnow::IResult;
//! # use winnow::bytes::take_while1;
//! # use winnow::branch::dispatch;
//! # use winnow::bytes::take;
//! # use winnow::combinator::fail;
//! use winnow::Parser;
//! use winnow::error::Error;
//!
//! #[derive(Debug, PartialEq, Eq)]
//! pub struct Hex(usize);
//!
//! impl std::str::FromStr for Hex {
//!     type Err = Error<String>;
//!
//!     fn from_str(input: &str) -> Result<Self, Self::Err> {
//!         parse_digits
//!             .map(Hex)
//!             .parse(input)
//!             .map_err(|e| e.into_owned())
//!     }
//! }
//!
//! // ...
//! # fn parse_digits(input: &str) -> IResult<&str, usize> {
//! #     dispatch!(take(2usize);
//! #         "0b" => parse_bin_digits.map_res(|s| usize::from_str_radix(s, 2)),
//! #         "0o" => parse_oct_digits.map_res(|s| usize::from_str_radix(s, 8)),
//! #         "0d" => parse_dec_digits.map_res(|s| usize::from_str_radix(s, 10)),
//! #         "0x" => parse_hex_digits.map_res(|s| usize::from_str_radix(s, 16)),
//! #         _ => fail,
//! #     ).parse_next(input)
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
//!     let input = "0x1a2b";
//!     assert_eq!(input.parse::<Hex>().unwrap(), Hex(0x1a2b));
//!
//!     let input = "0x1a2b Hello";
//!     assert!(input.parse::<Hex>().is_err());
//!     let input = "ghiHello";
//!     assert!(input.parse::<Hex>().is_err());
//! }
//! ```
//! - Ensures we hit [`eof`]
//! - Removes the [`ErrMode`] wrapper
//!
//! [`Error::into_owned`]:
//! - Converts the `&str` in `Error` to `String` which enables support for [`std::error::Error`]

#![allow(unused_imports)]
use super::chapter_1;
use crate::combinator::eof;
use crate::error::ErrMode;
use crate::error::Error;
use crate::IResult;

pub use super::chapter_6 as previous;
