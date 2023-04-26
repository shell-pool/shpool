//! # Chapter 2: Tokens and Tags
//!
//! The simplest *useful* parser you can write is one which matches tokens.
//!
//! ## Tokens
//!
//! Matching a single token literal is common enough that `Parser` is implemented for
//! `char`.
//!
//! ```rust
//! # use winnow::Parser;
//! # use winnow::IResult;
//! #
//! fn parse_prefix(input: &str) -> IResult<&str, char> {
//!     '0'.parse_next(input)
//! }
//!
//! fn main()  {
//!     let input = "0x1a2b Hello";
//!
//!     let (remainder, output) = parse_prefix.parse_next(input).unwrap();
//!
//!     assert_eq!(remainder, "x1a2b Hello");
//!     assert_eq!(output, '0');
//!
//!     assert!(parse_prefix("d").is_err());
//! }
//! ```
//!
//! ## Tags
//!
//! One of the most frequent way of matching a token is when they are combined into a string.
//! Again, this is common enough that `Parser` is implemented for `&str`:
//!
//! ```rust
//! # use winnow::Parser;
//! # use winnow::IResult;
//! #
//! fn parse_prefix(input: &str) -> IResult<&str, &str> {
//!     "0x".parse_next(input)
//! }
//!
//! fn main()  {
//!     let input = "0x1a2b Hello";
//!
//!     let (remainder, output) = parse_prefix.parse_next(input).unwrap();
//!     assert_eq!(remainder, "1a2b Hello");
//!     assert_eq!(output, "0x");
//!
//!     assert!(parse_prefix("0o123").is_err());
//! }
//! ```
//!
//! In `winnow`, we call this type of parser a [`tag`].
//!
//! ## Character Classes
//!
//! Selecting a single `char` or a [`tag`] is fairly limited.  Sometimes, you will want to select one of several
//! `chars` of a specific class, like digits. For this, we use the [`one_of`] parer:
//!
//! ```rust
//! # use winnow::Parser;
//! # use winnow::IResult;
//! use winnow::bytes::one_of;
//!
//! fn parse_digits(input: &str) -> IResult<&str, char> {
//!     one_of("0123456789abcdefgABCDEFG").parse_next(input)
//! }
//!
//! fn main() {
//!     let input = "1a2b Hello";
//!
//!     let (remainder, output) = parse_digits.parse_next(input).unwrap();
//!     assert_eq!(remainder, "a2b Hello");
//!     assert_eq!(output, '1');
//!
//!     assert!(parse_digits("Z").is_err());
//! }
//! ```
//!
//! > **Aside:** [`one_of`] might look straightforward, a function returning a value that implements `Parser`.
//! > Let's look at it more closely as its used above (resolving all generic parameters):
//! > ```rust
//! > # use winnow::prelude::*;
//! > # use winnow::error::Error;
//! > pub fn one_of<'i>(
//! >     list: &'static str
//! > ) -> impl Parser<&'i str, char, Error<&'i str>> {
//! >     // ...
//! > #    winnow::bytes::one_of(list)
//! > }
//! > ```
//! > If you have not programmed in a language where functions are values, the type signature of the
//! > [`one_of`] function might be a surprise.
//! > The function [`one_of`] *returns a function*. The function it returns is a
//! > `Parser`, taking a `&str` and returning an `IResult`. This is a common pattern in winnow for
//! > configurable or stateful parsers.
//!
//! Some of character classes are common enough that a named parser is provided, like with:
//! - [`line_ending`][crate::character::line_ending]: Recognizes an end of line (both `\n` and `\r\n`)
//! - [`newline`][crate::character::newline]: Matches a newline character `\n`
//! - [`tab`][crate::character::tab]: Matches a tab character `\t`
//!
//! You can then capture sequences of these characters with parsers like [`take_while1`].
//! ```rust
//! # use winnow::Parser;
//! # use winnow::IResult;
//! use winnow::bytes::take_while1;
//!
//! fn parse_digits(input: &str) -> IResult<&str, &str> {
//!     take_while1("0123456789abcdefgABCDEFG").parse_next(input)
//! }
//!
//! fn main() {
//!     let input = "1a2b Hello";
//!
//!     let (remainder, output) = parse_digits.parse_next(input).unwrap();
//!     assert_eq!(remainder, " Hello");
//!     assert_eq!(output, "1a2b");
//!
//!     assert!(parse_digits("Z").is_err());
//! }
//! ```
//!
//! We could simplify this further with by using one of the built-in character classes, [`hex_digit1`]:
//! ```rust
//! # use winnow::Parser;
//! # use winnow::IResult;
//! use winnow::character::hex_digit1;
//!
//! fn parse_digits(input: &str) -> IResult<&str, &str> {
//!     hex_digit1.parse_next(input)
//! }
//!
//! fn main() {
//!     let input = "1a2b Hello";
//!
//!     let (remainder, output) = parse_digits.parse_next(input).unwrap();
//!     assert_eq!(remainder, " Hello");
//!     assert_eq!(output, "1a2b");
//!
//!     assert!(parse_digits("Z").is_err());
//! }
//! ```

#![allow(unused_imports)]
use crate::bytes::one_of;
use crate::bytes::tag;
use crate::bytes::take_while1;
use crate::character::hex_digit1;
use crate::stream::ContainsToken;
use crate::Parser;
use std::ops::RangeInclusive;

pub use super::chapter_1 as previous;
pub use super::chapter_3 as next;
