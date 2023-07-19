//! # Error management
//!
//! nom's errors are designed with multiple needs in mind:
//! - indicate which parser failed and where in the input data
//! - accumulate more context as the error goes up the parser chain
//! - have a very low overhead, as errors are often discarded by the calling parser (examples: `many0`, `alt`)
//! - can be modified according to the user's needs, because some languages need a lot more information
//!
//! To match these requirements, nom parsers have to return the following result
//! type:
//!
//! ```rust
//! pub type IResult<I, O, E=nom8::error::Error<I>> = Result<(I, O), nom8::Err<E>>;
//!
//! #[derive(Debug, PartialEq, Eq, Clone, Copy)]
//! pub enum Needed {
//!   Unknown,
//!   Size(u32)
//! }
//!
//! pub enum Err<E> {
//!     Incomplete(Needed),
//!     Error(E),
//!     Failure(E),
//! }
//! ```
//!
//! The result is either an `Ok((I, O))` containing the remaining input and the
//! parsed value, or an `Err(nom8::Err<E>)` with `E` the error type.
//! `nom8::Err<E>` is an enum because combinators can have different behaviours
//! depending on the value:
//! - `Error` is a normal parser error. If a child parser of the `alt` combinator returns `Error`, it will try another child parser
//! - `Failure` is an error from which we cannot recover: The `alt` combinator will not try other branches if a child parser returns `Failure`. This is used when we know we were in the right branch of `alt` and do not need to try other branches
//! - `Incomplete` indicates that a parser did not have enough data to decide. This can be returned by parsers found in `streaming` submodules. Parsers in the `complete` submodules assume that they have the entire input data, so if it was not sufficient, they will instead return a `Err::Error`. When a parser returns `Incomplete`, we should accumulate more data in the buffer (example: reading from a socket) and call the parser again
//!
//! If we are running a parser and know it will not return `Err::Incomplete`, we can
//! directly extract the error type from `Err::Error` or `Err::Failure` with the
//! `finish()` method:
//!
//! ```rust,ignore
//! # use nom8::IResult;
//! # use nom8::prelude::*;
//! # let parser = nom8::bytes::take_while1(|c: char| c == ' ');
//! # let input = " ";
//! let parser_result: IResult<_, _, _> = parser(input);
//! let result: Result<_, _> = parser_result.finish();
//! ```
//!
//! If we used a borrowed type as input, like `&[u8]` or `&str`, we might want to
//! convert it to an owned type to transmit it somewhere, with the `to_owned()`
//! method:
//!
//! ```rust,ignore
//! # use nom8::Err;
//! # type Value<'s> = &'s [u8];
//! # let parser = nom8::bytes::take_while1(|c: u8| c == b' ');
//! # let data = " ";
//! let result: Result<(&[u8], Value<'_>), Err<Vec<u8>>> =
//!   parser(data).map_err(|e: E<&[u8]>| e.to_owned());
//! ```
//!
//! nom provides a powerful error system that can adapt to your needs: you can
//! get reduced error information if you want to improve performance, or you can
//! get a precise trace of parser application, with fine grained position information.
//!
//! This is done through the third type parameter of `IResult`, nom's parser result
//! type:
//!
//! ```rust
//! pub type IResult<I, O, E=nom8::error::Error<I>> = Result<(I, O), Err<E>>;
//!
//! #[derive(Debug, PartialEq, Eq, Clone, Copy)]
//! pub enum Needed {
//!   Unknown,
//!   Size(u32)
//! }
//!
//! pub enum Err<E> {
//!     Incomplete(Needed),
//!     Error(E),
//!     Failure(E),
//! }
//! ```
//!
//! This error type is completely generic in nom's combinators, so you can choose
//! exactly which error type you want to use when you define your parsers, or
//! directly at the call site.
//! See [the JSON parser](https://github.com/Geal/nom/blob/5405e1173f1052f7e006dcb0b9cfda2b06557b65/examples/json.rs#L209-L286)
//! for an example of choosing different error types at the call site.
//!
//! The `Err<E>` enum expresses 3 conditions for a parser error:
//! - `Incomplete` indicates that a parser did not have enough data to decide. This can be returned by parsers found in `streaming` submodules to indicate that we should buffer more data from a file or socket. Parsers in the `complete` submodules assume that they have the entire input data, so if it was not sufficient, they will instead return a `Err::Error`
//! - `Error` is a normal parser error. If a child parser of the `alt` combinator returns `Error`, it will try another child parser
//! - `Failure` is an error from which we cannot recover: The `alt` combinator will not try other branches if a child parser returns `Failure`. If we know we were in the right branch (example: we found a correct prefix character but input after that was wrong), we can transform a `Err::Error` into a `Err::Failure` with the `cut()` combinator
//!
//! ## Common error types
//!
//! ### the default error type: nom8::error::Error
//!
//! ```rust
//! # use nom8::error::ErrorKind;
//! #[derive(Debug, PartialEq)]
//! pub struct Error<I> {
//!   /// position of the error in the input data
//!   pub input: I,
//!   /// nom error code
//!   pub code: ErrorKind,
//! }
//! ```
//!
//! This structure contains a `nom8::error::ErrorKind` indicating which kind of
//! parser encountered an error (example: `ErrorKind::Tag` for the `tag()`
//! combinator), and the input position of the error.
//!
//! This error type is fast and has very low overhead, so it is suitable for
//! parsers that are called repeatedly, like in network protocols.
//! It is very limited though, it will not tell you about the chain of
//! parser calls, so it is not enough to write user friendly errors.
//!
//! Example error returned in a JSON-like parser (from `examples/json.rs`):
//!
//! ```rust,ignore
//! let data = "  { \"a\"\t: 42,
//! \"b\": [ \"x\", \"y\", 12 ] ,
//! \"c\": { 1\"hello\" : \"world\"
//! }
//! } ";
//!
//! // will print:
//! // Err(
//! //   Failure(
//! //       Error {
//! //           input: "1\"hello\" : \"world\"\n  }\n  } ",
//! //           code: Char,
//! //       },
//! //   ),
//! // )
//! println!(
//!   "{:#?}\n",
//!   json::<Error<&str>>(data)
//! );
//! ```
//!
//! ### getting more information: nom8::error::VerboseError
//!
//! The  `VerboseError<I>` type accumulates more information about the chain of
//! parsers that encountered an error:
//!
//! ```rust
//! # use nom8::error::ErrorKind;
//! #[derive(Clone, Debug, PartialEq)]
//! pub struct VerboseError<I> {
//!   /// List of errors accumulated by `VerboseError`, containing the affected
//!   /// part of input data, and some context
//!   pub errors: Vec<(I, VerboseErrorKind)>,
//! }
//!
//! #[derive(Clone, Debug, PartialEq)]
//! /// Error context for `VerboseError`
//! pub enum VerboseErrorKind {
//!   /// Static string added by the `context` function
//!   Context(&'static str),
//!   /// Indicates which character was expected by the `char` function
//!   Char(char),
//!   /// Error kind given by various nom parsers
//!   Nom(ErrorKind),
//! }
//! ```
//!
//! It contains the input position and error code for each of those parsers.
//! It does not accumulate errors from the different branches of `alt`, it will
//! only contain errors from the last branch it tried.
//!
//! It can be used along with the `nom8::error::context` combinator to inform about
//! the parser chain:
//!
//! ```rust,ignore
//! # use nom8::error::context;
//! # use nom8::sequence::preceded;
//! # use nom8::character::char;
//! # use nom8::combinator::cut;
//! # use nom8::sequence::terminated;
//! # let parse_str = nom8::bytes::take_while1(|c| c == ' ');
//! # let i = " ";
//! context(
//!   "string",
//!   preceded('\"', cut(terminated(parse_str, '\"'))),
//! )(i);
//! ```
//!
//! It is not very usable if printed directly:
//!
//! ```rust,ignore
//! // parsed verbose: Err(
//! //   Failure(
//! //       VerboseError {
//! //           errors: [
//! //               (
//! //                   "1\"hello\" : \"world\"\n  }\n  } ",
//! //                   Char(
//! //                       '}',
//! //                   ),
//! //               ),
//! //               (
//! //                   "{ 1\"hello\" : \"world\"\n  }\n  } ",
//! //                   Context(
//! //                       "map",
//! //                   ),
//! //               ),
//! //               (
//! //                   "{ \"a\"\t: 42,\n  \"b\": [ \"x\", \"y\", 12 ] ,\n  \"c\": { 1\"hello\" : \"world\"\n  }\n  } ",
//! //                   Context(
//! //                       "map",
//! //                   ),
//! //               ),
//! //           ],
//! //       },
//! //   ),
//! // )
//! println!("parsed verbose: {:#?}", json::<VerboseError<&str>>(data));
//! ```
//!
//! But by looking at the original input and the chain of errors, we can build
//! a more user friendly error message. The `nom8::error::convert_error` function
//! can build such a message.
//!
//! ```rust,ignore
//! let e = json::<VerboseError<&str>>(data).finish().err().unwrap();
//! // here we use the `convert_error` function, to transform a `VerboseError<&str>`
//! // into a printable trace.
//! //
//! // This will print:
//! // verbose errors - `json::<VerboseError<&str>>(data)`:
//! // 0: at line 2:
//! //   "c": { 1"hello" : "world"
//! //          ^
//! // expected '}', found 1
//! //
//! // 1: at line 2, in map:
//! //   "c": { 1"hello" : "world"
//! //        ^
//! //
//! // 2: at line 0, in map:
//! //   { "a" : 42,
//! //   ^
//! println!(
//!   "verbose errors - `json::<VerboseError<&str>>(data)`:\n{}",
//!   convert_error(data, e)
//! );
//! ```
//!
//! Note that `VerboseError` and `convert_error` are meant as a starting point for
//! language errors, but that they cannot cover all use cases. So a custom
//! `convert_error` function should probably be written.
//!
//! ### Improving usability: nom_locate and nom-supreme
//!
//! These crates were developed to improve the user experience when writing nom
//! parsers.
//!
//! #### nom_locate
//!
//! [nom_locate](https://docs.rs/nom_locate/) wraps the input data in a `Span`
//! type that can be understood by nom parsers. That type provides location
//! information, like line and column.
//!
//! #### nom-supreme
//!
//! [nom-supreme](https://docs.rs/nom-supreme/) provides the `ErrorTree<I>` error
//! type, that provides the same chain of parser errors as `VerboseError`, but also
//! accumulates errors from the various branches tried by `alt`.
//!
//! With this error type, you can explore everything that has been tried by the
//! parser.
//!
//! ## The `ParseError` trait
//!
//! If those error types are not enough, we can define our own, by implementing
//! the `ParseError<I>` trait. All nom combinators are generic over that trait
//! for their errors, so we only need to define it in the parser result type,
//! and it will be used everywhere.
//!
//! ```rust
//! # use nom8::error::ErrorKind;
//! pub trait ParseError<I>: Sized {
//!     /// Creates an error from the input position and an [ErrorKind]
//!     fn from_error_kind(input: I, kind: ErrorKind) -> Self;
//!
//!     /// Combines an existing error with a new one created from the input
//!     /// position and an [ErrorKind]. This is useful when backtracking
//!     /// through a parse tree, accumulating error context on the way
//!     fn append(input: I, kind: ErrorKind, other: Self) -> Self;
//!
//!     /// Combines two existing errors. This function is used to compare errors
//!     /// generated in various branches of `alt`
//!     fn or(self, other: Self) -> Self {
//!         other
//!     }
//! }
//! ```
//!
//! Any error type has to implement that trait, that requires ways to build an
//! error:
//! - `from_error_kind`: From the input position and the `ErrorKind` enum that indicates in which parser we got an error
//! - `append`: Allows the creation of a chain of errors as we backtrack through the parser tree (various combinators will add more context)
//! - `from_char`: Creates an error that indicates which character we were expecting
//! - `or`: In combinators like `alt`, allows choosing between errors from various branches (or accumulating them)
//!
//! We can also implement the `ContextError` trait to support the `context()`
//! combinator used by `VerboseError<I>`:
//!
//! ```rust
//! pub trait ContextError<I, C>: Sized {
//!     fn add_context(_input: I, _ctx: C, other: Self) -> Self {
//!         other
//!     }
//! }
//! ```
//!
//! And there is also the `FromExternalError<I, E>` used by `map_res` to wrap
//! errors returned by other functions:
//!
//! ```rust
//! # use nom8::error::ErrorKind;
//! pub trait FromExternalError<I, ExternalError> {
//!   fn from_external_error(input: I, kind: ErrorKind, e: ExternalError) -> Self;
//! }
//! ```
//!
//! ### Example usage
//!
//! Let's define a debugging error type, that will print something every time an
//! error is generated. This will give us a good insight into what the parser tried.
//! Since errors can be combined with each other, we want it to keep some info on
//! the error that was just returned. We'll just store that in a string:
//!
//! ```rust
//! struct DebugError {
//!     message: String,
//! }
//! ```
//!
//! Now let's implement `ParseError` and `ContextError` on it:
//!
//! ```rust
//! # use nom8::error::ParseError;
//! # use nom8::error::ErrorKind;
//! # use nom8::error::ContextError;
//! # struct DebugError {
//! #     message: String,
//! # }
//! impl ParseError<&str> for DebugError {
//!     // on one line, we show the error code and the input that caused it
//!     fn from_error_kind(input: &str, kind: ErrorKind) -> Self {
//!         let message = format!("{:?}:\t{:?}\n", kind, input);
//!         println!("{}", message);
//!         DebugError { message }
//!     }
//!
//!     // if combining multiple errors, we show them one after the other
//!     fn append(input: &str, kind: ErrorKind, other: Self) -> Self {
//!         let message = format!("{}{:?}:\t{:?}\n", other.message, kind, input);
//!         println!("{}", message);
//!         DebugError { message }
//!     }
//!
//!     fn or(self, other: Self) -> Self {
//!         let message = format!("{}\tOR\n{}\n", self.message, other.message);
//!         println!("{}", message);
//!         DebugError { message }
//!     }
//! }
//!
//! impl ContextError<&str, &'static str> for DebugError {
//!     fn add_context(input: &str, ctx: &'static str, other: Self) -> Self {
//!         let message = format!("{}\"{}\":\t{:?}\n", other.message, ctx, input);
//!         println!("{}", message);
//!         DebugError { message }
//!     }
//! }
//! ```
//!
//! So when calling our JSON parser with this error type, we will get a trace
//! of all the times a parser stoppped and backtracked:
//!
//! ```rust,ignore
//! println!("debug: {:#?}", root::<DebugError>(data));
//! ```
//!
//! ```text
//! AlphaNumeric:   "\"\t: 42,\n  \"b\": [ \"x\", \"y\", 12 ] ,\n  \"c\": { 1\"hello\" : \"world\"\n  }\n  } "
//!
//! '{':    "42,\n  \"b\": [ \"x\", \"y\", 12 ] ,\n  \"c\": { 1\"hello\" : \"world\"\n  }\n  } "
//!
//! '{':    "42,\n  \"b\": [ \"x\", \"y\", 12 ] ,\n  \"c\": { 1\"hello\" : \"world\"\n  }\n  } "
//! "map":  "42,\n  \"b\": [ \"x\", \"y\", 12 ] ,\n  \"c\": { 1\"hello\" : \"world\"\n  }\n  } "
//!
//! [..]
//!
//! AlphaNumeric:   "\": { 1\"hello\" : \"world\"\n  }\n  } "
//!
//! '"':    "1\"hello\" : \"world\"\n  }\n  } "
//!
//! '"':    "1\"hello\" : \"world\"\n  }\n  } "
//! "string":       "1\"hello\" : \"world\"\n  }\n  } "
//!
//! '}':    "1\"hello\" : \"world\"\n  }\n  } "
//!
//! '}':    "1\"hello\" : \"world\"\n  }\n  } "
//! "map":  "{ 1\"hello\" : \"world\"\n  }\n  } "
//!
//! '}':    "1\"hello\" : \"world\"\n  }\n  } "
//! "map":  "{ 1\"hello\" : \"world\"\n  }\n  } "
//! "map":  "{ \"a\"\t: 42,\n  \"b\": [ \"x\", \"y\", 12 ] ,\n  \"c\": { 1\"hello\" : \"world\"\n  }\n  } "
//!
//! debug: Err(
//!     Failure(
//!         DebugError {
//!             message: "'}':\t\"1\\\"hello\\\" : \\\"world\\\"\\n  }\\n  } \"\n\"map\":\t\"{ 1\\\"hello\\\" : \\\"world
//! \\"\\n  }\\n  } \"\n\"map\":\t\"{ \\\"a\\\"\\t: 42,\\n  \\\"b\\\": [ \\\"x\\\", \\\"y\\\", 12 ] ,\\n  \\\"c\\\": { 1\
//! \"hello\\\" : \\\"world\\\"\\n  }\\n  } \"\n",
//!         },
//!     ),
//! )
//! ```
//!
//! Here we can see that when parsing `{ 1\"hello\" : \"world\"\n  }\n  }`, after
//! getting past the initial `{`, we tried:
//! - parsing a `"` because we're expecting a key name, and that parser was part of the
//! "string" parser
//! - parsing a `}` because the map might be empty. When this fails, we backtrack,
//! through 2 recursive map parsers:
//!
//! ```text
//! '}':    "1\"hello\" : \"world\"\n  }\n  } "
//! "map":  "{ 1\"hello\" : \"world\"\n  }\n  } "
//! "map":  "{ \"a\"\t: 42,\n  \"b\": [ \"x\", \"y\", 12 ] ,\n  \"c\": { 1\"hello\" : \"world\"\n  }\n  } "
//! ```
//!
//! ## Debugging parsers
//!
//! While you are writing your parsers, you will sometimes need to follow
//! which part of the parser sees which part of the input.
//!
//! To that end, nom provides the `dbg_err` function that will observe
//! a parser's input and output, and print a hexdump of the input if there was an
//! error. Here is what it could return:
//!
#![cfg_attr(feature = "std", doc = "```")]
#![cfg_attr(not(feature = "std"), doc = "```ignore")]
//! use nom8::prelude::*;
//! # use nom8::bytes::tag;
//! fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
//!     tag("abcd").dbg_err("tag").parse(i)
//! }
//!
//! let a = &b"efghijkl"[..];
//!
//! // Will print the following message:
//! // tag: Error(Error(Error { input: [101, 102, 103, 104, 105, 106, 107, 108], code: Tag })) at:
//! // 00000000        65 66 67 68 69 6a 6b 6c         efghijkl
//! f(a);
//! ```
//!
//! You can go further with the [nom-trace crate](https://github.com/rust-bakery/nom-trace)

use crate::lib::std::fmt;
use crate::Parser;

/// This trait must be implemented by the error type of a nom parser.
///
/// There are already implementations of it for `(Input, ErrorKind)`
/// and `VerboseError<Input>`.
///
/// It provides methods to create an error from some combinators,
/// and combine existing errors in combinators like `alt`.
pub trait ParseError<I>: Sized {
  /// Creates an error from the input position and an [ErrorKind]
  fn from_error_kind(input: I, kind: ErrorKind) -> Self;

  /// Combines an existing error with a new one created from the input
  /// position and an [ErrorKind]. This is useful when backtracking
  /// through a parse tree, accumulating error context on the way
  fn append(input: I, kind: ErrorKind, other: Self) -> Self;

  /// Creates an error from an input position and an expected character
  #[deprecated(since = "8.0.0", note = "Replaced with `ContextError`")]
  fn from_char(input: I, _: char) -> Self {
    Self::from_error_kind(input, ErrorKind::Char)
  }

  /// Combines two existing errors. This function is used to compare errors
  /// generated in various branches of `alt`.
  fn or(self, other: Self) -> Self {
    other
  }
}

/// This trait is required by the `context` combinator to add a static string
/// to an existing error
pub trait ContextError<I, C>: Sized {
  /// Creates a new error from an input position, a static string and an existing error.
  /// This is used mainly in the [context] combinator, to add user friendly information
  /// to errors when backtracking through a parse tree
  fn add_context(_input: I, _ctx: C, other: Self) -> Self {
    other
  }
}

/// This trait is required by the `map_res` combinator to integrate
/// error types from external functions, like [std::str::FromStr]
pub trait FromExternalError<I, E> {
  /// Creates a new error from an input position, an [ErrorKind] indicating the
  /// wrapping parser, and an external error
  fn from_external_error(input: I, kind: ErrorKind, e: E) -> Self;
}

/// default error type, only contains the error' location and code
#[derive(Debug, PartialEq)]
pub struct Error<I> {
  /// position of the error in the input data
  pub input: I,
  /// nom error code
  pub code: ErrorKind,
}

impl<I> Error<I> {
  /// creates a new basic error
  pub fn new(input: I, code: ErrorKind) -> Error<I> {
    Error { input, code }
  }
}

impl<I> ParseError<I> for Error<I> {
  fn from_error_kind(input: I, kind: ErrorKind) -> Self {
    Error { input, code: kind }
  }

  fn append(_: I, _: ErrorKind, other: Self) -> Self {
    other
  }
}

impl<I, C> ContextError<I, C> for Error<I> {}

impl<I, E> FromExternalError<I, E> for Error<I> {
  /// Create a new error from an input position and an external error
  fn from_external_error(input: I, kind: ErrorKind, _e: E) -> Self {
    Error { input, code: kind }
  }
}

/// The Display implementation allows the std::error::Error implementation
impl<I: fmt::Display> fmt::Display for Error<I> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "error {:?} at: {}", self.code, self.input)
  }
}

#[cfg(feature = "std")]
impl<I: fmt::Debug + fmt::Display> std::error::Error for Error<I> {}

// for backward compatibility, keep those trait implementations
// for the previously used error type
impl<I> ParseError<I> for (I, ErrorKind) {
  fn from_error_kind(input: I, kind: ErrorKind) -> Self {
    (input, kind)
  }

  fn append(_: I, _: ErrorKind, other: Self) -> Self {
    other
  }
}

impl<I, C> ContextError<I, C> for (I, ErrorKind) {}

impl<I, E> FromExternalError<I, E> for (I, ErrorKind) {
  fn from_external_error(input: I, kind: ErrorKind, _e: E) -> Self {
    (input, kind)
  }
}

impl<I> ParseError<I> for () {
  fn from_error_kind(_: I, _: ErrorKind) -> Self {}

  fn append(_: I, _: ErrorKind, _: Self) -> Self {}
}

impl<I, C> ContextError<I, C> for () {}

impl<I, E> FromExternalError<I, E> for () {
  fn from_external_error(_input: I, _kind: ErrorKind, _e: E) -> Self {}
}

/// Creates an error from the input position and an [ErrorKind]
#[deprecated(since = "8.0.0", note = "Replaced with `ParseError::from_error_kind`")]
pub fn make_error<I, E: ParseError<I>>(input: I, kind: ErrorKind) -> E {
  E::from_error_kind(input, kind)
}

/// Combines an existing error with a new one created from the input
/// position and an [ErrorKind]. This is useful when backtracking
/// through a parse tree, accumulating error context on the way
#[deprecated(since = "8.0.0", note = "Replaced with `ParseError::append`")]
pub fn append_error<I, E: ParseError<I>>(input: I, kind: ErrorKind, other: E) -> E {
  E::append(input, kind, other)
}

/// This error type accumulates errors and their position when backtracking
/// through a parse tree. With some post processing (cf `examples/json.rs`),
/// it can be used to display user friendly error messages
#[cfg(feature = "alloc")]
#[derive(Clone, Debug, PartialEq)]
pub struct VerboseError<I> {
  /// List of errors accumulated by `VerboseError`, containing the affected
  /// part of input data, and some context
  pub errors: crate::lib::std::vec::Vec<(I, VerboseErrorKind)>,
}

#[cfg(feature = "alloc")]
#[derive(Clone, Debug, PartialEq)]
/// Error context for `VerboseError`
pub enum VerboseErrorKind {
  /// Static string added by the `context` function
  Context(&'static str),
  /// Error kind given by various nom parsers
  Nom(ErrorKind),
}

#[cfg(feature = "alloc")]
impl<I> ParseError<I> for VerboseError<I> {
  fn from_error_kind(input: I, kind: ErrorKind) -> Self {
    VerboseError {
      errors: vec![(input, VerboseErrorKind::Nom(kind))],
    }
  }

  fn append(input: I, kind: ErrorKind, mut other: Self) -> Self {
    other.errors.push((input, VerboseErrorKind::Nom(kind)));
    other
  }
}

#[cfg(feature = "alloc")]
impl<I> ContextError<I, &'static str> for VerboseError<I> {
  fn add_context(input: I, ctx: &'static str, mut other: Self) -> Self {
    other.errors.push((input, VerboseErrorKind::Context(ctx)));
    other
  }
}

#[cfg(feature = "alloc")]
impl<I, E> FromExternalError<I, E> for VerboseError<I> {
  /// Create a new error from an input position and an external error
  fn from_external_error(input: I, kind: ErrorKind, _e: E) -> Self {
    Self::from_error_kind(input, kind)
  }
}

#[cfg(feature = "alloc")]
impl<I: fmt::Display> fmt::Display for VerboseError<I> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "Parse error:")?;
    for (input, error) in &self.errors {
      match error {
        VerboseErrorKind::Nom(e) => writeln!(f, "{:?} at: {}", e, input)?,
        VerboseErrorKind::Context(s) => writeln!(f, "in section '{}', at: {}", s, input)?,
      }
    }

    Ok(())
  }
}

#[cfg(feature = "std")]
impl<I: fmt::Debug + fmt::Display> std::error::Error for VerboseError<I> {}

use crate::{Err, IResult};

/// Create a new error from an input position, a static string and an existing error.
/// This is used mainly in the [context] combinator, to add user friendly information
/// to errors when backtracking through a parse tree
///
/// **WARNING:** Deprecated, replaced with [`Parser::context`]
#[deprecated(since = "8.0.0", note = "Replaced with `Parser::context")]
pub fn context<I: Clone, E: ContextError<I, &'static str>, F, O>(
  context: &'static str,
  mut f: F,
) -> impl FnMut(I) -> IResult<I, O, E>
where
  F: Parser<I, O, E>,
{
  move |i: I| match f.parse(i.clone()) {
    Ok(o) => Ok(o),
    Err(Err::Incomplete(i)) => Err(Err::Incomplete(i)),
    Err(Err::Error(e)) => Err(Err::Error(E::add_context(i, context, e))),
    Err(Err::Failure(e)) => Err(Err::Failure(E::add_context(i, context, e))),
  }
}

/// Implementation of [`Parser::context`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub struct Context<F, O, C: Clone> {
  f: F,
  context: C,
  phantom: core::marker::PhantomData<O>,
}

impl<F, O, C: Clone> Context<F, O, C> {
  pub(crate) fn new(f: F, context: C) -> Self {
    Self {
      f,
      context,
      phantom: Default::default(),
    }
  }
}

impl<I, O, E, F: Parser<I, O, E>, C> Parser<I, O, E> for Context<F, O, C>
where
  I: Clone,
  C: Clone,
  E: ContextError<I, C>,
  F: Parser<I, O, E>,
{
  fn parse(&mut self, i: I) -> IResult<I, O, E> {
    match (self.f).parse(i.clone()) {
      Ok(o) => Ok(o),
      Err(Err::Incomplete(i)) => Err(Err::Incomplete(i)),
      Err(Err::Error(e)) => Err(Err::Error(E::add_context(i, self.context.clone(), e))),
      Err(Err::Failure(e)) => Err(Err::Failure(E::add_context(i, self.context.clone(), e))),
    }
  }
}

/// Transforms a `VerboseError` into a trace with input position information
#[cfg(feature = "alloc")]
pub fn convert_error<I: core::ops::Deref<Target = str>>(
  input: I,
  e: VerboseError<I>,
) -> crate::lib::std::string::String {
  use crate::input::Offset;
  use crate::lib::std::fmt::Write;

  let mut result = crate::lib::std::string::String::new();

  for (i, (substring, kind)) in e.errors.iter().enumerate() {
    let offset = input.offset(substring);

    if input.is_empty() {
      match kind {
        VerboseErrorKind::Context(s) => write!(&mut result, "{}: in {}, got empty input\n\n", i, s),
        VerboseErrorKind::Nom(e) => write!(&mut result, "{}: in {:?}, got empty input\n\n", i, e),
      }
    } else {
      let prefix = &input.as_bytes()[..offset];

      // Count the number of newlines in the first `offset` bytes of input
      let line_number = prefix.iter().filter(|&&b| b == b'\n').count() + 1;

      // Find the line that includes the subslice:
      // Find the *last* newline before the substring starts
      let line_begin = prefix
        .iter()
        .rev()
        .position(|&b| b == b'\n')
        .map(|pos| offset - pos)
        .unwrap_or(0);

      // Find the full line after that newline
      let line = input[line_begin..]
        .lines()
        .next()
        .unwrap_or(&input[line_begin..])
        .trim_end();

      // The (1-indexed) column number is the offset of our substring into that line
      let column_number = line.offset(substring) + 1;

      match kind {
        VerboseErrorKind::Context(s) => write!(
          &mut result,
          "{i}: at line {line_number}, in {context}:\n\
             {line}\n\
             {caret:>column$}\n\n",
          i = i,
          line_number = line_number,
          context = s,
          line = line,
          caret = '^',
          column = column_number,
        ),
        VerboseErrorKind::Nom(e) => write!(
          &mut result,
          "{i}: at line {line_number}, in {nom_err:?}:\n\
             {line}\n\
             {caret:>column$}\n\n",
          i = i,
          line_number = line_number,
          nom_err = e,
          line = line,
          caret = '^',
          column = column_number,
        ),
      }
    }
    // Because `write!` to a `String` is infallible, this `unwrap` is fine.
    .unwrap();
  }

  result
}

/// Indicates which parser returned an error
#[rustfmt::skip]
#[derive(Debug,PartialEq,Eq,Hash,Clone,Copy)]
#[allow(deprecated,missing_docs)]
pub enum ErrorKind {
  Tag,
  MapRes,
  MapOpt,
  Alt,
  IsNot,
  IsA,
  SeparatedList,
  SeparatedNonEmptyList,
  Many0,
  Many1,
  ManyTill,
  Count,
  TakeUntil,
  LengthValue,
  TagClosure,
  Alpha,
  Digit,
  HexDigit,
  OctDigit,
  AlphaNumeric,
  Space,
  MultiSpace,
  LengthValueFn,
  Eof,
  Switch,
  TagBits,
  OneOf,
  NoneOf,
  Char,
  CrLf,
  RegexpMatch,
  RegexpMatches,
  RegexpFind,
  RegexpCapture,
  RegexpCaptures,
  TakeWhile1,
  Complete,
  Fix,
  Escaped,
  EscapedTransform,
  NonEmpty,
  ManyMN,
  Not,
  Permutation,
  Verify,
  TakeTill1,
  TakeWhileMN,
  TooLarge,
  Many0Count,
  Many1Count,
  Float,
  Satisfy,
  Fail,
}

impl ErrorKind {
  #[rustfmt::skip]
  #[allow(deprecated)]
  /// Converts an ErrorKind to a text description
  pub fn description(&self) -> &str {
    match *self {
      ErrorKind::Tag                       => "Tag",
      ErrorKind::MapRes                    => "Map on Result",
      ErrorKind::MapOpt                    => "Map on Option",
      ErrorKind::Alt                       => "Alternative",
      ErrorKind::IsNot                     => "IsNot",
      ErrorKind::IsA                       => "IsA",
      ErrorKind::SeparatedList             => "Separated list",
      ErrorKind::SeparatedNonEmptyList     => "Separated non empty list",
      ErrorKind::Many0                     => "Many0",
      ErrorKind::Many1                     => "Many1",
      ErrorKind::Count                     => "Count",
      ErrorKind::TakeUntil                 => "Take until",
      ErrorKind::LengthValue               => "Length followed by value",
      ErrorKind::TagClosure                => "Tag closure",
      ErrorKind::Alpha                     => "Alphabetic",
      ErrorKind::Digit                     => "Digit",
      ErrorKind::AlphaNumeric              => "AlphaNumeric",
      ErrorKind::Space                     => "Space",
      ErrorKind::MultiSpace                => "Multiple spaces",
      ErrorKind::LengthValueFn             => "LengthValueFn",
      ErrorKind::Eof                       => "End of file",
      ErrorKind::Switch                    => "Switch",
      ErrorKind::TagBits                   => "Tag on bitstream",
      ErrorKind::OneOf                     => "OneOf",
      ErrorKind::NoneOf                    => "NoneOf",
      ErrorKind::Char                      => "Char",
      ErrorKind::CrLf                      => "CrLf",
      ErrorKind::RegexpMatch               => "RegexpMatch",
      ErrorKind::RegexpMatches             => "RegexpMatches",
      ErrorKind::RegexpFind                => "RegexpFind",
      ErrorKind::RegexpCapture             => "RegexpCapture",
      ErrorKind::RegexpCaptures            => "RegexpCaptures",
      ErrorKind::TakeWhile1                => "TakeWhile1",
      ErrorKind::Complete                  => "Complete",
      ErrorKind::Fix                       => "Fix",
      ErrorKind::Escaped                   => "Escaped",
      ErrorKind::EscapedTransform          => "EscapedTransform",
      ErrorKind::NonEmpty                  => "NonEmpty",
      ErrorKind::ManyMN                    => "Many(m, n)",
      ErrorKind::HexDigit                  => "Hexadecimal Digit",
      ErrorKind::OctDigit                  => "Octal digit",
      ErrorKind::Not                       => "Negation",
      ErrorKind::Permutation               => "Permutation",
      ErrorKind::ManyTill                  => "ManyTill",
      ErrorKind::Verify                    => "predicate verification",
      ErrorKind::TakeTill1                 => "TakeTill1",
      ErrorKind::TakeWhileMN               => "TakeWhileMN",
      ErrorKind::TooLarge                  => "Needed data size is too large",
      ErrorKind::Many0Count                => "Count occurrence of >=0 patterns",
      ErrorKind::Many1Count                => "Count occurrence of >=1 patterns",
      ErrorKind::Float                     => "Float",
      ErrorKind::Satisfy                   => "Satisfy",
      ErrorKind::Fail                      => "Fail",
    }
  }
}

/// Creates a parse error from a [`ErrorKind`]
/// and the position in the input
#[allow(unused_variables)]
#[macro_export(local_inner_macros)]
macro_rules! error_position(
  ($input:expr, $code:expr) => ({
    $crate::error::ParseError::from_error_kind($input, $code)
  });
);

/// Creates a parse error from a [`ErrorKind`],
/// the position in the input and the next error in
/// the parsing tree
#[allow(unused_variables)]
#[macro_export(local_inner_macros)]
macro_rules! error_node_position(
  ($input:expr, $code:expr, $next:expr) => ({
    $crate::error::ParseError::append($input, $code, $next)
  });
);

/// Prints a message and the input if the parser fails.
///
/// The message prints the `Error` or `Incomplete`
/// and the parser's calling code.
///
/// It also displays the input in hexdump format
///
/// **WARNING:** Deprecated, replaced with [`Parser::dbg_err`]
///
/// ```rust
/// use nom8::{IResult, error::dbg_dmp, bytes::tag};
///
/// fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
///   dbg_dmp(tag("abcd"), "tag")(i)
/// }
///
///   let a = &b"efghijkl"[..];
///
/// // Will print the following message:
/// // Error(Position(0, [101, 102, 103, 104, 105, 106, 107, 108])) at l.5 by ' tag ! ( "abcd" ) '
/// // 00000000        65 66 67 68 69 6a 6b 6c         efghijkl
/// f(a);
/// ```
#[deprecated(since = "8.0.0", note = "Replaced with `Parser::dbg_err")]
#[cfg(feature = "std")]
pub fn dbg_dmp<'a, F, O, E: std::fmt::Debug>(
  f: F,
  context: &'static str,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], O, E>
where
  F: Fn(&'a [u8]) -> IResult<&'a [u8], O, E>,
{
  use crate::input::HexDisplay;
  move |i: &'a [u8]| match f(i) {
    Err(e) => {
      println!("{}: Error({:?}) at:\n{}", context, e, i.to_hex(8));
      Err(e)
    }
    a => a,
  }
}

/// Implementation of [`Parser::dbg_err`]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
#[cfg(feature = "std")]
pub struct DbgErr<F, O, C> {
  f: F,
  context: C,
  phantom: core::marker::PhantomData<O>,
}

#[cfg(feature = "std")]
impl<F, O, C> DbgErr<F, O, C> {
  pub(crate) fn new(f: F, context: C) -> Self {
    Self {
      f,
      context,
      phantom: Default::default(),
    }
  }
}

#[cfg(feature = "std")]
impl<I, O, E, F: Parser<I, O, E>, C> Parser<I, O, E> for DbgErr<F, O, C>
where
  I: crate::input::AsBytes,
  I: Clone,
  E: std::fmt::Debug,
  F: Parser<I, O, E>,
  C: std::fmt::Display,
{
  fn parse(&mut self, input: I) -> IResult<I, O, E> {
    use crate::input::HexDisplay;
    let i = input.clone();
    match self.f.parse(i) {
      Err(e) => {
        let input = input.as_bytes();
        eprintln!("{}: Error({:?}) at:\n{}", self.context, e, input.to_hex(8));
        Err(e)
      }
      a => a,
    }
  }
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
  use super::*;
  use crate::bytes::one_of;

  #[test]
  fn convert_error_panic() {
    let input = "";

    let _result: IResult<_, _, VerboseError<&str>> = one_of('x')(input);
  }
}
