//! # List of parsers and combinators
//!
//! **Note**: this list is meant to provide a nicer way to find a parser than reading through the documentation on docs.rs. Function combinators are organized in module so they are a bit easier to find.
//!
//! ## Basic elements
//!
//! Those are used to recognize the lowest level elements of your grammar, like, "here is a dot", or "here is an big endian integer".
//!
//! | combinator | usage | input | output | comment |
//! |---|---|---|---|---|
//! | [`one_of`][crate::bytes::one_of] | `one_of("abc")` |  `"abc"` | `Ok(("bc", 'a'))` |Matches one of the provided characters (works with non ASCII characters too)|
//! | [`none_of`][crate::bytes::none_of] | `none_of("abc")` |  `"xyab"` | `Ok(("yab", 'x'))` |Matches anything but the provided characters|
//! | [`tag`][crate::bytes::tag] | `"hello"` |  `"hello world"` | `Ok((" world", "hello"))` |Recognizes a specific suite of characters or bytes|
//! | [`tag_no_case`][crate::bytes::tag_no_case] | `tag_no_case("hello")` |  `"HeLLo World"` | `Ok((" World", "HeLLo"))` |Case insensitive comparison. Note that case insensitive comparison is not well defined for unicode, and that you might have bad surprises|
//! | [`take`][crate::bytes::take] | `take(4)` |  `"hello"` | `Ok(("o", "hell"))` |Takes a specific number of bytes or characters|
//! | [`take_while0`][crate::bytes::take_while0] | `take_while0(is_alphabetic)` |  `"abc123"` | `Ok(("123", "abc"))` |Returns the longest list of bytes for which the provided pattern matches. `take_while1` does the same, but must return at least one character|
//! | [`take_till0`][crate::bytes::take_till0] | `take_till0(is_alphabetic)` |  `"123abc"` | `Ok(("abc", "123"))` |Returns the longest list of bytes or characters until the provided pattern matches. `take_till1` does the same, but must return at least one character. This is the reverse behaviour from `take_while0`: `take_till(f)` is equivalent to `take_while0(\|c\| !f(c))`|
//! | [`take_until0`][crate::bytes::take_until0] | `take_until0("world")` |  `"Hello world"` | `Ok(("world", "Hello "))` |Returns the longest list of bytes or characters until the provided tag is found. `take_until1` does the same, but must return at least one character|
//!
//! ## Choice combinators
//!
//! | combinator | usage | input | output | comment |
//! |---|---|---|---|---|
//! | [`alt`][crate::branch::alt] | `alt(("ab", "cd"))` |  `"cdef"` | `Ok(("ef", "cd"))` |Try a list of parsers and return the result of the first successful one|
//! | [`dispatch`][crate::branch::dispatch] | \- | \- | \- | `match` for parsers |
//! | [`permutation`][crate::branch::permutation] | `permutation(("ab", "cd", "12"))` | `"cd12abc"` | `Ok(("c", ("ab", "cd", "12"))` |Succeeds when all its child parser have succeeded, whatever the order|
//!
//! ## Sequence combinators
//!
//! | combinator | usage | input | output | comment |
//! |---|---|---|---|---|
//! | [`(...)` (tuples)][crate::Parser] | `("ab", "XY", take(1))` | `"abXYZ!"` | `Ok(("!", ("ab", "XY", "Z")))` |Chains parsers and assemble the sub results in a tuple. You can use as many child parsers as you can put elements in a tuple|
//! | [`delimited`][crate::sequence::delimited] | `delimited(char('('), take(2), char(')'))` | `"(ab)cd"` | `Ok(("cd", "ab"))` ||
//! | [`preceded`][crate::sequence::preceded] | `preceded("ab", "XY")` | `"abXYZ"` | `Ok(("Z", "XY"))` ||
//! | [`terminated`][crate::sequence::terminated] | `terminated("ab", "XY")` | `"abXYZ"` | `Ok(("Z", "ab"))` ||
//! | [`separated_pair`][crate::sequence::separated_pair] | `separated_pair("hello", char(','), "world")` | `"hello,world!"` | `Ok(("!", ("hello", "world")))` ||
//!
//! ## Applying a parser multiple times
//!
//! | combinator | usage | input | output | comment |
//! |---|---|---|---|---|
//! | [`count`][crate::multi::count] | `count(take(2), 3)` | `"abcdefgh"` | `Ok(("gh", vec!["ab", "cd", "ef"]))` |Applies the child parser a specified number of times|
//! | [`many0`][crate::multi::many0] | `many0("ab")` |  `"abababc"` | `Ok(("c", vec!["ab", "ab", "ab"]))` |Applies the parser 0 or more times and returns the list of results in a Vec. `many1` does the same operation but must return at least one element|
//! | [`many_m_n`][crate::multi::many_m_n] | `many_m_n(1, 3, "ab")` | `"ababc"` | `Ok(("c", vec!["ab", "ab"]))` |Applies the parser between m and n times (n included) and returns the list of results in a Vec|
//! | [`many_till0`][crate::multi::many_till0] | `many_till0(tag( "ab" ), tag( "ef" ))` | `"ababefg"` | `Ok(("g", (vec!["ab", "ab"], "ef")))` |Applies the first parser until the second applies. Returns a tuple containing the list of results from the first in a Vec and the result of the second|
//! | [`separated0`][crate::multi::separated0] | `separated0("ab", ",")` | `"ab,ab,ab."` | `Ok((".", vec!["ab", "ab", "ab"]))` |`separated1` works like `separated0` but must returns at least one element|
//! | [`fold_many0`][crate::multi::fold_many0] | `fold_many0(be_u8, \|\| 0, \|acc, item\| acc + item)` | `[1, 2, 3]` | `Ok(([], 6))` |Applies the parser 0 or more times and folds the list of return values. The `fold_many1` version must apply the child parser at least one time|
//! | [`fold_many_m_n`][crate::multi::fold_many_m_n] | `fold_many_m_n(1, 2, be_u8, \|\| 0, \|acc, item\| acc + item)` | `[1, 2, 3]` | `Ok(([3], 3))` |Applies the parser between m and n times (n included) and folds the list of return value|
//! | [`length_count`][crate::multi::length_count] | `length_count(number, "ab")` | `"2ababab"` | `Ok(("ab", vec!["ab", "ab"]))` |Gets a number from the first parser, then applies the second parser that many times|
//!
//! ## Partial related
//!
//! - [`eof`][eof]: Returns its input if it is at the end of input data
//! - [`Parser::complete_err`]: Replaces an `Incomplete` returned by the child parser with an `Backtrack`
//!
//! ## Modifiers
//!
//! - [`cond`][cond]: Conditional combinator. Wraps another parser and calls it if the condition is met
//! - [`Parser::flat_map`][crate::Parser::flat_map]: method to map a new parser from the output of the first parser, then apply that parser over the rest of the input
//! - [`Parser::value`][crate::Parser::value]: method to replace the result of a parser
//! - [`Parser::map`][crate::Parser::map]: method to map a function on the result of a parser
//! - [`Parser::and_then`][crate::Parser::and_then]: Applies a second parser over the output of the first one
//! - [`Parser::verify_map`][Parser::verify_map]: Maps a function returning an `Option` on the output of a parser
//! - [`Parser::map_res`][Parser::map_res]: Maps a function returning a `Result` on the output of a parser
//! - [`Parser::parse_to`][crate::Parser::parse_to]: Apply [`std::str::FromStr`] to the output of the parser
//! - [`not`][not]: Returns a result only if the embedded parser returns `Backtrack` or `Incomplete`. Does not consume the input
//! - [`opt`][opt]: Make the underlying parser optional
//! - [`peek`][peek]: Returns a result without consuming the input
//! - [`Parser::recognize`][Parser::recognize]: If the child parser was successful, return the consumed input as the produced value
//! - [`Parser::with_recognized`][Parser::with_recognized]: If the child parser was successful, return a tuple of the consumed input and the produced output.
//! - [`Parser::span`][Parser::span]: If the child parser was successful, return the location of the consumed input as the produced value
//! - [`Parser::with_span`][Parser::with_span]: If the child parser was successful, return a tuple of the location of the consumed input and the produced output.
//! - [`Parser::verify`]: Returns the result of the child parser if it satisfies a verification function
//!
//! ## Error management and debugging
//!
//! - [`cut_err`]: Commit the parse result, disallowing alternative parsers from being attempted
//! - [`backtrack_err`]: Attemmpts a parse, allowing alternative parsers to be attempted despite
//!   use of `cut_err`
//! - [`Parser::context`]: Add context to the error if the parser fails
//! - [`trace`][crate::trace::trace]: Print the parse state with the `debug` feature flag
//! - [`todo()`]: Placeholder parser
//!
//! ## Remaining combinators
//!
//! - [`success`][success]: Returns a value without consuming any input, always succeeds
//! - [`fail`][fail]: Inversion of `success`. Always fails.
//! - [`Parser::by_ref`]: Allow moving `&mut impl Parser` into other parsers
//!
//! ## Text parsing
//!
//! - [`any`][crate::bytes::any]: Matches one token
//! - [`tab`][crate::character::tab]: Matches a tab character `\t`
//! - [`crlf`][crate::character::crlf]: Recognizes the string `\r\n`
//! - [`line_ending`][crate::character::line_ending]: Recognizes an end of line (both `\n` and `\r\n`)
//! - [`newline`][crate::character::newline]: Matches a newline character `\n`
//! - [`not_line_ending`][crate::character::not_line_ending]: Recognizes a string of any char except `\r` or `\n`
//! - [`rest`][rest]: Return the remaining input
//!
//! - [`alpha0`][crate::character::alpha0]: Recognizes zero or more lowercase and uppercase alphabetic characters: `[a-zA-Z]`. [`alpha1`][crate::character::alpha1] does the same but returns at least one character
//! - [`alphanumeric0`][crate::character::alphanumeric0]: Recognizes zero or more numerical and alphabetic characters: `[0-9a-zA-Z]`. [`alphanumeric1`][crate::character::alphanumeric1] does the same but returns at least one character
//! - [`space0`][crate::character::space0]: Recognizes zero or more spaces and tabs. [`space1`][crate::character::space1] does the same but returns at least one character
//! - [`multispace0`][crate::character::multispace0]: Recognizes zero or more spaces, tabs, carriage returns and line feeds. [`multispace1`][crate::character::multispace1] does the same but returns at least one character
//! - [`digit0`][crate::character::digit0]: Recognizes zero or more numerical characters: `[0-9]`. [`digit1`][crate::character::digit1] does the same but returns at least one character
//! - [`hex_digit0`][crate::character::hex_digit0]: Recognizes zero or more hexadecimal numerical characters: `[0-9A-Fa-f]`. [`hex_digit1`][crate::character::hex_digit1] does the same but returns at least one character
//! - [`oct_digit0`][crate::character::oct_digit0]: Recognizes zero or more octal characters: `[0-7]`. [`oct_digit1`][crate::character::oct_digit1] does the same but returns at least one character
//!
//! - [`float`][crate::character::float]: Parse a floating point number in a byte string
//! - [`dec_int`][crate::character::dec_uint]: Decode a variable-width, decimal signed integer
//! - [`dec_uint`][crate::character::dec_uint]: Decode a variable-width, decimal unsigned integer
//! - [`hex_uint`][crate::character::hex_uint]: Decode a variable-width, hexadecimal integer
//!
//! - [`escaped`][crate::character::escaped]: Matches a byte string with escaped characters
//! - [`escaped_transform`][crate::character::escaped_transform]: Matches a byte string with escaped characters, and returns a new string with the escaped characters replaced
//!
//! ### Character test functions
//!
//! Use these functions with a combinator like `take_while0`:
//!
//! - [`AsChar::is_alpha`][crate::stream::AsChar::is_alpha]: Tests if byte is ASCII alphabetic: `[A-Za-z]`
//! - [`AsChar::is_alphanum`][crate::stream::AsChar::is_alphanum]: Tests if byte is ASCII alphanumeric: `[A-Za-z0-9]`
//! - [`AsChar::is_dec_digit`][crate::stream::AsChar::is_dec_digit]: Tests if byte is ASCII digit: `[0-9]`
//! - [`AsChar::is_hex_digit`][crate::stream::AsChar::is_hex_digit]: Tests if byte is ASCII hex digit: `[0-9A-Fa-f]`
//! - [`AsChar::is_oct_digit`][crate::stream::AsChar::is_oct_digit]: Tests if byte is ASCII octal digit: `[0-7]`
//! - [`AsChar::is_space`][crate::stream::AsChar::is_space]: Tests if byte is ASCII space or tab: `[ \t]`
//! - [`AsChar::is_newline`][crate::stream::AsChar::is_newline]: Tests if byte is ASCII newline: `[\n]`
//!
//! ## Binary format parsing
//!
//! - [`length_data`][crate::multi::length_data]: Gets a number from the first parser, then takes a subslice of the input of that size, and returns that subslice
//! - [`length_value`][crate::multi::length_value]: Gets a number from the first parser, takes a subslice of the input of that size, then applies the second parser on that subslice. If the second parser returns `Incomplete`, `length_value` will return an error
//!
//! ### Integers
//!
//! Parsing integers from binary formats can be done in two ways: With parser functions, or combinators with configurable endianness.
//!
//! - **configurable endianness:** [`i16`][crate::number::i16], [`i32`][crate::number::i32],
//!   [`i64`][crate::number::i64], [`u16`][crate::number::u16], [`u32`][crate::number::u32],
//!   [`u64`][crate::number::u64] are combinators that take as argument a
//!   [`winnow::number::Endianness`][crate::number::Endianness], like this: `i16(endianness)`. If the
//!   parameter is `winnow::number::Endianness::Big`, parse a big endian `i16` integer, otherwise a
//!   little endian `i16` integer.
//! - **fixed endianness**: The functions are prefixed by `be_` for big endian numbers, and by `le_` for little endian numbers, and the suffix is the type they parse to. As an example, `be_u32` parses a big endian unsigned integer stored in 32 bits.
//!   - [`be_f32`][crate::number::be_f32], [`be_f64`][crate::number::be_f64]: Big endian floating point numbers
//!   - [`le_f32`][crate::number::le_f32], [`le_f64`][crate::number::le_f64]: Little endian floating point numbers
//!   - [`be_i8`][crate::number::be_i8], [`be_i16`][crate::number::be_i16], [`be_i24`][crate::number::be_i24], [`be_i32`][crate::number::be_i32], [`be_i64`][crate::number::be_i64], [`be_i128`][crate::number::be_i128]: Big endian signed integers
//!   - [`be_u8`][crate::number::be_u8], [`be_u16`][crate::number::be_u16], [`be_u24`][crate::number::be_u24], [`be_u32`][crate::number::be_u32], [`be_u64`][crate::number::be_u64], [`be_u128`][crate::number::be_u128]: Big endian unsigned integers
//!   - [`le_i8`][crate::number::le_i8], [`le_i16`][crate::number::le_i16], [`le_i24`][crate::number::le_i24], [`le_i32`][crate::number::le_i32], [`le_i64`][crate::number::le_i64], [`le_i128`][crate::number::le_i128]: Little endian signed integers
//!   - [`le_u8`][crate::number::le_u8], [`le_u16`][crate::number::le_u16], [`le_u24`][crate::number::le_u24], [`le_u32`][crate::number::le_u32], [`le_u64`][crate::number::le_u64], [`le_u128`][crate::number::le_u128]: Little endian unsigned integers
//!
//! ### Bit stream parsing
//!
//! - [`bits`][crate::bits::bits]: Transforms the current input type (byte slice `&[u8]`) to a bit stream on which bit specific parsers and more general combinators can be applied
//! - [`bytes`][crate::bits::bytes]: Transforms its bits stream input back into a byte slice for the underlying parser
//! - [`take`][crate::bits::take]: Take a set number of its
//! - [`tag`][crate::bits::tag]: Check if a set number of bis matches a pattern
//! - [`bool`][crate::bits::bool]: Match any one bit

use crate::error::{ContextError, ErrMode, ErrorKind, FromExternalError, Needed, ParseError};
use crate::lib::std::borrow::Borrow;
use crate::lib::std::ops::Range;
use crate::stream::{Location, Stream};
use crate::stream::{Offset, StreamIsPartial};
use crate::trace::trace;
use crate::trace::trace_result;
use crate::*;

#[cfg(test)]
mod tests;

/// Return the remaining input.
///
/// # Example
///
/// ```rust
/// # use winnow::error::ErrorKind;
/// # use winnow::error::Error;
/// use winnow::combinator::rest;
/// assert_eq!(rest::<_,Error<_>>("abc"), Ok(("", "abc")));
/// assert_eq!(rest::<_,Error<_>>(""), Ok(("", "")));
/// ```
#[inline]
pub fn rest<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: Stream,
{
    trace("rest", move |input: I| {
        Ok(input.next_slice(input.eof_offset()))
    })
    .parse_next(input)
}

/// Return the length of the remaining input.
///
/// Note: this does not advance the [`Stream`]
///
/// # Example
///
/// ```rust
/// # use winnow::error::ErrorKind;
/// # use winnow::error::Error;
/// use winnow::combinator::rest_len;
/// assert_eq!(rest_len::<_,Error<_>>("abc"), Ok(("abc", 3)));
/// assert_eq!(rest_len::<_,Error<_>>(""), Ok(("", 0)));
/// ```
#[inline]
pub fn rest_len<I, E: ParseError<I>>(input: I) -> IResult<I, usize, E>
where
    I: Stream,
{
    trace("rest_len", move |input: I| {
        let len = input.eof_offset();
        Ok((input, len))
    })
    .parse_next(input)
}

/// Implementation of [`Parser::by_ref`][Parser::by_ref]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct ByRef<'p, P> {
    p: &'p mut P,
}

impl<'p, P> ByRef<'p, P> {
    pub(crate) fn new(p: &'p mut P) -> Self {
        Self { p }
    }
}

impl<'p, I, O, E, P: Parser<I, O, E>> Parser<I, O, E> for ByRef<'p, P> {
    fn parse_next(&mut self, i: I) -> IResult<I, O, E> {
        self.p.parse_next(i)
    }
}

/// Implementation of [`Parser::map`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct Map<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Fn(O) -> O2,
{
    parser: F,
    map: G,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    o2: core::marker::PhantomData<O2>,
    e: core::marker::PhantomData<E>,
}

impl<F, G, I, O, O2, E> Map<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Fn(O) -> O2,
{
    pub(crate) fn new(parser: F, map: G) -> Self {
        Self {
            parser,
            map,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, G, I, O, O2, E> Parser<I, O2, E> for Map<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Fn(O) -> O2,
{
    fn parse_next(&mut self, i: I) -> IResult<I, O2, E> {
        match self.parser.parse_next(i) {
            Err(e) => Err(e),
            Ok((i, o)) => Ok((i, (self.map)(o))),
        }
    }
}

/// Implementation of [`Parser::map_res`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct MapRes<F, G, I, O, O2, E, E2>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Result<O2, E2>,
    I: Clone,
    E: FromExternalError<I, E2>,
{
    parser: F,
    map: G,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    o2: core::marker::PhantomData<O2>,
    e: core::marker::PhantomData<E>,
    e2: core::marker::PhantomData<E2>,
}

impl<F, G, I, O, O2, E, E2> MapRes<F, G, I, O, O2, E, E2>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Result<O2, E2>,
    I: Clone,
    E: FromExternalError<I, E2>,
{
    pub(crate) fn new(parser: F, map: G) -> Self {
        Self {
            parser,
            map,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
            e2: Default::default(),
        }
    }
}

impl<F, G, I, O, O2, E, E2> Parser<I, O2, E> for MapRes<F, G, I, O, O2, E, E2>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Result<O2, E2>,
    I: Clone,
    E: FromExternalError<I, E2>,
{
    fn parse_next(&mut self, input: I) -> IResult<I, O2, E> {
        let i = input.clone();
        let (input, o) = self.parser.parse_next(input)?;
        let res = match (self.map)(o) {
            Ok(o2) => Ok((input, o2)),
            Err(e) => Err(ErrMode::from_external_error(i, ErrorKind::Verify, e)),
        };
        trace_result("verify", &res);
        res
    }
}

/// Implementation of [`Parser::verify_map`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct VerifyMap<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Option<O2>,
    I: Clone,
    E: ParseError<I>,
{
    parser: F,
    map: G,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    o2: core::marker::PhantomData<O2>,
    e: core::marker::PhantomData<E>,
}

impl<F, G, I, O, O2, E> VerifyMap<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Option<O2>,
    I: Clone,
    E: ParseError<I>,
{
    pub(crate) fn new(parser: F, map: G) -> Self {
        Self {
            parser,
            map,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, G, I, O, O2, E> Parser<I, O2, E> for VerifyMap<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Option<O2>,
    I: Clone,
    E: ParseError<I>,
{
    fn parse_next(&mut self, input: I) -> IResult<I, O2, E> {
        let i = input.clone();
        let (input, o) = self.parser.parse_next(input)?;
        let res = match (self.map)(o) {
            Some(o2) => Ok((input, o2)),
            None => Err(ErrMode::from_error_kind(i, ErrorKind::Verify)),
        };
        trace_result("verify", &res);
        res
    }
}

/// Implementation of [`Parser::and_then`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct AndThen<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Parser<O, O2, E>,
    O: StreamIsPartial,
{
    outer: F,
    inner: G,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    o2: core::marker::PhantomData<O2>,
    e: core::marker::PhantomData<E>,
}

impl<F, G, I, O, O2, E> AndThen<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Parser<O, O2, E>,
    O: StreamIsPartial,
{
    pub(crate) fn new(outer: F, inner: G) -> Self {
        Self {
            outer,
            inner,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, G, I, O, O2, E> Parser<I, O2, E> for AndThen<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Parser<O, O2, E>,
    O: StreamIsPartial,
{
    fn parse_next(&mut self, i: I) -> IResult<I, O2, E> {
        let (i, mut o) = self.outer.parse_next(i)?;
        let _ = o.complete();
        let (_, o2) = self.inner.parse_next(o)?;
        Ok((i, o2))
    }
}

/// Implementation of [`Parser::parse_to`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct ParseTo<P, I, O, O2, E>
where
    P: Parser<I, O, E>,
    I: Stream,
    O: crate::stream::ParseSlice<O2>,
    E: ParseError<I>,
{
    p: P,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    o2: core::marker::PhantomData<O2>,
    e: core::marker::PhantomData<E>,
}

impl<P, I, O, O2, E> ParseTo<P, I, O, O2, E>
where
    P: Parser<I, O, E>,
    I: Stream,
    O: crate::stream::ParseSlice<O2>,
    E: ParseError<I>,
{
    pub(crate) fn new(p: P) -> Self {
        Self {
            p,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }
}

impl<P, I, O, O2, E> Parser<I, O2, E> for ParseTo<P, I, O, O2, E>
where
    P: Parser<I, O, E>,
    I: Stream,
    O: crate::stream::ParseSlice<O2>,
    E: ParseError<I>,
{
    fn parse_next(&mut self, i: I) -> IResult<I, O2, E> {
        let input = i.clone();
        let (i, o) = self.p.parse_next(i)?;

        let res = o
            .parse_slice()
            .ok_or_else(|| ErrMode::from_error_kind(input, ErrorKind::Verify));
        trace_result("verify", &res);
        Ok((i, res?))
    }
}

/// Implementation of [`Parser::flat_map`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct FlatMap<F, G, H, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> H,
    H: Parser<I, O2, E>,
{
    f: F,
    g: G,
    h: core::marker::PhantomData<H>,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    o2: core::marker::PhantomData<O2>,
    e: core::marker::PhantomData<E>,
}

impl<F, G, H, I, O, O2, E> FlatMap<F, G, H, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> H,
    H: Parser<I, O2, E>,
{
    pub(crate) fn new(f: F, g: G) -> Self {
        Self {
            f,
            g,
            h: Default::default(),
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, G, H, I, O, O2, E> Parser<I, O2, E> for FlatMap<F, G, H, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> H,
    H: Parser<I, O2, E>,
{
    fn parse_next(&mut self, i: I) -> IResult<I, O2, E> {
        let (i, o) = self.f.parse_next(i)?;
        (self.g)(o).parse_next(i)
    }
}

/// Apply a [`Parser`], producing `None` on [`ErrMode::Backtrack`].
///
/// To chain an error up, see [`cut_err`].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::prelude::*;
/// use winnow::combinator::opt;
/// use winnow::character::alpha1;
/// # fn main() {
///
/// fn parser(i: &str) -> IResult<&str, Option<&str>> {
///   opt(alpha1).parse_next(i)
/// }
///
/// assert_eq!(parser("abcd;"), Ok((";", Some("abcd"))));
/// assert_eq!(parser("123;"), Ok(("123;", None)));
/// # }
/// ```
pub fn opt<I: Stream, O, E: ParseError<I>, F>(mut f: F) -> impl Parser<I, Option<O>, E>
where
    F: Parser<I, O, E>,
{
    trace("opt", move |input: I| {
        let i = input.clone();
        match f.parse_next(input) {
            Ok((i, o)) => Ok((i, Some(o))),
            Err(ErrMode::Backtrack(_)) => Ok((i, None)),
            Err(e) => Err(e),
        }
    })
}

/// Calls the parser if the condition is met.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult};
/// # use winnow::prelude::*;
/// use winnow::combinator::cond;
/// use winnow::character::alpha1;
/// # fn main() {
///
/// fn parser(b: bool, i: &str) -> IResult<&str, Option<&str>> {
///   cond(b, alpha1).parse_next(i)
/// }
///
/// assert_eq!(parser(true, "abcd;"), Ok((";", Some("abcd"))));
/// assert_eq!(parser(false, "abcd;"), Ok(("abcd;", None)));
/// assert_eq!(parser(true, "123;"), Err(ErrMode::Backtrack(Error::new("123;", ErrorKind::Slice))));
/// assert_eq!(parser(false, "123;"), Ok(("123;", None)));
/// # }
/// ```
pub fn cond<I, O, E: ParseError<I>, F>(b: bool, mut f: F) -> impl Parser<I, Option<O>, E>
where
    I: Stream,
    F: Parser<I, O, E>,
{
    trace("cond", move |input: I| {
        if b {
            match f.parse_next(input) {
                Ok((i, o)) => Ok((i, Some(o))),
                Err(e) => Err(e),
            }
        } else {
            Ok((input, None))
        }
    })
}

/// Tries to apply its parser without consuming the input.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult};
/// # use winnow::prelude::*;
/// use winnow::combinator::peek;
/// use winnow::character::alpha1;
/// # fn main() {
///
/// let mut parser = peek(alpha1);
///
/// assert_eq!(parser.parse_next("abcd;"), Ok(("abcd;", "abcd")));
/// assert_eq!(parser.parse_next("123;"), Err(ErrMode::Backtrack(Error::new("123;", ErrorKind::Slice))));
/// # }
/// ```
#[doc(alias = "look_ahead")]
#[doc(alias = "rewind")]
pub fn peek<I: Stream, O, E: ParseError<I>, F>(mut f: F) -> impl Parser<I, O, E>
where
    F: Parser<I, O, E>,
{
    trace("peek", move |input: I| {
        let i = input.clone();
        match f.parse_next(input) {
            Ok((_, o)) => Ok((i, o)),
            Err(e) => Err(e),
        }
    })
}

/// Match the end of the [`Stream`]
///
/// Otherwise, it will error.
///
/// # Example
///
/// ```rust
/// # use std::str;
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::combinator::eof;
/// # use winnow::prelude::*;
///
/// let mut parser = eof;
/// assert_eq!(parser.parse_next("abc"), Err(ErrMode::Backtrack(Error::new("abc", ErrorKind::Eof))));
/// assert_eq!(parser.parse_next(""), Ok(("", "")));
/// ```
#[doc(alias = "end")]
#[doc(alias = "eoi")]
pub fn eof<I, E: ParseError<I>>(input: I) -> IResult<I, <I as Stream>::Slice, E>
where
    I: Stream,
{
    trace("eof", move |input: I| {
        if input.eof_offset() == 0 {
            Ok(input.next_slice(0))
        } else {
            Err(ErrMode::from_error_kind(input, ErrorKind::Eof))
        }
    })
    .parse_next(input)
}

/// Implementation of [`Parser::complete_err`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct CompleteErr<F> {
    f: F,
}

impl<F> CompleteErr<F> {
    pub(crate) fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F, I, O, E> Parser<I, O, E> for CompleteErr<F>
where
    I: Stream,
    F: Parser<I, O, E>,
    E: ParseError<I>,
{
    fn parse_next(&mut self, input: I) -> IResult<I, O, E> {
        trace("complete_err", |input: I| {
            let i = input.clone();
            match (self.f).parse_next(input) {
                Err(ErrMode::Incomplete(_)) => {
                    Err(ErrMode::from_error_kind(i, ErrorKind::Complete))
                }
                rest => rest,
            }
        })
        .parse_next(input)
    }
}

/// Implementation of [`Parser::verify`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct Verify<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Fn(&O2) -> bool,
    I: Clone,
    O: Borrow<O2>,
    O2: ?Sized,
    E: ParseError<I>,
{
    parser: F,
    filter: G,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    o2: core::marker::PhantomData<O2>,
    e: core::marker::PhantomData<E>,
}

impl<F, G, I, O, O2, E> Verify<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Fn(&O2) -> bool,
    I: Clone,
    O: Borrow<O2>,
    O2: ?Sized,
    E: ParseError<I>,
{
    pub(crate) fn new(parser: F, filter: G) -> Self {
        Self {
            parser,
            filter,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, G, I, O, O2, E> Parser<I, O, E> for Verify<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Fn(&O2) -> bool,
    I: Clone,
    O: Borrow<O2>,
    O2: ?Sized,
    E: ParseError<I>,
{
    fn parse_next(&mut self, input: I) -> IResult<I, O, E> {
        let i = input.clone();
        let (input, o) = self.parser.parse_next(input)?;

        let res = if (self.filter)(o.borrow()) {
            Ok((input, o))
        } else {
            Err(ErrMode::from_error_kind(i, ErrorKind::Verify))
        };
        trace_result("verify", &res);
        res
    }
}

/// Implementation of [`Parser::value`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct Value<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O2: Clone,
{
    parser: F,
    val: O2,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    e: core::marker::PhantomData<E>,
}

impl<F, I, O, O2, E> Value<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O2: Clone,
{
    pub(crate) fn new(parser: F, val: O2) -> Self {
        Self {
            parser,
            val,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, I, O, O2, E> Parser<I, O2, E> for Value<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O2: Clone,
{
    fn parse_next(&mut self, input: I) -> IResult<I, O2, E> {
        (self.parser)
            .parse_next(input)
            .map(|(i, _)| (i, self.val.clone()))
    }
}

/// Implementation of [`Parser::void`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct Void<F, I, O, E>
where
    F: Parser<I, O, E>,
{
    parser: F,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    e: core::marker::PhantomData<E>,
}

impl<F, I, O, E> Void<F, I, O, E>
where
    F: Parser<I, O, E>,
{
    pub(crate) fn new(parser: F) -> Self {
        Self {
            parser,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, I, O, E> Parser<I, (), E> for Void<F, I, O, E>
where
    F: Parser<I, O, E>,
{
    fn parse_next(&mut self, input: I) -> IResult<I, (), E> {
        (self.parser).parse_next(input).map(|(i, _)| (i, ()))
    }
}

/// Succeeds if the child parser returns an error.
///
/// **Note:** This does not advance the [`Stream`]
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult};
/// # use winnow::prelude::*;
/// use winnow::combinator::not;
/// use winnow::character::alpha1;
/// # fn main() {
///
/// let mut parser = not(alpha1);
///
/// assert_eq!(parser.parse_next("123"), Ok(("123", ())));
/// assert_eq!(parser.parse_next("abcd"), Err(ErrMode::Backtrack(Error::new("abcd", ErrorKind::Not))));
/// # }
/// ```
pub fn not<I: Stream, O, E: ParseError<I>, F>(mut parser: F) -> impl Parser<I, (), E>
where
    F: Parser<I, O, E>,
{
    trace("not", move |input: I| {
        let i = input.clone();
        match parser.parse_next(input) {
            Ok(_) => Err(ErrMode::from_error_kind(i, ErrorKind::Not)),
            Err(ErrMode::Backtrack(_)) => Ok((i, ())),
            Err(e) => Err(e),
        }
    })
}

/// Implementation of [`Parser::recognize`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct Recognize<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Offset,
{
    parser: F,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    e: core::marker::PhantomData<E>,
}

impl<F, I, O, E> Recognize<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Offset,
{
    pub(crate) fn new(parser: F) -> Self {
        Self {
            parser,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }
}

impl<I, O, E, F> Parser<I, <I as Stream>::Slice, E> for Recognize<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Offset,
{
    fn parse_next(&mut self, input: I) -> IResult<I, <I as Stream>::Slice, E> {
        let i = input.clone();
        match (self.parser).parse_next(i) {
            Ok((i, _)) => {
                let offset = input.offset_to(&i);
                Ok(input.next_slice(offset))
            }
            Err(e) => Err(e),
        }
    }
}

/// Implementation of [`Parser::with_recognized`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct WithRecognized<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Offset,
{
    parser: F,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    e: core::marker::PhantomData<E>,
}

impl<F, I, O, E> WithRecognized<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Offset,
{
    pub(crate) fn new(parser: F) -> Self {
        Self {
            parser,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, I, O, E> Parser<I, (O, <I as Stream>::Slice), E> for WithRecognized<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Offset,
{
    fn parse_next(&mut self, input: I) -> IResult<I, (O, <I as Stream>::Slice), E> {
        let i = input.clone();
        match (self.parser).parse_next(i) {
            Ok((remaining, result)) => {
                let offset = input.offset_to(&remaining);
                let (remaining, recognized) = input.next_slice(offset);
                Ok((remaining, (result, recognized)))
            }
            Err(e) => Err(e),
        }
    }
}

/// Implementation of [`Parser::span`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct Span<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Clone + Location,
{
    parser: F,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    e: core::marker::PhantomData<E>,
}

impl<F, I, O, E> Span<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Clone + Location,
{
    pub(crate) fn new(parser: F) -> Self {
        Self {
            parser,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }
}

impl<I, O, E, F> Parser<I, Range<usize>, E> for Span<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Clone + Location,
{
    fn parse_next(&mut self, input: I) -> IResult<I, Range<usize>, E> {
        let start = input.location();
        self.parser.parse_next(input).map(move |(remaining, _)| {
            let end = remaining.location();
            (remaining, (start..end))
        })
    }
}

/// Implementation of [`Parser::with_span`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct WithSpan<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Clone + Location,
{
    parser: F,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    e: core::marker::PhantomData<E>,
}

impl<F, I, O, E> WithSpan<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Clone + Location,
{
    pub(crate) fn new(parser: F) -> Self {
        Self {
            parser,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, I, O, E> Parser<I, (O, Range<usize>), E> for WithSpan<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Clone + Location,
{
    fn parse_next(&mut self, input: I) -> IResult<I, (O, Range<usize>), E> {
        let start = input.location();
        self.parser
            .parse_next(input)
            .map(move |(remaining, output)| {
                let end = remaining.location();
                (remaining, (output, (start..end)))
            })
    }
}

/// Transforms an [`ErrMode::Backtrack`] (recoverable) to [`ErrMode::Cut`] (unrecoverable)
///
/// This commits the parse result, preventing alternative branch paths like with
/// [`winnow::branch::alt`][crate::branch::alt].
///
/// # Example
///
/// Without `cut_err`:
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::bytes::one_of;
/// # use winnow::character::digit1;
/// # use winnow::combinator::rest;
/// # use winnow::branch::alt;
/// # use winnow::sequence::preceded;
/// # use winnow::prelude::*;
/// # fn main() {
///
/// fn parser(input: &str) -> IResult<&str, &str> {
///   alt((
///     preceded(one_of("+-"), digit1),
///     rest
///   )).parse_next(input)
/// }
///
/// assert_eq!(parser("+10 ab"), Ok((" ab", "10")));
/// assert_eq!(parser("ab"), Ok(("", "ab")));
/// assert_eq!(parser("+"), Ok(("", "+")));
/// # }
/// ```
///
/// With `cut_err`:
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::prelude::*;
/// # use winnow::bytes::one_of;
/// # use winnow::character::digit1;
/// # use winnow::combinator::rest;
/// # use winnow::branch::alt;
/// # use winnow::sequence::preceded;
/// use winnow::combinator::cut_err;
/// # fn main() {
///
/// fn parser(input: &str) -> IResult<&str, &str> {
///   alt((
///     preceded(one_of("+-"), cut_err(digit1)),
///     rest
///   )).parse_next(input)
/// }
///
/// assert_eq!(parser("+10 ab"), Ok((" ab", "10")));
/// assert_eq!(parser("ab"), Ok(("", "ab")));
/// assert_eq!(parser("+"), Err(ErrMode::Cut(Error { input: "", kind: ErrorKind::Slice })));
/// # }
/// ```
pub fn cut_err<I, O, E: ParseError<I>, F>(mut parser: F) -> impl Parser<I, O, E>
where
    I: Stream,
    F: Parser<I, O, E>,
{
    trace("cut_err", move |input: I| {
        parser.parse_next(input).map_err(|e| e.cut())
    })
}

/// Transforms an [`ErrMode::Cut`] (unrecoverable) to [`ErrMode::Backtrack`] (recoverable)
///
/// This attempts the parse, allowing other parsers to be tried on failure, like with
/// [`winnow::branch::alt`][crate::branch::alt].
pub fn backtrack_err<I, O, E: ParseError<I>, F>(mut parser: F) -> impl Parser<I, O, E>
where
    I: Stream,
    F: Parser<I, O, E>,
{
    trace("backtrack_err", move |input: I| {
        parser.parse_next(input).map_err(|e| e.backtrack())
    })
}

/// A placeholder for a not-yet-implemented [`Parser`]
///
/// This is analogous to the [`todo!`] macro and helps with prototyping.
///
/// # Panic
///
/// This will panic when parsing
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::combinator::todo;
///
/// fn parser(input: &str) -> IResult<&str, u64> {
///     todo(input)
/// }
/// ```
#[track_caller]
pub fn todo<I, O, E>(input: I) -> IResult<I, O, E>
where
    I: Stream,
{
    #![allow(clippy::todo)]
    trace("todo", move |_input: I| todo!("unimplemented parse")).parse_next(input)
}

/// Implementation of [`Parser::output_into`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct OutputInto<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O: Into<O2>,
{
    parser: F,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    o2: core::marker::PhantomData<O2>,
    e: core::marker::PhantomData<E>,
}

impl<F, I, O, O2, E> OutputInto<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O: Into<O2>,
{
    pub(crate) fn new(parser: F) -> Self {
        Self {
            parser,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, I, O, O2, E> Parser<I, O2, E> for OutputInto<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O: Into<O2>,
{
    fn parse_next(&mut self, i: I) -> IResult<I, O2, E> {
        match self.parser.parse_next(i) {
            Ok((i, o)) => Ok((i, o.into())),
            Err(err) => Err(err),
        }
    }
}

/// Implementation of [`Parser::err_into`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct ErrInto<F, I, O, E, E2>
where
    F: Parser<I, O, E>,
    E: Into<E2>,
{
    parser: F,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    e: core::marker::PhantomData<E>,
    e2: core::marker::PhantomData<E2>,
}

impl<F, I, O, E, E2> ErrInto<F, I, O, E, E2>
where
    F: Parser<I, O, E>,
    E: Into<E2>,
{
    pub(crate) fn new(parser: F) -> Self {
        Self {
            parser,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
            e2: Default::default(),
        }
    }
}

impl<F, I, O, E, E2> Parser<I, O, E2> for ErrInto<F, I, O, E, E2>
where
    F: Parser<I, O, E>,
    E: Into<E2>,
{
    fn parse_next(&mut self, i: I) -> IResult<I, O, E2> {
        match self.parser.parse_next(i) {
            Ok(ok) => Ok(ok),
            Err(ErrMode::Backtrack(e)) => Err(ErrMode::Backtrack(e.into())),
            Err(ErrMode::Cut(e)) => Err(ErrMode::Cut(e.into())),
            Err(ErrMode::Incomplete(e)) => Err(ErrMode::Incomplete(e)),
        }
    }
}

/// Creates an iterator from input data and a parser.
///
/// Call the iterator's [`ParserIterator::finish`] method to get the remaining input if successful,
/// or the error value if we encountered an error.
///
/// On [`ErrMode::Backtrack`], iteration will stop.  To instead chain an error up, see [`cut_err`].
///
/// # Example
///
/// ```rust
/// use winnow::{combinator::iterator, IResult, bytes::tag, character::alpha1, sequence::terminated};
/// use std::collections::HashMap;
///
/// let data = "abc|defg|hijkl|mnopqr|123";
/// let mut it = iterator(data, terminated(alpha1, "|"));
///
/// let parsed = it.map(|v| (v, v.len())).collect::<HashMap<_,_>>();
/// let res: IResult<_,_> = it.finish();
///
/// assert_eq!(parsed, [("abc", 3usize), ("defg", 4), ("hijkl", 5), ("mnopqr", 6)].iter().cloned().collect());
/// assert_eq!(res, Ok(("123", ())));
/// ```
pub fn iterator<I, O, E, F>(input: I, parser: F) -> ParserIterator<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream,
    E: ParseError<I>,
{
    ParserIterator {
        parser,
        input,
        state: Some(State::Running),
        o: Default::default(),
    }
}

/// Main structure associated to [`iterator`].
pub struct ParserIterator<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream,
{
    parser: F,
    input: I,
    state: Option<State<E>>,
    o: core::marker::PhantomData<O>,
}

impl<F, I, O, E> ParserIterator<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream,
{
    /// Returns the remaining input if parsing was successful, or the error if we encountered an error.
    pub fn finish(mut self) -> IResult<I, (), E> {
        match self.state.take().unwrap() {
            State::Running | State::Done => Ok((self.input, ())),
            State::Failure(e) => Err(ErrMode::Cut(e)),
            State::Incomplete(i) => Err(ErrMode::Incomplete(i)),
        }
    }
}

impl<'a, F, I, O, E> core::iter::Iterator for &'a mut ParserIterator<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream,
{
    type Item = O;

    fn next(&mut self) -> Option<Self::Item> {
        if let State::Running = self.state.take().unwrap() {
            let input = self.input.clone();

            match self.parser.parse_next(input) {
                Ok((i, o)) => {
                    self.input = i;
                    self.state = Some(State::Running);
                    Some(o)
                }
                Err(ErrMode::Backtrack(_)) => {
                    self.state = Some(State::Done);
                    None
                }
                Err(ErrMode::Cut(e)) => {
                    self.state = Some(State::Failure(e));
                    None
                }
                Err(ErrMode::Incomplete(i)) => {
                    self.state = Some(State::Incomplete(i));
                    None
                }
            }
        } else {
            None
        }
    }
}

enum State<E> {
    Running,
    Done,
    Failure(E),
    Incomplete(Needed),
}

/// Always succeeds with given value without consuming any input.
///
/// For example, it can be used as the last alternative in `alt` to
/// specify the default case.
///
/// **Note:** This never advances the [`Stream`]
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// # use winnow::prelude::*;
/// use winnow::branch::alt;
/// use winnow::combinator::success;
///
/// let mut parser = success::<_,_,Error<_>>(10);
/// assert_eq!(parser.parse_next("xyz"), Ok(("xyz", 10)));
///
/// fn sign(input: &str) -> IResult<&str, isize> {
///     alt((
///         '-'.value(-1),
///         '+'.value(1),
///         success::<_,_,Error<_>>(1)
///     )).parse_next(input)
/// }
/// assert_eq!(sign("+10"), Ok(("10", 1)));
/// assert_eq!(sign("-10"), Ok(("10", -1)));
/// assert_eq!(sign("10"), Ok(("10", 1)));
/// ```
#[doc(alias = "value")]
#[doc(alias = "empty")]
pub fn success<I: Stream, O: Clone, E: ParseError<I>>(val: O) -> impl Parser<I, O, E> {
    trace("success", move |input: I| Ok((input, val.clone())))
}

/// A parser which always fails.
///
/// For example, it can be used as the last alternative in `alt` to
/// control the error message given.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult};
/// use winnow::combinator::fail;
///
/// let s = "string";
/// assert_eq!(fail::<_, &str, _>(s), Err(ErrMode::Backtrack(Error::new(s, ErrorKind::Fail))));
/// ```
#[doc(alias = "unexpected")]
pub fn fail<I: Stream, O, E: ParseError<I>>(i: I) -> IResult<I, O, E> {
    trace("fail", |i| {
        Err(ErrMode::from_error_kind(i, ErrorKind::Fail))
    })
    .parse_next(i)
}

/// Implementation of [`Parser::context`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct Context<F, I, O, E, C>
where
    F: Parser<I, O, E>,
    I: Stream,
    E: ContextError<I, C>,
    C: Clone + crate::lib::std::fmt::Debug,
{
    parser: F,
    context: C,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    e: core::marker::PhantomData<E>,
}

impl<F, I, O, E, C> Context<F, I, O, E, C>
where
    F: Parser<I, O, E>,
    I: Stream,
    E: ContextError<I, C>,
    C: Clone + crate::lib::std::fmt::Debug,
{
    pub(crate) fn new(parser: F, context: C) -> Self {
        Self {
            parser,
            context,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }
}

impl<F, I, O, E, C> Parser<I, O, E> for Context<F, I, O, E, C>
where
    F: Parser<I, O, E>,
    I: Stream,
    E: ContextError<I, C>,
    C: Clone + crate::lib::std::fmt::Debug,
{
    fn parse_next(&mut self, i: I) -> IResult<I, O, E> {
        #[cfg(feature = "debug")]
        let name = format!("context={:?}", self.context);
        #[cfg(not(feature = "debug"))]
        let name = "context";
        trace(name, move |i: I| {
            (self.parser)
                .parse_next(i.clone())
                .map_err(|err| err.map(|err| err.add_context(i, self.context.clone())))
        })
        .parse_next(i)
    }
}
