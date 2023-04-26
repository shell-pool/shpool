//! # Chapter 3: Sequencing and Alternatives
//!
//! In the last chapter, we saw how to create simple parsers using prebuilt parsers.
//!
//! In this chapter, we explore two other widely used features:
//! alternatives and composition.
//!
//! ## Sequencing
//!
//! Now that we can create more interesting parsers, we can sequence them together, like:
//!
//! ```rust
//! # use winnow::bytes::take_while1;
//! # use winnow::Parser;
//! # use winnow::IResult;
//! #
//! fn parse_prefix(input: &str) -> IResult<&str, &str> {
//!     "0x".parse_next(input)
//! }
//!
//! fn parse_digits(input: &str) -> IResult<&str, &str> {
//!     take_while1((
//!         ('0'..='9'),
//!         ('A'..='F'),
//!         ('a'..='f'),
//!     )).parse_next(input)
//! }
//!
//! fn main()  {
//!     let input = "0x1a2b Hello";
//!
//!     let (remainder, prefix) = parse_prefix.parse_next(input).unwrap();
//!     let (remainder, digits) = parse_digits.parse_next(remainder).unwrap();
//!
//!     assert_eq!(prefix, "0x");
//!     assert_eq!(digits, "1a2b");
//!     assert_eq!(remainder, " Hello");
//! }
//! ```
//!
//! To sequence these together, you can just put them in a tuple:
//! ```rust
//! # use winnow::bytes::take_while1;
//! # use winnow::Parser;
//! # use winnow::IResult;
//! #
//! # fn parse_prefix(input: &str) -> IResult<&str, &str> {
//! #     "0x".parse_next(input)
//! # }
//! #
//! # fn parse_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! #
//! //...
//!
//! fn main()  {
//!     let input = "0x1a2b Hello";
//!
//!     let (remainder, (prefix, digits)) = (
//!         parse_prefix,
//!         parse_digits
//!     ).parse_next(input).unwrap();
//!
//!     assert_eq!(prefix, "0x");
//!     assert_eq!(digits, "1a2b");
//!     assert_eq!(remainder, " Hello");
//! }
//! ```
//!
//! Frequently, you won't care about the tag and you can instead use one of the provided combinators,
//! like [`preceded`]:
//! ```rust
//! # use winnow::bytes::take_while1;
//! # use winnow::Parser;
//! # use winnow::IResult;
//! use winnow::sequence::preceded;
//!
//! # fn parse_prefix(input: &str) -> IResult<&str, &str> {
//! #     "0x".parse_next(input)
//! # }
//! #
//! # fn parse_digits(input: &str) -> IResult<&str, &str> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//! #
//! //...
//!
//! fn main() {
//!     let input = "0x1a2b Hello";
//!
//!     let (remainder, digits) = preceded(
//!         parse_prefix,
//!         parse_digits
//!     ).parse_next(input).unwrap();
//!
//!     assert_eq!(digits, "1a2b");
//!     assert_eq!(remainder, " Hello");
//! }
//! ```
//!
//! ## Alternatives
//!
//! Sometimes, we might want to choose between two parsers; and we're happy with
//! either being used.
//!
//! The de facto way to do this in winnow is with the [`alt()`] combinator which will execute each
//! parser in a tuple until it finds one that does not error. If all error, then by default you are
//! given the error from the last parser.
//!
//! We can see a basic example of `alt()` below.
//! ```rust
//! # use winnow::IResult;
//! # use winnow::Parser;
//! # use winnow::bytes::take_while1;
//! use winnow::branch::alt;
//!
//! fn parse_digits(input: &str) -> IResult<&str, (&str, &str)> {
//!     alt((
//!         ("0b", parse_bin_digits),
//!         ("0o", parse_oct_digits),
//!         ("0d", parse_dec_digits),
//!         ("0x", parse_hex_digits),
//!     )).parse_next(input)
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
//!     let (remainder, (prefix, digits)) = parse_digits.parse_next(input).unwrap();
//!
//!     assert_eq!(remainder, " Hello");
//!     assert_eq!(prefix, "0x");
//!     assert_eq!(digits, "1a2b");
//!
//!     assert!(parse_digits("ghiWorld").is_err());
//! }
//! ```
//!
//! Sometimes a giant if/else-if ladder can be slow and you'd rather have a `match` statement for
//! branches of your parser that have unique prefixes.  In this case, you can use the
//! [`dispatch`][crate::branch::dispatch] macro:
//!
//! ```rust
//! # use winnow::IResult;
//! # use winnow::Parser;
//! # use winnow::bytes::take_while1;
//! use winnow::branch::dispatch;
//! use winnow::bytes::take;
//! use winnow::combinator::fail;
//!
//! fn parse_digits(input: &str) -> IResult<&str, &str> {
//!     dispatch!(take(2usize);
//!         "0b" => parse_bin_digits,
//!         "0o" => parse_oct_digits,
//!         "0d" => parse_dec_digits,
//!         "0x" => parse_hex_digits,
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
//!     assert_eq!(digits, "1a2b");
//!
//!     assert!(parse_digits("ghiWorld").is_err());
//! }
//! ```

#![allow(unused_imports)]
use crate::branch::alt;
use crate::branch::dispatch;
use crate::sequence::preceded;

pub use super::chapter_2 as previous;
pub use super::chapter_4 as next;
