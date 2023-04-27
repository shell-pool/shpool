use winnow::bytes::one_of;
use winnow::combinator::cut_err;
use winnow::multi::separated0;
use winnow::sequence::delimited;

use crate::key::Key;
use crate::parser::errors::CustomError;
use crate::parser::key::key;
use crate::parser::prelude::*;
use crate::parser::trivia::ws;
use crate::parser::value::value;
use crate::table::TableKeyValue;
use crate::{InlineTable, InternalString, Item, RawString, Value};

use indexmap::map::Entry;

// ;; Inline Table

// inline-table = inline-table-open inline-table-keyvals inline-table-close
pub(crate) fn inline_table(
    check: RecursionCheck,
) -> impl FnMut(Input<'_>) -> IResult<Input<'_>, InlineTable, ParserError<'_>> {
    move |input| {
        delimited(
            INLINE_TABLE_OPEN,
            cut_err(inline_table_keyvals(check).map_res(|(kv, p)| table_from_pairs(kv, p))),
            cut_err(INLINE_TABLE_CLOSE)
                .context(Context::Expression("inline table"))
                .context(Context::Expected(ParserValue::CharLiteral('}'))),
        )
        .parse_next(input)
    }
}

fn table_from_pairs(
    v: Vec<(Vec<Key>, TableKeyValue)>,
    preamble: RawString,
) -> Result<InlineTable, CustomError> {
    let mut root = InlineTable::new();
    root.set_preamble(preamble);
    // Assuming almost all pairs will be directly in `root`
    root.items.reserve(v.len());

    for (path, kv) in v {
        let table = descend_path(&mut root, &path)?;
        let key: InternalString = kv.key.get_internal().into();
        match table.items.entry(key) {
            Entry::Vacant(o) => {
                o.insert(kv);
            }
            Entry::Occupied(o) => {
                return Err(CustomError::DuplicateKey {
                    key: o.key().as_str().into(),
                    table: None,
                });
            }
        }
    }
    Ok(root)
}

fn descend_path<'a>(
    mut table: &'a mut InlineTable,
    path: &'a [Key],
) -> Result<&'a mut InlineTable, CustomError> {
    for (i, key) in path.iter().enumerate() {
        let entry = table.entry_format(key).or_insert_with(|| {
            let mut new_table = InlineTable::new();
            new_table.set_dotted(true);

            Value::InlineTable(new_table)
        });
        match *entry {
            Value::InlineTable(ref mut sweet_child_of_mine) => {
                table = sweet_child_of_mine;
            }
            ref v => {
                return Err(CustomError::extend_wrong_type(path, i, v.type_name()));
            }
        }
    }
    Ok(table)
}

// inline-table-open  = %x7B ws     ; {
pub(crate) const INLINE_TABLE_OPEN: u8 = b'{';
// inline-table-close = ws %x7D     ; }
const INLINE_TABLE_CLOSE: u8 = b'}';
// inline-table-sep   = ws %x2C ws  ; , Comma
const INLINE_TABLE_SEP: u8 = b',';
// keyval-sep = ws %x3D ws ; =
pub(crate) const KEYVAL_SEP: u8 = b'=';

// inline-table-keyvals = [ inline-table-keyvals-non-empty ]
// inline-table-keyvals-non-empty =
// ( key keyval-sep val inline-table-sep inline-table-keyvals-non-empty ) /
// ( key keyval-sep val )

fn inline_table_keyvals(
    check: RecursionCheck,
) -> impl FnMut(
    Input<'_>,
) -> IResult<Input<'_>, (Vec<(Vec<Key>, TableKeyValue)>, RawString), ParserError<'_>> {
    move |input| {
        let check = check.recursing(input)?;
        (
            separated0(keyval(check), INLINE_TABLE_SEP),
            ws.span().map(RawString::with_span),
        )
            .parse_next(input)
    }
}

fn keyval(
    check: RecursionCheck,
) -> impl FnMut(Input<'_>) -> IResult<Input<'_>, (Vec<Key>, TableKeyValue), ParserError<'_>> {
    move |input| {
        (
            key,
            cut_err((
                one_of(KEYVAL_SEP)
                    .context(Context::Expected(ParserValue::CharLiteral('.')))
                    .context(Context::Expected(ParserValue::CharLiteral('='))),
                (ws.span(), value(check), ws.span()),
            )),
        )
            .map(|(key, (_, v))| {
                let mut path = key;
                let key = path.pop().expect("grammar ensures at least 1");

                let (pre, v, suf) = v;
                let pre = RawString::with_span(pre);
                let suf = RawString::with_span(suf);
                let v = v.decorated(pre, suf);
                (
                    path,
                    TableKeyValue {
                        key,
                        value: Item::Value(v),
                    },
                )
            })
            .parse_next(input)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn inline_tables() {
        let inputs = [
            r#"{}"#,
            r#"{   }"#,
            r#"{a = 1e165}"#,
            r#"{ hello = "world", a = 1}"#,
            r#"{ hello.world = "a" }"#,
        ];
        for input in inputs {
            dbg!(input);
            let mut parsed = inline_table(Default::default())
                .parse_next(new_input(input))
                .finish();
            if let Ok(parsed) = &mut parsed {
                parsed.despan(input);
            }
            assert_eq!(parsed.map(|a| a.to_string()), Ok(input.to_owned()));
        }
    }

    #[test]
    fn invalid_inline_tables() {
        let invalid_inputs = [r#"{a = 1e165"#, r#"{ hello = "world", a = 2, hello = 1}"#];
        for input in invalid_inputs {
            dbg!(input);
            let mut parsed = inline_table(Default::default())
                .parse_next(new_input(input))
                .finish();
            if let Ok(parsed) = &mut parsed {
                parsed.despan(input);
            }
            assert!(parsed.is_err());
        }
    }
}
