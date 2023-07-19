use super::*;

use crate::error::ErrorKind;
use crate::input::AsChar;
use crate::input::Streaming;
use crate::multi::length_data;
use crate::sequence::delimited;
use crate::Err;
use crate::IResult;
use crate::Needed;
#[cfg(feature = "alloc")]
use crate::{branch::alt, lib::std::string::String, lib::std::vec::Vec};

#[test]
fn complete_take_while_m_n_utf8_all_matching() {
  let result: IResult<&str, &str> = super::take_while_m_n(1, 4, |c: char| c.is_alphabetic())("øn");
  assert_eq!(result, Ok(("", "øn")));
}

#[test]
fn complete_take_while_m_n_utf8_all_matching_substring() {
  let result: IResult<&str, &str> = super::take_while_m_n(1, 1, |c: char| c.is_alphabetic())("øn");
  assert_eq!(result, Ok(("n", "ø")));
}

// issue #1336 "escaped hangs if normal parser accepts empty"
#[test]
fn complete_escaped_hang() {
  // issue #1336 "escaped hangs if normal parser accepts empty"
  fn escaped_string(input: &str) -> IResult<&str, &str> {
    use crate::bytes::one_of;
    use crate::character::alpha0;
    escaped(alpha0, '\\', one_of("n"))(input)
  }

  escaped_string("7").unwrap();
  escaped_string("a7").unwrap();
}

#[test]
fn complete_escaped_hang_1118() {
  // issue ##1118 escaped does not work with empty string
  fn unquote<'a>(input: &'a str) -> IResult<&'a str, &'a str> {
    use crate::bytes::one_of;
    use crate::combinator::opt;
    use crate::sequence::delimited;

    delimited(
      '"',
      escaped(opt(none_of(r#"\""#)), '\\', one_of(r#"\"rnt"#)),
      '"',
    )(input)
  }

  assert_eq!(unquote(r#""""#), Ok(("", "")));
}

#[cfg(feature = "alloc")]
#[allow(unused_variables)]
#[test]
fn complete_escaping() {
  use crate::bytes::one_of;
  use crate::character::{alpha1 as alpha, digit1 as digit};

  fn esc(i: &[u8]) -> IResult<&[u8], &[u8]> {
    escaped(alpha, '\\', one_of("\"n\\"))(i)
  }
  assert_eq!(esc(&b"abcd;"[..]), Ok((&b";"[..], &b"abcd"[..])));
  assert_eq!(esc(&b"ab\\\"cd;"[..]), Ok((&b";"[..], &b"ab\\\"cd"[..])));
  assert_eq!(esc(&b"\\\"abcd;"[..]), Ok((&b";"[..], &b"\\\"abcd"[..])));
  assert_eq!(esc(&b"\\n;"[..]), Ok((&b";"[..], &b"\\n"[..])));
  assert_eq!(esc(&b"ab\\\"12"[..]), Ok((&b"12"[..], &b"ab\\\""[..])));
  assert_eq!(
    esc(&b"AB\\"[..]),
    Err(Err::Error(error_position!(
      &b"AB\\"[..],
      ErrorKind::Escaped
    )))
  );
  assert_eq!(
    esc(&b"AB\\A"[..]),
    Err(Err::Error(error_node_position!(
      &b"AB\\A"[..],
      ErrorKind::Escaped,
      error_position!(&b"A"[..], ErrorKind::OneOf)
    )))
  );

  fn esc2(i: &[u8]) -> IResult<&[u8], &[u8]> {
    escaped(digit, '\\', one_of("\"n\\"))(i)
  }
  assert_eq!(esc2(&b"12\\nnn34"[..]), Ok((&b"nn34"[..], &b"12\\n"[..])));
}

#[cfg(feature = "alloc")]
#[test]
fn complete_escaping_str() {
  use crate::bytes::one_of;
  use crate::character::{alpha1 as alpha, digit1 as digit};

  fn esc(i: &str) -> IResult<&str, &str> {
    escaped(alpha, '\\', one_of("\"n\\"))(i)
  }
  assert_eq!(esc("abcd;"), Ok((";", "abcd")));
  assert_eq!(esc("ab\\\"cd;"), Ok((";", "ab\\\"cd")));
  assert_eq!(esc("\\\"abcd;"), Ok((";", "\\\"abcd")));
  assert_eq!(esc("\\n;"), Ok((";", "\\n")));
  assert_eq!(esc("ab\\\"12"), Ok(("12", "ab\\\"")));
  assert_eq!(
    esc("AB\\"),
    Err(Err::Error(error_position!("AB\\", ErrorKind::Escaped)))
  );
  assert_eq!(
    esc("AB\\A"),
    Err(Err::Error(error_node_position!(
      "AB\\A",
      ErrorKind::Escaped,
      error_position!("A", ErrorKind::OneOf)
    )))
  );

  fn esc2(i: &str) -> IResult<&str, &str> {
    escaped(digit, '\\', one_of("\"n\\"))(i)
  }
  assert_eq!(esc2("12\\nnn34"), Ok(("nn34", "12\\n")));

  fn esc3(i: &str) -> IResult<&str, &str> {
    escaped(alpha, '\u{241b}', one_of("\"n"))(i)
  }
  assert_eq!(esc3("ab␛ncd;"), Ok((";", "ab␛ncd")));
}

#[cfg(feature = "alloc")]
fn to_s(i: Vec<u8>) -> String {
  String::from_utf8_lossy(&i).into_owned()
}

#[cfg(feature = "alloc")]
#[test]
fn complete_escape_transform() {
  use crate::character::alpha1 as alpha;

  fn esc(i: &[u8]) -> IResult<&[u8], String> {
    escaped_transform(
      alpha,
      '\\',
      alt((
        tag("\\").value(&b"\\"[..]),
        tag("\"").value(&b"\""[..]),
        tag("n").value(&b"\n"[..]),
      )),
    )
    .map(to_s)
    .parse(i)
  }

  assert_eq!(esc(&b"abcd;"[..]), Ok((&b";"[..], String::from("abcd"))));
  assert_eq!(
    esc(&b"ab\\\"cd;"[..]),
    Ok((&b";"[..], String::from("ab\"cd")))
  );
  assert_eq!(
    esc(&b"\\\"abcd;"[..]),
    Ok((&b";"[..], String::from("\"abcd")))
  );
  assert_eq!(esc(&b"\\n;"[..]), Ok((&b";"[..], String::from("\n"))));
  assert_eq!(
    esc(&b"ab\\\"12"[..]),
    Ok((&b"12"[..], String::from("ab\"")))
  );
  assert_eq!(
    esc(&b"AB\\"[..]),
    Err(Err::Error(error_position!(
      &b"\\"[..],
      ErrorKind::EscapedTransform
    )))
  );
  assert_eq!(
    esc(&b"AB\\A"[..]),
    Err(Err::Error(error_node_position!(
      &b"AB\\A"[..],
      ErrorKind::EscapedTransform,
      error_position!(&b"A"[..], ErrorKind::Tag)
    )))
  );

  fn esc2(i: &[u8]) -> IResult<&[u8], String> {
    escaped_transform(
      alpha,
      '&',
      alt((
        tag("egrave;").value("è".as_bytes()),
        tag("agrave;").value("à".as_bytes()),
      )),
    )
    .map(to_s)
    .parse(i)
  }
  assert_eq!(
    esc2(&b"ab&egrave;DEF;"[..]),
    Ok((&b";"[..], String::from("abèDEF")))
  );
  assert_eq!(
    esc2(&b"ab&egrave;D&agrave;EF;"[..]),
    Ok((&b";"[..], String::from("abèDàEF")))
  );
}

#[cfg(feature = "std")]
#[test]
fn complete_escape_transform_str() {
  use crate::character::alpha1 as alpha;

  fn esc(i: &str) -> IResult<&str, String> {
    escaped_transform(
      alpha,
      '\\',
      alt((
        tag("\\").value("\\"),
        tag("\"").value("\""),
        tag("n").value("\n"),
      )),
    )(i)
  }

  assert_eq!(esc("abcd;"), Ok((";", String::from("abcd"))));
  assert_eq!(esc("ab\\\"cd;"), Ok((";", String::from("ab\"cd"))));
  assert_eq!(esc("\\\"abcd;"), Ok((";", String::from("\"abcd"))));
  assert_eq!(esc("\\n;"), Ok((";", String::from("\n"))));
  assert_eq!(esc("ab\\\"12"), Ok(("12", String::from("ab\""))));
  assert_eq!(
    esc("AB\\"),
    Err(Err::Error(error_position!(
      "\\",
      ErrorKind::EscapedTransform
    )))
  );
  assert_eq!(
    esc("AB\\A"),
    Err(Err::Error(error_node_position!(
      "AB\\A",
      ErrorKind::EscapedTransform,
      error_position!("A", ErrorKind::Tag)
    )))
  );

  fn esc2(i: &str) -> IResult<&str, String> {
    escaped_transform(
      alpha,
      '&',
      alt((tag("egrave;").value("è"), tag("agrave;").value("à"))),
    )(i)
  }
  assert_eq!(esc2("ab&egrave;DEF;"), Ok((";", String::from("abèDEF"))));
  assert_eq!(
    esc2("ab&egrave;D&agrave;EF;"),
    Ok((";", String::from("abèDàEF")))
  );

  fn esc3(i: &str) -> IResult<&str, String> {
    escaped_transform(
      alpha,
      '␛',
      alt((tag("0").value("\0"), tag("n").value("\n"))),
    )(i)
  }
  assert_eq!(esc3("a␛0bc␛n"), Ok(("", String::from("a\0bc\n"))));
}

#[test]
fn streaming_any_str() {
  use super::any;
  assert_eq!(
    any::<_, (Streaming<&str>, ErrorKind), true>(Streaming("Ә")),
    Ok((Streaming(""), 'Ә'))
  );
}

#[test]
fn streaming_one_of_test() {
  fn f(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u8> {
    one_of("ab")(i)
  }

  let a = &b"abcd"[..];
  assert_eq!(f(Streaming(a)), Ok((Streaming(&b"bcd"[..]), b'a')));

  let b = &b"cde"[..];
  assert_eq!(
    f(Streaming(b)),
    Err(Err::Error(error_position!(Streaming(b), ErrorKind::OneOf)))
  );

  fn utf8(i: Streaming<&str>) -> IResult<Streaming<&str>, char> {
    one_of("+\u{FF0B}")(i)
  }

  assert!(utf8(Streaming("+")).is_ok());
  assert!(utf8(Streaming("\u{FF0B}")).is_ok());
}

#[test]
fn char_byteslice() {
  fn f(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u8> {
    one_of('c')(i)
  }

  let a = &b"abcd"[..];
  assert_eq!(
    f(Streaming(a)),
    Err(Err::Error(error_position!(Streaming(a), ErrorKind::OneOf)))
  );

  let b = &b"cde"[..];
  assert_eq!(f(Streaming(b)), Ok((Streaming(&b"de"[..]), b'c')));
}

#[test]
fn char_str() {
  fn f(i: Streaming<&str>) -> IResult<Streaming<&str>, char> {
    one_of('c')(i)
  }

  let a = &"abcd"[..];
  assert_eq!(
    f(Streaming(a)),
    Err(Err::Error(error_position!(Streaming(a), ErrorKind::OneOf)))
  );

  let b = &"cde"[..];
  assert_eq!(f(Streaming(b)), Ok((Streaming(&"de"[..]), 'c')));
}

#[test]
fn streaming_none_of_test() {
  fn f(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, u8> {
    none_of("ab")(i)
  }

  let a = &b"abcd"[..];
  assert_eq!(
    f(Streaming(a)),
    Err(Err::Error(error_position!(Streaming(a), ErrorKind::NoneOf)))
  );

  let b = &b"cde"[..];
  assert_eq!(f(Streaming(b)), Ok((Streaming(&b"de"[..]), b'c')));
}

#[test]
fn streaming_is_a() {
  fn a_or_b(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_while1("ab")(i)
  }

  let a = Streaming(&b"abcd"[..]);
  assert_eq!(a_or_b(a), Ok((Streaming(&b"cd"[..]), &b"ab"[..])));

  let b = Streaming(&b"bcde"[..]);
  assert_eq!(a_or_b(b), Ok((Streaming(&b"cde"[..]), &b"b"[..])));

  let c = Streaming(&b"cdef"[..]);
  assert_eq!(
    a_or_b(c),
    Err(Err::Error(error_position!(c, ErrorKind::TakeWhile1)))
  );

  let d = Streaming(&b"bacdef"[..]);
  assert_eq!(a_or_b(d), Ok((Streaming(&b"cdef"[..]), &b"ba"[..])));
}

#[test]
fn streaming_is_not() {
  fn a_or_b(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_till1("ab")(i)
  }

  let a = Streaming(&b"cdab"[..]);
  assert_eq!(a_or_b(a), Ok((Streaming(&b"ab"[..]), &b"cd"[..])));

  let b = Streaming(&b"cbde"[..]);
  assert_eq!(a_or_b(b), Ok((Streaming(&b"bde"[..]), &b"c"[..])));

  let c = Streaming(&b"abab"[..]);
  assert_eq!(
    a_or_b(c),
    Err(Err::Error(error_position!(c, ErrorKind::TakeTill1)))
  );

  let d = Streaming(&b"cdefba"[..]);
  assert_eq!(a_or_b(d), Ok((Streaming(&b"ba"[..]), &b"cdef"[..])));

  let e = Streaming(&b"e"[..]);
  assert_eq!(a_or_b(e), Err(Err::Incomplete(Needed::new(1))));
}

#[test]
fn streaming_take_until_incomplete() {
  fn y(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_until("end")(i)
  }
  assert_eq!(
    y(Streaming(&b"nd"[..])),
    Err(Err::Incomplete(Needed::Unknown))
  );
  assert_eq!(
    y(Streaming(&b"123"[..])),
    Err(Err::Incomplete(Needed::Unknown))
  );
  assert_eq!(
    y(Streaming(&b"123en"[..])),
    Err(Err::Incomplete(Needed::Unknown))
  );
}

#[test]
fn streaming_take_until_incomplete_s() {
  fn ys(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take_until("end")(i)
  }
  assert_eq!(
    ys(Streaming("123en")),
    Err(Err::Incomplete(Needed::Unknown))
  );
}

#[test]
fn streaming_recognize() {
  use crate::character::{
    alpha1 as alpha, alphanumeric1 as alphanumeric, digit1 as digit, hex_digit1 as hex_digit,
    multispace1 as multispace, oct_digit1 as oct_digit, space1 as space,
  };

  fn x(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    delimited(tag("<!--"), take(5_usize), tag("-->"))
      .recognize()
      .parse(i)
  }
  let r = x(Streaming(&b"<!-- abc --> aaa"[..]));
  assert_eq!(r, Ok((Streaming(&b" aaa"[..]), &b"<!-- abc -->"[..])));

  let semicolon = &b";"[..];

  fn ya(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    alpha.recognize().parse(i)
  }
  let ra = ya(Streaming(&b"abc;"[..]));
  assert_eq!(ra, Ok((Streaming(semicolon), &b"abc"[..])));

  fn yd(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    digit.recognize().parse(i)
  }
  let rd = yd(Streaming(&b"123;"[..]));
  assert_eq!(rd, Ok((Streaming(semicolon), &b"123"[..])));

  fn yhd(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    hex_digit.recognize().parse(i)
  }
  let rhd = yhd(Streaming(&b"123abcDEF;"[..]));
  assert_eq!(rhd, Ok((Streaming(semicolon), &b"123abcDEF"[..])));

  fn yod(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    oct_digit.recognize().parse(i)
  }
  let rod = yod(Streaming(&b"1234567;"[..]));
  assert_eq!(rod, Ok((Streaming(semicolon), &b"1234567"[..])));

  fn yan(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    alphanumeric.recognize().parse(i)
  }
  let ran = yan(Streaming(&b"123abc;"[..]));
  assert_eq!(ran, Ok((Streaming(semicolon), &b"123abc"[..])));

  fn ys(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    space.recognize().parse(i)
  }
  let rs = ys(Streaming(&b" \t;"[..]));
  assert_eq!(rs, Ok((Streaming(semicolon), &b" \t"[..])));

  fn yms(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    multispace.recognize().parse(i)
  }
  let rms = yms(Streaming(&b" \t\r\n;"[..]));
  assert_eq!(rms, Ok((Streaming(semicolon), &b" \t\r\n"[..])));
}

#[test]
fn streaming_take_while() {
  fn f(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_while(AsChar::is_alpha)(i)
  }
  let a = &b""[..];
  let b = &b"abcd"[..];
  let c = &b"abcd123"[..];
  let d = &b"123"[..];

  assert_eq!(f(Streaming(a)), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming(b)), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming(c)), Ok((Streaming(d), b)));
  assert_eq!(f(Streaming(d)), Ok((Streaming(d), a)));
}

#[test]
fn streaming_take_while1() {
  fn f(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_while1(AsChar::is_alpha)(i)
  }
  let a = &b""[..];
  let b = &b"abcd"[..];
  let c = &b"abcd123"[..];
  let d = &b"123"[..];

  assert_eq!(f(Streaming(a)), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming(b)), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming(c)), Ok((Streaming(&b"123"[..]), b)));
  assert_eq!(
    f(Streaming(d)),
    Err(Err::Error(error_position!(
      Streaming(d),
      ErrorKind::TakeWhile1
    )))
  );
}

#[test]
fn streaming_take_while_m_n() {
  fn x(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_while_m_n(2, 4, AsChar::is_alpha)(i)
  }
  let a = &b""[..];
  let b = &b"a"[..];
  let c = &b"abc"[..];
  let d = &b"abc123"[..];
  let e = &b"abcde"[..];
  let f = &b"123"[..];

  assert_eq!(x(Streaming(a)), Err(Err::Incomplete(Needed::new(2))));
  assert_eq!(x(Streaming(b)), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(x(Streaming(c)), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(x(Streaming(d)), Ok((Streaming(&b"123"[..]), c)));
  assert_eq!(x(Streaming(e)), Ok((Streaming(&b"e"[..]), &b"abcd"[..])));
  assert_eq!(
    x(Streaming(f)),
    Err(Err::Error(error_position!(
      Streaming(f),
      ErrorKind::TakeWhileMN
    )))
  );
}

#[test]
fn streaming_take_till() {
  fn f(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_till(AsChar::is_alpha)(i)
  }
  let a = &b""[..];
  let b = &b"abcd"[..];
  let c = &b"123abcd"[..];
  let d = &b"123"[..];

  assert_eq!(f(Streaming(a)), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming(b)), Ok((Streaming(&b"abcd"[..]), &b""[..])));
  assert_eq!(f(Streaming(c)), Ok((Streaming(&b"abcd"[..]), &b"123"[..])));
  assert_eq!(f(Streaming(d)), Err(Err::Incomplete(Needed::new(1))));
}

#[test]
fn streaming_take_till1() {
  fn f(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_till1(AsChar::is_alpha)(i)
  }
  let a = &b""[..];
  let b = &b"abcd"[..];
  let c = &b"123abcd"[..];
  let d = &b"123"[..];

  assert_eq!(f(Streaming(a)), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(
    f(Streaming(b)),
    Err(Err::Error(error_position!(
      Streaming(b),
      ErrorKind::TakeTill1
    )))
  );
  assert_eq!(f(Streaming(c)), Ok((Streaming(&b"abcd"[..]), &b"123"[..])));
  assert_eq!(f(Streaming(d)), Err(Err::Incomplete(Needed::new(1))));
}

#[test]
fn streaming_take_while_utf8() {
  fn f(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take_while(|c| c != '點')(i)
  }

  assert_eq!(f(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming("abcd")), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming("abcd點")), Ok((Streaming("點"), "abcd")));
  assert_eq!(f(Streaming("abcd點a")), Ok((Streaming("點a"), "abcd")));

  fn g(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take_while(|c| c == '點')(i)
  }

  assert_eq!(g(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(g(Streaming("點abcd")), Ok((Streaming("abcd"), "點")));
  assert_eq!(g(Streaming("點點點a")), Ok((Streaming("a"), "點點點")));
}

#[test]
fn streaming_take_till_utf8() {
  fn f(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take_till(|c| c == '點')(i)
  }

  assert_eq!(f(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming("abcd")), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(Streaming("abcd點")), Ok((Streaming("點"), "abcd")));
  assert_eq!(f(Streaming("abcd點a")), Ok((Streaming("點a"), "abcd")));

  fn g(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take_till(|c| c != '點')(i)
  }

  assert_eq!(g(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(g(Streaming("點abcd")), Ok((Streaming("abcd"), "點")));
  assert_eq!(g(Streaming("點點點a")), Ok((Streaming("a"), "點點點")));
}

#[test]
fn streaming_take_utf8() {
  fn f(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take(3_usize)(i)
  }

  assert_eq!(f(Streaming("")), Err(Err::Incomplete(Needed::Unknown)));
  assert_eq!(f(Streaming("ab")), Err(Err::Incomplete(Needed::Unknown)));
  assert_eq!(f(Streaming("點")), Err(Err::Incomplete(Needed::Unknown)));
  assert_eq!(f(Streaming("ab點cd")), Ok((Streaming("cd"), "ab點")));
  assert_eq!(f(Streaming("a點bcd")), Ok((Streaming("cd"), "a點b")));
  assert_eq!(f(Streaming("a點b")), Ok((Streaming(""), "a點b")));

  fn g(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take_while(|c| c == '點')(i)
  }

  assert_eq!(g(Streaming("")), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(g(Streaming("點abcd")), Ok((Streaming("abcd"), "點")));
  assert_eq!(g(Streaming("點點點a")), Ok((Streaming("a"), "點點點")));
}

#[test]
fn streaming_take_while_m_n_utf8() {
  fn parser(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take_while_m_n(1, 1, |c| c == 'A' || c == '😃')(i)
  }
  assert_eq!(parser(Streaming("A!")), Ok((Streaming("!"), "A")));
  assert_eq!(parser(Streaming("😃!")), Ok((Streaming("!"), "😃")));
}

#[test]
fn streaming_take_while_m_n_utf8_full_match() {
  fn parser(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    take_while_m_n(1, 1, |c: char| c.is_alphabetic())(i)
  }
  assert_eq!(parser(Streaming("øn")), Ok((Streaming("n"), "ø")));
}

#[test]
#[cfg(feature = "std")]
fn streaming_recognize_take_while() {
  fn x(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    take_while(AsChar::is_alphanum)(i)
  }
  fn y(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    x.recognize().parse(i)
  }
  assert_eq!(
    x(Streaming(&b"ab."[..])),
    Ok((Streaming(&b"."[..]), &b"ab"[..]))
  );
  assert_eq!(
    y(Streaming(&b"ab."[..])),
    Ok((Streaming(&b"."[..]), &b"ab"[..]))
  );
}

#[test]
fn streaming_length_bytes() {
  use crate::number::le_u8;

  fn x(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    length_data(le_u8)(i)
  }
  assert_eq!(
    x(Streaming(b"\x02..>>")),
    Ok((Streaming(&b">>"[..]), &b".."[..]))
  );
  assert_eq!(
    x(Streaming(b"\x02..")),
    Ok((Streaming(&[][..]), &b".."[..]))
  );
  assert_eq!(x(Streaming(b"\x02.")), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(x(Streaming(b"\x02")), Err(Err::Incomplete(Needed::new(2))));

  fn y(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    let (i, _) = tag("magic")(i)?;
    length_data(le_u8)(i)
  }
  assert_eq!(
    y(Streaming(b"magic\x02..>>")),
    Ok((Streaming(&b">>"[..]), &b".."[..]))
  );
  assert_eq!(
    y(Streaming(b"magic\x02..")),
    Ok((Streaming(&[][..]), &b".."[..]))
  );
  assert_eq!(
    y(Streaming(b"magic\x02.")),
    Err(Err::Incomplete(Needed::new(1)))
  );
  assert_eq!(
    y(Streaming(b"magic\x02")),
    Err(Err::Incomplete(Needed::new(2)))
  );
}

#[cfg(feature = "alloc")]
#[test]
fn streaming_case_insensitive() {
  fn test(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    tag_no_case("ABcd")(i)
  }
  assert_eq!(
    test(Streaming(&b"aBCdefgh"[..])),
    Ok((Streaming(&b"efgh"[..]), &b"aBCd"[..]))
  );
  assert_eq!(
    test(Streaming(&b"abcdefgh"[..])),
    Ok((Streaming(&b"efgh"[..]), &b"abcd"[..]))
  );
  assert_eq!(
    test(Streaming(&b"ABCDefgh"[..])),
    Ok((Streaming(&b"efgh"[..]), &b"ABCD"[..]))
  );
  assert_eq!(
    test(Streaming(&b"ab"[..])),
    Err(Err::Incomplete(Needed::new(2)))
  );
  assert_eq!(
    test(Streaming(&b"Hello"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"Hello"[..]),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    test(Streaming(&b"Hel"[..])),
    Err(Err::Error(error_position!(
      Streaming(&b"Hel"[..]),
      ErrorKind::Tag
    )))
  );

  fn test2(i: Streaming<&str>) -> IResult<Streaming<&str>, &str> {
    tag_no_case("ABcd")(i)
  }
  assert_eq!(
    test2(Streaming("aBCdefgh")),
    Ok((Streaming("efgh"), "aBCd"))
  );
  assert_eq!(
    test2(Streaming("abcdefgh")),
    Ok((Streaming("efgh"), "abcd"))
  );
  assert_eq!(
    test2(Streaming("ABCDefgh")),
    Ok((Streaming("efgh"), "ABCD"))
  );
  assert_eq!(test2(Streaming("ab")), Err(Err::Incomplete(Needed::new(2))));
  assert_eq!(
    test2(Streaming("Hello")),
    Err(Err::Error(error_position!(
      Streaming("Hello"),
      ErrorKind::Tag
    )))
  );
  assert_eq!(
    test2(Streaming("Hel")),
    Err(Err::Error(error_position!(
      Streaming("Hel"),
      ErrorKind::Tag
    )))
  );
}

#[test]
fn streaming_tag_fixed_size_array() {
  fn test(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    tag([0x42])(i)
  }
  fn test2(i: Streaming<&[u8]>) -> IResult<Streaming<&[u8]>, &[u8]> {
    tag(&[0x42])(i)
  }
  let input = Streaming(&[0x42, 0x00][..]);
  assert_eq!(test(input), Ok((Streaming(&b"\x00"[..]), &b"\x42"[..])));
  assert_eq!(test2(input), Ok((Streaming(&b"\x00"[..]), &b"\x42"[..])));
}
