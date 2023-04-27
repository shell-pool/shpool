use std::collections::HashMap;
use std::str;

use winnow::prelude::*;
use winnow::{
    branch::alt,
    bytes::{any, none_of, tag, take, take_while0},
    character::float,
    combinator::cut_err,
    error::{ContextError, ParseError},
    multi::{fold_many0, separated0},
    sequence::{delimited, preceded, separated_pair, terminated},
};

use crate::json::JsonValue;

pub type Stream<'i> = &'i str;

/// The root element of a JSON parser is any value
///
/// A parser has the following signature:
/// `Stream -> IResult<Stream, Output, Error>`, with `IResult` defined as:
/// `type IResult<I, O, E = (I, ErrorKind)> = Result<(I, O), Err<E>>;`
///
/// most of the times you can ignore the error type and use the default (but this
/// examples shows custom error types later on!)
///
/// Here we use `&str` as input type, but parsers can be generic over
/// the input type, work directly with `&[u8]`, or any other type that
/// implements the required traits.
pub fn json<'i, E: ParseError<Stream<'i>> + ContextError<Stream<'i>, &'static str>>(
    input: Stream<'i>,
) -> IResult<Stream<'i>, JsonValue, E> {
    delimited(ws, json_value, ws)(input)
}

/// `alt` is a combinator that tries multiple parsers one by one, until
/// one of them succeeds
fn json_value<'i, E: ParseError<Stream<'i>> + ContextError<Stream<'i>, &'static str>>(
    input: Stream<'i>,
) -> IResult<Stream<'i>, JsonValue, E> {
    // `alt` combines the each value parser. It returns the result of the first
    // successful parser, or an error
    alt((
        null.value(JsonValue::Null),
        boolean.map(JsonValue::Boolean),
        string.map(JsonValue::Str),
        float.map(JsonValue::Num),
        array.map(JsonValue::Array),
        object.map(JsonValue::Object),
    ))(input)
}

/// `tag(string)` generates a parser that recognizes the argument string.
///
/// This also shows returning a sub-slice of the original input
fn null<'i, E: ParseError<Stream<'i>>>(input: Stream<'i>) -> IResult<Stream<'i>, &'i str, E> {
    // This is a parser that returns `"null"` if it sees the string "null", and
    // an error otherwise
    tag("null").parse_next(input)
}

/// We can combine `tag` with other functions, like `value` which returns a given constant value on
/// success.
fn boolean<'i, E: ParseError<Stream<'i>>>(input: Stream<'i>) -> IResult<Stream<'i>, bool, E> {
    // This is a parser that returns `true` if it sees the string "true", and
    // an error otherwise
    let parse_true = tag("true").value(true);

    // This is a parser that returns `false` if it sees the string "false", and
    // an error otherwise
    let parse_false = tag("false").value(false);

    alt((parse_true, parse_false))(input)
}

/// This parser gathers all `char`s up into a `String`with a parse to recognize the double quote
/// character, before the string (using `preceded`) and after the string (using `terminated`).
fn string<'i, E: ParseError<Stream<'i>> + ContextError<Stream<'i>, &'static str>>(
    input: Stream<'i>,
) -> IResult<Stream<'i>, String, E> {
    preceded(
        '\"',
        // `cut_err` transforms an `ErrMode::Backtrack(e)` to `ErrMode::Cut(e)`, signaling to
        // combinators like  `alt` that they should not try other parsers. We were in the
        // right branch (since we found the `"` character) but encountered an error when
        // parsing the string
        cut_err(terminated(
            fold_many0(character, String::new, |mut string, c| {
                string.push(c);
                string
            }),
            '\"',
        )),
    )
    // `context` lets you add a static string to errors to provide more information in the
    // error chain (to indicate which parser had an error)
    .context("string")
    .parse_next(input)
}

/// You can mix the above declarative parsing with an imperative style to handle more unique cases,
/// like escaping
fn character<'i, E: ParseError<Stream<'i>>>(input: Stream<'i>) -> IResult<Stream<'i>, char, E> {
    let (input, c) = none_of("\"")(input)?;
    if c == '\\' {
        alt((
            any.verify_map(|c| {
                Some(match c {
                    '"' | '\\' | '/' => c,
                    'b' => '\x08',
                    'f' => '\x0C',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => return None,
                })
            }),
            preceded('u', unicode_escape),
        ))(input)
    } else {
        Ok((input, c))
    }
}

fn unicode_escape<'i, E: ParseError<Stream<'i>>>(
    input: Stream<'i>,
) -> IResult<Stream<'i>, char, E> {
    alt((
        // Not a surrogate
        u16_hex
            .verify(|cp| !(0xD800..0xE000).contains(cp))
            .map(|cp| cp as u32),
        // See https://en.wikipedia.org/wiki/UTF-16#Code_points_from_U+010000_to_U+10FFFF for details
        separated_pair(u16_hex, "\\u", u16_hex)
            .verify(|(high, low)| (0xD800..0xDC00).contains(high) && (0xDC00..0xE000).contains(low))
            .map(|(high, low)| {
                let high_ten = (high as u32) - 0xD800;
                let low_ten = (low as u32) - 0xDC00;
                (high_ten << 10) + low_ten + 0x10000
            }),
    ))
    .verify_map(
        // Could be probably replaced with .unwrap() or _unchecked due to the verify checks
        std::char::from_u32,
    )
    .parse_next(input)
}

fn u16_hex<'i, E: ParseError<Stream<'i>>>(input: Stream<'i>) -> IResult<Stream<'i>, u16, E> {
    take(4usize)
        .verify_map(|s| u16::from_str_radix(s, 16).ok())
        .parse_next(input)
}

/// Some combinators, like `separated0` or `many0`, will call a parser repeatedly,
/// accumulating results in a `Vec`, until it encounters an error.
/// If you want more control on the parser application, check out the `iterator`
/// combinator (cf `examples/iterator.rs`)
fn array<'i, E: ParseError<Stream<'i>> + ContextError<Stream<'i>, &'static str>>(
    input: Stream<'i>,
) -> IResult<Stream<'i>, Vec<JsonValue>, E> {
    preceded(
        ('[', ws),
        cut_err(terminated(separated0(json_value, (ws, ',', ws)), (ws, ']'))),
    )
    .context("array")
    .parse_next(input)
}

fn object<'i, E: ParseError<Stream<'i>> + ContextError<Stream<'i>, &'static str>>(
    input: Stream<'i>,
) -> IResult<Stream<'i>, HashMap<String, JsonValue>, E> {
    preceded(
        ('{', ws),
        cut_err(terminated(separated0(key_value, (ws, ',', ws)), (ws, '}'))),
    )
    .context("object")
    .parse_next(input)
}

fn key_value<'i, E: ParseError<Stream<'i>> + ContextError<Stream<'i>, &'static str>>(
    input: Stream<'i>,
) -> IResult<Stream<'i>, (String, JsonValue), E> {
    separated_pair(string, cut_err((ws, ':', ws)), json_value)(input)
}

/// Parser combinators are constructed from the bottom up:
/// first we write parsers for the smallest elements (here a space character),
/// then we'll combine them in larger parsers
fn ws<'i, E: ParseError<Stream<'i>>>(input: Stream<'i>) -> IResult<Stream<'i>, &'i str, E> {
    // Combinators like `take_while0` return a function. That function is the
    // parser,to which we can pass the input
    take_while0(WS)(input)
}

const WS: &str = " \t\r\n";

#[cfg(test)]
mod test {
    #[allow(clippy::useless_attribute)]
    #[allow(dead_code)] // its dead for benches
    use super::*;

    #[allow(clippy::useless_attribute)]
    #[allow(dead_code)] // its dead for benches
    type Error<'i> = winnow::error::Error<&'i str>;

    #[test]
    fn json_string() {
        assert_eq!(string::<Error<'_>>("\"\""), Ok(("", "".to_string())));
        assert_eq!(string::<Error<'_>>("\"abc\""), Ok(("", "abc".to_string())));
        assert_eq!(
            string::<Error<'_>>("\"abc\\\"\\\\\\/\\b\\f\\n\\r\\t\\u0001\\u2014\u{2014}def\""),
            Ok(("", "abc\"\\/\x08\x0C\n\r\t\x01——def".to_string())),
        );
        assert_eq!(
            string::<Error<'_>>("\"\\uD83D\\uDE10\""),
            Ok(("", "😐".to_string()))
        );

        assert!(string::<Error<'_>>("\"").is_err());
        assert!(string::<Error<'_>>("\"abc").is_err());
        assert!(string::<Error<'_>>("\"\\\"").is_err());
        assert!(string::<Error<'_>>("\"\\u123\"").is_err());
        assert!(string::<Error<'_>>("\"\\uD800\"").is_err());
        assert!(string::<Error<'_>>("\"\\uD800\\uD800\"").is_err());
        assert!(string::<Error<'_>>("\"\\uDC00\"").is_err());
    }

    #[test]
    fn json_object() {
        use JsonValue::{Num, Object, Str};

        let input = r#"{"a":42,"b":"x"}"#;

        let expected = Object(
            vec![
                ("a".to_string(), Num(42.0)),
                ("b".to_string(), Str("x".to_string())),
            ]
            .into_iter()
            .collect(),
        );

        assert_eq!(json::<Error<'_>>(input), Ok(("", expected)));
    }

    #[test]
    fn json_array() {
        use JsonValue::{Array, Num, Str};

        let input = r#"[42,"x"]"#;

        let expected = Array(vec![Num(42.0), Str("x".to_string())]);

        assert_eq!(json::<Error<'_>>(input), Ok(("", expected)));
    }

    #[test]
    fn json_whitespace() {
        use JsonValue::{Array, Boolean, Null, Num, Object, Str};

        let input = r#"
  {
    "null" : null,
    "true"  :true ,
    "false":  false  ,
    "number" : 123e4 ,
    "string" : " abc 123 " ,
    "array" : [ false , 1 , "two" ] ,
    "object" : { "a" : 1.0 , "b" : "c" } ,
    "empty_array" : [  ] ,
    "empty_object" : {   }
  }
  "#;

        assert_eq!(
            json::<Error<'_>>(input),
            Ok((
                "",
                Object(
                    vec![
                        ("null".to_string(), Null),
                        ("true".to_string(), Boolean(true)),
                        ("false".to_string(), Boolean(false)),
                        ("number".to_string(), Num(123e4)),
                        ("string".to_string(), Str(" abc 123 ".to_string())),
                        (
                            "array".to_string(),
                            Array(vec![Boolean(false), Num(1.0), Str("two".to_string())])
                        ),
                        (
                            "object".to_string(),
                            Object(
                                vec![
                                    ("a".to_string(), Num(1.0)),
                                    ("b".to_string(), Str("c".to_string())),
                                ]
                                .into_iter()
                                .collect()
                            )
                        ),
                        ("empty_array".to_string(), Array(vec![]),),
                        ("empty_object".to_string(), Object(HashMap::new()),),
                    ]
                    .into_iter()
                    .collect()
                )
            ))
        );
    }
}
