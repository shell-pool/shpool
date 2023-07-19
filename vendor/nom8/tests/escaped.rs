use nom8::bytes::escaped;
use nom8::bytes::one_of;
use nom8::character::digit1;
use nom8::{error::ErrorKind, Err, IResult};

fn esc(s: &str) -> IResult<&str, &str, (&str, ErrorKind)> {
  escaped(digit1, '\\', one_of("\"n\\"))(s)
}

#[cfg(feature = "alloc")]
fn esc_trans(s: &str) -> IResult<&str, String, (&str, ErrorKind)> {
  use nom8::bytes::escaped_transform;
  escaped_transform(digit1, '\\', "n")(s)
}

#[test]
fn test_escaped() {
  assert_eq!(esc("abcd"), Err(Err::Error(("abcd", ErrorKind::Escaped))));
}

#[test]
#[cfg(feature = "alloc")]
fn test_escaped_transform() {
  assert_eq!(
    esc_trans("abcd"),
    Err(Err::Error(("abcd", ErrorKind::EscapedTransform)))
  );
}
