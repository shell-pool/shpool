use winnow::combinator::cut_err;
use winnow::combinator::opt;
use winnow::multi::separated1;
use winnow::sequence::delimited;

use crate::parser::trivia::ws_comment_newline;
use crate::parser::value::value;
use crate::{Array, Item, RawString, Value};

use crate::parser::prelude::*;

// ;; Array

// array = array-open array-values array-close
pub(crate) fn array(
    check: RecursionCheck,
) -> impl FnMut(Input<'_>) -> IResult<Input<'_>, Array, ParserError<'_>> {
    move |input| {
        delimited(
            ARRAY_OPEN,
            cut_err(array_values(check)),
            cut_err(ARRAY_CLOSE)
                .context(Context::Expression("array"))
                .context(Context::Expected(ParserValue::CharLiteral(']'))),
        )
        .parse_next(input)
    }
}

// note: we're omitting ws and newlines here, because
// they should be part of the formatted values
// array-open  = %x5B ws-newline  ; [
pub(crate) const ARRAY_OPEN: u8 = b'[';
// array-close = ws-newline %x5D  ; ]
const ARRAY_CLOSE: u8 = b']';
// array-sep = ws %x2C ws  ; , Comma
const ARRAY_SEP: u8 = b',';

// note: this rule is modified
// array-values = [ ( array-value array-sep array-values ) /
//                  array-value / ws-comment-newline ]
pub(crate) fn array_values(
    check: RecursionCheck,
) -> impl FnMut(Input<'_>) -> IResult<Input<'_>, Array, ParserError<'_>> {
    move |input| {
        let check = check.recursing(input)?;
        (
            opt(
                (separated1(array_value(check), ARRAY_SEP), opt(ARRAY_SEP)).map(
                    |(v, trailing): (Vec<Value>, Option<u8>)| {
                        (
                            Array::with_vec(v.into_iter().map(Item::Value).collect()),
                            trailing.is_some(),
                        )
                    },
                ),
            ),
            ws_comment_newline.span(),
        )
            .map_res::<_, _, std::str::Utf8Error>(|(array, trailing)| {
                let (mut array, comma) = array.unwrap_or_default();
                array.set_trailing_comma(comma);
                array.set_trailing(RawString::with_span(trailing));
                Ok(array)
            })
            .parse_next(input)
    }
}

pub(crate) fn array_value(
    check: RecursionCheck,
) -> impl FnMut(Input<'_>) -> IResult<Input<'_>, Value, ParserError<'_>> {
    move |input| {
        (
            ws_comment_newline.span(),
            value(check),
            ws_comment_newline.span(),
        )
            .map(|(ws1, v, ws2)| v.decorated(RawString::with_span(ws1), RawString::with_span(ws2)))
            .parse_next(input)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn arrays() {
        let inputs = [
            r#"[]"#,
            r#"[   ]"#,
            r#"[
  1, 2, 3
]"#,
            r#"[
  1,
  2, # this is ok
]"#,
            r#"[# comment
# comment2


   ]"#,
            r#"[# comment
# comment2
      1

#sd
,
# comment3

   ]"#,
            r#"[1]"#,
            r#"[1,]"#,
            r#"[ "all", 'strings', """are the same""", '''type''']"#,
            r#"[ 100, -2,]"#,
            r#"[1, 2, 3]"#,
            r#"[1.1, 2.1, 3.1]"#,
            r#"["a", "b", "c"]"#,
            r#"[ [ 1, 2 ], [3, 4, 5] ]"#,
            r#"[ [ 1, 2 ], ["a", "b", "c"] ]"#,
            r#"[ { x = 1, a = "2" }, {a = "a",b = "b",     c =    "c"} ]"#,
        ];
        for input in inputs {
            dbg!(input);
            let mut parsed = array(Default::default())
                .parse_next(new_input(input))
                .finish();
            if let Ok(parsed) = &mut parsed {
                parsed.despan(input);
            }
            assert_eq!(parsed.map(|a| a.to_string()), Ok(input.to_owned()));
        }
    }

    #[test]
    fn invalid_arrays() {
        let invalid_inputs = [r#"["#, r#"[,]"#, r#"[,2]"#, r#"[1e165,,]"#];
        for input in invalid_inputs {
            dbg!(input);
            let mut parsed = array(Default::default())
                .parse_next(new_input(input))
                .finish();
            if let Ok(parsed) = &mut parsed {
                parsed.despan(input);
            }
            assert!(parsed.is_err());
        }
    }
}
