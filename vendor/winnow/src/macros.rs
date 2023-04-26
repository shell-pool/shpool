/// `match` for parsers
///
/// When parsers have unique prefixes to test for, this offers better performance over
/// [`alt`][crate::branch::alt] though it might be at the cost of duplicating parts of your grammar
/// if you needed to [`peek`][crate::combinator::peek].
///
/// For tight control over the error in a catch-all case, use [`fail`][crate::combinator::fail].
///
/// # Example
///
/// ```rust
/// use winnow::prelude::*;
/// use winnow::branch::dispatch;
/// # use winnow::bytes::any;
/// # use winnow::combinator::peek;
/// # use winnow::sequence::preceded;
/// # use winnow::combinator::success;
/// # use winnow::combinator::fail;
///
/// fn escaped(input: &str) -> IResult<&str, char> {
///     preceded('\\', escape_seq_char).parse_next(input)
/// }
///
/// fn escape_seq_char(input: &str) -> IResult<&str, char> {
///     dispatch! {any;
///         'b' => success('\u{8}'),
///         'f' => success('\u{c}'),
///         'n' => success('\n'),
///         'r' => success('\r'),
///         't' => success('\t'),
///         '\\' => success('\\'),
///         '"' => success('"'),
///         _ => fail::<_, char, _>,
///     }
///     .parse_next(input)
/// }
///
/// assert_eq!(escaped.parse_next("\\nHello"), Ok(("Hello", '\n')));
/// ```
#[macro_export]
macro_rules! dispatch {
    ($match_parser: expr; $( $pat:pat $(if $pred:expr)? => $expr: expr ),+ $(,)? ) => {
        $crate::trace::trace("dispatch", move |i|
        {
            use $crate::Parser;
            let (i, initial) = $match_parser.parse_next(i)?;
            match initial {
                $(
                    $pat $(if $pred)? => $expr.parse_next(i),
                )*
            }
        })
    }
}

macro_rules! succ (
  (0, $submac:ident ! ($($rest:tt)*)) => ($submac!(1, $($rest)*));
  (1, $submac:ident ! ($($rest:tt)*)) => ($submac!(2, $($rest)*));
  (2, $submac:ident ! ($($rest:tt)*)) => ($submac!(3, $($rest)*));
  (3, $submac:ident ! ($($rest:tt)*)) => ($submac!(4, $($rest)*));
  (4, $submac:ident ! ($($rest:tt)*)) => ($submac!(5, $($rest)*));
  (5, $submac:ident ! ($($rest:tt)*)) => ($submac!(6, $($rest)*));
  (6, $submac:ident ! ($($rest:tt)*)) => ($submac!(7, $($rest)*));
  (7, $submac:ident ! ($($rest:tt)*)) => ($submac!(8, $($rest)*));
  (8, $submac:ident ! ($($rest:tt)*)) => ($submac!(9, $($rest)*));
  (9, $submac:ident ! ($($rest:tt)*)) => ($submac!(10, $($rest)*));
  (10, $submac:ident ! ($($rest:tt)*)) => ($submac!(11, $($rest)*));
  (11, $submac:ident ! ($($rest:tt)*)) => ($submac!(12, $($rest)*));
  (12, $submac:ident ! ($($rest:tt)*)) => ($submac!(13, $($rest)*));
  (13, $submac:ident ! ($($rest:tt)*)) => ($submac!(14, $($rest)*));
  (14, $submac:ident ! ($($rest:tt)*)) => ($submac!(15, $($rest)*));
  (15, $submac:ident ! ($($rest:tt)*)) => ($submac!(16, $($rest)*));
  (16, $submac:ident ! ($($rest:tt)*)) => ($submac!(17, $($rest)*));
  (17, $submac:ident ! ($($rest:tt)*)) => ($submac!(18, $($rest)*));
  (18, $submac:ident ! ($($rest:tt)*)) => ($submac!(19, $($rest)*));
  (19, $submac:ident ! ($($rest:tt)*)) => ($submac!(20, $($rest)*));
  (20, $submac:ident ! ($($rest:tt)*)) => ($submac!(21, $($rest)*));
);
