use std::borrow::Cow;
use std::char;
use std::ops::RangeInclusive;

use nom8::branch::alt;
use nom8::bytes::any;
use nom8::bytes::none_of;
use nom8::bytes::one_of;
use nom8::bytes::tag;
use nom8::bytes::take_while;
use nom8::bytes::take_while1;
use nom8::bytes::take_while_m_n;
use nom8::combinator::cut;
use nom8::combinator::fail;
use nom8::combinator::opt;
use nom8::combinator::peek;
use nom8::combinator::success;
use nom8::multi::many0_count;
use nom8::multi::many1_count;
use nom8::sequence::delimited;
use nom8::sequence::preceded;
use nom8::sequence::terminated;

use crate::parser::errors::CustomError;
use crate::parser::numbers::HEXDIG;
use crate::parser::prelude::*;
use crate::parser::trivia::{from_utf8_unchecked, newline, ws, ws_newlines, NON_ASCII, WSCHAR};

// ;; String

// string = ml-basic-string / basic-string / ml-literal-string / literal-string
pub(crate) fn string(input: Input<'_>) -> IResult<Input<'_>, Cow<'_, str>, ParserError<'_>> {
    alt((
        ml_basic_string,
        basic_string,
        ml_literal_string,
        literal_string.map(Cow::Borrowed),
    ))
    .parse(input)
}

// ;; Basic String

// basic-string = quotation-mark *basic-char quotation-mark
pub(crate) fn basic_string(input: Input<'_>) -> IResult<Input<'_>, Cow<'_, str>, ParserError<'_>> {
    let (mut input, _) = one_of(QUOTATION_MARK).parse(input)?;

    let mut c = Cow::Borrowed("");
    if let Some((i, ci)) = ok_error(basic_chars.parse(input))? {
        input = i;
        c = ci;
    }
    while let Some((i, ci)) = ok_error(basic_chars.parse(input))? {
        input = i;
        c.to_mut().push_str(&ci);
    }

    let (input, _) = cut(one_of(QUOTATION_MARK))
        .context(Context::Expression("basic string"))
        .parse(input)?;

    Ok((input, c))
}

// quotation-mark = %x22            ; "
pub(crate) const QUOTATION_MARK: u8 = b'"';

// basic-char = basic-unescaped / escaped
fn basic_chars(input: Input<'_>) -> IResult<Input<'_>, Cow<'_, str>, ParserError<'_>> {
    alt((
        // Deviate from the official grammar by batching the unescaped chars so we build a string a
        // chunk at a time, rather than a `char` at a time.
        take_while1(BASIC_UNESCAPED)
            .map_res(std::str::from_utf8)
            .map(Cow::Borrowed),
        escaped.map(|c| Cow::Owned(String::from(c))),
    ))
    .parse(input)
}

// basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
pub(crate) const BASIC_UNESCAPED: (
    (u8, u8),
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (WSCHAR, 0x21, 0x23..=0x5B, 0x5D..=0x7E, NON_ASCII);

// escaped = escape escape-seq-char
fn escaped(input: Input<'_>) -> IResult<Input<'_>, char, ParserError<'_>> {
    preceded(ESCAPE, escape_seq_char).parse(input)
}

// escape = %x5C                    ; \
pub(crate) const ESCAPE: u8 = b'\\';

// escape-seq-char =  %x22         ; "    quotation mark  U+0022
// escape-seq-char =/ %x5C         ; \    reverse solidus U+005C
// escape-seq-char =/ %x62         ; b    backspace       U+0008
// escape-seq-char =/ %x66         ; f    form feed       U+000C
// escape-seq-char =/ %x6E         ; n    line feed       U+000A
// escape-seq-char =/ %x72         ; r    carriage return U+000D
// escape-seq-char =/ %x74         ; t    tab             U+0009
// escape-seq-char =/ %x75 4HEXDIG ; uXXXX                U+XXXX
// escape-seq-char =/ %x55 8HEXDIG ; UXXXXXXXX            U+XXXXXXXX
fn escape_seq_char(input: Input<'_>) -> IResult<Input<'_>, char, ParserError<'_>> {
    dispatch! {any;
        b'b' => success('\u{8}'),
        b'f' => success('\u{c}'),
        b'n' => success('\n'),
        b'r' => success('\r'),
        b't' => success('\t'),
        b'u' => cut(hexescape::<4>).context(Context::Expression("unicode 4-digit hex code")),
        b'U' => cut(hexescape::<8>).context(Context::Expression("unicode 8-digit hex code")),
        b'\\' => success('\\'),
        b'"' => success('"'),
        _ => {
            cut(fail::<_, char, _>)
            .context(Context::Expression("escape sequence"))
            .context(Context::Expected(ParserValue::CharLiteral('b')))
            .context(Context::Expected(ParserValue::CharLiteral('f')))
            .context(Context::Expected(ParserValue::CharLiteral('n')))
            .context(Context::Expected(ParserValue::CharLiteral('r')))
            .context(Context::Expected(ParserValue::CharLiteral('t')))
            .context(Context::Expected(ParserValue::CharLiteral('u')))
            .context(Context::Expected(ParserValue::CharLiteral('U')))
            .context(Context::Expected(ParserValue::CharLiteral('\\')))
            .context(Context::Expected(ParserValue::CharLiteral('"')))
        }
    }
    .parse(input)
}

pub(crate) fn hexescape<const N: usize>(
    input: Input<'_>,
) -> IResult<Input<'_>, char, ParserError<'_>> {
    take_while_m_n(0, N, HEXDIG)
        .verify(|b: &[u8]| b.len() == N)
        .map(|b: &[u8]| unsafe { from_utf8_unchecked(b, "`is_ascii_digit` filters out on-ASCII") })
        .map_opt(|s| u32::from_str_radix(s, 16).ok())
        .map_res(|h| char::from_u32(h).ok_or(CustomError::OutOfRange))
        .parse(input)
}

// ;; Multiline Basic String

// ml-basic-string = ml-basic-string-delim [ newline ] ml-basic-body
//                   ml-basic-string-delim
fn ml_basic_string(input: Input<'_>) -> IResult<Input<'_>, Cow<'_, str>, ParserError<'_>> {
    delimited(
        ML_BASIC_STRING_DELIM,
        preceded(opt(newline), cut(ml_basic_body)),
        cut(ML_BASIC_STRING_DELIM),
    )
    .context(Context::Expression("multiline basic string"))
    .parse(input)
}

// ml-basic-string-delim = 3quotation-mark
pub(crate) const ML_BASIC_STRING_DELIM: &[u8] = b"\"\"\"";

// ml-basic-body = *mlb-content *( mlb-quotes 1*mlb-content ) [ mlb-quotes ]
fn ml_basic_body(mut input: Input<'_>) -> IResult<Input<'_>, Cow<'_, str>, ParserError<'_>> {
    let mut c = Cow::Borrowed("");
    if let Some((i, ci)) = ok_error(mlb_content.parse(input))? {
        input = i;
        c = ci;
    }
    while let Some((i, ci)) = ok_error(mlb_content.parse(input))? {
        input = i;
        c.to_mut().push_str(&ci);
    }

    while let Some((i, qi)) = ok_error(mlb_quotes(none_of(b'\"').value(())).parse(input))? {
        if let Some((i, ci)) = ok_error(mlb_content.parse(i))? {
            input = i;
            c.to_mut().push_str(qi);
            c.to_mut().push_str(&ci);
            while let Some((i, ci)) = ok_error(mlb_content.parse(input))? {
                input = i;
                c.to_mut().push_str(&ci);
            }
        } else {
            break;
        }
    }

    if let Some((i, qi)) = ok_error(mlb_quotes(tag(ML_BASIC_STRING_DELIM).value(())).parse(input))?
    {
        input = i;
        c.to_mut().push_str(qi);
    }

    Ok((input, c))
}

// mlb-content = mlb-char / newline / mlb-escaped-nl
// mlb-char = mlb-unescaped / escaped
fn mlb_content(input: Input<'_>) -> IResult<Input<'_>, Cow<'_, str>, ParserError<'_>> {
    alt((
        // Deviate from the official grammar by batching the unescaped chars so we build a string a
        // chunk at a time, rather than a `char` at a time.
        take_while1(MLB_UNESCAPED)
            .map_res(std::str::from_utf8)
            .map(Cow::Borrowed),
        // Order changed fromg grammar so `escaped` can more easily `cut` on bad escape sequences
        mlb_escaped_nl.map(|_| Cow::Borrowed("")),
        escaped.map(|c| Cow::Owned(String::from(c))),
        newline.map(|_| Cow::Borrowed("\n")),
    ))
    .parse(input)
}

// mlb-quotes = 1*2quotation-mark
fn mlb_quotes<'i>(
    mut term: impl nom8::Parser<Input<'i>, (), ParserError<'i>>,
) -> impl FnMut(Input<'i>) -> IResult<Input<'i>, &str, ParserError<'i>> {
    move |input| {
        let res = terminated(b"\"\"", peek(term.by_ref()))
            .map(|b| unsafe { from_utf8_unchecked(b, "`bytes` out non-ASCII") })
            .parse(input);

        match res {
            Err(nom8::Err::Error(_)) => terminated(b"\"", peek(term.by_ref()))
                .map(|b| unsafe { from_utf8_unchecked(b, "`bytes` out non-ASCII") })
                .parse(input),
            res => res,
        }
    }
}

// mlb-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
pub(crate) const MLB_UNESCAPED: (
    (u8, u8),
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (WSCHAR, 0x21, 0x23..=0x5B, 0x5D..=0x7E, NON_ASCII);

// mlb-escaped-nl = escape ws newline *( wschar / newline
// When the last non-whitespace character on a line is a \,
// it will be trimmed along with all whitespace
// (including newlines) up to the next non-whitespace
// character or closing delimiter.
fn mlb_escaped_nl(input: Input<'_>) -> IResult<Input<'_>, (), ParserError<'_>> {
    many1_count((ESCAPE, ws, ws_newlines))
        .value(())
        .parse(input)
}

// ;; Literal String

// literal-string = apostrophe *literal-char apostrophe
pub(crate) fn literal_string(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    delimited(APOSTROPHE, cut(take_while(LITERAL_CHAR)), cut(APOSTROPHE))
        .map_res(std::str::from_utf8)
        .context(Context::Expression("literal string"))
        .parse(input)
}

// apostrophe = %x27 ; ' apostrophe
pub(crate) const APOSTROPHE: u8 = b'\'';

// literal-char = %x09 / %x20-26 / %x28-7E / non-ascii
pub(crate) const LITERAL_CHAR: (
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (0x9, 0x20..=0x26, 0x28..=0x7E, NON_ASCII);

// ;; Multiline Literal String

// ml-literal-string = ml-literal-string-delim [ newline ] ml-literal-body
//                     ml-literal-string-delim
fn ml_literal_string(input: Input<'_>) -> IResult<Input<'_>, Cow<'_, str>, ParserError<'_>> {
    delimited(
        (ML_LITERAL_STRING_DELIM, opt(newline)),
        cut(ml_literal_body.map(|t| {
            if t.contains("\r\n") {
                Cow::Owned(t.replace("\r\n", "\n"))
            } else {
                Cow::Borrowed(t)
            }
        })),
        cut(ML_LITERAL_STRING_DELIM),
    )
    .context(Context::Expression("multiline literal string"))
    .parse(input)
}

// ml-literal-string-delim = 3apostrophe
pub(crate) const ML_LITERAL_STRING_DELIM: &[u8] = b"'''";

// ml-literal-body = *mll-content *( mll-quotes 1*mll-content ) [ mll-quotes ]
fn ml_literal_body(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    (
        many0_count(mll_content),
        many0_count((
            mll_quotes(none_of(APOSTROPHE).value(())),
            many1_count(mll_content),
        )),
        opt(mll_quotes(tag(ML_LITERAL_STRING_DELIM).value(()))),
    )
        .recognize()
        .map_res(std::str::from_utf8)
        .parse(input)
}

// mll-content = mll-char / newline
fn mll_content(input: Input<'_>) -> IResult<Input<'_>, u8, ParserError<'_>> {
    alt((one_of(MLL_CHAR), newline)).parse(input)
}

// mll-char = %x09 / %x20-26 / %x28-7E / non-ascii
const MLL_CHAR: (
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (0x9, 0x20..=0x26, 0x28..=0x7E, NON_ASCII);

// mll-quotes = 1*2apostrophe
fn mll_quotes<'i>(
    mut term: impl nom8::Parser<Input<'i>, (), ParserError<'i>>,
) -> impl FnMut(Input<'i>) -> IResult<Input<'i>, &str, ParserError<'i>> {
    move |input| {
        let res = terminated(b"''", peek(term.by_ref()))
            .map(|b| unsafe { from_utf8_unchecked(b, "`bytes` out non-ASCII") })
            .parse(input);

        match res {
            Err(nom8::Err::Error(_)) => terminated(b"'", peek(term.by_ref()))
                .map(|b| unsafe { from_utf8_unchecked(b, "`bytes` out non-ASCII") })
                .parse(input),
            res => res,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic_string() {
        let input =
            r#""I'm a string. \"You can quote me\". Name\tJos\u00E9\nLocation\tSF. \U0002070E""#;
        let expected = "I\'m a string. \"You can quote me\". Name\tJosé\nLocation\tSF. \u{2070E}";
        let parsed = string.parse(new_input(input)).finish();
        assert_eq!(parsed.as_deref(), Ok(expected), "Parsing {input:?}");
    }

    #[test]
    fn ml_basic_string() {
        let cases = [
            (
                r#""""
Roses are red
Violets are blue""""#,
                r#"Roses are red
Violets are blue"#,
            ),
            (r#"""" \""" """"#, " \"\"\" "),
            (r#"""" \\""""#, " \\"),
        ];

        for &(input, expected) in &cases {
            let parsed = string.parse(new_input(input)).finish();
            assert_eq!(parsed.as_deref(), Ok(expected), "Parsing {input:?}");
        }

        let invalid_cases = [r#""""  """#, r#""""  \""""#];

        for input in &invalid_cases {
            let parsed = string.parse(new_input(input)).finish();
            assert!(parsed.is_err());
        }
    }

    #[test]
    fn ml_basic_string_escape_ws() {
        let inputs = [
            r#""""
The quick brown \


  fox jumps over \
    the lazy dog.""""#,
            r#""""\
       The quick brown \
       fox jumps over \
       the lazy dog.\
       """"#,
        ];
        for input in &inputs {
            let expected = "The quick brown fox jumps over the lazy dog.";
            let parsed = string.parse(new_input(input)).finish();
            assert_eq!(parsed.as_deref(), Ok(expected), "Parsing {input:?}");
        }
        let empties = [
            r#""""\
       """"#,
            r#""""
\
  \
""""#,
        ];
        for input in &empties {
            let expected = "";
            let parsed = string.parse(new_input(input)).finish();
            assert_eq!(parsed.as_deref(), Ok(expected), "Parsing {input:?}");
        }
    }

    #[test]
    fn literal_string() {
        let inputs = [
            r#"'C:\Users\nodejs\templates'"#,
            r#"'\\ServerX\admin$\system32\'"#,
            r#"'Tom "Dubs" Preston-Werner'"#,
            r#"'<\i\c*\s*>'"#,
        ];

        for input in &inputs {
            let expected = &input[1..input.len() - 1];
            let parsed = string.parse(new_input(input)).finish();
            assert_eq!(parsed.as_deref(), Ok(expected), "Parsing {input:?}");
        }
    }

    #[test]
    fn ml_literal_string() {
        let inputs = [
            r#"'''I [dw]on't need \d{2} apples'''"#,
            r#"''''one_quote''''"#,
        ];
        for input in &inputs {
            let expected = &input[3..input.len() - 3];
            let parsed = string.parse(new_input(input)).finish();
            assert_eq!(parsed.as_deref(), Ok(expected), "Parsing {input:?}");
        }

        let input = r#"'''
The first newline is
trimmed in raw strings.
   All other whitespace
   is preserved.
'''"#;
        let expected = &input[4..input.len() - 3];
        let parsed = string.parse(new_input(input)).finish();
        assert_eq!(parsed.as_deref(), Ok(expected), "Parsing {input:?}");
    }
}
