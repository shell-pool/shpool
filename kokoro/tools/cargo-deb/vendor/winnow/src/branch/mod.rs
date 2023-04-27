//! Choice combinators

#[cfg(test)]
mod tests;

use crate::error::ErrMode;
use crate::error::ErrorKind;
use crate::error::ParseError;
use crate::stream::Stream;
use crate::trace::trace;
use crate::{IResult, Parser};

#[doc(inline)]
pub use crate::dispatch;

/// Helper trait for the [alt()] combinator.
///
/// This trait is implemented for tuples of up to 21 elements
pub trait Alt<I, O, E> {
    /// Tests each parser in the tuple and returns the result of the first one that succeeds
    fn choice(&mut self, input: I) -> IResult<I, O, E>;
}

/// Pick the first successful parser
///
/// For tight control over the error, add a final case using [`fail`][crate::combinator::fail].
/// Alternatively, with a [custom error type][crate::_topic::error], it is possible to track all
/// errors or return the error of the parser that went the farthest in the input data.
///
/// When the alternative cases have unique prefixes, [`dispatch`] can offer better performance.
///
/// # Example
///
/// ```rust
/// # use winnow::error_position;
/// # use winnow::{error::ErrMode,error::ErrorKind, error::Needed, IResult};
/// use winnow::character::{alpha1, digit1};
/// use winnow::branch::alt;
/// # fn main() {
/// fn parser(input: &str) -> IResult<&str, &str> {
///   alt((alpha1, digit1))(input)
/// };
///
/// // the first parser, alpha1, recognizes the input
/// assert_eq!(parser("abc"), Ok(("", "abc")));
///
/// // the first parser returns an error, so alt tries the second one
/// assert_eq!(parser("123456"), Ok(("", "123456")));
///
/// // both parsers failed, and with the default error type, alt will return the last error
/// assert_eq!(parser(" "), Err(ErrMode::Backtrack(error_position!(" ", ErrorKind::Digit))));
/// # }
/// ```
#[doc(alias = "choice")]
pub fn alt<I: Stream, O, E: ParseError<I>, List: Alt<I, O, E>>(
    mut l: List,
) -> impl FnMut(I) -> IResult<I, O, E> {
    trace("alt", move |i: I| l.choice(i))
}

/// Helper trait for the [permutation()] combinator.
///
/// This trait is implemented for tuples of up to 21 elements
pub trait Permutation<I, O, E> {
    /// Tries to apply all parsers in the tuple in various orders until all of them succeed
    fn permutation(&mut self, input: I) -> IResult<I, O, E>;
}

/// Applies a list of parsers in any order.
///
/// Permutation will succeed if all of the child parsers succeeded.
/// It takes as argument a tuple of parsers, and returns a
/// tuple of the parser results.
///
/// ```rust
/// # use winnow::{error::ErrMode,error::{Error, ErrorKind}, error::Needed, IResult};
/// use winnow::character::{alpha1, digit1};
/// use winnow::branch::permutation;
/// # fn main() {
/// fn parser(input: &str) -> IResult<&str, (&str, &str)> {
///   permutation((alpha1, digit1))(input)
/// }
///
/// // permutation recognizes alphabetic characters then digit
/// assert_eq!(parser("abc123"), Ok(("", ("abc", "123"))));
///
/// // but also in inverse order
/// assert_eq!(parser("123abc"), Ok(("", ("abc", "123"))));
///
/// // it will fail if one of the parsers failed
/// assert_eq!(parser("abc;"), Err(ErrMode::Backtrack(Error::new(";", ErrorKind::Digit))));
/// # }
/// ```
///
/// The parsers are applied greedily: if there are multiple unapplied parsers
/// that could parse the next slice of input, the first one is used.
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, IResult};
/// use winnow::branch::permutation;
/// use winnow::bytes::any;
///
/// fn parser(input: &str) -> IResult<&str, (char, char)> {
///   permutation((any, 'a'))(input)
/// }
///
/// // any parses 'b', then char('a') parses 'a'
/// assert_eq!(parser("ba"), Ok(("", ('b', 'a'))));
///
/// // any parses 'a', then char('a') fails on 'b',
/// // even though char('a') followed by any would succeed
/// assert_eq!(parser("ab"), Err(ErrMode::Backtrack(Error::new("b", ErrorKind::OneOf))));
/// ```
///
pub fn permutation<I: Stream, O, E: ParseError<I>, List: Permutation<I, O, E>>(
    mut l: List,
) -> impl FnMut(I) -> IResult<I, O, E> {
    trace("permutation", move |i: I| l.permutation(i))
}

macro_rules! alt_trait(
  ($first:ident $second:ident $($id: ident)+) => (
    alt_trait!(__impl $first $second; $($id)+);
  );
  (__impl $($current:ident)*; $head:ident $($id: ident)+) => (
    alt_trait_impl!($($current)*);

    alt_trait!(__impl $($current)* $head; $($id)+);
  );
  (__impl $($current:ident)*; $head:ident) => (
    alt_trait_impl!($($current)*);
    alt_trait_impl!($($current)* $head);
  );
);

macro_rules! alt_trait_impl(
  ($($id:ident)+) => (
    impl<
      I: Clone, Output, Error: ParseError<I>,
      $($id: Parser<I, Output, Error>),+
    > Alt<I, Output, Error> for ( $($id),+ ) {

      fn choice(&mut self, input: I) -> IResult<I, Output, Error> {
        match self.0.parse_next(input.clone()) {
          Err(ErrMode::Backtrack(e)) => alt_trait_inner!(1, self, input, e, $($id)+),
          res => res,
        }
      }
    }
  );
);

macro_rules! alt_trait_inner(
  ($it:tt, $self:expr, $input:expr, $err:expr, $head:ident $($id:ident)+) => (
    match $self.$it.parse_next($input.clone()) {
      Err(ErrMode::Backtrack(e)) => {
        let err = $err.or(e);
        succ!($it, alt_trait_inner!($self, $input, err, $($id)+))
      }
      res => res,
    }
  );
  ($it:tt, $self:expr, $input:expr, $err:expr, $head:ident) => (
    Err(ErrMode::Backtrack($err.append($input, ErrorKind::Alt)))
  );
);

alt_trait!(Alt2 Alt3 Alt4 Alt5 Alt6 Alt7 Alt8 Alt9 Alt10 Alt11 Alt12 Alt13 Alt14 Alt15 Alt16 Alt17 Alt18 Alt19 Alt20 Alt21 Alt22);

// Manually implement Alt for (A,), the 1-tuple type
impl<I, O, E: ParseError<I>, A: Parser<I, O, E>> Alt<I, O, E> for (A,) {
    fn choice(&mut self, input: I) -> IResult<I, O, E> {
        self.0.parse_next(input)
    }
}

macro_rules! permutation_trait(
  (
    $name1:ident $ty1:ident $item1:ident
    $name2:ident $ty2:ident $item2:ident
    $($name3:ident $ty3:ident $item3:ident)*
  ) => (
    permutation_trait!(__impl $name1 $ty1 $item1, $name2 $ty2 $item2; $($name3 $ty3 $item3)*);
  );
  (
    __impl $($name:ident $ty:ident $item:ident),+;
    $name1:ident $ty1:ident $item1:ident $($name2:ident $ty2:ident $item2:ident)*
  ) => (
    permutation_trait_impl!($($name $ty $item),+);
    permutation_trait!(__impl $($name $ty $item),+ , $name1 $ty1 $item1; $($name2 $ty2 $item2)*);
  );
  (__impl $($name:ident $ty:ident $item:ident),+;) => (
    permutation_trait_impl!($($name $ty $item),+);
  );
);

macro_rules! permutation_trait_impl(
  ($($name:ident $ty:ident $item:ident),+) => (
    impl<
      I: Clone, $($ty),+ , Error: ParseError<I>,
      $($name: Parser<I, $ty, Error>),+
    > Permutation<I, ( $($ty),+ ), Error> for ( $($name),+ ) {

      fn permutation(&mut self, mut input: I) -> IResult<I, ( $($ty),+ ), Error> {
        let mut res = ($(Option::<$ty>::None),+);

        loop {
          let mut err: Option<Error> = None;
          permutation_trait_inner!(0, self, input, res, err, $($name)+);

          // If we reach here, every iterator has either been applied before,
          // or errored on the remaining input
          if let Some(err) = err {
            // There are remaining parsers, and all errored on the remaining input
            return Err(ErrMode::Backtrack(err.append(input, ErrorKind::Permutation)));
          }

          // All parsers were applied
          match res {
            ($(Some($item)),+) => return Ok((input, ($($item),+))),
            _ => unreachable!(),
          }
        }
      }
    }
  );
);

macro_rules! permutation_trait_inner(
  ($it:tt, $self:expr, $input:ident, $res:expr, $err:expr, $head:ident $($id:ident)*) => (
    if $res.$it.is_none() {
      match $self.$it.parse_next($input.clone()) {
        Ok((i, o)) => {
          $input = i;
          $res.$it = Some(o);
          continue;
        }
        Err(ErrMode::Backtrack(e)) => {
          $err = Some(match $err {
            Some(err) => err.or(e),
            None => e,
          });
        }
        Err(e) => return Err(e),
      };
    }
    succ!($it, permutation_trait_inner!($self, $input, $res, $err, $($id)*));
  );
  ($it:tt, $self:expr, $input:ident, $res:expr, $err:expr,) => ();
);

permutation_trait!(
  P1 O1 o1
  P2 O2 o2
  P3 O3 o3
  P4 O4 o4
  P5 O5 o5
  P6 O6 o6
  P7 O7 o7
  P8 O8 o8
  P9 O9 o9
  P10 O10 o10
  P11 O11 o11
  P12 O12 o12
  P13 O13 o13
  P14 O14 o14
  P15 O15 o15
  P16 O16 o16
  P17 O17 o17
  P18 O18 o18
  P19 O19 o19
  P20 O20 o20
  P21 O21 o21
);
