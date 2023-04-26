//! # Error management
//!
//! Errors are designed with multiple needs in mind:
//! - Accumulate more [context][Parser::context] as the error goes up the parser chain
//! - Distinguish between [recoverable errors,
//!   unrecoverable errors, and more data is needed][ErrMode]
//! - Have a very low overhead, as errors are often discarded by the calling parser (examples: `many0`, `alt`)
//! - Can be modified according to the user's needs, because some languages need a lot more information
//! - Help thread-through the [stream][crate::stream]
//!
//! To abstract these needs away from the user, generally `winnow` parsers use the [`IResult`]
//! alias, rather than [`Result`][std::result::Result].  [`finish`][FinishIResult::finish] is
//! provided for top-level parsers to integrate with your application's error reporting.
//!
//! Error types include:
//! - `()`
//! - [`Error`]
//! - [`VerboseError`]
//! - [Custom errors][crate::_topic::error]

#[cfg(feature = "alloc")]
use crate::lib::std::borrow::ToOwned;
use crate::lib::std::fmt;
use core::num::NonZeroUsize;

use crate::stream::Stream;
use crate::stream::StreamIsPartial;
#[allow(unused_imports)] // Here for intra-doc links
use crate::Parser;

/// Holds the result of [`Parser`]
///
/// - `Ok((I, O))` is the remaining [input][crate::stream] and the parsed value
/// - [`Err(ErrMode<E>)`][ErrMode] is the error along with how to respond to it
///
/// By default, the error type (`E`) is [`Error`]
///
/// At the top-level of your parser, you can use the [`FinishIResult::finish`] method to convert
/// it to a more common result type
pub type IResult<I, O, E = Error<I>> = Result<(I, O), ErrMode<E>>;

/// Extension trait to convert a parser's [`IResult`] to a more manageable type
#[deprecated(since = "0.4.0", note = "Replaced with `Parser::parse`")]
pub trait FinishIResult<I, O, E> {
    /// Converts the parser's [`IResult`] to a type that is more consumable by callers.
    ///
    /// Errors if the parser is not at the [end of input][crate::combinator::eof].  See
    /// [`FinishIResult::finish_err`] if the remaining input is needed.
    ///
    /// # Panic
    ///
    /// If the result is `Err(ErrMode::Incomplete(_))`, this method will panic.
    /// - **Complete parsers:** It will not be an issue, `Incomplete` is never used
    /// - **Partial parsers:** `Incomplete` will be returned if there's not enough data
    /// for the parser to decide, and you should gather more data before parsing again.
    /// Once the parser returns either `Ok(_)`, `Err(ErrMode::Backtrack(_))` or `Err(ErrMode::Cut(_))`,
    /// you can get out of the parsing loop and call `finish_err()` on the parser's result
    ///
    /// # Example
    ///
    #[cfg_attr(not(feature = "std"), doc = "```ignore")]
    #[cfg_attr(feature = "std", doc = "```")]
    /// use winnow::prelude::*;
    /// use winnow::character::hex_uint;
    /// use winnow::error::Error;
    ///
    /// struct Hex(u64);
    ///
    /// fn parse(value: &str) -> Result<Hex, Error<String>> {
    ///     hex_uint.map(Hex).parse_next(value).finish().map_err(Error::into_owned)
    /// }
    /// ```
    #[deprecated(since = "0.4.0", note = "Replaced with `Parser::parse`")]
    fn finish(self) -> Result<O, E>;

    /// Converts the parser's [`IResult`] to a type that is more consumable by errors.
    ///
    ///  It keeps the same `Ok` branch, and merges `ErrMode::Backtrack` and `ErrMode::Cut` into the `Err`
    ///  side.
    ///
    /// # Panic
    ///
    /// If the result is `Err(ErrMode::Incomplete(_))`, this method will panic as [`ErrMode::Incomplete`]
    /// should only be set when the input is [`StreamIsPartial<false>`] which this isn't implemented
    /// for.
    #[deprecated(since = "0.4.0", note = "Replaced with `Parser::parse`")]
    fn finish_err(self) -> Result<(I, O), E>;
}

#[allow(deprecated)]
impl<I, O, E> FinishIResult<I, O, E> for IResult<I, O, E>
where
    I: Stream,
    // Force users to deal with `Incomplete` when `StreamIsPartial<true>`
    I: StreamIsPartial,
    I: Clone,
    E: ParseError<I>,
{
    fn finish(self) -> Result<O, E> {
        debug_assert!(
            !I::is_partial_supported(),
            "partial streams need to handle `ErrMode::Incomplete`"
        );

        let (i, o) = self.finish_err()?;
        crate::combinator::eof(i).finish_err()?;
        Ok(o)
    }

    fn finish_err(self) -> Result<(I, O), E> {
        debug_assert!(
            !I::is_partial_supported(),
            "partial streams need to handle `ErrMode::Incomplete`"
        );

        match self {
            Ok(res) => Ok(res),
            Err(ErrMode::Backtrack(e)) | Err(ErrMode::Cut(e)) => Err(e),
            Err(ErrMode::Incomplete(_)) => {
                panic!("complete parsers should not report `Err(ErrMode::Incomplete(_))`")
            }
        }
    }
}

/// Contains information on needed data if a parser returned `Incomplete`
///
/// **Note:** This is only possible for `Stream` that are [partial][`StreamIsPartial`],
/// like [`Partial`][crate::Partial].
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub enum Needed {
    /// Needs more data, but we do not know how much
    Unknown,
    /// Contains the required data size in bytes
    Size(NonZeroUsize),
}

impl Needed {
    /// Creates `Needed` instance, returns `Needed::Unknown` if the argument is zero
    pub fn new(s: usize) -> Self {
        match NonZeroUsize::new(s) {
            Some(sz) => Needed::Size(sz),
            None => Needed::Unknown,
        }
    }

    /// Indicates if we know how many bytes we need
    pub fn is_known(&self) -> bool {
        *self != Needed::Unknown
    }

    /// Maps a `Needed` to `Needed` by applying a function to a contained `Size` value.
    #[inline]
    pub fn map<F: Fn(NonZeroUsize) -> usize>(self, f: F) -> Needed {
        match self {
            Needed::Unknown => Needed::Unknown,
            Needed::Size(n) => Needed::new(f(n)),
        }
    }
}

/// The `Err` enum indicates the parser was not successful
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(nightly, warn(rustdoc::missing_doc_code_examples))]
pub enum ErrMode<E> {
    /// There was not enough data to determine the appropriate action
    ///
    /// More data needs to be buffered before retrying the parse.
    ///
    /// This must only be set when the [`Stream`] is [partial][`StreamIsPartial`], like with
    /// [`Partial`][crate::Partial]
    ///
    /// Convert this into an `Backtrack` with [`Parser::complete_err`]
    Incomplete(Needed),
    /// The parser failed with a recoverable error (the default).
    ///
    /// For example, a parser for json values might include a
    /// [`dec_uint`][crate::character::dec_uint] as one case in an [`alt`][crate::branch::alt]
    /// combiantor.  If it fails, the next case should be tried.
    Backtrack(E),
    /// The parser had an unrecoverable error.
    ///
    /// The parser was on the right branch, so directly report it to the user rather than trying
    /// other branches. You can use [`cut_err()`][crate::combinator::cut_err] combinator to switch
    /// from `ErrMode::Backtrack` to `ErrMode::Cut`.
    ///
    /// For example, one case in an [`alt`][crate::branch::alt] combinator found a unique prefix
    /// and you want any further errors parsing the case to be reported to the user.
    Cut(E),
}

impl<E> ErrMode<E> {
    /// Tests if the result is Incomplete
    pub fn is_incomplete(&self) -> bool {
        matches!(self, ErrMode::Incomplete(_))
    }

    /// Prevent backtracking, bubbling the error up to the top
    pub fn cut(self) -> Self {
        match self {
            ErrMode::Backtrack(e) => ErrMode::Cut(e),
            rest => rest,
        }
    }

    /// Enable backtracking support
    pub fn backtrack(self) -> Self {
        match self {
            ErrMode::Cut(e) => ErrMode::Backtrack(e),
            rest => rest,
        }
    }

    /// Applies the given function to the inner error
    pub fn map<E2, F>(self, f: F) -> ErrMode<E2>
    where
        F: FnOnce(E) -> E2,
    {
        match self {
            ErrMode::Incomplete(n) => ErrMode::Incomplete(n),
            ErrMode::Cut(t) => ErrMode::Cut(f(t)),
            ErrMode::Backtrack(t) => ErrMode::Backtrack(f(t)),
        }
    }

    /// Automatically converts between errors if the underlying type supports it
    pub fn convert<F>(self) -> ErrMode<F>
    where
        E: ErrorConvert<F>,
    {
        self.map(ErrorConvert::convert)
    }
}

impl<I, E: ParseError<I>> ParseError<I> for ErrMode<E> {
    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        ErrMode::Backtrack(E::from_error_kind(input, kind))
    }

    #[cfg_attr(debug_assertions, track_caller)]
    fn assert(input: I, message: &'static str) -> Self
    where
        I: crate::lib::std::fmt::Debug,
    {
        ErrMode::Backtrack(E::assert(input, message))
    }

    fn append(self, input: I, kind: ErrorKind) -> Self {
        match self {
            ErrMode::Backtrack(e) => ErrMode::Backtrack(e.append(input, kind)),
            e => e,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (ErrMode::Backtrack(e), ErrMode::Backtrack(o)) => ErrMode::Backtrack(e.or(o)),
            (ErrMode::Incomplete(e), _) | (_, ErrMode::Incomplete(e)) => ErrMode::Incomplete(e),
            (ErrMode::Cut(e), _) | (_, ErrMode::Cut(e)) => ErrMode::Cut(e),
        }
    }
}

impl<I, EXT, E> FromExternalError<I, EXT> for ErrMode<E>
where
    E: FromExternalError<I, EXT>,
{
    fn from_external_error(input: I, kind: ErrorKind, e: EXT) -> Self {
        ErrMode::Backtrack(E::from_external_error(input, kind, e))
    }
}

impl<T> ErrMode<Error<T>> {
    /// Maps `ErrMode<Error<T>>` to `ErrMode<Error<U>>` with the given `F: T -> U`
    pub fn map_input<U, F>(self, f: F) -> ErrMode<Error<U>>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            ErrMode::Incomplete(n) => ErrMode::Incomplete(n),
            ErrMode::Cut(Error { input, kind }) => ErrMode::Cut(Error {
                input: f(input),
                kind,
            }),
            ErrMode::Backtrack(Error { input, kind }) => ErrMode::Backtrack(Error {
                input: f(input),
                kind,
            }),
        }
    }
}

impl<E: Eq> Eq for ErrMode<E> {}

impl<E> fmt::Display for ErrMode<E>
where
    E: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrMode::Incomplete(Needed::Size(u)) => write!(f, "Parsing requires {} bytes/chars", u),
            ErrMode::Incomplete(Needed::Unknown) => write!(f, "Parsing requires more data"),
            ErrMode::Cut(c) => write!(f, "Parsing Failure: {:?}", c),
            ErrMode::Backtrack(c) => write!(f, "Parsing Error: {:?}", c),
        }
    }
}

/// The basic [`Parser`] trait for errors
///
/// It provides methods to create an error from some combinators,
/// and combine existing errors in combinators like `alt`.
pub trait ParseError<I>: Sized {
    /// Creates an error from the input position and an [`ErrorKind`]
    fn from_error_kind(input: I, kind: ErrorKind) -> Self;

    /// Process a parser assertion
    #[cfg_attr(debug_assertions, track_caller)]
    fn assert(input: I, _message: &'static str) -> Self
    where
        I: crate::lib::std::fmt::Debug,
    {
        #[cfg(debug_assertions)]
        panic!("assert `{}` failed at {:#?}", _message, input);
        #[cfg(not(debug_assertions))]
        Self::from_error_kind(input, ErrorKind::Assert)
    }

    /// Like [`ParseError::from_error_kind`] but merges it with the existing error.
    ///
    /// This is useful when backtracking through a parse tree, accumulating error context on the
    /// way.
    fn append(self, input: I, kind: ErrorKind) -> Self;

    /// Combines errors from two different parse branches.
    ///
    /// For example, this would be used by [`alt`][crate::branch::alt] to report the error from
    /// each case.
    fn or(self, other: Self) -> Self {
        other
    }
}

/// Used by [`Parser::context`] to add custom data to error while backtracking
///
/// May be implemented multiple times for different kinds of context.
pub trait ContextError<I, C = &'static str>: Sized {
    /// Append to an existing error custom data
    ///
    /// This is used mainly by [`Parser::context`], to add user friendly information
    /// to errors when backtracking through a parse tree
    fn add_context(self, _input: I, _ctx: C) -> Self {
        self
    }
}

/// Create a new error with an external error, from [`std::str::FromStr`]
///
/// This trait is required by the [`Parser::map_res`] combinator.
pub trait FromExternalError<I, E> {
    /// Like [`ParseError::from_error_kind`] but also include an external error.
    fn from_external_error(input: I, kind: ErrorKind, e: E) -> Self;
}

/// Equivalent of `From` implementation to avoid orphan rules in bits parsers
pub trait ErrorConvert<E> {
    /// Transform to another error type
    fn convert(self) -> E;
}

/// Default error type, only contains the error' location and kind
///
/// This is a low-overhead error that only provides basic information.  For less overhead, see
/// `()`.  Fore more information, see [`VerboseError`].
///:w
/// **Note:** [context][Parser::context] and inner errors (like from [`Parser::map_res`]) will be
/// dropped.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Error<I> {
    /// The input stream, pointing to the location where the error occurred
    pub input: I,
    /// A rudimentary error kind
    pub kind: ErrorKind,
}

impl<I> Error<I> {
    /// Creates a new basic error
    pub fn new(input: I, kind: ErrorKind) -> Error<I> {
        Error { input, kind }
    }
}

#[cfg(feature = "alloc")]
impl<'i, I: ToOwned + ?Sized> Error<&'i I> {
    /// Obtaining ownership
    pub fn into_owned(self) -> Error<<I as ToOwned>::Owned> {
        Error {
            input: self.input.to_owned(),
            kind: self.kind,
        }
    }
}

impl<I> ParseError<I> for Error<I> {
    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        Error { input, kind }
    }

    fn append(self, _: I, _: ErrorKind) -> Self {
        self
    }
}

impl<I, C> ContextError<I, C> for Error<I> {}

impl<I, E> FromExternalError<I, E> for Error<I> {
    /// Create a new error from an input position and an external error
    fn from_external_error(input: I, kind: ErrorKind, _e: E) -> Self {
        Error { input, kind }
    }
}

impl<I> ErrorConvert<Error<(I, usize)>> for Error<I> {
    fn convert(self) -> Error<(I, usize)> {
        Error {
            input: (self.input, 0),
            kind: self.kind,
        }
    }
}

impl<I> ErrorConvert<Error<I>> for Error<(I, usize)> {
    fn convert(self) -> Error<I> {
        Error {
            input: self.input.0,
            kind: self.kind,
        }
    }
}

/// The Display implementation allows the `std::error::Error` implementation
impl<I: fmt::Display> fmt::Display for Error<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error {:?} at: {}", self.kind, self.input)
    }
}

#[cfg(feature = "std")]
impl<I: fmt::Debug + fmt::Display + Sync + Send + 'static> std::error::Error for Error<I> {}

impl<I> ParseError<I> for () {
    fn from_error_kind(_: I, _: ErrorKind) -> Self {}

    fn append(self, _: I, _: ErrorKind) -> Self {}
}

impl<I, C> ContextError<I, C> for () {}

impl<I, E> FromExternalError<I, E> for () {
    fn from_external_error(_input: I, _kind: ErrorKind, _e: E) -> Self {}
}

impl ErrorConvert<()> for () {
    fn convert(self) {}
}

/// Accumulates error information while backtracking
///
/// For less overhead (and information), see [`Error`].
///
/// [`convert_error`] provides an example of how to render this for end-users.
///
/// **Note:** This will only capture the last failed branch for combinators like
/// [`alt`][crate::branch::alt].
#[cfg(feature = "alloc")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerboseError<I> {
    /// Accumulated error information
    pub errors: crate::lib::std::vec::Vec<(I, VerboseErrorKind)>,
}

#[cfg(feature = "alloc")]
impl<'i, I: ToOwned + ?Sized> VerboseError<&'i I> {
    /// Obtaining ownership
    pub fn into_owned(self) -> VerboseError<<I as ToOwned>::Owned> {
        #[allow(clippy::redundant_clone)] // false positive
        VerboseError {
            errors: self
                .errors
                .into_iter()
                .map(|(i, k)| (i.to_owned(), k))
                .collect(),
        }
    }
}

#[cfg(feature = "alloc")]
#[derive(Clone, Debug, Eq, PartialEq)]
/// Error context for `VerboseError`
pub enum VerboseErrorKind {
    /// Static string added by the `context` function
    Context(&'static str),
    /// Error kind given by various parsers
    Winnow(ErrorKind),
}

#[cfg(feature = "alloc")]
impl<I> ParseError<I> for VerboseError<I> {
    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        VerboseError {
            errors: vec![(input, VerboseErrorKind::Winnow(kind))],
        }
    }

    fn append(mut self, input: I, kind: ErrorKind) -> Self {
        self.errors.push((input, VerboseErrorKind::Winnow(kind)));
        self
    }
}

#[cfg(feature = "alloc")]
impl<I> ContextError<I, &'static str> for VerboseError<I> {
    fn add_context(mut self, input: I, ctx: &'static str) -> Self {
        self.errors.push((input, VerboseErrorKind::Context(ctx)));
        self
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
impl<I> ErrorConvert<VerboseError<I>> for VerboseError<(I, usize)> {
    fn convert(self) -> VerboseError<I> {
        VerboseError {
            errors: self.errors.into_iter().map(|(i, e)| (i.0, e)).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl<I> ErrorConvert<VerboseError<(I, usize)>> for VerboseError<I> {
    fn convert(self) -> VerboseError<(I, usize)> {
        VerboseError {
            errors: self.errors.into_iter().map(|(i, e)| ((i, 0), e)).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl<I: fmt::Display> fmt::Display for VerboseError<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Parse error:")?;
        for (input, error) in &self.errors {
            match error {
                VerboseErrorKind::Winnow(e) => writeln!(f, "{:?} at: {}", e, input)?,
                VerboseErrorKind::Context(s) => writeln!(f, "in section '{}', at: {}", s, input)?,
            }
        }

        Ok(())
    }
}

#[cfg(feature = "std")]
impl<I: fmt::Debug + fmt::Display + Sync + Send + 'static> std::error::Error for VerboseError<I> {}

/// Transforms a `VerboseError` into a trace with input position information
#[cfg(feature = "alloc")]
pub fn convert_error<I: core::ops::Deref<Target = str>>(
    input: I,
    e: VerboseError<I>,
) -> crate::lib::std::string::String {
    use crate::lib::std::fmt::Write;
    use crate::stream::Offset;

    let mut result = crate::lib::std::string::String::new();

    for (i, (substring, kind)) in e.errors.iter().enumerate() {
        let offset = input.offset_to(substring);

        if input.is_empty() {
            match kind {
                VerboseErrorKind::Context(s) => {
                    write!(&mut result, "{}: in {}, got empty input\n\n", i, s)
                }
                VerboseErrorKind::Winnow(e) => {
                    write!(&mut result, "{}: in {:?}, got empty input\n\n", i, e)
                }
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
            let column_number = line.offset_to(substring) + 1;

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
                VerboseErrorKind::Winnow(e) => write!(
                    &mut result,
                    "{i}: at line {line_number}, in {kind:?}:\n\
             {line}\n\
             {caret:>column$}\n\n",
                    i = i,
                    line_number = line_number,
                    kind = e,
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

/// Provide some minor debug context for errors
#[rustfmt::skip]
#[derive(Debug,PartialEq,Eq,Hash,Clone,Copy)]
#[allow(missing_docs)]
pub enum ErrorKind {
  Assert,
  Token,
  Tag,
  Alt,
  Many,
  Eof,
  Slice,
  Complete,
  Not,
  Verify,
  Fail,
}

impl ErrorKind {
    #[rustfmt::skip]
    /// Converts an `ErrorKind` to a text description
    pub fn description(&self) -> &str {
    match *self {
      ErrorKind::Assert                    => "assert",
      ErrorKind::Token                     => "token",
      ErrorKind::Tag                       => "tag",
      ErrorKind::Alt                       => "alternative",
      ErrorKind::Many                      => "many",
      ErrorKind::Eof                       => "end of file",
      ErrorKind::Slice                     => "slice",
      ErrorKind::Complete                  => "complete",
      ErrorKind::Not                       => "negation",
      ErrorKind::Verify                    => "predicate verification",
      ErrorKind::Fail                      => "fail",
    }
  }
}

/// Creates a parse error from a [`ErrorKind`]
/// and the position in the input
#[cfg(test)]
macro_rules! error_position(
  ($input:expr, $code:expr) => ({
    $crate::error::ParseError::from_error_kind($input, $code)
  });
);

#[cfg(test)]
macro_rules! error_node_position(
  ($input:expr, $code:expr, $next:expr) => ({
    $crate::error::ParseError::append($next, $input, $code)
  });
);

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::*;

    #[test]
    fn convert_error_panic() {
        let input = "";

        let _result: IResult<_, _, VerboseError<&str>> = 'x'.parse_next(input);
    }
}
