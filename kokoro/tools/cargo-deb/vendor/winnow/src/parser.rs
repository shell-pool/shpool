//! Basic types to build the parsers

use crate::combinator::*;
use crate::error::{ContextError, IResult, ParseError};
use crate::stream::{AsChar, Compare, Location, Stream, StreamIsPartial};

/// Core trait for parsing
///
/// The simplest way to implement a `Parser` is with a function
/// ```rust
/// use winnow::prelude::*;
///
/// fn success(input: &str) -> IResult<&str, ()> {
///     let output = ();
///     Ok((input, output))
/// }
///
/// let (input, output) = success.parse_next("Hello").unwrap();
/// assert_eq!(input, "Hello");  // We didn't consume any input
/// ```
///
/// which can be made stateful by returning a function
/// ```rust
/// use winnow::prelude::*;
///
/// fn success<O: Clone>(output: O) -> impl FnMut(&str) -> IResult<&str, O> {
///     move |input: &str| {
///         let output = output.clone();
///         Ok((input, output))
///     }
/// }
///
/// let (input, output) = success("World").parse_next("Hello").unwrap();
/// assert_eq!(input, "Hello");  // We didn't consume any input
/// assert_eq!(output, "World");
/// ```
///
/// Additionally, some basic types implement `Parser` as well, including
/// - `u8` and `char`, see [`winnow::character::char`][crate::bytes::one_of]
/// - `&[u8]` and `&str`, see [`winnow::character::char`][crate::bytes::tag]
pub trait Parser<I, O, E> {
    /// A parser takes in input type, and returns a `Result` containing
    /// either the remaining input and the output value, or an error
    #[deprecated(since = "0.3.0", note = "Replaced with `Parser::parse_next`")]
    #[cfg_attr(feature = "unstable-doc", doc(hidden))]
    fn parse(&mut self, input: I) -> IResult<I, O, E> {
        self.parse_next(input)
    }

    /// Take tokens from the [`Stream`], turning it into the output
    ///
    /// This includes advancing the [`Stream`] to the next location.
    fn parse_next(&mut self, input: I) -> IResult<I, O, E>;

    /// Treat `&mut Self` as a parser
    ///
    /// This helps when needing to move a `Parser` when all you have is a `&mut Parser`.
    ///
    /// # Example
    ///
    /// Because parsers are `FnMut`, they can be called multiple times.  This prevents moving `f`
    /// into [`length_data`][crate::multi::length_data] and `g` into
    /// [`Parser::complete_err`]:
    /// ```rust,compile_fail
    /// # use winnow::prelude::*;
    /// # use winnow::IResult;
    /// # use winnow::Parser;
    /// # use winnow::error::ParseError;
    /// # use winnow::multi::length_data;
    /// pub fn length_value<'i, O, E: ParseError<&'i [u8]>>(
    ///     mut f: impl Parser<&'i [u8], usize, E>,
    ///     mut g: impl Parser<&'i [u8], O, E>
    /// ) -> impl FnMut(&'i [u8]) -> IResult<&'i [u8], O, E> {
    ///   move |i: &'i [u8]| {
    ///     let (i, data) = length_data(f).parse_next(i)?;
    ///     let (_, o) = g.complete().parse_next(data)?;
    ///     Ok((i, o))
    ///   }
    /// }
    /// ```
    ///
    /// By adding `by_ref`, we can make this work:
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::IResult;
    /// # use winnow::Parser;
    /// # use winnow::error::ParseError;
    /// # use winnow::multi::length_data;
    /// pub fn length_value<'i, O, E: ParseError<&'i [u8]>>(
    ///     mut f: impl Parser<&'i [u8], usize, E>,
    ///     mut g: impl Parser<&'i [u8], O, E>
    /// ) -> impl FnMut(&'i [u8]) -> IResult<&'i [u8], O, E> {
    ///   move |i: &'i [u8]| {
    ///     let (i, data) = length_data(f.by_ref()).parse_next(i)?;
    ///     let (_, o) = g.by_ref().complete_err().parse_next(data)?;
    ///     Ok((i, o))
    ///   }
    /// }
    /// ```
    fn by_ref(&mut self) -> ByRef<'_, Self>
    where
        Self: core::marker::Sized,
    {
        ByRef::new(self)
    }

    /// Produce the provided value
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// use winnow::character::alpha1;
    /// # fn main() {
    ///
    /// let mut parser = alpha1.value(1234);
    ///
    /// assert_eq!(parser.parse_next("abcd"), Ok(("", 1234)));
    /// assert_eq!(parser.parse_next("123abcd;"), Err(ErrMode::Backtrack(Error::new("123abcd;", ErrorKind::Alpha))));
    /// # }
    /// ```
    #[doc(alias = "to")]
    fn value<O2>(self, val: O2) -> Value<Self, O, O2>
    where
        Self: core::marker::Sized,
        O2: Clone,
    {
        Value::new(self, val)
    }

    /// Discards the output of the `Parser`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// use winnow::character::alpha1;
    /// # fn main() {
    ///
    /// let mut parser = alpha1.void();
    ///
    /// assert_eq!(parser.parse_next("abcd"), Ok(("", ())));
    /// assert_eq!(parser.parse_next("123abcd;"), Err(ErrMode::Backtrack(Error::new("123abcd;", ErrorKind::Alpha))));
    /// # }
    /// ```
    fn void(self) -> Void<Self, O>
    where
        Self: core::marker::Sized,
    {
        Void::new(self)
    }

    /// Convert the parser's output to another type using [`std::convert::From`]
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::IResult;
    /// # use winnow::Parser;
    /// use winnow::character::alpha1;
    /// # fn main() {
    ///
    ///  fn parser1(i: &str) -> IResult<&str, &str> {
    ///    alpha1(i)
    ///  }
    ///
    ///  let mut parser2 = parser1.output_into();
    ///
    /// // the parser converts the &str output of the child parser into a Vec<u8>
    /// let bytes: IResult<&str, Vec<u8>> = parser2.parse_next("abcd");
    /// assert_eq!(bytes, Ok(("", vec![97, 98, 99, 100])));
    /// # }
    /// ```
    fn output_into<O2>(self) -> OutputInto<Self, O, O2>
    where
        Self: core::marker::Sized,
        O: Into<O2>,
    {
        OutputInto::new(self)
    }

    /// Produce the consumed input as produced value.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// use winnow::character::{alpha1};
    /// use winnow::sequence::separated_pair;
    /// # fn main() {
    ///
    /// let mut parser = separated_pair(alpha1, ',', alpha1).recognize();
    ///
    /// assert_eq!(parser.parse_next("abcd,efgh"), Ok(("", "abcd,efgh")));
    /// assert_eq!(parser.parse_next("abcd;"),Err(ErrMode::Backtrack(Error::new(";", ErrorKind::OneOf))));
    /// # }
    /// ```
    fn recognize(self) -> Recognize<Self, O>
    where
        Self: core::marker::Sized,
    {
        Recognize::new(self)
    }

    /// Produce the consumed input with the output
    ///
    /// Functions similarly to [recognize][Parser::recognize] except it
    /// returns the parser output as well.
    ///
    /// This can be useful especially in cases where the output is not the same type
    /// as the input, or the input is a user defined type.
    ///
    /// Returned tuple is of the format `(produced output, consumed input)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult};
    /// use winnow::character::{alpha1};
    /// use winnow::bytes::tag;
    /// use winnow::sequence::separated_pair;
    ///
    /// fn inner_parser(input: &str) -> IResult<&str, bool> {
    ///     tag("1234").value(true).parse_next(input)
    /// }
    ///
    /// # fn main() {
    ///
    /// let mut consumed_parser = separated_pair(alpha1, ',', alpha1).value(true).with_recognized();
    ///
    /// assert_eq!(consumed_parser.parse_next("abcd,efgh1"), Ok(("1", (true, "abcd,efgh"))));
    /// assert_eq!(consumed_parser.parse_next("abcd;"),Err(ErrMode::Backtrack(Error::new(";", ErrorKind::OneOf))));
    ///
    /// // the second output (representing the consumed input)
    /// // should be the same as that of the `recognize` parser.
    /// let mut recognize_parser = inner_parser.recognize();
    /// let mut consumed_parser = inner_parser.with_recognized().map(|(output, consumed)| consumed);
    ///
    /// assert_eq!(recognize_parser.parse_next("1234"), consumed_parser.parse_next("1234"));
    /// assert_eq!(recognize_parser.parse_next("abcd"), consumed_parser.parse_next("abcd"));
    /// # }
    /// ```
    #[doc(alias = "consumed")]
    fn with_recognized(self) -> WithRecognized<Self, O>
    where
        Self: core::marker::Sized,
    {
        WithRecognized::new(self)
    }

    /// Produce the location of the consumed input as produced value.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, stream::Stream};
    /// use winnow::stream::Located;
    /// use winnow::character::alpha1;
    /// use winnow::sequence::separated_pair;
    ///
    /// let mut parser = separated_pair(alpha1.span(), ',', alpha1.span());
    ///
    /// assert_eq!(parser.parse_next(Located::new("abcd,efgh")).finish(), Ok((0..4, 5..9)));
    /// assert_eq!(parser.parse_next(Located::new("abcd;")),Err(ErrMode::Backtrack(Error::new(Located::new("abcd;").next_slice(4).0, ErrorKind::OneOf))));
    /// ```
    fn span(self) -> Span<Self, O>
    where
        Self: core::marker::Sized,
        I: Location + Clone,
    {
        Span::new(self)
    }

    /// Produce the location of consumed input with the output
    ///
    /// Functions similarly to [`Parser::span`] except it
    /// returns the parser output as well.
    ///
    /// This can be useful especially in cases where the output is not the same type
    /// as the input, or the input is a user defined type.
    ///
    /// Returned tuple is of the format `(produced output, consumed input)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, stream::Stream};
    /// use winnow::stream::Located;
    /// use winnow::character::alpha1;
    /// use winnow::bytes::tag;
    /// use winnow::sequence::separated_pair;
    ///
    /// fn inner_parser(input: Located<&str>) -> IResult<Located<&str>, bool> {
    ///     tag("1234").value(true).parse_next(input)
    /// }
    ///
    /// # fn main() {
    ///
    /// let mut consumed_parser = separated_pair(alpha1.value(1).with_span(), ',', alpha1.value(2).with_span());
    ///
    /// assert_eq!(consumed_parser.parse_next(Located::new("abcd,efgh")).finish(), Ok(((1, 0..4), (2, 5..9))));
    /// assert_eq!(consumed_parser.parse_next(Located::new("abcd;")),Err(ErrMode::Backtrack(Error::new(Located::new("abcd;").next_slice(4).0, ErrorKind::OneOf))));
    ///
    /// // the second output (representing the consumed input)
    /// // should be the same as that of the `span` parser.
    /// let mut recognize_parser = inner_parser.span();
    /// let mut consumed_parser = inner_parser.with_span().map(|(output, consumed)| consumed);
    ///
    /// assert_eq!(recognize_parser.parse_next(Located::new("1234")), consumed_parser.parse_next(Located::new("1234")));
    /// assert_eq!(recognize_parser.parse_next(Located::new("abcd")), consumed_parser.parse_next(Located::new("abcd")));
    /// # }
    /// ```
    fn with_span(self) -> WithSpan<Self, O>
    where
        Self: core::marker::Sized,
        I: Location + Clone,
    {
        WithSpan::new(self)
    }

    /// Maps a function over the output of a parser
    ///
    /// # Example
    ///
    /// ```rust
    /// use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult,Parser};
    /// use winnow::character::digit1;
    /// # fn main() {
    ///
    /// let mut parser = digit1.map(|s: &str| s.len());
    ///
    /// // the parser will count how many characters were returned by digit1
    /// assert_eq!(parser.parse_next("123456"), Ok(("", 6)));
    ///
    /// // this will fail if digit1 fails
    /// assert_eq!(parser.parse_next("abc"), Err(ErrMode::Backtrack(Error::new("abc", ErrorKind::Digit))));
    /// # }
    /// ```
    fn map<G, O2>(self, g: G) -> Map<Self, G, O>
    where
        G: Fn(O) -> O2,
        Self: core::marker::Sized,
    {
        Map::new(self, g)
    }

    /// Applies a function returning a `Result` over the output of a parser.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// use winnow::character::digit1;
    /// # fn main() {
    ///
    /// let mut parse = digit1.map_res(|s: &str| s.parse::<u8>());
    ///
    /// // the parser will convert the result of digit1 to a number
    /// assert_eq!(parse.parse_next("123"), Ok(("", 123)));
    ///
    /// // this will fail if digit1 fails
    /// assert_eq!(parse.parse_next("abc"), Err(ErrMode::Backtrack(Error::new("abc", ErrorKind::Digit))));
    ///
    /// // this will fail if the mapped function fails (a `u8` is too small to hold `123456`)
    /// assert_eq!(parse.parse_next("123456"), Err(ErrMode::Backtrack(Error::new("123456", ErrorKind::MapRes))));
    /// # }
    /// ```
    fn map_res<G, O2, E2>(self, g: G) -> MapRes<Self, G, O>
    where
        Self: core::marker::Sized,
        G: FnMut(O) -> Result<O2, E2>,
    {
        MapRes::new(self, g)
    }

    /// Apply both [`Parser::verify`] and [`Parser::map`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// use winnow::character::digit1;
    /// # fn main() {
    ///
    /// let mut parse = digit1.verify_map(|s: &str| s.parse::<u8>().ok());
    ///
    /// // the parser will convert the result of digit1 to a number
    /// assert_eq!(parse.parse_next("123"), Ok(("", 123)));
    ///
    /// // this will fail if digit1 fails
    /// assert_eq!(parse.parse_next("abc"), Err(ErrMode::Backtrack(Error::new("abc", ErrorKind::Digit))));
    ///
    /// // this will fail if the mapped function fails (a `u8` is too small to hold `123456`)
    /// assert_eq!(parse.parse_next("123456"), Err(ErrMode::Backtrack(Error::new("123456", ErrorKind::Verify))));
    /// # }
    /// ```
    #[doc(alias = "satisfy_map")]
    #[doc(alias = "filter_map")]
    #[doc(alias = "map_opt")]
    fn verify_map<G, O2>(self, g: G) -> VerifyMap<Self, G, O>
    where
        Self: core::marker::Sized,
        G: FnMut(O) -> Option<O2>,
    {
        VerifyMap::new(self, g)
    }

    /// Creates a parser from the output of this one
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// use winnow::bytes::take;
    /// use winnow::number::u8;
    ///
    /// fn length_data(input: &[u8]) -> IResult<&[u8], &[u8]> {
    ///     u8.flat_map(take).parse_next(input)
    /// }
    ///
    /// assert_eq!(length_data.parse_next(&[2, 0, 1, 2][..]), Ok((&[2][..], &[0, 1][..])));
    /// assert_eq!(length_data.parse_next(&[4, 0, 1, 2][..]), Err(ErrMode::Backtrack(Error::new(&[0, 1, 2][..], ErrorKind::Eof))));
    /// ```
    ///
    /// which is the same as
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// use winnow::bytes::take;
    /// use winnow::number::u8;
    ///
    /// fn length_data(input: &[u8]) -> IResult<&[u8], &[u8]> {
    ///     let (input, length) = u8.parse_next(input)?;
    ///     let (input, data) = take(length).parse_next(input)?;
    ///     Ok((input, data))
    /// }
    ///
    /// assert_eq!(length_data.parse_next(&[2, 0, 1, 2][..]), Ok((&[2][..], &[0, 1][..])));
    /// assert_eq!(length_data.parse_next(&[4, 0, 1, 2][..]), Err(ErrMode::Backtrack(Error::new(&[0, 1, 2][..], ErrorKind::Eof))));
    /// ```
    fn flat_map<G, H, O2>(self, g: G) -> FlatMap<Self, G, O>
    where
        G: FnMut(O) -> H,
        H: Parser<I, O2, E>,
        Self: core::marker::Sized,
    {
        FlatMap::new(self, g)
    }

    /// Applies a second parser over the output of the first one
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// use winnow::character::digit1;
    /// use winnow::bytes::take;
    /// # fn main() {
    ///
    /// let mut digits = take(5u8).and_then(digit1);
    ///
    /// assert_eq!(digits.parse_next("12345"), Ok(("", "12345")));
    /// assert_eq!(digits.parse_next("123ab"), Ok(("", "123")));
    /// assert_eq!(digits.parse_next("123"), Err(ErrMode::Backtrack(Error::new("123", ErrorKind::Eof))));
    /// # }
    /// ```
    fn and_then<G, O2>(self, g: G) -> AndThen<Self, G, O>
    where
        O: StreamIsPartial,
        G: Parser<O, O2, E>,
        Self: core::marker::Sized,
    {
        AndThen::new(self, g)
    }

    /// Apply [`std::str::FromStr`] to the output of the parser
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult,Parser};
    /// use winnow::character::digit1;
    ///
    /// fn parser(input: &str) -> IResult<&str, u64> {
    ///     digit1.parse_to().parse_next(input)
    /// }
    ///
    /// // the parser will count how many characters were returned by digit1
    /// assert_eq!(parser.parse_next("123456"), Ok(("", 123456)));
    ///
    /// // this will fail if digit1 fails
    /// assert_eq!(parser.parse_next("abc"), Err(ErrMode::Backtrack(Error::new("abc", ErrorKind::Digit))));
    /// ```
    #[doc(alias = "from_str")]
    fn parse_to<O2>(self) -> ParseTo<Self, O, O2>
    where
        Self: core::marker::Sized,
        O: crate::stream::ParseSlice<O2>,
    {
        ParseTo::new(self)
    }

    /// Returns the output of the child parser if it satisfies a verification function.
    ///
    /// The verification function takes as argument a reference to the output of the
    /// parser.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode,error::ErrorKind, error::Error, IResult, Parser};
    /// # use winnow::character::alpha1;
    /// # fn main() {
    ///
    /// let mut parser = alpha1.verify(|s: &str| s.len() == 4);
    ///
    /// assert_eq!(parser.parse_next("abcd"), Ok(("", "abcd")));
    /// assert_eq!(parser.parse_next("abcde"), Err(ErrMode::Backtrack(Error::new("abcde", ErrorKind::Verify))));
    /// assert_eq!(parser.parse_next("123abcd;"),Err(ErrMode::Backtrack(Error::new("123abcd;", ErrorKind::Alpha))));
    /// # }
    /// ```
    #[doc(alias = "satisfy")]
    #[doc(alias = "filter")]
    fn verify<G, O2: ?Sized>(self, second: G) -> Verify<Self, G, O2>
    where
        Self: core::marker::Sized,
        G: Fn(&O2) -> bool,
    {
        Verify::new(self, second)
    }

    /// If parsing fails, add context to the error
    ///
    /// This is used mainly to add user friendly information
    /// to errors when backtracking through a parse tree.
    #[doc(alias = "labelled")]
    fn context<C>(self, context: C) -> Context<Self, O, C>
    where
        Self: core::marker::Sized,
        C: Clone + crate::lib::std::fmt::Debug,
        E: ContextError<I, C>,
    {
        Context::new(self, context)
    }

    /// Transforms [`Incomplete`][crate::error::ErrMode::Incomplete] into [`Backtrack`][crate::error::ErrMode::Backtrack]
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, IResult, stream::Partial, Parser};
    /// # use winnow::bytes::take;
    /// # fn main() {
    ///
    /// let mut parser = take(5u8).complete_err();
    ///
    /// assert_eq!(parser.parse_next(Partial::new("abcdefg")), Ok((Partial::new("fg"), "abcde")));
    /// assert_eq!(parser.parse_next(Partial::new("abcd")), Err(ErrMode::Backtrack(Error::new(Partial::new("abcd"), ErrorKind::Complete))));
    /// # }
    /// ```
    fn complete_err(self) -> CompleteErr<Self>
    where
        Self: core::marker::Sized,
    {
        CompleteErr::new(self)
    }

    /// Convert the parser's error to another type using [`std::convert::From`]
    fn err_into<E2>(self) -> ErrInto<Self, E, E2>
    where
        Self: core::marker::Sized,
        E: Into<E2>,
    {
        ErrInto::new(self)
    }

    /// Applies a second parser after the first one, return their results as a tuple
    ///
    /// **WARNING:** Deprecated, replaced with [`winnow::sequence::tuple`][crate::sequence::tuple]
    #[deprecated(since = "0.1.0", note = "Replaced with `winnow::sequence::tuple")]
    #[cfg_attr(feature = "unstable-doc", doc(hidden))]
    fn and<G, O2>(self, g: G) -> And<Self, G>
    where
        G: Parser<I, O2, E>,
        Self: core::marker::Sized,
    {
        And::new(self, g)
    }

    /// Applies a second parser over the input if the first one failed
    ///
    /// **WARNING:** Deprecated, replaced with [`winnow::branch::alt`][crate::branch::alt]
    #[deprecated(since = "0.1.0", note = "Replaced with `winnow::branch::alt")]
    #[cfg_attr(feature = "unstable-doc", doc(hidden))]
    fn or<G>(self, g: G) -> Or<Self, G>
    where
        G: Parser<I, O, E>,
        Self: core::marker::Sized,
    {
        Or::new(self, g)
    }
}

impl<'a, I, O, E, F> Parser<I, O, E> for F
where
    F: FnMut(I) -> IResult<I, O, E> + 'a,
{
    fn parse_next(&mut self, i: I) -> IResult<I, O, E> {
        self(i)
    }
}

/// This is a shortcut for [`one_of`][crate::bytes::one_of].
///
/// # Example
///
/// ```
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::{ErrorKind, Error}};
/// fn parser(i: &[u8]) -> IResult<&[u8], u8> {
///     b'a'.parse_next(i)
/// }
/// assert_eq!(parser(&b"abc"[..]), Ok((&b"bc"[..], b'a')));
/// assert_eq!(parser(&b" abc"[..]), Err(ErrMode::Backtrack(Error::new(&b" abc"[..], ErrorKind::OneOf))));
/// assert_eq!(parser(&b"bc"[..]), Err(ErrMode::Backtrack(Error::new(&b"bc"[..], ErrorKind::OneOf))));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&b""[..], ErrorKind::OneOf))));
/// ```
impl<I, E> Parser<I, u8, E> for u8
where
    I: StreamIsPartial,
    I: Stream<Token = u8>,
    E: ParseError<I>,
{
    fn parse_next(&mut self, i: I) -> IResult<I, u8, E> {
        crate::bytes::one_of(*self).parse_next(i)
    }
}

/// This is a shortcut for [`one_of`][crate::bytes::one_of].
///
/// # Example
///
/// ```
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::{ErrorKind, Error}};
/// fn parser(i: &str) -> IResult<&str, char> {
///     'a'.parse_next(i)
/// }
/// assert_eq!(parser("abc"), Ok(("bc", 'a')));
/// assert_eq!(parser(" abc"), Err(ErrMode::Backtrack(Error::new(" abc", ErrorKind::OneOf))));
/// assert_eq!(parser("bc"), Err(ErrMode::Backtrack(Error::new("bc", ErrorKind::OneOf))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::OneOf))));
/// ```
impl<I, E> Parser<I, <I as Stream>::Token, E> for char
where
    I: StreamIsPartial,
    I: Stream,
    <I as Stream>::Token: AsChar + Copy,
    E: ParseError<I>,
{
    fn parse_next(&mut self, i: I) -> IResult<I, <I as Stream>::Token, E> {
        crate::bytes::one_of(*self).parse_next(i)
    }
}

/// This is a shortcut for [`tag`][crate::bytes::tag].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::branch::alt;
/// # use winnow::bytes::take;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   alt((&"Hello"[..], take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser(&b"Hello, World!"[..]), Ok((&b", World!"[..], &b"Hello"[..])));
/// assert_eq!(parser(&b"Something"[..]), Ok((&b"hing"[..], &b"Somet"[..])));
/// assert_eq!(parser(&b"Some"[..]), Err(ErrMode::Backtrack(Error::new(&b"Some"[..], ErrorKind::Eof))));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&b""[..], ErrorKind::Eof))));
/// ```
impl<'s, I, E: ParseError<I>> Parser<I, <I as Stream>::Slice, E> for &'s [u8]
where
    I: Compare<&'s [u8]> + StreamIsPartial,
    I: Stream,
{
    fn parse_next(&mut self, i: I) -> IResult<I, <I as Stream>::Slice, E> {
        crate::bytes::tag(*self).parse_next(i)
    }
}

/// This is a shortcut for [`tag`][crate::bytes::tag].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::branch::alt;
/// # use winnow::bytes::take;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   alt((b"Hello", take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser(&b"Hello, World!"[..]), Ok((&b", World!"[..], &b"Hello"[..])));
/// assert_eq!(parser(&b"Something"[..]), Ok((&b"hing"[..], &b"Somet"[..])));
/// assert_eq!(parser(&b"Some"[..]), Err(ErrMode::Backtrack(Error::new(&b"Some"[..], ErrorKind::Eof))));
/// assert_eq!(parser(&b""[..]), Err(ErrMode::Backtrack(Error::new(&b""[..], ErrorKind::Eof))));
/// ```
impl<'s, I, E: ParseError<I>, const N: usize> Parser<I, <I as Stream>::Slice, E> for &'s [u8; N]
where
    I: Compare<&'s [u8; N]> + StreamIsPartial,
    I: Stream,
{
    fn parse_next(&mut self, i: I) -> IResult<I, <I as Stream>::Slice, E> {
        crate::bytes::tag(*self).parse_next(i)
    }
}

/// This is a shortcut for [`tag`][crate::bytes::tag].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}};
/// # use winnow::branch::alt;
/// # use winnow::bytes::take;
///
/// fn parser(s: &str) -> IResult<&str, &str> {
///   alt(("Hello", take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser("Something"), Ok(("hing", "Somet")));
/// assert_eq!(parser("Some"), Err(ErrMode::Backtrack(Error::new("Some", ErrorKind::Eof))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Eof))));
/// ```
impl<'s, I, E: ParseError<I>> Parser<I, <I as Stream>::Slice, E> for &'s str
where
    I: Compare<&'s str> + StreamIsPartial,
    I: Stream,
{
    fn parse_next(&mut self, i: I) -> IResult<I, <I as Stream>::Slice, E> {
        crate::bytes::tag(*self).parse_next(i)
    }
}

impl<I, E: ParseError<I>> Parser<I, (), E> for () {
    fn parse_next(&mut self, i: I) -> IResult<I, (), E> {
        Ok((i, ()))
    }
}

macro_rules! impl_parser_for_tuple {
  ($($parser:ident $output:ident),+) => (
    #[allow(non_snake_case)]
    impl<I, $($output),+, E: ParseError<I>, $($parser),+> Parser<I, ($($output),+,), E> for ($($parser),+,)
    where
      $($parser: Parser<I, $output, E>),+
    {
      fn parse_next(&mut self, i: I) -> IResult<I, ($($output),+,), E> {
        let ($(ref mut $parser),+,) = *self;

        $(let(i, $output) = $parser.parse_next(i)?;)+

        Ok((i, ($($output),+,)))
      }
    }
  )
}

macro_rules! impl_parser_for_tuples {
    ($parser1:ident $output1:ident, $($parser:ident $output:ident),+) => {
        impl_parser_for_tuples!(__impl $parser1 $output1; $($parser $output),+);
    };
    (__impl $($parser:ident $output:ident),+; $parser1:ident $output1:ident $(,$parser2:ident $output2:ident)*) => {
        impl_parser_for_tuple!($($parser $output),+);
        impl_parser_for_tuples!(__impl $($parser $output),+, $parser1 $output1; $($parser2 $output2),*);
    };
    (__impl $($parser:ident $output:ident),+;) => {
        impl_parser_for_tuple!($($parser $output),+);
    }
}

impl_parser_for_tuples!(
  P1 O1,
  P2 O2,
  P3 O3,
  P4 O4,
  P5 O5,
  P6 O6,
  P7 O7,
  P8 O8,
  P9 O9,
  P10 O10,
  P11 O11,
  P12 O12,
  P13 O13,
  P14 O14,
  P15 O15,
  P16 O16,
  P17 O17,
  P18 O18,
  P19 O19,
  P20 O20,
  P21 O21
);

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

#[cfg(feature = "alloc")]
impl<'a, I, O, E> Parser<I, O, E> for Box<dyn Parser<I, O, E> + 'a> {
    fn parse_next(&mut self, input: I) -> IResult<I, O, E> {
        (**self).parse_next(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytes::{tag, take};
    use crate::error::ErrMode;
    use crate::error::Error;
    use crate::error::ErrorKind;
    use crate::error::Needed;
    use crate::number::be_u16;
    use crate::Partial;

    #[doc(hidden)]
    #[macro_export]
    macro_rules! assert_size (
    ($t:ty, $sz:expr) => (
      assert!($crate::lib::std::mem::size_of::<$t>() <= $sz, "{} <= {} failed", $crate::lib::std::mem::size_of::<$t>(), $sz);
    );
  );

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn size_test() {
        assert_size!(IResult<&[u8], &[u8], (&[u8], u32)>, 40);
        assert_size!(IResult<&str, &str, u32>, 40);
        assert_size!(Needed, 8);
        assert_size!(ErrMode<u32>, 16);
        assert_size!(ErrorKind, 1);
    }

    #[test]
    fn err_map_test() {
        let e = ErrMode::Backtrack(1);
        assert_eq!(e.map(|v| v + 1), ErrMode::Backtrack(2));
    }

    #[test]
    fn single_element_tuples() {
        use crate::character::alpha1;
        use crate::error::ErrorKind;

        let mut parser = (alpha1,);
        assert_eq!(parser.parse_next("abc123def"), Ok(("123def", ("abc",))));
        assert_eq!(
            parser.parse_next("123def"),
            Err(ErrMode::Backtrack(Error {
                input: "123def",
                kind: ErrorKind::Alpha
            }))
        );
    }

    #[test]
    fn tuple_test() {
        #[allow(clippy::type_complexity)]
        fn tuple_3(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, (u16, &[u8], &[u8])> {
            (be_u16, take(3u8), tag("fg")).parse_next(i)
        }

        assert_eq!(
            tuple_3(Partial::new(&b"abcdefgh"[..])),
            Ok((
                Partial::new(&b"h"[..]),
                (0x6162u16, &b"cde"[..], &b"fg"[..])
            ))
        );
        assert_eq!(
            tuple_3(Partial::new(&b"abcd"[..])),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            tuple_3(Partial::new(&b"abcde"[..])),
            Err(ErrMode::Incomplete(Needed::new(2)))
        );
        assert_eq!(
            tuple_3(Partial::new(&b"abcdejk"[..])),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(&b"jk"[..]),
                ErrorKind::Tag
            )))
        );
    }

    #[test]
    fn unit_type() {
        fn parser(i: &str) -> IResult<&str, ()> {
            ().parse_next(i)
        }
        assert_eq!(parser.parse_next("abxsbsh"), Ok(("abxsbsh", ())));
        assert_eq!(parser.parse_next("sdfjakdsas"), Ok(("sdfjakdsas", ())));
        assert_eq!(parser.parse_next(""), Ok(("", ())));
    }
}
