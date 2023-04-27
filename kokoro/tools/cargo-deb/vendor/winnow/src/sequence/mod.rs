//! Combinators applying parsers in sequence

#[cfg(test)]
mod tests;

use crate::error::ParseError;
use crate::stream::Stream;
use crate::trace::trace;
use crate::{IResult, Parser};

/// Gets an object from the first parser,
/// then gets another object from the second parser.
///
/// **WARNING:** Deprecated, [`Parser`] is directly implemented for tuples
///
/// # Arguments
/// * `first` The first parser to apply.
/// * `second` The second parser to apply.
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::sequence::pair;
/// use winnow::bytes::tag;
///
/// let mut parser = pair(tag("abc"), tag("efg"));
///
/// assert_eq!(parser("abcefg"), Ok(("", ("abc", "efg"))));
/// assert_eq!(parser("abcefghij"), Ok(("hij", ("abc", "efg"))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("123"), Err(ErrMode::Backtrack(Error::new("123", ErrorKind::Tag))));
/// ```
#[deprecated(since = "0.1.0", note = "`Parser` is directly implemented for tuples")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub fn pair<I, O1, O2, E: ParseError<I>, F, G>(
    mut first: F,
    mut second: G,
) -> impl FnMut(I) -> IResult<I, (O1, O2), E>
where
    F: Parser<I, O1, E>,
    G: Parser<I, O2, E>,
{
    move |input: I| {
        let (input, o1) = first.parse_next(input)?;
        second.parse_next(input).map(|(i, o2)| (i, (o1, o2)))
    }
}

/// Apply two parsers, only returning the output from the second.
///
/// # Arguments
/// * `first` The opening parser.
/// * `second` The second parser to get object.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::sequence::preceded;
/// use winnow::bytes::tag;
///
/// let mut parser = preceded(tag("abc"), tag("efg"));
///
/// assert_eq!(parser("abcefg"), Ok(("", "efg")));
/// assert_eq!(parser("abcefghij"), Ok(("hij", "efg")));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("123"), Err(ErrMode::Backtrack(Error::new("123", ErrorKind::Tag))));
/// ```
#[doc(alias = "ignore_then")]
pub fn preceded<I, O1, O2, E: ParseError<I>, F, G>(
    mut first: F,
    mut second: G,
) -> impl FnMut(I) -> IResult<I, O2, E>
where
    I: Stream,
    F: Parser<I, O1, E>,
    G: Parser<I, O2, E>,
{
    trace("preceded", move |input: I| {
        let (input, _) = first.parse_next(input)?;
        second.parse_next(input)
    })
}

/// Apply two parsers, only returning the output of the first.
///
/// # Arguments
/// * `first` The first parser to apply.
/// * `second` The second parser to match an object.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::sequence::terminated;
/// use winnow::bytes::tag;
///
/// let mut parser = terminated(tag("abc"), tag("efg"));
///
/// assert_eq!(parser("abcefg"), Ok(("", "abc")));
/// assert_eq!(parser("abcefghij"), Ok(("hij", "abc")));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("123"), Err(ErrMode::Backtrack(Error::new("123", ErrorKind::Tag))));
/// ```
#[doc(alias = "then_ignore")]
pub fn terminated<I, O1, O2, E: ParseError<I>, F, G>(
    mut first: F,
    mut second: G,
) -> impl FnMut(I) -> IResult<I, O1, E>
where
    I: Stream,
    F: Parser<I, O1, E>,
    G: Parser<I, O2, E>,
{
    trace("terminated", move |input: I| {
        let (input, o1) = first.parse_next(input)?;
        second.parse_next(input).map(|(i, _)| (i, o1))
    })
}

/// Apply three parsers, only returning the values of the first and third.
///
/// # Arguments
/// * `first` The first parser to apply.
/// * `sep` The separator parser to apply.
/// * `second` The second parser to apply.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::sequence::separated_pair;
/// use winnow::bytes::tag;
///
/// let mut parser = separated_pair(tag("abc"), tag("|"), tag("efg"));
///
/// assert_eq!(parser("abc|efg"), Ok(("", ("abc", "efg"))));
/// assert_eq!(parser("abc|efghij"), Ok(("hij", ("abc", "efg"))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("123"), Err(ErrMode::Backtrack(Error::new("123", ErrorKind::Tag))));
/// ```
pub fn separated_pair<I, O1, O2, O3, E: ParseError<I>, F, G, H>(
    mut first: F,
    mut sep: G,
    mut second: H,
) -> impl FnMut(I) -> IResult<I, (O1, O3), E>
where
    I: Stream,
    F: Parser<I, O1, E>,
    G: Parser<I, O2, E>,
    H: Parser<I, O3, E>,
{
    trace("separated_pair", move |input: I| {
        let (input, o1) = first.parse_next(input)?;
        let (input, _) = sep.parse_next(input)?;
        second.parse_next(input).map(|(i, o2)| (i, (o1, o2)))
    })
}

/// Apply three parsers, only returning the output of the second.
///
/// # Arguments
/// * `first` The first parser to apply and discard.
/// * `second` The second parser to apply.
/// * `third` The third parser to apply and discard.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error, error::Needed};
/// # use winnow::error::Needed::Size;
/// use winnow::sequence::delimited;
/// use winnow::bytes::tag;
///
/// let mut parser = delimited(tag("("), tag("abc"), tag(")"));
///
/// assert_eq!(parser("(abc)"), Ok(("", "abc")));
/// assert_eq!(parser("(abc)def"), Ok(("def", "abc")));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("123"), Err(ErrMode::Backtrack(Error::new("123", ErrorKind::Tag))));
/// ```
#[doc(alias = "between")]
#[doc(alias = "padded")]
pub fn delimited<I, O1, O2, O3, E: ParseError<I>, F, G, H>(
    mut first: F,
    mut second: G,
    mut third: H,
) -> impl FnMut(I) -> IResult<I, O2, E>
where
    I: Stream,
    F: Parser<I, O1, E>,
    G: Parser<I, O2, E>,
    H: Parser<I, O3, E>,
{
    trace("delimited", move |input: I| {
        let (input, _) = first.parse_next(input)?;
        let (input, o2) = second.parse_next(input)?;
        third.parse_next(input).map(|(i, _)| (i, o2))
    })
}

/// Helper trait for the tuple combinator.
///
/// This trait is implemented for tuples of parsers of up to 21 elements.
#[deprecated(since = "0.1.0", note = "Replaced with `Parser`")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
pub trait Tuple<I, O, E> {
    /// Parses the input and returns a tuple of results of each parser.
    fn parse(&mut self, input: I) -> IResult<I, O, E>;
}

#[allow(deprecated)]
impl<I, O, E: ParseError<I>, F: Parser<I, O, E>> Tuple<I, (O,), E> for (F,) {
    fn parse(&mut self, input: I) -> IResult<I, (O,), E> {
        self.0.parse_next(input).map(|(i, o)| (i, (o,)))
    }
}

macro_rules! tuple_trait(
  ($name1:ident $ty1:ident, $name2: ident $ty2:ident, $($name:ident $ty:ident),*) => (
    tuple_trait!(__impl $name1 $ty1, $name2 $ty2; $($name $ty),*);
  );
  (__impl $($name:ident $ty: ident),+; $name1:ident $ty1:ident, $($name2:ident $ty2:ident),*) => (
    tuple_trait_impl!($($name $ty),+);
    tuple_trait!(__impl $($name $ty),+ , $name1 $ty1; $($name2 $ty2),*);
  );
  (__impl $($name:ident $ty: ident),+; $name1:ident $ty1:ident) => (
    tuple_trait_impl!($($name $ty),+);
    tuple_trait_impl!($($name $ty),+, $name1 $ty1);
  );
);

macro_rules! tuple_trait_impl(
  ($($name:ident $ty: ident),+) => (
    #[allow(deprecated)]
    impl<
      I: Stream, $($ty),+ , Error: ParseError<I>,
      $($name: Parser<I, $ty, Error>),+
    > Tuple<I, ( $($ty),+ ), Error> for ( $($name),+ ) {

      fn parse(&mut self, input: I) -> IResult<I, ( $($ty),+ ), Error> {
        trace("tuple", move |input: I| tuple_trait_inner!(0, self, input, (), $($name)+))(input)

      }
    }
  );
);

macro_rules! tuple_trait_inner(
  ($it:tt, $self:expr, $input:expr, (), $head:ident $($id:ident)+) => ({
    let (i, o) = $self.$it.parse_next($input.clone())?;

    succ!($it, tuple_trait_inner!($self, i, ( o ), $($id)+))
  });
  ($it:tt, $self:expr, $input:expr, ($($parsed:tt)*), $head:ident $($id:ident)+) => ({
    let (i, o) = $self.$it.parse_next($input.clone())?;

    succ!($it, tuple_trait_inner!($self, i, ($($parsed)* , o), $($id)+))
  });
  ($it:tt, $self:expr, $input:expr, ($($parsed:tt)*), $head:ident) => ({
    let (i, o) = $self.$it.parse_next($input.clone())?;

    Ok((i, ($($parsed)* , o)))
  });
);

tuple_trait!(
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

// Special case: implement `Tuple` for `()`, the unit type.
// This can come up in macros which accept a variable number of arguments.
// Literally, `()` is an empty tuple, so it should simply parse nothing.
#[allow(deprecated)]
impl<I, E: ParseError<I>> Tuple<I, (), E> for () {
    fn parse(&mut self, input: I) -> IResult<I, (), E> {
        Ok((input, ()))
    }
}

///Applies a tuple of parsers one by one and returns their results as a tuple.
///There is a maximum of 21 parsers
///
/// **WARNING:** Deprecated, [`Parser`] is directly implemented for tuples
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Error};
/// use winnow::sequence::tuple;
/// use winnow::character::{alpha1, digit1};
/// let mut parser = tuple((alpha1, digit1, alpha1));
///
/// assert_eq!(parser("abc123def"), Ok(("", ("abc", "123", "def"))));
/// assert_eq!(parser("123def"), Err(ErrMode::Backtrack(Error::new("123def", ErrorKind::Alpha))));
/// ```
#[deprecated(since = "0.1.0", note = "`Parser` is directly implemented for tuples")]
#[cfg_attr(feature = "unstable-doc", doc(hidden))]
#[allow(deprecated)]
pub fn tuple<I, O, E: ParseError<I>, List: Tuple<I, O, E>>(
    mut l: List,
) -> impl FnMut(I) -> IResult<I, O, E> {
    move |i: I| l.parse(i)
}
