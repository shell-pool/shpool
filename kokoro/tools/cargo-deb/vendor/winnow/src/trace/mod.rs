//! Parser execution tracing
//!
//! By default, nothing happens and tracing gets compiled away as a no-op.  To enable tracing, use
//! `--features debug`.
//!
//! # Example
//!
//!![Trace output from string example](https://raw.githubusercontent.com/winnow-rs/winnow/main/assets/trace.svg "Example output")

#[cfg(feature = "debug")]
mod internals;

use crate::error::ErrMode;
use crate::stream::Stream;
use crate::IResult;
use crate::Parser;

#[cfg(all(feature = "debug", not(feature = "std")))]
compile_error!("`debug` requires `std`");

/// Trace the execution of the parser
///
/// Note that [`Parser::context` also provides high level trace information.
///
/// See [`trace` module][self] for more details.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::{Error, ErrorKind}, error::Needed, IResult};
/// # use winnow::bytes::take_while_m_n;
/// # use winnow::stream::AsChar;
/// use winnow::trace::trace;
///
/// fn short_alpha(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   trace("short_alpha",
///     take_while_m_n(3, 6, AsChar::is_alpha)
///   )(s)
/// }
///
/// assert_eq!(short_alpha(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"lengthy"), Ok((&b"y"[..], &b"length"[..])));
/// assert_eq!(short_alpha(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(short_alpha(b"ed"), Err(ErrMode::Backtrack(Error::new(&b"ed"[..], ErrorKind::TakeWhileMN))));
/// assert_eq!(short_alpha(b"12345"), Err(ErrMode::Backtrack(Error::new(&b"12345"[..], ErrorKind::TakeWhileMN))));
/// ```
#[cfg_attr(not(feature = "debug"), allow(unused_variables))]
pub fn trace<I: Stream, O, E>(
    name: impl crate::lib::std::fmt::Display,
    mut parser: impl Parser<I, O, E>,
) -> impl FnMut(I) -> IResult<I, O, E> {
    #[cfg(feature = "debug")]
    {
        let mut call_count = 0;
        move |i| {
            let depth = internals::Depth::new();
            let original = i.clone();
            internals::start(*depth, &name, call_count, &original);

            let res = parser.parse_next(i);

            let consumed = res.as_ref().ok().map(|(i, _)| {
                if i.eof_offset() == 0 {
                    // Sometimes, an unrelated empty string is returned which can break `offset_to`
                    original.eof_offset()
                } else {
                    original.offset_to(i)
                }
            });
            let severity = internals::Severity::with_result(&res);
            internals::end(*depth, &name, call_count, consumed, severity);
            call_count += 1;

            res
        }
    }
    #[cfg(not(feature = "debug"))]
    {
        move |i| parser.parse_next(i)
    }
}

#[cfg_attr(not(feature = "debug"), allow(unused_variables))]
pub(crate) fn trace_result<T, E>(
    name: impl crate::lib::std::fmt::Display,
    res: &Result<T, ErrMode<E>>,
) {
    #[cfg(feature = "debug")]
    {
        let depth = internals::Depth::existing();
        let severity = internals::Severity::with_result(res);
        internals::result(*depth, &name, severity);
    }
}

#[test]
#[cfg(feature = "std")]
#[cfg_attr(miri, ignore)]
#[cfg(unix)]
#[cfg(feature = "debug")]
fn example() {
    use term_transcript::{test::TestConfig, ShellOptions};

    let path = snapbox::cmd::compile_example("string", ["--features=debug"]).unwrap();

    let current_dir = path.parent().unwrap();
    let cmd = path.file_name().unwrap();
    // HACK: term_transcript doesn't allow non-UTF8 paths
    let cmd = format!("./{}", cmd.to_string_lossy());

    TestConfig::new(
        ShellOptions::default()
            .with_current_dir(current_dir)
            .with_env("CLICOLOR_FORCE", "1"),
    )
    .test("assets/trace.svg", [cmd.as_str()]);
}
