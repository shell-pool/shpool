use std::error::Error as StdError;
use std::fmt::{Display, Formatter, Result};

use crate::parser::prelude::*;
use crate::Key;

use winnow::BStr;

/// Type representing a TOML parse error
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TomlError {
    message: String,
    original: Option<String>,
    keys: Vec<String>,
    span: Option<std::ops::Range<usize>>,
}

impl TomlError {
    pub(crate) fn new(error: ParserError<'_>, original: Input<'_>) -> Self {
        use winnow::stream::Offset;
        use winnow::stream::Stream;

        let offset = original.offset_to(&error.input);
        let span = if offset == original.len() {
            offset..offset
        } else {
            offset..(offset + 1)
        };

        let message = error.to_string();
        let original = original.next_slice(original.eof_offset()).1;

        Self {
            message,
            original: Some(
                String::from_utf8(original.to_owned()).expect("original document was utf8"),
            ),
            keys: Vec::new(),
            span: Some(span),
        }
    }

    #[cfg(feature = "serde")]
    pub(crate) fn custom(message: String, span: Option<std::ops::Range<usize>>) -> Self {
        Self {
            message,
            original: None,
            keys: Vec::new(),
            span,
        }
    }

    #[cfg(feature = "serde")]
    pub(crate) fn add_key(&mut self, key: String) {
        self.keys.insert(0, key);
    }

    /// What went wrong
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The start/end index into the original document where the error occurred
    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        self.span.clone()
    }

    #[cfg(feature = "serde")]
    pub(crate) fn set_span(&mut self, span: Option<std::ops::Range<usize>>) {
        self.span = span;
    }

    #[cfg(feature = "serde")]
    pub(crate) fn set_original(&mut self, original: Option<String>) {
        self.original = original;
    }
}

/// Displays a TOML parse error
///
/// # Example
///
/// TOML parse error at line 1, column 10
///   |
/// 1 | 00:32:00.a999999
///   |          ^
/// Unexpected `a`
/// Expected `digit`
/// While parsing a Time
/// While parsing a Date-Time
impl Display for TomlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut context = false;
        if let (Some(original), Some(span)) = (&self.original, self.span()) {
            context = true;

            let (line, column) = translate_position(original.as_bytes(), span.start);
            let line_num = line + 1;
            let col_num = column + 1;
            let gutter = line_num.to_string().len();
            let content = original.split('\n').nth(line).expect("valid line number");

            writeln!(
                f,
                "TOML parse error at line {}, column {}",
                line_num, col_num
            )?;
            //   |
            for _ in 0..=gutter {
                write!(f, " ")?;
            }
            writeln!(f, "|")?;

            // 1 | 00:32:00.a999999
            write!(f, "{} | ", line_num)?;
            writeln!(f, "{}", content)?;

            //   |          ^
            for _ in 0..=gutter {
                write!(f, " ")?;
            }
            write!(f, "|")?;
            for _ in 0..=column {
                write!(f, " ")?;
            }
            // The span will be empty at eof, so we need to make sure we always print at least
            // one `^`
            write!(f, "^")?;
            for _ in (span.start + 1)..(span.end.min(span.start + content.len())) {
                write!(f, "^")?;
            }
            writeln!(f)?;
        }
        writeln!(f, "{}", self.message)?;
        if !context && !self.keys.is_empty() {
            writeln!(f, "in `{}`", self.keys.join("."))?;
        }

        Ok(())
    }
}

impl StdError for TomlError {
    fn description(&self) -> &'static str {
        "TOML parse error"
    }
}

#[derive(Debug)]
pub(crate) struct ParserError<'b> {
    input: Input<'b>,
    context: Vec<Context>,
    cause: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl<'b> winnow::error::ParseError<Input<'b>> for ParserError<'b> {
    fn from_error_kind(input: Input<'b>, _kind: winnow::error::ErrorKind) -> Self {
        Self {
            input,
            context: Default::default(),
            cause: Default::default(),
        }
    }

    fn append(self, _input: Input<'b>, _kind: winnow::error::ErrorKind) -> Self {
        self
    }

    fn or(self, other: Self) -> Self {
        other
    }
}

impl<'b> winnow::error::ParseError<&'b str> for ParserError<'b> {
    fn from_error_kind(input: &'b str, _kind: winnow::error::ErrorKind) -> Self {
        Self {
            input: Input::new(BStr::new(input)),
            context: Default::default(),
            cause: Default::default(),
        }
    }

    fn append(self, _input: &'b str, _kind: winnow::error::ErrorKind) -> Self {
        self
    }

    fn or(self, other: Self) -> Self {
        other
    }
}

impl<'b> winnow::error::ContextError<Input<'b>, Context> for ParserError<'b> {
    fn add_context(mut self, _input: Input<'b>, ctx: Context) -> Self {
        self.context.push(ctx);
        self
    }
}

impl<'b, E: std::error::Error + Send + Sync + 'static>
    winnow::error::FromExternalError<Input<'b>, E> for ParserError<'b>
{
    fn from_external_error(input: Input<'b>, _kind: winnow::error::ErrorKind, e: E) -> Self {
        Self {
            input,
            context: Default::default(),
            cause: Some(Box::new(e)),
        }
    }
}

impl<'b, E: std::error::Error + Send + Sync + 'static> winnow::error::FromExternalError<&'b str, E>
    for ParserError<'b>
{
    fn from_external_error(input: &'b str, _kind: winnow::error::ErrorKind, e: E) -> Self {
        Self {
            input: Input::new(BStr::new(input)),
            context: Default::default(),
            cause: Some(Box::new(e)),
        }
    }
}

// For tests
impl<'b> std::cmp::PartialEq for ParserError<'b> {
    fn eq(&self, other: &Self) -> bool {
        self.input == other.input
            && self.context == other.context
            && self.cause.as_ref().map(ToString::to_string)
                == other.cause.as_ref().map(ToString::to_string)
    }
}

impl<'a> std::fmt::Display for ParserError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let expression = self.context.iter().find_map(|c| match c {
            Context::Expression(c) => Some(c),
            _ => None,
        });
        let expected = self
            .context
            .iter()
            .filter_map(|c| match c {
                Context::Expected(c) => Some(c),
                _ => None,
            })
            .collect::<Vec<_>>();

        let mut newline = false;

        if let Some(expression) = expression {
            newline = true;

            write!(f, "invalid {}", expression)?;
        }

        if !expected.is_empty() {
            if newline {
                writeln!(f)?;
            }
            newline = true;

            write!(f, "expected ")?;
            for (i, expected) in expected.iter().enumerate() {
                if i != 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", expected)?;
            }
        }
        if let Some(cause) = &self.cause {
            if newline {
                writeln!(f)?;
            }
            write!(f, "{}", cause)?;
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Context {
    Expression(&'static str),
    Expected(ParserValue),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum ParserValue {
    CharLiteral(char),
    StringLiteral(&'static str),
    Description(&'static str),
}

impl std::fmt::Display for ParserValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserValue::CharLiteral('\n') => "newline".fmt(f),
            ParserValue::CharLiteral('`') => "'`'".fmt(f),
            ParserValue::CharLiteral(c) if c.is_ascii_control() => {
                write!(f, "`{}`", c.escape_debug())
            }
            ParserValue::CharLiteral(c) => write!(f, "`{}`", c),
            ParserValue::StringLiteral(c) => write!(f, "`{}`", c),
            ParserValue::Description(c) => write!(f, "{}", c),
        }
    }
}

fn translate_position(input: &[u8], index: usize) -> (usize, usize) {
    if input.is_empty() {
        return (0, index);
    }

    let safe_index = index.min(input.len() - 1);
    let column_offset = index - safe_index;
    let index = safe_index;

    let nl = input[0..index]
        .iter()
        .rev()
        .enumerate()
        .find(|(_, b)| **b == b'\n')
        .map(|(nl, _)| index - nl - 1);
    let line_start = match nl {
        Some(nl) => nl + 1,
        None => 0,
    };
    let line = input[0..line_start].iter().filter(|b| **b == b'\n').count();
    let line = line;

    let column = std::str::from_utf8(&input[line_start..=index])
        .map(|s| s.chars().count() - 1)
        .unwrap_or_else(|_| index - line_start);
    let column = column + column_offset;

    (line, column)
}

#[cfg(test)]
mod test_translate_position {
    use super::*;

    #[test]
    fn empty() {
        let input = b"";
        let index = 0;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, 0));
    }

    #[test]
    fn start() {
        let input = b"Hello";
        let index = 0;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, 0));
    }

    #[test]
    fn end() {
        let input = b"Hello";
        let index = input.len() - 1;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, input.len() - 1));
    }

    #[test]
    fn after() {
        let input = b"Hello";
        let index = input.len();
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, input.len()));
    }

    #[test]
    fn first_line() {
        let input = b"Hello\nWorld\n";
        let index = 2;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, 2));
    }

    #[test]
    fn end_of_line() {
        let input = b"Hello\nWorld\n";
        let index = 5;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (0, 5));
    }

    #[test]
    fn start_of_second_line() {
        let input = b"Hello\nWorld\n";
        let index = 6;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (1, 0));
    }

    #[test]
    fn second_line() {
        let input = b"Hello\nWorld\n";
        let index = 8;
        let position = translate_position(&input[..], index);
        assert_eq!(position, (1, 2));
    }
}

#[derive(Debug, Clone)]
pub(crate) enum CustomError {
    DuplicateKey {
        key: String,
        table: Option<Vec<Key>>,
    },
    DottedKeyExtendWrongType {
        key: Vec<Key>,
        actual: &'static str,
    },
    OutOfRange,
    #[cfg_attr(feature = "unbounded", allow(dead_code))]
    RecursionLimitExceeded,
}

impl CustomError {
    pub(crate) fn duplicate_key(path: &[Key], i: usize) -> Self {
        assert!(i < path.len());
        let key = &path[i];
        let repr = key.display_repr();
        Self::DuplicateKey {
            key: repr.into(),
            table: Some(path[..i].to_vec()),
        }
    }

    pub(crate) fn extend_wrong_type(path: &[Key], i: usize, actual: &'static str) -> Self {
        assert!(i < path.len());
        Self::DottedKeyExtendWrongType {
            key: path[..=i].to_vec(),
            actual,
        }
    }
}

impl StdError for CustomError {
    fn description(&self) -> &'static str {
        "TOML parse error"
    }
}

impl Display for CustomError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            CustomError::DuplicateKey { key, table } => {
                if let Some(table) = table {
                    if table.is_empty() {
                        write!(f, "duplicate key `{}` in document root", key)
                    } else {
                        let path = table.iter().map(|k| k.get()).collect::<Vec<_>>().join(".");
                        write!(f, "duplicate key `{}` in table `{}`", key, path)
                    }
                } else {
                    write!(f, "duplicate key `{}`", key)
                }
            }
            CustomError::DottedKeyExtendWrongType { key, actual } => {
                let path = key.iter().map(|k| k.get()).collect::<Vec<_>>().join(".");
                write!(
                    f,
                    "dotted key `{}` attempted to extend non-table type ({})",
                    path, actual
                )
            }
            CustomError::OutOfRange => write!(f, "value is out of range"),
            CustomError::RecursionLimitExceeded => write!(f, "recursion limit exceded"),
        }
    }
}
