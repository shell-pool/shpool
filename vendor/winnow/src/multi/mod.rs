//! Combinators applying their child parser multiple times

#[cfg(test)]
mod tests;

use crate::error::ErrMode;
use crate::error::ErrorKind;
use crate::error::ParseError;
use crate::stream::Accumulate;
use crate::stream::{Stream, StreamIsPartial, ToUsize, UpdateSlice};
use crate::trace::trace;
use crate::Parser;

/// [`Accumulate`] the output of a parser into a container, like `Vec`
///
/// This stops on [`ErrMode::Backtrack`].  To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// To recognize a series of tokens, [`Accumulate`] into a `()` and then [`Parser::recognize`].
///
/// **Warning:** if the parser passed in accepts empty inputs (like `alpha0` or `digit0`), `many0` will
/// return an error, to prevent going into an infinite loop
///
/// # Example
///
#[cfg_attr(not(feature = "std"), doc = "```ignore")]
#[cfg_attr(feature = "std", doc = "```")]
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::many0;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   many0("abc").parse_next(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// ```
#[doc(alias = "skip_many")]
#[doc(alias = "repeated")]
#[doc(alias = "many0_count")]
pub fn many0<I, O, C, E, F>(mut f: F) -> impl Parser<I, C, E>
where
    I: Stream,
    C: Accumulate<O>,
    F: Parser<I, O, E>,
    E: ParseError<I>,
{
    trace("many0", move |mut i: I| {
        let mut acc = C::initial(None);
        loop {
            let len = i.eof_offset();
            match f.parse_next(i.clone()) {
                Err(ErrMode::Backtrack(_)) => return Ok((i, acc)),
                Err(e) => return Err(e),
                Ok((i1, o)) => {
                    // infinite loop check: the parser must always consume
                    if i1.eof_offset() == len {
                        return Err(ErrMode::assert(i, "many parsers must always consume"));
                    }

                    i = i1;
                    acc.accumulate(o);
                }
            }
        }
    })
}

/// [`Accumulate`] the output of a parser into a container, like `Vec`
///
///
/// This stops on [`ErrMode::Backtrack`] if there is at least one result.  To instead chain an error up,
/// see [`cut_err`][crate::combinator::cut_err].
///
/// # Arguments
/// * `f` The parser to apply.
///
/// To recognize a series of tokens, [`Accumulate`] into a `()` and then [`Parser::recognize`].
///
/// **Warning:** If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// # Example
///
#[cfg_attr(not(feature = "std"), doc = "```ignore")]
#[cfg_attr(feature = "std", doc = "```")]
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::many1;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   many1("abc").parse_next(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Err(ErrMode::Backtrack(Error::new("123123", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// ```
#[doc(alias = "skip_many1")]
#[doc(alias = "repeated")]
#[doc(alias = "many1_count")]
pub fn many1<I, O, C, E, F>(mut f: F) -> impl Parser<I, C, E>
where
    I: Stream,
    C: Accumulate<O>,
    F: Parser<I, O, E>,
    E: ParseError<I>,
{
    trace("many1", move |mut i: I| match f.parse_next(i.clone()) {
        Err(e) => Err(e.append(i, ErrorKind::Many)),
        Ok((i1, o)) => {
            let mut acc = C::initial(None);
            acc.accumulate(o);
            i = i1;

            loop {
                let len = i.eof_offset();
                match f.parse_next(i.clone()) {
                    Err(ErrMode::Backtrack(_)) => return Ok((i, acc)),
                    Err(e) => return Err(e),
                    Ok((i1, o)) => {
                        // infinite loop check: the parser must always consume
                        if i1.eof_offset() == len {
                            return Err(ErrMode::assert(i, "many parsers must always consume"));
                        }

                        i = i1;
                        acc.accumulate(o);
                    }
                }
            }
        }
    })
}

/// Applies the parser `f` until the parser `g` produces a result.
///
/// Returns a tuple of the results of `f` in a `Vec` and the result of `g`.
///
/// `f` keeps going so long as `g` produces [`ErrMode::Backtrack`]. To instead chain an error up, see [`cut_err`][crate::combinator::cut_err].
///
/// To recognize a series of tokens, [`Accumulate`] into a `()` and then [`Parser::recognize`].
///
/// # Example
///
#[cfg_attr(not(feature = "std"), doc = "```ignore")]
#[cfg_attr(feature = "std", doc = "```")]
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::many_till0;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, (Vec<&str>, &str)> {
///   many_till0("abc", "end").parse_next(s)
/// };
///
/// assert_eq!(parser("abcabcend"), Ok(("", (vec!["abc", "abc"], "end"))));
/// assert_eq!(parser("abc123end"), Err(ErrMode::Backtrack(Error::new("123end", ErrorKind::Tag))));
/// assert_eq!(parser("123123end"), Err(ErrMode::Backtrack(Error::new("123123end", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("abcendefg"), Ok(("efg", (vec!["abc"], "end"))));
/// ```
pub fn many_till0<I, O, C, P, E, F, G>(mut f: F, mut g: G) -> impl Parser<I, (C, P), E>
where
    I: Stream,
    C: Accumulate<O>,
    F: Parser<I, O, E>,
    G: Parser<I, P, E>,
    E: ParseError<I>,
{
    trace("many_till0", move |mut i: I| {
        let mut res = C::initial(None);
        loop {
            let len = i.eof_offset();
            match g.parse_next(i.clone()) {
                Ok((i1, o)) => return Ok((i1, (res, o))),
                Err(ErrMode::Backtrack(_)) => {
                    match f.parse_next(i.clone()) {
                        Err(e) => return Err(e.append(i, ErrorKind::Many)),
                        Ok((i1, o)) => {
                            // infinite loop check: the parser must always consume
                            if i1.eof_offset() == len {
                                return Err(ErrMode::assert(i, "many parsers must always consume"));
                            }

                            res.accumulate(o);
                            i = i1;
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
    })
}

/// Alternates between two parsers to produce a list of elements.
///
/// This stops when either parser returns [`ErrMode::Backtrack`].  To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Arguments
/// * `parser` Parses the elements of the list.
/// * `sep` Parses the separator between list elements.
///
/// # Example
///
#[cfg_attr(not(feature = "std"), doc = "```ignore")]
#[cfg_attr(feature = "std", doc = "```")]
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::separated0;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   separated0("abc", "|").parse_next(s)
/// }
///
/// assert_eq!(parser("abc|abc|abc"), Ok(("", vec!["abc", "abc", "abc"])));
/// assert_eq!(parser("abc123abc"), Ok(("123abc", vec!["abc"])));
/// assert_eq!(parser("abc|def"), Ok(("|def", vec!["abc"])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// assert_eq!(parser("def|abc"), Ok(("def|abc", vec![])));
/// ```
#[doc(alias = "sep_by")]
#[doc(alias = "separated_list0")]
pub fn separated0<I, O, C, O2, E, P, S>(mut parser: P, mut sep: S) -> impl Parser<I, C, E>
where
    I: Stream,
    C: Accumulate<O>,
    P: Parser<I, O, E>,
    S: Parser<I, O2, E>,
    E: ParseError<I>,
{
    trace("separated0", move |mut i: I| {
        let mut res = C::initial(None);

        match parser.parse_next(i.clone()) {
            Err(ErrMode::Backtrack(_)) => return Ok((i, res)),
            Err(e) => return Err(e),
            Ok((i1, o)) => {
                res.accumulate(o);
                i = i1;
            }
        }

        loop {
            let len = i.eof_offset();
            match sep.parse_next(i.clone()) {
                Err(ErrMode::Backtrack(_)) => return Ok((i, res)),
                Err(e) => return Err(e),
                Ok((i1, _)) => {
                    // infinite loop check: the parser must always consume
                    if i1.eof_offset() == len {
                        return Err(ErrMode::assert(i, "sep parsers must always consume"));
                    }

                    match parser.parse_next(i1.clone()) {
                        Err(ErrMode::Backtrack(_)) => return Ok((i, res)),
                        Err(e) => return Err(e),
                        Ok((i2, o)) => {
                            res.accumulate(o);
                            i = i2;
                        }
                    }
                }
            }
        }
    })
}

/// Alternates between two parsers to produce a list of elements until [`ErrMode::Backtrack`].
///
/// Fails if the element parser does not produce at least one element.$
///
/// This stops when either parser returns [`ErrMode::Backtrack`].  To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Arguments
/// * `sep` Parses the separator between list elements.
/// * `f` Parses the elements of the list.
///
/// # Example
///
#[cfg_attr(not(feature = "std"), doc = "```ignore")]
#[cfg_attr(feature = "std", doc = "```")]
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::separated1;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   separated1("abc", "|").parse_next(s)
/// }
///
/// assert_eq!(parser("abc|abc|abc"), Ok(("", vec!["abc", "abc", "abc"])));
/// assert_eq!(parser("abc123abc"), Ok(("123abc", vec!["abc"])));
/// assert_eq!(parser("abc|def"), Ok(("|def", vec!["abc"])));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("def|abc"), Err(ErrMode::Backtrack(Error::new("def|abc", ErrorKind::Tag))));
/// ```
#[doc(alias = "sep_by1")]
#[doc(alias = "separated_list1")]
pub fn separated1<I, O, C, O2, E, P, S>(mut parser: P, mut sep: S) -> impl Parser<I, C, E>
where
    I: Stream,
    C: Accumulate<O>,
    P: Parser<I, O, E>,
    S: Parser<I, O2, E>,
    E: ParseError<I>,
{
    trace("separated1", move |mut i: I| {
        let mut res = C::initial(None);

        // Parse the first element
        match parser.parse_next(i.clone()) {
            Err(e) => return Err(e),
            Ok((i1, o)) => {
                res.accumulate(o);
                i = i1;
            }
        }

        loop {
            let len = i.eof_offset();
            match sep.parse_next(i.clone()) {
                Err(ErrMode::Backtrack(_)) => return Ok((i, res)),
                Err(e) => return Err(e),
                Ok((i1, _)) => {
                    // infinite loop check: the parser must always consume
                    if i1.eof_offset() == len {
                        return Err(ErrMode::assert(i, "sep parsers must always consume"));
                    }

                    match parser.parse_next(i1.clone()) {
                        Err(ErrMode::Backtrack(_)) => return Ok((i, res)),
                        Err(e) => return Err(e),
                        Ok((i2, o)) => {
                            res.accumulate(o);
                            i = i2;
                        }
                    }
                }
            }
        }
    })
}

/// Alternates between two parsers, merging the results (left associative)
///
/// This stops when either parser returns [`ErrMode::Backtrack`].  To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::separated_foldl1;
/// use winnow::character::dec_int;
///
/// fn parser(s: &str) -> IResult<&str, i32> {
///   separated_foldl1(dec_int, "-", |l, _, r| l - r).parse_next(s)
/// }
///
/// assert_eq!(parser("9-3-5"), Ok(("", 1)));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
/// assert_eq!(parser("def|abc"), Err(ErrMode::Backtrack(Error::new("def|abc", ErrorKind::Slice))));
/// ```
pub fn separated_foldl1<I, O, O2, E, P, S, Op>(
    mut parser: P,
    mut sep: S,
    op: Op,
) -> impl Parser<I, O, E>
where
    I: Stream,
    P: Parser<I, O, E>,
    S: Parser<I, O2, E>,
    E: ParseError<I>,
    Op: Fn(O, O2, O) -> O,
{
    trace("separated_foldl1", move |i: I| {
        let (mut i, mut ol) = parser.parse_next(i)?;

        loop {
            let len = i.eof_offset();
            match sep.parse_next(i.clone()) {
                Err(ErrMode::Backtrack(_)) => return Ok((i, ol)),
                Err(e) => return Err(e),
                Ok((i1, s)) => {
                    // infinite loop check: the parser must always consume
                    if i1.eof_offset() == len {
                        return Err(ErrMode::assert(i, "many parsers must always consume"));
                    }

                    match parser.parse_next(i1.clone()) {
                        Err(ErrMode::Backtrack(_)) => return Ok((i, ol)),
                        Err(e) => return Err(e),
                        Ok((i2, or)) => {
                            ol = op(ol, s, or);
                            i = i2;
                        }
                    }
                }
            }
        }
    })
}

/// Alternates between two parsers, merging the results (right associative)
///
/// This stops when either parser returns [`ErrMode::Backtrack`].  To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Example
///
/// ```
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::separated_foldr1;
/// use winnow::character::dec_uint;
///
/// fn parser(s: &str) -> IResult<&str, u32> {
///   separated_foldr1(dec_uint, "^", |l: u32, _, r: u32| l.pow(r)).parse_next(s)
/// }
///
/// assert_eq!(parser("2^3^2"), Ok(("", 512)));
/// assert_eq!(parser("2"), Ok(("", 2)));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Slice))));
/// assert_eq!(parser("def|abc"), Err(ErrMode::Backtrack(Error::new("def|abc", ErrorKind::Slice))));
/// ```
#[cfg(feature = "alloc")]
pub fn separated_foldr1<I, O, O2, E, P, S, Op>(
    mut parser: P,
    mut sep: S,
    op: Op,
) -> impl Parser<I, O, E>
where
    I: Stream,
    P: Parser<I, O, E>,
    S: Parser<I, O2, E>,
    E: ParseError<I>,
    Op: Fn(O, O2, O) -> O,
{
    trace("separated_foldr1", move |i: I| {
        let (i, ol) = parser.parse_next(i)?;
        let (i, all): (_, crate::lib::std::vec::Vec<(O2, O)>) =
            many0((sep.by_ref(), parser.by_ref())).parse_next(i)?;
        if let Some((s, or)) = all
            .into_iter()
            .rev()
            .reduce(|(sr, or), (sl, ol)| (sl, op(ol, sr, or)))
        {
            let merged = op(ol, s, or);
            Ok((i, merged))
        } else {
            Ok((i, ol))
        }
    })
}

/// Repeats the embedded parser `m..=n` times
///
/// This stops before `n` when the parser returns [`ErrMode::Backtrack`].  To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Arguments
/// * `m` The minimum number of iterations.
/// * `n` The maximum number of iterations.
/// * `f` The parser to apply.
///
/// To recognize a series of tokens, [`Accumulate`] into a `()` and then [`Parser::recognize`].
///
/// **Warning:** If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// # Example
///
#[cfg_attr(not(feature = "std"), doc = "```ignore")]
#[cfg_attr(feature = "std", doc = "```")]
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::many_m_n;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   many_m_n(0, 2, "abc").parse_next(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// assert_eq!(parser("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
/// ```
#[doc(alias = "repeated")]
pub fn many_m_n<I, O, C, E, F>(min: usize, max: usize, mut parse: F) -> impl Parser<I, C, E>
where
    I: Stream,
    C: Accumulate<O>,
    F: Parser<I, O, E>,
    E: ParseError<I>,
{
    trace("many_m_n", move |mut input: I| {
        if min > max {
            return Err(ErrMode::Cut(E::from_error_kind(input, ErrorKind::Many)));
        }

        let mut res = C::initial(Some(min));
        for count in 0..max {
            let len = input.eof_offset();
            match parse.parse_next(input.clone()) {
                Ok((tail, value)) => {
                    // infinite loop check: the parser must always consume
                    if tail.eof_offset() == len {
                        return Err(ErrMode::assert(input, "many parsers must always consume"));
                    }

                    res.accumulate(value);
                    input = tail;
                }
                Err(ErrMode::Backtrack(e)) => {
                    if count < min {
                        return Err(ErrMode::Backtrack(e.append(input, ErrorKind::Many)));
                    } else {
                        return Ok((input, res));
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok((input, res))
    })
}

/// [`Accumulate`] the output of a parser into a container, like `Vec`
///
/// # Arguments
/// * `f` The parser to apply.
/// * `count` How often to apply the parser.
///
/// To recognize a series of tokens, [`Accumulate`] into a `()` and then [`Parser::recognize`].
///
/// # Example
///
#[cfg_attr(not(feature = "std"), doc = "```ignore")]
#[cfg_attr(feature = "std", doc = "```")]
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::count;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   count("abc", 2).parse_next(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Err(ErrMode::Backtrack(Error::new("123", ErrorKind::Tag))));
/// assert_eq!(parser("123123"), Err(ErrMode::Backtrack(Error::new("123123", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
/// ```
#[doc(alias = "skip_counskip_count")]
pub fn count<I, O, C, E, F>(mut f: F, count: usize) -> impl Parser<I, C, E>
where
    I: Stream,
    C: Accumulate<O>,
    F: Parser<I, O, E>,
    E: ParseError<I>,
{
    trace("count", move |i: I| {
        let mut input = i.clone();
        let mut res = C::initial(Some(count));

        for _ in 0..count {
            let input_ = input.clone();
            match f.parse_next(input_) {
                Ok((i, o)) => {
                    res.accumulate(o);
                    input = i;
                }
                Err(e) => {
                    return Err(e.append(i, ErrorKind::Many));
                }
            }
        }

        Ok((input, res))
    })
}

/// Runs the embedded parser repeatedly, filling the given slice with results.
///
/// This parser fails if the input runs out before the given slice is full.
///
/// # Arguments
/// * `f` The parser to apply.
/// * `buf` The slice to fill
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::fill;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, [&str; 2]> {
///   let mut buf = ["", ""];
///   let (rest, ()) = fill("abc", &mut buf).parse_next(s)?;
///   Ok((rest, buf))
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", ["abc", "abc"])));
/// assert_eq!(parser("abc123"), Err(ErrMode::Backtrack(Error::new("123", ErrorKind::Tag))));
/// assert_eq!(parser("123123"), Err(ErrMode::Backtrack(Error::new("123123", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("abcabcabc"), Ok(("abc", ["abc", "abc"])));
/// ```
pub fn fill<'a, I, O, E, F>(mut f: F, buf: &'a mut [O]) -> impl Parser<I, (), E> + 'a
where
    I: Stream + 'a,
    F: Parser<I, O, E> + 'a,
    E: ParseError<I> + 'a,
{
    trace("fill", move |i: I| {
        let mut input = i.clone();

        for elem in buf.iter_mut() {
            let input_ = input.clone();
            match f.parse_next(input_) {
                Ok((i, o)) => {
                    *elem = o;
                    input = i;
                }
                Err(e) => {
                    return Err(e.append(i, ErrorKind::Many));
                }
            }
        }

        Ok((input, ()))
    })
}

/// Repeats the embedded parser, calling `g` to gather the results.
///
/// This stops on [`ErrMode::Backtrack`].  To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Arguments
/// * `f` The parser to apply.
/// * `init` A function returning the initial value.
/// * `g` The function that combines a result of `f` with
///       the current accumulator.
///
/// **Warning:** if the parser passed in accepts empty inputs (like `alpha0` or `digit0`), `many0` will
/// return an error, to prevent going into an infinite loop
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::fold_many0;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   fold_many0(
///     "abc",
///     Vec::new,
///     |mut acc: Vec<_>, item| {
///       acc.push(item);
///       acc
///     }
///   ).parse_next(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// ```
pub fn fold_many0<I, O, E, F, G, H, R>(mut f: F, mut init: H, mut g: G) -> impl Parser<I, R, E>
where
    I: Stream,
    F: Parser<I, O, E>,
    G: FnMut(R, O) -> R,
    H: FnMut() -> R,
    E: ParseError<I>,
{
    trace("fold_many0", move |i: I| {
        let mut res = init();
        let mut input = i;

        loop {
            let i_ = input.clone();
            let len = input.eof_offset();
            match f.parse_next(i_) {
                Ok((i, o)) => {
                    // infinite loop check: the parser must always consume
                    if i.eof_offset() == len {
                        return Err(ErrMode::assert(i, "many parsers must always consume"));
                    }

                    res = g(res, o);
                    input = i;
                }
                Err(ErrMode::Backtrack(_)) => {
                    return Ok((input, res));
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    })
}

/// Repeats the embedded parser, calling `g` to gather the results.
///
/// This stops on [`ErrMode::Backtrack`] if there is at least one result.  To instead chain an error up,
/// see [`cut_err`][crate::combinator::cut_err].
///
/// # Arguments
/// * `f` The parser to apply.
/// * `init` A function returning the initial value.
/// * `g` The function that combines a result of `f` with
///       the current accumulator.
///
/// **Warning:** If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::fold_many1;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   fold_many1(
///     "abc",
///     Vec::new,
///     |mut acc: Vec<_>, item| {
///       acc.push(item);
///       acc
///     }
///   ).parse_next(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Err(ErrMode::Backtrack(Error::new("123123", ErrorKind::Many))));
/// assert_eq!(parser(""), Err(ErrMode::Backtrack(Error::new("", ErrorKind::Many))));
/// ```
pub fn fold_many1<I, O, E, F, G, H, R>(mut f: F, mut init: H, mut g: G) -> impl Parser<I, R, E>
where
    I: Stream,
    F: Parser<I, O, E>,
    G: FnMut(R, O) -> R,
    H: FnMut() -> R,
    E: ParseError<I>,
{
    trace("fold_many1", move |i: I| {
        let _i = i.clone();
        let init = init();
        match f.parse_next(_i) {
            Err(ErrMode::Backtrack(_)) => Err(ErrMode::from_error_kind(i, ErrorKind::Many)),
            Err(e) => Err(e),
            Ok((i1, o1)) => {
                let mut acc = g(init, o1);
                let mut input = i1;

                loop {
                    let _input = input.clone();
                    let len = input.eof_offset();
                    match f.parse_next(_input) {
                        Err(ErrMode::Backtrack(_)) => {
                            break;
                        }
                        Err(e) => return Err(e),
                        Ok((i, o)) => {
                            // infinite loop check: the parser must always consume
                            if i.eof_offset() == len {
                                return Err(ErrMode::assert(i, "many parsers must always consume"));
                            }

                            acc = g(acc, o);
                            input = i;
                        }
                    }
                }

                Ok((input, acc))
            }
        }
    })
}

/// Repeats the embedded parser `m..=n` times, calling `g` to gather the results
///
/// This stops before `n` when the parser returns [`ErrMode::Backtrack`].  To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Arguments
/// * `m` The minimum number of iterations.
/// * `n` The maximum number of iterations.
/// * `f` The parser to apply.
/// * `init` A function returning the initial value.
/// * `g` The function that combines a result of `f` with
///       the current accumulator.
///
/// **Warning:** If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::multi::fold_many_m_n;
/// use winnow::bytes::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   fold_many_m_n(
///     0,
///     2,
///     "abc",
///     Vec::new,
///     |mut acc: Vec<_>, item| {
///       acc.push(item);
///       acc
///     }
///   ).parse_next(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// assert_eq!(parser("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
/// ```
pub fn fold_many_m_n<I, O, E, F, G, H, R>(
    min: usize,
    max: usize,
    mut parse: F,
    mut init: H,
    mut fold: G,
) -> impl Parser<I, R, E>
where
    I: Stream,
    F: Parser<I, O, E>,
    G: FnMut(R, O) -> R,
    H: FnMut() -> R,
    E: ParseError<I>,
{
    trace("fold_many_m_n", move |mut input: I| {
        if min > max {
            return Err(ErrMode::Cut(E::from_error_kind(input, ErrorKind::Many)));
        }

        let mut acc = init();
        for count in 0..max {
            let len = input.eof_offset();
            match parse.parse_next(input.clone()) {
                Ok((tail, value)) => {
                    // infinite loop check: the parser must always consume
                    if tail.eof_offset() == len {
                        return Err(ErrMode::assert(input, "many parsers must always consume"));
                    }

                    acc = fold(acc, value);
                    input = tail;
                }
                //FInputXMError: handle failure properly
                Err(ErrMode::Backtrack(err)) => {
                    if count < min {
                        return Err(ErrMode::Backtrack(err.append(input, ErrorKind::Many)));
                    } else {
                        break;
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok((input, acc))
    })
}

/// Gets a number from the parser and returns a
/// subslice of the input of that size.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Arguments
/// * `f` The parser to apply.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ErrorKind, error::Needed, stream::Partial};
/// # use winnow::prelude::*;
/// use winnow::Bytes;
/// use winnow::number::be_u16;
/// use winnow::multi::length_data;
/// use winnow::bytes::tag;
///
/// type Stream<'i> = Partial<&'i Bytes>;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Partial::new(Bytes::new(b))
/// }
///
/// fn parser(s: Stream<'_>) -> IResult<Stream<'_>, &[u8]> {
///   length_data(be_u16).parse_next(s)
/// }
///
/// assert_eq!(parser(stream(b"\x00\x03abcefg")), Ok((stream(&b"efg"[..]), &b"abc"[..])));
/// assert_eq!(parser(stream(b"\x00\x03a")), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
pub fn length_data<I, N, E, F>(mut f: F) -> impl Parser<I, <I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    N: ToUsize,
    F: Parser<I, N, E>,
    E: ParseError<I>,
{
    trace("length_data", move |i: I| {
        let (i, length) = f.parse_next(i)?;

        crate::bytes::take(length).parse_next(i)
    })
}

/// Gets a number from the first parser,
/// takes a subslice of the input of that size,
/// then applies the second parser on that subslice.
/// If the second parser returns `Incomplete`,
/// `length_value` will return an error.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *Partial version*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Arguments
/// * `f` The parser to apply.
/// * `g` The parser to apply on the subslice.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, stream::{Partial, StreamIsPartial}};
/// # use winnow::prelude::*;
/// use winnow::Bytes;
/// use winnow::number::be_u16;
/// use winnow::multi::length_value;
/// use winnow::bytes::tag;
///
/// type Stream<'i> = Partial<&'i Bytes>;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Partial::new(Bytes::new(b))
/// }
///
/// fn complete_stream(b: &[u8]) -> Stream<'_> {
///     let mut p = Partial::new(Bytes::new(b));
///     let _ = p.complete();
///     p
/// }
///
/// fn parser(s: Stream<'_>) -> IResult<Stream<'_>, &[u8]> {
///   length_value(be_u16, "abc").parse_next(s)
/// }
///
/// assert_eq!(parser(stream(b"\x00\x03abcefg")), Ok((stream(&b"efg"[..]), &b"abc"[..])));
/// assert_eq!(parser(stream(b"\x00\x03123123")), Err(ErrMode::Backtrack(Error::new(complete_stream(&b"123"[..]), ErrorKind::Tag))));
/// assert_eq!(parser(stream(b"\x00\x03a")), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
pub fn length_value<I, O, N, E, F, G>(mut f: F, mut g: G) -> impl Parser<I, O, E>
where
    I: StreamIsPartial,
    I: Stream + UpdateSlice,
    N: ToUsize,
    F: Parser<I, N, E>,
    G: Parser<I, O, E>,
    E: ParseError<I>,
{
    trace("length_value", move |i: I| {
        let (i, data) = length_data(f.by_ref()).parse_next(i)?;
        let mut data = I::update_slice(i.clone(), data);
        let _ = data.complete();
        let (_, o) = g.by_ref().complete_err().parse_next(data)?;
        Ok((i, o))
    })
}

/// Gets a number from the first parser,
/// then applies the second parser that many times.
///
/// # Arguments
/// * `f` The parser to apply to obtain the count.
/// * `g` The parser to apply repeatedly.
///
/// # Example
///
#[cfg_attr(not(feature = "std"), doc = "```ignore")]
#[cfg_attr(feature = "std", doc = "```")]
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::Bytes;
/// use winnow::number::u8;
/// use winnow::multi::length_count;
/// use winnow::bytes::tag;
///
/// type Stream<'i> = &'i Bytes;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Bytes::new(b)
/// }
///
/// fn parser(s: Stream<'_>) -> IResult<Stream<'_>, Vec<&[u8]>> {
///   length_count(u8.map(|i| {
///      println!("got number: {}", i);
///      i
///   }), "abc").parse_next(s)
/// }
///
/// assert_eq!(parser(stream(b"\x02abcabcabc")), Ok((stream(b"abc"), vec![&b"abc"[..], &b"abc"[..]])));
/// assert_eq!(parser(stream(b"\x03123123123")), Err(ErrMode::Backtrack(Error::new(stream(b"123123123"), ErrorKind::Tag))));
/// ```
pub fn length_count<I, O, C, N, E, F, G>(mut f: F, mut g: G) -> impl Parser<I, C, E>
where
    I: Stream,
    N: ToUsize,
    C: Accumulate<O>,
    F: Parser<I, N, E>,
    G: Parser<I, O, E>,
    E: ParseError<I>,
{
    trace("length_count", move |i: I| {
        let (i, n) = f.parse_next(i)?;
        let n = n.to_usize();
        count(g.by_ref(), n).parse_next(i)
    })
}
