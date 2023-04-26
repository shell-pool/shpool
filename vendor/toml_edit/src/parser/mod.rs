#![allow(clippy::type_complexity)]

#[macro_use]
pub(crate) mod macros;

pub(crate) mod array;
pub(crate) mod datetime;
pub(crate) mod document;
pub(crate) mod errors;
pub(crate) mod inline_table;
pub(crate) mod key;
pub(crate) mod numbers;
pub(crate) mod state;
pub(crate) mod strings;
pub(crate) mod table;
pub(crate) mod trivia;
pub(crate) mod value;

pub use errors::TomlError;

pub(crate) fn parse_document(raw: &str) -> Result<crate::Document, TomlError> {
    use prelude::*;

    let b = new_input(raw);
    let mut doc = document::document
        .parse(b)
        .map_err(|e| TomlError::new(e, b))?;
    doc.span = Some(0..(raw.len()));
    doc.original = Some(raw.to_owned());
    Ok(doc)
}

pub(crate) fn parse_key(raw: &str) -> Result<crate::Key, TomlError> {
    use prelude::*;

    let b = new_input(raw);
    let result = key::simple_key.parse(b);
    match result {
        Ok((raw, key)) => {
            Ok(crate::Key::new(key).with_repr_unchecked(crate::Repr::new_unchecked(raw)))
        }
        Err(e) => Err(TomlError::new(e, b)),
    }
}

pub(crate) fn parse_key_path(raw: &str) -> Result<Vec<crate::Key>, TomlError> {
    use prelude::*;

    let b = new_input(raw);
    let result = key::key.parse(b);
    match result {
        Ok(mut keys) => {
            for key in &mut keys {
                key.despan(raw);
            }
            Ok(keys)
        }
        Err(e) => Err(TomlError::new(e, b)),
    }
}

pub(crate) fn parse_value(raw: &str) -> Result<crate::Value, TomlError> {
    use prelude::*;

    let b = new_input(raw);
    let parsed = value::value(RecursionCheck::default()).parse(b);
    match parsed {
        Ok(mut value) => {
            // Only take the repr and not decor, as its probably not intended
            value.decor_mut().clear();
            value.despan(raw);
            Ok(value)
        }
        Err(e) => Err(TomlError::new(e, b)),
    }
}

pub(crate) mod prelude {
    pub(crate) use super::errors::Context;
    pub(crate) use super::errors::ParserError;
    pub(crate) use super::errors::ParserValue;
    pub(crate) use winnow::IResult;
    pub(crate) use winnow::Parser as _;

    pub(crate) type Input<'b> = winnow::Located<&'b winnow::BStr>;

    pub(crate) fn new_input(s: &str) -> Input<'_> {
        winnow::Located::new(winnow::BStr::new(s))
    }

    pub(crate) fn ok_error<I, O, E>(
        res: IResult<I, O, E>,
    ) -> Result<Option<(I, O)>, winnow::error::ErrMode<E>> {
        match res {
            Ok(ok) => Ok(Some(ok)),
            Err(winnow::error::ErrMode::Backtrack(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn trace<I: std::fmt::Debug, O: std::fmt::Debug, E: std::fmt::Debug>(
        context: impl std::fmt::Display,
        mut parser: impl winnow::Parser<I, O, E>,
    ) -> impl FnMut(I) -> IResult<I, O, E> {
        static DEPTH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        move |input: I| {
            let depth = DEPTH.fetch_add(1, std::sync::atomic::Ordering::SeqCst) * 2;
            eprintln!("{:depth$}--> {} {:?}", "", context, input);
            match parser.parse_next(input) {
                Ok((i, o)) => {
                    DEPTH.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    eprintln!("{:depth$}<-- {} {:?}", "", context, i);
                    Ok((i, o))
                }
                Err(err) => {
                    DEPTH.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    eprintln!("{:depth$}<-- {} {:?}", "", context, err);
                    Err(err)
                }
            }
        }
    }

    #[cfg(not(feature = "unbounded"))]
    #[derive(Copy, Clone, Debug, Default)]
    pub(crate) struct RecursionCheck {
        current: usize,
    }

    #[cfg(not(feature = "unbounded"))]
    impl RecursionCheck {
        pub(crate) fn check_depth(depth: usize) -> Result<(), super::errors::CustomError> {
            if depth < 128 {
                Ok(())
            } else {
                Err(super::errors::CustomError::RecursionLimitExceeded)
            }
        }

        pub(crate) fn recursing(
            mut self,
            input: Input<'_>,
        ) -> Result<Self, winnow::error::ErrMode<ParserError<'_>>> {
            self.current += 1;
            if self.current < 128 {
                Ok(self)
            } else {
                Err(winnow::error::ErrMode::Backtrack(
                    winnow::error::FromExternalError::from_external_error(
                        input,
                        winnow::error::ErrorKind::Eof,
                        super::errors::CustomError::RecursionLimitExceeded,
                    ),
                ))
            }
        }
    }

    #[cfg(feature = "unbounded")]
    #[derive(Copy, Clone, Debug, Default)]
    pub(crate) struct RecursionCheck {}

    #[cfg(feature = "unbounded")]
    impl RecursionCheck {
        pub(crate) fn check_depth(_depth: usize) -> Result<(), super::errors::CustomError> {
            Ok(())
        }

        pub(crate) fn recursing(
            self,
            _input: Input<'_>,
        ) -> Result<Self, winnow::error::ErrMode<ParserError<'_>>> {
            Ok(self)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn documents() {
        let documents = [
            "",
            r#"
# This is a TOML document.

title = "TOML Example"

    [owner]
    name = "Tom Preston-Werner"
    dob = 1979-05-27T07:32:00-08:00 # First class dates

    [database]
    server = "192.168.1.1"
    ports = [ 8001, 8001, 8002 ]
    connection_max = 5000
    enabled = true

    [servers]

    # Indentation (tabs and/or spaces) is allowed but not required
[servers.alpha]
    ip = "10.0.0.1"
    dc = "eqdc10"

    [servers.beta]
    ip = "10.0.0.2"
    dc = "eqdc10"

    [clients]
    data = [ ["gamma", "delta"], [1, 2] ]

    # Line breaks are OK when inside arrays
hosts = [
    "alpha",
    "omega"
]

   'some.wierd .stuff'   =  """
                         like
                         that
                      #   """ # this broke my sintax highlighting
   " also. like " = '''
that
'''
   double = 2e39 # this number looks familiar
# trailing comment"#,
            r#""#,
            r#"  "#,
            r#" hello = 'darkness' # my old friend
"#,
            r#"[parent . child]
key = "value"
"#,
            r#"hello.world = "a"
"#,
            r#"foo = 1979-05-27 # Comment
"#,
        ];
        for input in documents {
            dbg!(input);
            let mut parsed = parse_document(input);
            if let Ok(parsed) = &mut parsed {
                parsed.despan();
            }
            let doc = match parsed {
                Ok(doc) => doc,
                Err(err) => {
                    panic!(
                        "Parse error: {:?}\nFailed to parse:\n```\n{}\n```",
                        err, input
                    )
                }
            };

            snapbox::assert_eq(input, doc.to_string());
        }
    }

    #[test]
    fn documents_parse_only() {
        let parse_only = ["\u{FEFF}
[package]
name = \"foo\"
version = \"0.0.1\"
authors = []
"];
        for input in parse_only {
            dbg!(input);
            let mut parsed = parse_document(input);
            if let Ok(parsed) = &mut parsed {
                parsed.despan();
            }
            match parsed {
                Ok(_) => (),
                Err(err) => {
                    panic!(
                        "Parse error: {:?}\nFailed to parse:\n```\n{}\n```",
                        err, input
                    )
                }
            }
        }
    }

    #[test]
    fn invalid_documents() {
        let invalid_inputs = [r#" hello = 'darkness' # my old friend
$"#];
        for input in invalid_inputs {
            dbg!(input);
            let mut parsed = parse_document(input);
            if let Ok(parsed) = &mut parsed {
                parsed.despan();
            }
            assert!(parsed.is_err(), "Input: {:?}", input);
        }
    }
}
