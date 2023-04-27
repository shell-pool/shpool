use std::cell::RefCell;

use winnow::bytes::any;
use winnow::bytes::one_of;
use winnow::combinator::cut_err;
use winnow::combinator::eof;
use winnow::combinator::opt;
use winnow::combinator::peek;
use winnow::error::FromExternalError;
use winnow::multi::many0;

use crate::document::Document;
use crate::key::Key;
use crate::parser::inline_table::KEYVAL_SEP;
use crate::parser::key::key;
use crate::parser::prelude::*;
use crate::parser::state::ParseState;
use crate::parser::table::table;
use crate::parser::trivia::{comment, line_ending, line_trailing, newline, ws};
use crate::parser::value::value;
use crate::table::TableKeyValue;
use crate::Item;
use crate::RawString;

// ;; TOML

// toml = expression *( newline expression )

// expression = ( ( ws comment ) /
//                ( ws keyval ws [ comment ] ) /
//                ( ws table ws [ comment ] ) /
//                  ws )
pub(crate) fn document(input: Input<'_>) -> IResult<Input<'_>, Document, ParserError<'_>> {
    let state = RefCell::new(ParseState::default());
    let state_ref = &state;

    let (i, _o) = (
        // Remove BOM if present
        opt(b"\xEF\xBB\xBF"),
        parse_ws(state_ref),
        many0((
            dispatch! {peek(any);
                crate::parser::trivia::COMMENT_START_SYMBOL => cut_err(parse_comment(state_ref)),
                crate::parser::table::STD_TABLE_OPEN => cut_err(table(state_ref)),
                crate::parser::trivia::LF |
                crate::parser::trivia::CR => parse_newline(state_ref),
                _ => cut_err(keyval(state_ref)),
            },
            parse_ws(state_ref),
        ))
        .map(|()| ()),
        eof,
    )
        .parse_next(input)?;
    state
        .into_inner()
        .into_document()
        .map(|document| (i, document))
        .map_err(|err| {
            winnow::error::ErrMode::Backtrack(ParserError::from_external_error(
                i,
                winnow::error::ErrorKind::MapRes,
                err,
            ))
        })
}

pub(crate) fn parse_comment<'s, 'i>(
    state: &'s RefCell<ParseState>,
) -> impl FnMut(Input<'i>) -> IResult<Input<'i>, (), ParserError<'_>> + 's {
    move |i| {
        (comment, line_ending)
            .span()
            .map(|span| {
                state.borrow_mut().on_comment(span);
            })
            .parse_next(i)
    }
}

pub(crate) fn parse_ws<'s, 'i>(
    state: &'s RefCell<ParseState>,
) -> impl FnMut(Input<'i>) -> IResult<Input<'i>, (), ParserError<'i>> + 's {
    move |i| {
        ws.span()
            .map(|span| state.borrow_mut().on_ws(span))
            .parse_next(i)
    }
}

pub(crate) fn parse_newline<'s, 'i>(
    state: &'s RefCell<ParseState>,
) -> impl FnMut(Input<'i>) -> IResult<Input<'i>, (), ParserError<'i>> + 's {
    move |i| {
        newline
            .span()
            .map(|span| state.borrow_mut().on_ws(span))
            .parse_next(i)
    }
}

pub(crate) fn keyval<'s, 'i>(
    state: &'s RefCell<ParseState>,
) -> impl FnMut(Input<'i>) -> IResult<Input<'i>, (), ParserError<'i>> + 's {
    move |i| {
        parse_keyval
            .map_res(|(p, kv)| state.borrow_mut().on_keyval(p, kv))
            .parse_next(i)
    }
}

// keyval = key keyval-sep val
pub(crate) fn parse_keyval(
    input: Input<'_>,
) -> IResult<Input<'_>, (Vec<Key>, TableKeyValue), ParserError<'_>> {
    (
        key,
        cut_err((
            one_of(KEYVAL_SEP)
                .context(Context::Expected(ParserValue::CharLiteral('.')))
                .context(Context::Expected(ParserValue::CharLiteral('='))),
            (
                ws.span(),
                value(RecursionCheck::default()),
                line_trailing
                    .context(Context::Expected(ParserValue::CharLiteral('\n')))
                    .context(Context::Expected(ParserValue::CharLiteral('#'))),
            ),
        )),
    )
        .map_res::<_, _, std::str::Utf8Error>(|(key, (_, v))| {
            let mut path = key;
            let key = path.pop().expect("grammar ensures at least 1");

            let (pre, v, suf) = v;
            let pre = RawString::with_span(pre);
            let suf = RawString::with_span(suf);
            let v = v.decorated(pre, suf);
            Ok((
                path,
                TableKeyValue {
                    key,
                    value: Item::Value(v),
                },
            ))
        })
        .parse_next(input)
}
