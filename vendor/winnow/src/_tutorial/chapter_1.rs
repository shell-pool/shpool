//! # Chapter 1: The Winnow Way
//!
//! First of all, we need to understand the way that winnow thinks about parsing.
//! As discussed in the introduction, winnow lets us build simple parsers, and
//! then combine them (using "combinators").
//!
//! Let's discuss what a "parser" actually does. A parser takes an input and returns
//! a result, where:
//!  - `Ok` indicates the parser successfully found what it was looking for; or
//!  - `Err` indicates the parser could not find what it was looking for.
//!
//! Parsers do more than just return a binary "success"/"failure" code. If
//! the parser was successful, then it will return a tuple where the first field
//! will contain everything the parser did not process. The second will contain
//! everything the parser processed. The idea is that a parser can happily parse the first
//! *part* of an input, without being able to parse the whole thing.
//!
//! If the parser failed, then there are multiple errors that could be returned.
//! For simplicity, however, in the next chapters we will leave these unexplored.
//!
//! ```text
//!                                    ┌─► Ok(
//!                                    │      what the parser didn't touch,
//!                                    │      what matched the parser
//!                                    │   )
//!              ┌─────────┐           │
//!  my input───►│my parser├──►either──┤
//!              └─────────┘           └─► Err(...)
//! ```
//!
//!
//! To represent this model of the world, winnow uses the [`IResult<I, O>`] type.
//! The `Ok` variant has a tuple of `(remainder: I, output: O)`;
//! whereas the `Err` variant stores an error.
//!
//! You can import that from:
//!
//! ```rust
//! use winnow::IResult;
//! ```
//!
//! You'll note that `I` and `O` are parameterized -- while most of the examples in this book
//! will be with `&str` (i.e. parsing a string); they do not have to be strings; nor do they
//! have to be the same type (consider the simple example where `I = &str`, and `O = u64` -- this
//! parses a string into an unsigned integer.)
//!
//! To combine parsers, we need a common way to refer to them which is where the [`Parser`]
//! trait comes in with [`Parser::parse_next`] being the primary way to drive
//! parsing forward.
//!
//! # Let's write our first parser!
//!
//! The simplest parser we can write is one which successfully does nothing.
//!
//! To make it easier to implement a [`Parser`], the trait is implemented for
//! functions of the form `Fn(I) -> IResult<I, O>`.
//!
//! This parser function should take in a `&str`:
//!
//!  - Since it is supposed to succeed, we know it will return the Ok Variant.
//!  - Since it does nothing to our input, the remaining input is the same as the input.
//!  - Since it doesn't parse anything, it also should just return an empty string.
//!
//! ```rust
//! use winnow::IResult;
//! use winnow::Parser;
//!
//! pub fn do_nothing_parser(input: &str) -> IResult<&str, &str> {
//!     Ok((input, ""))
//! }
//!
//! fn main() {
//!     let input = "0x1a2b Hello";
//!
//!     let (remainder, output) = do_nothing_parser.parse_next(input).unwrap();
//!     // Same as:
//!     // let (remainder, output) = do_nothing_parser(input).unwrap();
//!
//!     assert_eq!(remainder, "0x1a2b Hello");
//!     assert_eq!(output, "");
//! }
//! ```

#![allow(unused_imports)]
use crate::IResult;
use crate::Parser;

pub use super::chapter_0 as previous;
pub use super::chapter_2 as next;
