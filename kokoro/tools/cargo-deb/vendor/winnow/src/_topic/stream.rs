//! # Custom [`Stream`][crate::stream::Stream]
//!
//! `winnow` is batteries included with support for
//! - Basic inputs like `&str`, newtypes with
//! - Improved debug output like [`Bytes`][crate::Bytes]
//! - [`Stateful`][crate::Stateful] for passing state through your parser, like tracking recursion
//!   depth
//! - [`Located`][crate::Located] for looking up the absolute position of a token
//!
//! But that won't always cut it for your parser.  For example, you might lex `&str` into
//! a series of tokens and then want to parse a `TokenStream`.
//!
//! ## Implementing a custom stream
//!
//! Let's assume we have an input type we'll call `MyStream`. `MyStream` is a sequence of `MyItem` type.
//! The goal is to define parsers with this signature: `MyStream -> IResult<MyStream, Output>`.
//!
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::bytes::tag;
//! # type MyStream<'i> = &'i str;
//! # type Output<'i> = &'i str;
//! fn parser(i: MyStream<'_>) -> IResult<MyStream<'_>, Output<'_>> {
//!     tag("test")(i)
//! }
//! ```
//!
//! Here are the traits we have to implement for `MyStream`:
//!
//! | trait | usage |
//! |---|---|
//! | [`Stream`] |Core trait for driving parsing|
//! | [`StreamIsPartial`] | Marks the input as being the complete buffer or a partial buffer for streaming input |
//! | [`AsBytes`] |Casts the input type to a byte slice|
//! | [`AsBStr`] |Casts the input type to a slice of ASCII / UTF-8-like bytes|
//! | [`Compare`] |Character comparison operations|
//! | [`FindSlice`] |Look for a substring in self|
//! | [`Location`] |Calculate location within initial input|
//! | [`Offset`] |Calculate the offset between slices|
//!
//! Here are the traits we have to implement for `MyItem`:
//!
//! | trait | usage |
//! |---|---|
//! | [`AsChar`] |Transforms common types to a char for basic token parsing|
//! | [`ContainsToken`] |Look for the token in the given set|
//!
//! And traits for slices of `MyItem`:
//!
//! | [`SliceLen`] |Calculate the input length|
//! | [`ParseSlice`] |Used to integrate `&str`'s `parse()` method|

#[allow(unused_imports)] // Here for intra-dock links
use crate::stream::*;
