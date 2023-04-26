//! # Chapter 6: Error Reporting
//!
//! ## `Error`
//!
//! Back in [`chapter_1`], we glossed over the `Err` side of [`IResult`].  `IResult<I, O>` is
//! actually short for `IResult<I, O, E=Error>` where [`Error`] is a cheap, universal error type
//! for getting started.  When humans are producing the file, like with `toml`, you might want to
//! sacrifice some performance for providing more details on how to resolve the problem
//!
//! winnow includes [`VerboseError`] for this but you can [customize the error as you
//! wish][_topic::error].  You can use [`Parser::context`] to annotate the error with custom types
//! while unwinding to further improve the error quality.
//!
//! ```rust
//! # use winnow::IResult;
//! # use winnow::Parser;
//! # use winnow::bytes::take_while1;
//! # use winnow::branch::alt;
//! use winnow::error::VerboseError;
//!
//! fn parse_digits(input: &str) -> IResult<&str, (&str, &str), VerboseError<&str>> {
//!     alt((
//!         ("0b", parse_bin_digits).context("binary"),
//!         ("0o", parse_oct_digits).context("octal"),
//!         ("0d", parse_dec_digits).context("decimal"),
//!         ("0x", parse_hex_digits).context("hexadecimal"),
//!     )).parse_next(input)
//! }
//!
//! // ...
//! # fn parse_bin_digits(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
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
//! }
//! ```
//!
//! At first glance, this looks correct but what `context` will be reported when parsing `"0b5"`?
//! If you remember back to [`chapter_3`], [`alt`] will only report the last error by default which
//! means when parsing `"0b5"`, the `context` will be `"hexadecimal"`.
//!
//! ## `ErrMode`
//!
//! Let's break down `IResult<I, O, E>` one step further:
//! ```rust
//! # use winnow::error::Error;
//! # use winnow::error::ErrMode;
//! pub type IResult<I, O, E = Error<I>> = Result<(I, O), ErrMode<E>>;
//! ```
//! `IResult` is just a fancy wrapper around `Result` that wraps our error in an [`ErrMode`]
//! type.
//!
//! `ErrMode` is an enum with `Backtrack` and `Cut` variants (ignore `Incomplete` as its only
//! relevant for [streaming][_topic::stream]).  By default, errors are `Backtrack`, meaning that
//! other parsing branches will be attempted on failure, like the next case of an `alt`.  `Cut`
//! shortcircuits all other branches, immediately reporting the error.
//!
//! So we can get the correct `context` by modifying the above example with [`cut_err`]:
//! ```rust
//! # use winnow::IResult;
//! # use winnow::Parser;
//! # use winnow::bytes::take_while1;
//! # use winnow::branch::alt;
//! # use winnow::error::VerboseError;
//! use winnow::combinator::cut_err;
//!
//! fn parse_digits(input: &str) -> IResult<&str, (&str, &str), VerboseError<&str>> {
//!     alt((
//!         ("0b", cut_err(parse_bin_digits)).context("binary"),
//!         ("0o", cut_err(parse_oct_digits)).context("octal"),
//!         ("0d", cut_err(parse_dec_digits)).context("decimal"),
//!         ("0x", cut_err(parse_hex_digits)).context("hexadecimal"),
//!     )).parse_next(input)
//! }
//!
//! // ...
//! # fn parse_bin_digits(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
//! #     take_while1((
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
//! #     take_while1((
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
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
//! }
//! ```
//! Now, when parsing `"0b5"`, the `context` will be `"binary"`.

#![allow(unused_imports)]
use super::chapter_1;
use super::chapter_3;
use crate::branch::alt;
use crate::combinator::cut_err;
use crate::error::ErrMode;
use crate::error::Error;
use crate::error::VerboseError;
use crate::IResult;
use crate::Parser;
use crate::_topic;

pub use super::chapter_5 as previous;
pub use super::chapter_7 as next;
