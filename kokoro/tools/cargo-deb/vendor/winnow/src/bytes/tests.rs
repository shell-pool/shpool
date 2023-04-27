use super::*;

#[cfg(feature = "std")]
use proptest::prelude::*;

use crate::bytes::tag;
use crate::error::ErrMode;
use crate::error::Error;
use crate::error::ErrorKind;
use crate::error::Needed;
use crate::multi::length_data;
use crate::sequence::delimited;
use crate::stream::AsChar;
use crate::IResult;
use crate::Parser;
use crate::Partial;

#[test]
fn complete_take_while_m_n_utf8_all_matching() {
    let result: IResult<&str, &str> = take_while_m_n(1, 4, |c: char| c.is_alphabetic())("øn");
    assert_eq!(result, Ok(("", "øn")));
}

#[test]
fn complete_take_while_m_n_utf8_all_matching_substring() {
    let result: IResult<&str, &str> = take_while_m_n(1, 1, |c: char| c.is_alphabetic())("øn");
    assert_eq!(result, Ok(("n", "ø")));
}

#[cfg(feature = "std")]
fn model_complete_take_while_m_n(
    m: usize,
    n: usize,
    valid: usize,
    input: &str,
) -> IResult<&str, &str> {
    if n < m {
        Err(crate::error::ErrMode::from_error_kind(
            input,
            crate::error::ErrorKind::TakeWhileMN,
        ))
    } else if m <= valid {
        let offset = n.min(valid);
        Ok((&input[offset..], &input[0..offset]))
    } else {
        Err(crate::error::ErrMode::from_error_kind(
            input,
            crate::error::ErrorKind::TakeWhileMN,
        ))
    }
}

#[cfg(feature = "std")]
proptest! {
  #[test]
  #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
  fn complete_take_while_m_n_bounds(m in 0..20usize, n in 0..20usize, valid in 0..20usize, invalid in 0..20usize) {
      let input = format!("{:a<valid$}{:b<invalid$}", "", "", valid=valid, invalid=invalid);
      let expected = model_complete_take_while_m_n(m, n, valid, &input);
      let actual = take_while_m_n(m, n, |c: char| c == 'a')(input.as_str());
      assert_eq!(expected, actual);
  }
}

#[test]
fn partial_any_str() {
    use super::any;
    assert_eq!(
        any::<_, Error<Partial<&str>>>(Partial::new("Ә")),
        Ok((Partial::new(""), 'Ә'))
    );
}

#[test]
fn partial_one_of_test() {
    fn f(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u8> {
        one_of("ab")(i)
    }

    let a = &b"abcd"[..];
    assert_eq!(f(Partial::new(a)), Ok((Partial::new(&b"bcd"[..]), b'a')));

    let b = &b"cde"[..];
    assert_eq!(
        f(Partial::new(b)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(b),
            ErrorKind::OneOf
        )))
    );

    fn utf8(i: Partial<&str>) -> IResult<Partial<&str>, char> {
        one_of("+\u{FF0B}")(i)
    }

    assert!(utf8(Partial::new("+")).is_ok());
    assert!(utf8(Partial::new("\u{FF0B}")).is_ok());
}

#[test]
fn char_byteslice() {
    fn f(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u8> {
        one_of('c')(i)
    }

    let a = &b"abcd"[..];
    assert_eq!(
        f(Partial::new(a)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(a),
            ErrorKind::OneOf
        )))
    );

    let b = &b"cde"[..];
    assert_eq!(f(Partial::new(b)), Ok((Partial::new(&b"de"[..]), b'c')));
}

#[test]
fn char_str() {
    fn f(i: Partial<&str>) -> IResult<Partial<&str>, char> {
        one_of('c')(i)
    }

    let a = "abcd";
    assert_eq!(
        f(Partial::new(a)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(a),
            ErrorKind::OneOf
        )))
    );

    let b = "cde";
    assert_eq!(f(Partial::new(b)), Ok((Partial::new("de"), 'c')));
}

#[test]
fn partial_none_of_test() {
    fn f(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u8> {
        none_of("ab")(i)
    }

    let a = &b"abcd"[..];
    assert_eq!(
        f(Partial::new(a)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(a),
            ErrorKind::NoneOf
        )))
    );

    let b = &b"cde"[..];
    assert_eq!(f(Partial::new(b)), Ok((Partial::new(&b"de"[..]), b'c')));
}

#[test]
fn partial_is_a() {
    fn a_or_b(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_while1("ab")(i)
    }

    let a = Partial::new(&b"abcd"[..]);
    assert_eq!(a_or_b(a), Ok((Partial::new(&b"cd"[..]), &b"ab"[..])));

    let b = Partial::new(&b"bcde"[..]);
    assert_eq!(a_or_b(b), Ok((Partial::new(&b"cde"[..]), &b"b"[..])));

    let c = Partial::new(&b"cdef"[..]);
    assert_eq!(
        a_or_b(c),
        Err(ErrMode::Backtrack(error_position!(
            c,
            ErrorKind::TakeWhile1
        )))
    );

    let d = Partial::new(&b"bacdef"[..]);
    assert_eq!(a_or_b(d), Ok((Partial::new(&b"cdef"[..]), &b"ba"[..])));
}

#[test]
fn partial_is_not() {
    fn a_or_b(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_till1("ab")(i)
    }

    let a = Partial::new(&b"cdab"[..]);
    assert_eq!(a_or_b(a), Ok((Partial::new(&b"ab"[..]), &b"cd"[..])));

    let b = Partial::new(&b"cbde"[..]);
    assert_eq!(a_or_b(b), Ok((Partial::new(&b"bde"[..]), &b"c"[..])));

    let c = Partial::new(&b"abab"[..]);
    assert_eq!(
        a_or_b(c),
        Err(ErrMode::Backtrack(error_position!(c, ErrorKind::TakeTill1)))
    );

    let d = Partial::new(&b"cdefba"[..]);
    assert_eq!(a_or_b(d), Ok((Partial::new(&b"ba"[..]), &b"cdef"[..])));

    let e = Partial::new(&b"e"[..]);
    assert_eq!(a_or_b(e), Err(ErrMode::Incomplete(Needed::new(1))));
}

#[test]
fn partial_take_until_incomplete() {
    fn y(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_until0("end")(i)
    }
    assert_eq!(
        y(Partial::new(&b"nd"[..])),
        Err(ErrMode::Incomplete(Needed::Unknown))
    );
    assert_eq!(
        y(Partial::new(&b"123"[..])),
        Err(ErrMode::Incomplete(Needed::Unknown))
    );
    assert_eq!(
        y(Partial::new(&b"123en"[..])),
        Err(ErrMode::Incomplete(Needed::Unknown))
    );
}

#[test]
fn partial_take_until_incomplete_s() {
    fn ys(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_until0("end")(i)
    }
    assert_eq!(
        ys(Partial::new("123en")),
        Err(ErrMode::Incomplete(Needed::Unknown))
    );
}

#[test]
fn partial_recognize() {
    use crate::character::{
        alpha1 as alpha, alphanumeric1 as alphanumeric, digit1 as digit, hex_digit1 as hex_digit,
        multispace1 as multispace, oct_digit1 as oct_digit, space1 as space,
    };

    fn x(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        delimited(tag("<!--"), take(5_usize), tag("-->"))
            .recognize()
            .parse_next(i)
    }
    let r = x(Partial::new(&b"<!-- abc --> aaa"[..]));
    assert_eq!(r, Ok((Partial::new(&b" aaa"[..]), &b"<!-- abc -->"[..])));

    let semicolon = &b";"[..];

    fn ya(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        alpha.recognize().parse_next(i)
    }
    let ra = ya(Partial::new(&b"abc;"[..]));
    assert_eq!(ra, Ok((Partial::new(semicolon), &b"abc"[..])));

    fn yd(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        digit.recognize().parse_next(i)
    }
    let rd = yd(Partial::new(&b"123;"[..]));
    assert_eq!(rd, Ok((Partial::new(semicolon), &b"123"[..])));

    fn yhd(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        hex_digit.recognize().parse_next(i)
    }
    let rhd = yhd(Partial::new(&b"123abcDEF;"[..]));
    assert_eq!(rhd, Ok((Partial::new(semicolon), &b"123abcDEF"[..])));

    fn yod(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        oct_digit.recognize().parse_next(i)
    }
    let rod = yod(Partial::new(&b"1234567;"[..]));
    assert_eq!(rod, Ok((Partial::new(semicolon), &b"1234567"[..])));

    fn yan(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        alphanumeric.recognize().parse_next(i)
    }
    let ran = yan(Partial::new(&b"123abc;"[..]));
    assert_eq!(ran, Ok((Partial::new(semicolon), &b"123abc"[..])));

    fn ys(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        space.recognize().parse_next(i)
    }
    let rs = ys(Partial::new(&b" \t;"[..]));
    assert_eq!(rs, Ok((Partial::new(semicolon), &b" \t"[..])));

    fn yms(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        multispace.recognize().parse_next(i)
    }
    let rms = yms(Partial::new(&b" \t\r\n;"[..]));
    assert_eq!(rms, Ok((Partial::new(semicolon), &b" \t\r\n"[..])));
}

#[test]
fn partial_take_while0() {
    fn f(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_while0(AsChar::is_alpha)(i)
    }
    let a = &b""[..];
    let b = &b"abcd"[..];
    let c = &b"abcd123"[..];
    let d = &b"123"[..];

    assert_eq!(f(Partial::new(a)), Err(ErrMode::Incomplete(Needed::new(1))));
    assert_eq!(f(Partial::new(b)), Err(ErrMode::Incomplete(Needed::new(1))));
    assert_eq!(f(Partial::new(c)), Ok((Partial::new(d), b)));
    assert_eq!(f(Partial::new(d)), Ok((Partial::new(d), a)));
}

#[test]
fn partial_take_while1() {
    fn f(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_while1(AsChar::is_alpha)(i)
    }
    let a = &b""[..];
    let b = &b"abcd"[..];
    let c = &b"abcd123"[..];
    let d = &b"123"[..];

    assert_eq!(f(Partial::new(a)), Err(ErrMode::Incomplete(Needed::new(1))));
    assert_eq!(f(Partial::new(b)), Err(ErrMode::Incomplete(Needed::new(1))));
    assert_eq!(f(Partial::new(c)), Ok((Partial::new(&b"123"[..]), b)));
    assert_eq!(
        f(Partial::new(d)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(d),
            ErrorKind::TakeWhile1
        )))
    );
}

#[test]
fn partial_take_while_m_n() {
    fn x(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_while_m_n(2, 4, AsChar::is_alpha)(i)
    }
    let a = &b""[..];
    let b = &b"a"[..];
    let c = &b"abc"[..];
    let d = &b"abc123"[..];
    let e = &b"abcde"[..];
    let f = &b"123"[..];

    assert_eq!(x(Partial::new(a)), Err(ErrMode::Incomplete(Needed::new(2))));
    assert_eq!(x(Partial::new(b)), Err(ErrMode::Incomplete(Needed::new(1))));
    assert_eq!(x(Partial::new(c)), Err(ErrMode::Incomplete(Needed::new(1))));
    assert_eq!(x(Partial::new(d)), Ok((Partial::new(&b"123"[..]), c)));
    assert_eq!(
        x(Partial::new(e)),
        Ok((Partial::new(&b"e"[..]), &b"abcd"[..]))
    );
    assert_eq!(
        x(Partial::new(f)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(f),
            ErrorKind::TakeWhileMN
        )))
    );
}

#[test]
fn partial_take_till0() {
    fn f(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_till0(AsChar::is_alpha)(i)
    }
    let a = &b""[..];
    let b = &b"abcd"[..];
    let c = &b"123abcd"[..];
    let d = &b"123"[..];

    assert_eq!(f(Partial::new(a)), Err(ErrMode::Incomplete(Needed::new(1))));
    assert_eq!(
        f(Partial::new(b)),
        Ok((Partial::new(&b"abcd"[..]), &b""[..]))
    );
    assert_eq!(
        f(Partial::new(c)),
        Ok((Partial::new(&b"abcd"[..]), &b"123"[..]))
    );
    assert_eq!(f(Partial::new(d)), Err(ErrMode::Incomplete(Needed::new(1))));
}

#[test]
fn partial_take_till1() {
    fn f(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_till1(AsChar::is_alpha)(i)
    }
    let a = &b""[..];
    let b = &b"abcd"[..];
    let c = &b"123abcd"[..];
    let d = &b"123"[..];

    assert_eq!(f(Partial::new(a)), Err(ErrMode::Incomplete(Needed::new(1))));
    assert_eq!(
        f(Partial::new(b)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(b),
            ErrorKind::TakeTill1
        )))
    );
    assert_eq!(
        f(Partial::new(c)),
        Ok((Partial::new(&b"abcd"[..]), &b"123"[..]))
    );
    assert_eq!(f(Partial::new(d)), Err(ErrMode::Incomplete(Needed::new(1))));
}

#[test]
fn partial_take_while_utf8() {
    fn f(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_while0(|c| c != '點')(i)
    }

    assert_eq!(
        f(Partial::new("")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        f(Partial::new("abcd")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(f(Partial::new("abcd點")), Ok((Partial::new("點"), "abcd")));
    assert_eq!(
        f(Partial::new("abcd點a")),
        Ok((Partial::new("點a"), "abcd"))
    );

    fn g(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_while0(|c| c == '點')(i)
    }

    assert_eq!(
        g(Partial::new("")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(g(Partial::new("點abcd")), Ok((Partial::new("abcd"), "點")));
    assert_eq!(
        g(Partial::new("點點點a")),
        Ok((Partial::new("a"), "點點點"))
    );
}

#[test]
fn partial_take_till0_utf8() {
    fn f(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_till0(|c| c == '點')(i)
    }

    assert_eq!(
        f(Partial::new("")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        f(Partial::new("abcd")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(f(Partial::new("abcd點")), Ok((Partial::new("點"), "abcd")));
    assert_eq!(
        f(Partial::new("abcd點a")),
        Ok((Partial::new("點a"), "abcd"))
    );

    fn g(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_till0(|c| c != '點')(i)
    }

    assert_eq!(
        g(Partial::new("")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(g(Partial::new("點abcd")), Ok((Partial::new("abcd"), "點")));
    assert_eq!(
        g(Partial::new("點點點a")),
        Ok((Partial::new("a"), "點點點"))
    );
}

#[test]
fn partial_take_utf8() {
    fn f(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take(3_usize)(i)
    }

    assert_eq!(
        f(Partial::new("")),
        Err(ErrMode::Incomplete(Needed::Unknown))
    );
    assert_eq!(
        f(Partial::new("ab")),
        Err(ErrMode::Incomplete(Needed::Unknown))
    );
    assert_eq!(
        f(Partial::new("點")),
        Err(ErrMode::Incomplete(Needed::Unknown))
    );
    assert_eq!(f(Partial::new("ab點cd")), Ok((Partial::new("cd"), "ab點")));
    assert_eq!(f(Partial::new("a點bcd")), Ok((Partial::new("cd"), "a點b")));
    assert_eq!(f(Partial::new("a點b")), Ok((Partial::new(""), "a點b")));

    fn g(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_while0(|c| c == '點')(i)
    }

    assert_eq!(
        g(Partial::new("")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(g(Partial::new("點abcd")), Ok((Partial::new("abcd"), "點")));
    assert_eq!(
        g(Partial::new("點點點a")),
        Ok((Partial::new("a"), "點點點"))
    );
}

#[test]
fn partial_take_while_m_n_utf8_fixed() {
    fn parser(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_while_m_n(1, 1, |c| c == 'A' || c == '😃')(i)
    }
    assert_eq!(parser(Partial::new("A!")), Ok((Partial::new("!"), "A")));
    assert_eq!(parser(Partial::new("😃!")), Ok((Partial::new("!"), "😃")));
}

#[test]
fn partial_take_while_m_n_utf8_range() {
    fn parser(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_while_m_n(1, 2, |c| c == 'A' || c == '😃')(i)
    }
    assert_eq!(parser(Partial::new("A!")), Ok((Partial::new("!"), "A")));
    assert_eq!(parser(Partial::new("😃!")), Ok((Partial::new("!"), "😃")));
}

#[test]
fn partial_take_while_m_n_utf8_full_match_fixed() {
    fn parser(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_while_m_n(1, 1, |c: char| c.is_alphabetic())(i)
    }
    assert_eq!(parser(Partial::new("øn")), Ok((Partial::new("n"), "ø")));
}

#[test]
fn partial_take_while_m_n_utf8_full_match_range() {
    fn parser(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        take_while_m_n(1, 2, |c: char| c.is_alphabetic())(i)
    }
    assert_eq!(parser(Partial::new("øn")), Ok((Partial::new(""), "øn")));
}

#[test]
#[cfg(feature = "std")]
fn partial_recognize_take_while0() {
    fn x(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take_while0(AsChar::is_alphanum)(i)
    }
    fn y(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        x.recognize().parse_next(i)
    }
    assert_eq!(
        x(Partial::new(&b"ab."[..])),
        Ok((Partial::new(&b"."[..]), &b"ab"[..]))
    );
    assert_eq!(
        y(Partial::new(&b"ab."[..])),
        Ok((Partial::new(&b"."[..]), &b"ab"[..]))
    );
}

#[test]
fn partial_length_bytes() {
    use crate::number::le_u8;

    fn x(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        length_data(le_u8)(i)
    }
    assert_eq!(
        x(Partial::new(b"\x02..>>")),
        Ok((Partial::new(&b">>"[..]), &b".."[..]))
    );
    assert_eq!(
        x(Partial::new(b"\x02..")),
        Ok((Partial::new(&[][..]), &b".."[..]))
    );
    assert_eq!(
        x(Partial::new(b"\x02.")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        x(Partial::new(b"\x02")),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );

    fn y(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        let (i, _) = tag("magic")(i)?;
        length_data(le_u8)(i)
    }
    assert_eq!(
        y(Partial::new(b"magic\x02..>>")),
        Ok((Partial::new(&b">>"[..]), &b".."[..]))
    );
    assert_eq!(
        y(Partial::new(b"magic\x02..")),
        Ok((Partial::new(&[][..]), &b".."[..]))
    );
    assert_eq!(
        y(Partial::new(b"magic\x02.")),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        y(Partial::new(b"magic\x02")),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
}

#[cfg(feature = "alloc")]
#[test]
fn partial_case_insensitive() {
    fn test(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        tag_no_case("ABcd")(i)
    }
    assert_eq!(
        test(Partial::new(&b"aBCdefgh"[..])),
        Ok((Partial::new(&b"efgh"[..]), &b"aBCd"[..]))
    );
    assert_eq!(
        test(Partial::new(&b"abcdefgh"[..])),
        Ok((Partial::new(&b"efgh"[..]), &b"abcd"[..]))
    );
    assert_eq!(
        test(Partial::new(&b"ABCDefgh"[..])),
        Ok((Partial::new(&b"efgh"[..]), &b"ABCD"[..]))
    );
    assert_eq!(
        test(Partial::new(&b"ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        test(Partial::new(&b"Hello"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"Hello"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        test(Partial::new(&b"Hel"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"Hel"[..]),
            ErrorKind::Tag
        )))
    );

    fn test2(i: Partial<&str>) -> IResult<Partial<&str>, &str> {
        tag_no_case("ABcd")(i)
    }
    assert_eq!(
        test2(Partial::new("aBCdefgh")),
        Ok((Partial::new("efgh"), "aBCd"))
    );
    assert_eq!(
        test2(Partial::new("abcdefgh")),
        Ok((Partial::new("efgh"), "abcd"))
    );
    assert_eq!(
        test2(Partial::new("ABCDefgh")),
        Ok((Partial::new("efgh"), "ABCD"))
    );
    assert_eq!(
        test2(Partial::new("ab")),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        test2(Partial::new("Hello")),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new("Hello"),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        test2(Partial::new("Hel")),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new("Hel"),
            ErrorKind::Tag
        )))
    );
}

#[test]
fn partial_tag_fixed_size_array() {
    fn test(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        tag([0x42])(i)
    }
    fn test2(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        tag(&[0x42])(i)
    }
    let input = Partial::new(&[0x42, 0x00][..]);
    assert_eq!(test(input), Ok((Partial::new(&b"\x00"[..]), &b"\x42"[..])));
    assert_eq!(test2(input), Ok((Partial::new(&b"\x00"[..]), &b"\x42"[..])));
}
