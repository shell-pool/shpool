use super::*;

mod complete {
    use super::*;
    use crate::branch::alt;
    use crate::bytes::none_of;
    use crate::bytes::one_of;
    use crate::combinator::opt;
    use crate::error::ErrMode;
    use crate::error::Error;
    use crate::error::ErrorKind;
    use crate::stream::ParseSlice;
    #[cfg(feature = "alloc")]
    use crate::{lib::std::string::String, lib::std::vec::Vec};
    use proptest::prelude::*;

    macro_rules! assert_parse(
    ($left: expr, $right: expr) => {
      let res: $crate::IResult<_, _, Error<_>> = $left;
      assert_eq!(res, $right);
    };
  );

    #[test]
    fn character() {
        let empty: &[u8] = b"";
        let a: &[u8] = b"abcd";
        let b: &[u8] = b"1234";
        let c: &[u8] = b"a123";
        let d: &[u8] = "azé12".as_bytes();
        let e: &[u8] = b" ";
        let f: &[u8] = b" ;";
        //assert_eq!(alpha1::<_, Error>(a), Err(ErrMode::Incomplete(Needed::Size(1))));
        assert_parse!(alpha1(a), Ok((empty, a)));
        assert_eq!(
            alpha1(b),
            Err(ErrMode::Backtrack(Error::new(b, ErrorKind::Alpha)))
        );
        assert_eq!(alpha1::<_, Error<_>>(c), Ok((&c[1..], &b"a"[..])));
        assert_eq!(alpha1::<_, Error<_>>(d), Ok(("é12".as_bytes(), &b"az"[..])));
        assert_eq!(
            digit1(a),
            Err(ErrMode::Backtrack(Error::new(a, ErrorKind::Digit)))
        );
        assert_eq!(digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(
            digit1(c),
            Err(ErrMode::Backtrack(Error::new(c, ErrorKind::Digit)))
        );
        assert_eq!(
            digit1(d),
            Err(ErrMode::Backtrack(Error::new(d, ErrorKind::Digit)))
        );
        assert_eq!(hex_digit1::<_, Error<_>>(a), Ok((empty, a)));
        assert_eq!(hex_digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(hex_digit1::<_, Error<_>>(c), Ok((empty, c)));
        assert_eq!(
            hex_digit1::<_, Error<_>>(d),
            Ok(("zé12".as_bytes(), &b"a"[..]))
        );
        assert_eq!(
            hex_digit1(e),
            Err(ErrMode::Backtrack(Error::new(e, ErrorKind::HexDigit)))
        );
        assert_eq!(
            oct_digit1(a),
            Err(ErrMode::Backtrack(Error::new(a, ErrorKind::OctDigit)))
        );
        assert_eq!(oct_digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(
            oct_digit1(c),
            Err(ErrMode::Backtrack(Error::new(c, ErrorKind::OctDigit)))
        );
        assert_eq!(
            oct_digit1(d),
            Err(ErrMode::Backtrack(Error::new(d, ErrorKind::OctDigit)))
        );
        assert_eq!(alphanumeric1::<_, Error<_>>(a), Ok((empty, a)));
        //assert_eq!(fix_error!(b,(), alphanumeric), Ok((empty, b)));
        assert_eq!(alphanumeric1::<_, Error<_>>(c), Ok((empty, c)));
        assert_eq!(
            alphanumeric1::<_, Error<_>>(d),
            Ok(("é12".as_bytes(), &b"az"[..]))
        );
        assert_eq!(space1::<_, Error<_>>(e), Ok((empty, e)));
        assert_eq!(space1::<_, Error<_>>(f), Ok((&b";"[..], &b" "[..])));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn character_s() {
        let empty = "";
        let a = "abcd";
        let b = "1234";
        let c = "a123";
        let d = "azé12";
        let e = " ";
        assert_eq!(alpha1::<_, Error<_>>(a), Ok((empty, a)));
        assert_eq!(
            alpha1(b),
            Err(ErrMode::Backtrack(Error::new(b, ErrorKind::Alpha)))
        );
        assert_eq!(alpha1::<_, Error<_>>(c), Ok((&c[1..], "a")));
        assert_eq!(alpha1::<_, Error<_>>(d), Ok(("é12", "az")));
        assert_eq!(
            digit1(a),
            Err(ErrMode::Backtrack(Error::new(a, ErrorKind::Digit)))
        );
        assert_eq!(digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(
            digit1(c),
            Err(ErrMode::Backtrack(Error::new(c, ErrorKind::Digit)))
        );
        assert_eq!(
            digit1(d),
            Err(ErrMode::Backtrack(Error::new(d, ErrorKind::Digit)))
        );
        assert_eq!(hex_digit1::<_, Error<_>>(a), Ok((empty, a)));
        assert_eq!(hex_digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(hex_digit1::<_, Error<_>>(c), Ok((empty, c)));
        assert_eq!(hex_digit1::<_, Error<_>>(d), Ok(("zé12", "a")));
        assert_eq!(
            hex_digit1(e),
            Err(ErrMode::Backtrack(Error::new(e, ErrorKind::HexDigit)))
        );
        assert_eq!(
            oct_digit1(a),
            Err(ErrMode::Backtrack(Error::new(a, ErrorKind::OctDigit)))
        );
        assert_eq!(oct_digit1::<_, Error<_>>(b), Ok((empty, b)));
        assert_eq!(
            oct_digit1(c),
            Err(ErrMode::Backtrack(Error::new(c, ErrorKind::OctDigit)))
        );
        assert_eq!(
            oct_digit1(d),
            Err(ErrMode::Backtrack(Error::new(d, ErrorKind::OctDigit)))
        );
        assert_eq!(alphanumeric1::<_, Error<_>>(a), Ok((empty, a)));
        //assert_eq!(fix_error!(b,(), alphanumeric), Ok((empty, b)));
        assert_eq!(alphanumeric1::<_, Error<_>>(c), Ok((empty, c)));
        assert_eq!(alphanumeric1::<_, Error<_>>(d), Ok(("é12", "az")));
        assert_eq!(space1::<_, Error<_>>(e), Ok((empty, e)));
    }

    use crate::stream::Offset;
    #[test]
    fn offset() {
        let a = &b"abcd;"[..];
        let b = &b"1234;"[..];
        let c = &b"a123;"[..];
        let d = &b" \t;"[..];
        let e = &b" \t\r\n;"[..];
        let f = &b"123abcDEF;"[..];

        match alpha1::<_, Error<_>>(a) {
            Ok((i, _)) => {
                assert_eq!(a.offset_to(i) + i.len(), a.len());
            }
            _ => panic!("wrong return type in offset test for alpha"),
        }
        match digit1::<_, Error<_>>(b) {
            Ok((i, _)) => {
                assert_eq!(b.offset_to(i) + i.len(), b.len());
            }
            _ => panic!("wrong return type in offset test for digit"),
        }
        match alphanumeric1::<_, Error<_>>(c) {
            Ok((i, _)) => {
                assert_eq!(c.offset_to(i) + i.len(), c.len());
            }
            _ => panic!("wrong return type in offset test for alphanumeric"),
        }
        match space1::<_, Error<_>>(d) {
            Ok((i, _)) => {
                assert_eq!(d.offset_to(i) + i.len(), d.len());
            }
            _ => panic!("wrong return type in offset test for space"),
        }
        match multispace1::<_, Error<_>>(e) {
            Ok((i, _)) => {
                assert_eq!(e.offset_to(i) + i.len(), e.len());
            }
            _ => panic!("wrong return type in offset test for multispace"),
        }
        match hex_digit1::<_, Error<_>>(f) {
            Ok((i, _)) => {
                assert_eq!(f.offset_to(i) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for hex_digit"),
        }
        match oct_digit1::<_, Error<_>>(f) {
            Ok((i, _)) => {
                assert_eq!(f.offset_to(i) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for oct_digit"),
        }
    }

    #[test]
    fn is_not_line_ending_bytes() {
        let a: &[u8] = b"ab12cd\nefgh";
        assert_eq!(
            not_line_ending::<_, Error<_>>(a),
            Ok((&b"\nefgh"[..], &b"ab12cd"[..]))
        );

        let b: &[u8] = b"ab12cd\nefgh\nijkl";
        assert_eq!(
            not_line_ending::<_, Error<_>>(b),
            Ok((&b"\nefgh\nijkl"[..], &b"ab12cd"[..]))
        );

        let c: &[u8] = b"ab12cd\r\nefgh\nijkl";
        assert_eq!(
            not_line_ending::<_, Error<_>>(c),
            Ok((&b"\r\nefgh\nijkl"[..], &b"ab12cd"[..]))
        );

        let d: &[u8] = b"ab12cd";
        assert_eq!(not_line_ending::<_, Error<_>>(d), Ok((&[][..], d)));
    }

    #[test]
    fn is_not_line_ending_str() {
        let f = "βèƒôřè\rÂßÇáƒƭèř";
        assert_eq!(
            not_line_ending(f),
            Err(ErrMode::Backtrack(Error::new(f, ErrorKind::Tag)))
        );

        let g2: &str = "ab12cd";
        assert_eq!(not_line_ending::<_, Error<_>>(g2), Ok(("", g2)));
    }

    #[test]
    fn hex_digit_test() {
        let i = &b"0123456789abcdefABCDEF;"[..];
        assert_parse!(hex_digit1(i), Ok((&b";"[..], &i[..i.len() - 1])));

        let i = &b"g"[..];
        assert_parse!(
            hex_digit1(i),
            Err(ErrMode::Backtrack(error_position!(i, ErrorKind::HexDigit)))
        );

        let i = &b"G"[..];
        assert_parse!(
            hex_digit1(i),
            Err(ErrMode::Backtrack(error_position!(i, ErrorKind::HexDigit)))
        );

        assert!(AsChar::is_hex_digit(b'0'));
        assert!(AsChar::is_hex_digit(b'9'));
        assert!(AsChar::is_hex_digit(b'a'));
        assert!(AsChar::is_hex_digit(b'f'));
        assert!(AsChar::is_hex_digit(b'A'));
        assert!(AsChar::is_hex_digit(b'F'));
        assert!(!AsChar::is_hex_digit(b'g'));
        assert!(!AsChar::is_hex_digit(b'G'));
        assert!(!AsChar::is_hex_digit(b'/'));
        assert!(!AsChar::is_hex_digit(b':'));
        assert!(!AsChar::is_hex_digit(b'@'));
        assert!(!AsChar::is_hex_digit(b'\x60'));
    }

    #[test]
    fn oct_digit_test() {
        let i = &b"01234567;"[..];
        assert_parse!(oct_digit1(i), Ok((&b";"[..], &i[..i.len() - 1])));

        let i = &b"8"[..];
        assert_parse!(
            oct_digit1(i),
            Err(ErrMode::Backtrack(error_position!(i, ErrorKind::OctDigit)))
        );

        assert!(AsChar::is_oct_digit(b'0'));
        assert!(AsChar::is_oct_digit(b'7'));
        assert!(!AsChar::is_oct_digit(b'8'));
        assert!(!AsChar::is_oct_digit(b'9'));
        assert!(!AsChar::is_oct_digit(b'a'));
        assert!(!AsChar::is_oct_digit(b'A'));
        assert!(!AsChar::is_oct_digit(b'/'));
        assert!(!AsChar::is_oct_digit(b':'));
        assert!(!AsChar::is_oct_digit(b'@'));
        assert!(!AsChar::is_oct_digit(b'\x60'));
    }

    #[test]
    fn full_line_windows() {
        use crate::sequence::pair;
        fn take_full_line(i: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
            pair(not_line_ending, line_ending)(i)
        }
        let input = b"abc\r\n";
        let output = take_full_line(input);
        assert_eq!(output, Ok((&b""[..], (&b"abc"[..], &b"\r\n"[..]))));
    }

    #[test]
    fn full_line_unix() {
        use crate::sequence::pair;
        fn take_full_line(i: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
            pair(not_line_ending, line_ending)(i)
        }
        let input = b"abc\n";
        let output = take_full_line(input);
        assert_eq!(output, Ok((&b""[..], (&b"abc"[..], &b"\n"[..]))));
    }

    #[test]
    fn check_windows_lineending() {
        let input = b"\r\n";
        let output = line_ending(&input[..]);
        assert_parse!(output, Ok((&b""[..], &b"\r\n"[..])));
    }

    #[test]
    fn check_unix_lineending() {
        let input = b"\n";
        let output = line_ending(&input[..]);
        assert_parse!(output, Ok((&b""[..], &b"\n"[..])));
    }

    #[test]
    fn cr_lf() {
        assert_parse!(crlf(&b"\r\na"[..]), Ok((&b"a"[..], &b"\r\n"[..])));
        assert_parse!(
            crlf(&b"\r"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"\r"[..],
                ErrorKind::CrLf
            )))
        );
        assert_parse!(
            crlf(&b"\ra"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"\ra"[..],
                ErrorKind::CrLf
            )))
        );

        assert_parse!(crlf("\r\na"), Ok(("a", "\r\n")));
        assert_parse!(
            crlf("\r"),
            Err(ErrMode::Backtrack(error_position!("\r", ErrorKind::CrLf)))
        );
        assert_parse!(
            crlf("\ra"),
            Err(ErrMode::Backtrack(error_position!("\ra", ErrorKind::CrLf)))
        );
    }

    #[test]
    fn end_of_line() {
        assert_parse!(line_ending(&b"\na"[..]), Ok((&b"a"[..], &b"\n"[..])));
        assert_parse!(line_ending(&b"\r\na"[..]), Ok((&b"a"[..], &b"\r\n"[..])));
        assert_parse!(
            line_ending(&b"\r"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"\r"[..],
                ErrorKind::CrLf
            )))
        );
        assert_parse!(
            line_ending(&b"\ra"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b"\ra"[..],
                ErrorKind::CrLf
            )))
        );

        assert_parse!(line_ending("\na"), Ok(("a", "\n")));
        assert_parse!(line_ending("\r\na"), Ok(("a", "\r\n")));
        assert_parse!(
            line_ending("\r"),
            Err(ErrMode::Backtrack(error_position!("\r", ErrorKind::CrLf)))
        );
        assert_parse!(
            line_ending("\ra"),
            Err(ErrMode::Backtrack(error_position!("\ra", ErrorKind::CrLf)))
        );
    }

    fn digit_to_i16(input: &str) -> IResult<&str, i16> {
        let i = input;
        let (i, opt_sign) = opt(alt(('+', '-')))(i)?;
        let sign = match opt_sign {
            Some('+') | None => true,
            Some('-') => false,
            _ => unreachable!(),
        };

        let (i, s) = match digit1::<_, crate::error::Error<_>>(i) {
            Ok((i, s)) => (i, s),
            Err(_) => return Err(ErrMode::from_error_kind(input, ErrorKind::Digit)),
        };

        match s.parse_slice() {
            Some(n) => {
                if sign {
                    Ok((i, n))
                } else {
                    Ok((i, -n))
                }
            }
            None => Err(ErrMode::from_error_kind(i, ErrorKind::Digit)),
        }
    }

    fn digit_to_u32(i: &str) -> IResult<&str, u32> {
        let (i, s) = digit1(i)?;
        match s.parse_slice() {
            Some(n) => Ok((i, n)),
            None => Err(ErrMode::from_error_kind(i, ErrorKind::Digit)),
        }
    }

    proptest! {
      #[test]
      #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
      fn ints(s in "\\PC*") {
          let res1 = digit_to_i16(&s);
          let res2 = dec_int(s.as_str());
          assert_eq!(res1, res2);
      }

      #[test]
      #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
      fn uints(s in "\\PC*") {
          let res1 = digit_to_u32(&s);
          let res2 = dec_uint(s.as_str());
          assert_eq!(res1, res2);
      }
    }

    #[test]
    fn hex_uint_tests() {
        fn hex_u32(input: &[u8]) -> IResult<&[u8], u32> {
            hex_uint(input)
        }

        assert_parse!(
            hex_u32(&b";"[..]),
            Err(ErrMode::Backtrack(error_position!(
                &b";"[..],
                ErrorKind::IsA
            )))
        );
        assert_parse!(hex_u32(&b"ff;"[..]), Ok((&b";"[..], 255)));
        assert_parse!(hex_u32(&b"1be2;"[..]), Ok((&b";"[..], 7138)));
        assert_parse!(hex_u32(&b"c5a31be2;"[..]), Ok((&b";"[..], 3_315_801_058)));
        assert_parse!(hex_u32(&b"C5A31be2;"[..]), Ok((&b";"[..], 3_315_801_058)));
        assert_parse!(
            hex_u32(&b"00c5a31be2;"[..]), // overflow
            Err(ErrMode::Backtrack(error_position!(
                &b"00c5a31be2;"[..],
                ErrorKind::IsA
            )))
        );
        assert_parse!(
            hex_u32(&b"c5a31be201;"[..]), // overflow
            Err(ErrMode::Backtrack(error_position!(
                &b"c5a31be201;"[..],
                ErrorKind::IsA
            )))
        );
        assert_parse!(hex_u32(&b"ffffffff;"[..]), Ok((&b";"[..], 4_294_967_295)));
        assert_parse!(
            hex_u32(&b"ffffffffffffffff;"[..]), // overflow
            Err(ErrMode::Backtrack(error_position!(
                &b"ffffffffffffffff;"[..],
                ErrorKind::IsA
            )))
        );
        assert_parse!(
            hex_u32(&b"ffffffffffffffff"[..]), // overflow
            Err(ErrMode::Backtrack(error_position!(
                &b"ffffffffffffffff"[..],
                ErrorKind::IsA
            )))
        );
        assert_parse!(hex_u32(&b"0x1be2;"[..]), Ok((&b"x1be2;"[..], 0)));
        assert_parse!(hex_u32(&b"12af"[..]), Ok((&b""[..], 0x12af)));
    }

    #[test]
    #[cfg(feature = "std")]
    fn float_test() {
        let mut test_cases = vec![
            "+3.14",
            "3.14",
            "-3.14",
            "0",
            "0.0",
            "1.",
            ".789",
            "-.5",
            "1e7",
            "-1E-7",
            ".3e-2",
            "1.e4",
            "1.2e4",
            "12.34",
            "-1.234E-12",
            "-1.234e-12",
            "0.00000000000000000087",
        ];

        for test in test_cases.drain(..) {
            let expected32 = str::parse::<f32>(test).unwrap();
            let expected64 = str::parse::<f64>(test).unwrap();

            println!("now parsing: {} -> {}", test, expected32);

            let larger = test.to_string();

            assert_parse!(float(larger.as_bytes()), Ok((&b""[..], expected32)));
            assert_parse!(float(&larger[..]), Ok(("", expected32)));

            assert_parse!(float(larger.as_bytes()), Ok((&b""[..], expected64)));
            assert_parse!(float(&larger[..]), Ok(("", expected64)));
        }

        let remaining_exponent = "-1.234E-";
        assert_parse!(
            float::<_, f64, _>(remaining_exponent),
            Err(ErrMode::Cut(Error {
                input: "-1.234E-",
                kind: ErrorKind::Float
            }))
        );

        let (_i, nan) = float::<_, f32, ()>("NaN").unwrap();
        assert!(nan.is_nan());

        let (_i, inf) = float::<_, f32, ()>("inf").unwrap();
        assert!(inf.is_infinite());
        let (_i, inf) = float::<_, f32, ()>("infinite").unwrap();
        assert!(inf.is_infinite());
    }

    #[cfg(feature = "std")]
    fn parse_f64(i: &str) -> IResult<&str, f64, ()> {
        #[allow(deprecated)] // will just become `pub(crate)` later
        match crate::number::complete::recognize_float_or_exceptions(i) {
            Err(e) => Err(e),
            Ok((i, s)) => {
                if s.is_empty() {
                    return Err(ErrMode::Backtrack(()));
                }
                match s.parse_slice() {
                    Some(n) => Ok((i, n)),
                    None => Err(ErrMode::Backtrack(())),
                }
            }
        }
    }

    proptest! {
      #[test]
      #[cfg(feature = "std")]
      #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
      fn floats(s in "\\PC*") {
          println!("testing {}", s);
          let res1 = parse_f64(&s);
          let res2 = float::<_, f64, ()>(s.as_str());
          assert_eq!(res1, res2);
      }
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
        fn unquote(input: &str) -> IResult<&str, &str> {
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
            Err(ErrMode::Backtrack(error_position!(
                &b"AB\\"[..],
                ErrorKind::Escaped
            )))
        );
        assert_eq!(
            esc(&b"AB\\A"[..]),
            Err(ErrMode::Backtrack(error_node_position!(
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
            Err(ErrMode::Backtrack(error_position!(
                "AB\\",
                ErrorKind::Escaped
            )))
        );
        assert_eq!(
            esc("AB\\A"),
            Err(ErrMode::Backtrack(error_node_position!(
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

    #[test]
    fn test_escaped_error() {
        fn esc(s: &str) -> IResult<&str, &str> {
            use crate::character::digit1;
            escaped(digit1, '\\', one_of("\"n\\"))(s)
        }

        assert_eq!(
            esc("abcd"),
            Err(ErrMode::Backtrack(Error {
                input: "abcd",
                kind: ErrorKind::Escaped
            }))
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn complete_escape_transform() {
        use crate::bytes::tag;
        use crate::character::alpha1 as alpha;

        #[cfg(feature = "alloc")]
        fn to_s(i: Vec<u8>) -> String {
            String::from_utf8_lossy(&i).into_owned()
        }

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
            .parse_next(i)
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
            Err(ErrMode::Backtrack(error_position!(
                &b"\\"[..],
                ErrorKind::EscapedTransform
            )))
        );
        assert_eq!(
            esc(&b"AB\\A"[..]),
            Err(ErrMode::Backtrack(error_node_position!(
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
            .parse_next(i)
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
        use crate::bytes::tag;
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
            Err(ErrMode::Backtrack(error_position!(
                "\\",
                ErrorKind::EscapedTransform
            )))
        );
        assert_eq!(
            esc("AB\\A"),
            Err(ErrMode::Backtrack(error_node_position!(
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
    #[cfg(feature = "alloc")]
    fn test_escaped_transform_error() {
        fn esc_trans(s: &str) -> IResult<&str, String> {
            use crate::character::digit1;
            escaped_transform(digit1, '\\', "n")(s)
        }

        assert_eq!(
            esc_trans("abcd"),
            Err(ErrMode::Backtrack(Error {
                input: "abcd",
                kind: ErrorKind::EscapedTransform
            }))
        );
    }
}

mod partial {
    use super::*;
    use crate::combinator::opt;
    use crate::error::Error;
    use crate::error::ErrorKind;
    use crate::error::{ErrMode, Needed};
    use crate::sequence::pair;
    use crate::stream::ParseSlice;
    use crate::IResult;
    use crate::Partial;
    use proptest::prelude::*;

    macro_rules! assert_parse(
    ($left: expr, $right: expr) => {
      let res: $crate::IResult<_, _, Error<_>> = $left;
      assert_eq!(res, $right);
    };
  );

    #[test]
    fn character() {
        let a: &[u8] = b"abcd";
        let b: &[u8] = b"1234";
        let c: &[u8] = b"a123";
        let d: &[u8] = "azé12".as_bytes();
        let e: &[u8] = b" ";
        let f: &[u8] = b" ;";
        //assert_eq!(alpha1::<_, Error<_>>(a), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_parse!(
            alpha1(Partial::new(a)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            alpha1(Partial::new(b)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(b),
                ErrorKind::Alpha
            )))
        );
        assert_eq!(
            alpha1::<_, Error<_>>(Partial::new(c)),
            Ok((Partial::new(&c[1..]), &b"a"[..]))
        );
        assert_eq!(
            alpha1::<_, Error<_>>(Partial::new(d)),
            Ok((Partial::new("é12".as_bytes()), &b"az"[..]))
        );
        assert_eq!(
            digit1(Partial::new(a)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(a),
                ErrorKind::Digit
            )))
        );
        assert_eq!(
            digit1::<_, Error<_>>(Partial::new(b)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            digit1(Partial::new(c)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(c),
                ErrorKind::Digit
            )))
        );
        assert_eq!(
            digit1(Partial::new(d)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(d),
                ErrorKind::Digit
            )))
        );
        assert_eq!(
            hex_digit1::<_, Error<_>>(Partial::new(a)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            hex_digit1::<_, Error<_>>(Partial::new(b)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            hex_digit1::<_, Error<_>>(Partial::new(c)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            hex_digit1::<_, Error<_>>(Partial::new(d)),
            Ok((Partial::new("zé12".as_bytes()), &b"a"[..]))
        );
        assert_eq!(
            hex_digit1(Partial::new(e)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(e),
                ErrorKind::HexDigit
            )))
        );
        assert_eq!(
            oct_digit1(Partial::new(a)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(a),
                ErrorKind::OctDigit
            )))
        );
        assert_eq!(
            oct_digit1::<_, Error<_>>(Partial::new(b)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            oct_digit1(Partial::new(c)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(c),
                ErrorKind::OctDigit
            )))
        );
        assert_eq!(
            oct_digit1(Partial::new(d)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(d),
                ErrorKind::OctDigit
            )))
        );
        assert_eq!(
            alphanumeric1::<_, Error<_>>(Partial::new(a)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        //assert_eq!(fix_error!(b,(), alphanumeric1), Ok((empty, b)));
        assert_eq!(
            alphanumeric1::<_, Error<_>>(Partial::new(c)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            alphanumeric1::<_, Error<_>>(Partial::new(d)),
            Ok((Partial::new("é12".as_bytes()), &b"az"[..]))
        );
        assert_eq!(
            space1::<_, Error<_>>(Partial::new(e)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            space1::<_, Error<_>>(Partial::new(f)),
            Ok((Partial::new(&b";"[..]), &b" "[..]))
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn character_s() {
        let a = "abcd";
        let b = "1234";
        let c = "a123";
        let d = "azé12";
        let e = " ";
        assert_eq!(
            alpha1::<_, Error<_>>(Partial::new(a)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            alpha1(Partial::new(b)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(b),
                ErrorKind::Alpha
            )))
        );
        assert_eq!(
            alpha1::<_, Error<_>>(Partial::new(c)),
            Ok((Partial::new(&c[1..]), "a"))
        );
        assert_eq!(
            alpha1::<_, Error<_>>(Partial::new(d)),
            Ok((Partial::new("é12"), "az"))
        );
        assert_eq!(
            digit1(Partial::new(a)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(a),
                ErrorKind::Digit
            )))
        );
        assert_eq!(
            digit1::<_, Error<_>>(Partial::new(b)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            digit1(Partial::new(c)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(c),
                ErrorKind::Digit
            )))
        );
        assert_eq!(
            digit1(Partial::new(d)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(d),
                ErrorKind::Digit
            )))
        );
        assert_eq!(
            hex_digit1::<_, Error<_>>(Partial::new(a)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            hex_digit1::<_, Error<_>>(Partial::new(b)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            hex_digit1::<_, Error<_>>(Partial::new(c)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            hex_digit1::<_, Error<_>>(Partial::new(d)),
            Ok((Partial::new("zé12"), "a"))
        );
        assert_eq!(
            hex_digit1(Partial::new(e)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(e),
                ErrorKind::HexDigit
            )))
        );
        assert_eq!(
            oct_digit1(Partial::new(a)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(a),
                ErrorKind::OctDigit
            )))
        );
        assert_eq!(
            oct_digit1::<_, Error<_>>(Partial::new(b)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            oct_digit1(Partial::new(c)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(c),
                ErrorKind::OctDigit
            )))
        );
        assert_eq!(
            oct_digit1(Partial::new(d)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(d),
                ErrorKind::OctDigit
            )))
        );
        assert_eq!(
            alphanumeric1::<_, Error<_>>(Partial::new(a)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        //assert_eq!(fix_error!(b,(), alphanumeric1), Ok((empty, b)));
        assert_eq!(
            alphanumeric1::<_, Error<_>>(Partial::new(c)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        assert_eq!(
            alphanumeric1::<_, Error<_>>(Partial::new(d)),
            Ok((Partial::new("é12"), "az"))
        );
        assert_eq!(
            space1::<_, Error<_>>(Partial::new(e)),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
    }

    use crate::stream::Offset;
    #[test]
    fn offset() {
        let a = &b"abcd;"[..];
        let b = &b"1234;"[..];
        let c = &b"a123;"[..];
        let d = &b" \t;"[..];
        let e = &b" \t\r\n;"[..];
        let f = &b"123abcDEF;"[..];

        match alpha1::<_, Error<_>>(Partial::new(a)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(a.offset_to(i) + i.len(), a.len());
            }
            _ => panic!("wrong return type in offset test for alpha"),
        }
        match digit1::<_, Error<_>>(Partial::new(b)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(b.offset_to(i) + i.len(), b.len());
            }
            _ => panic!("wrong return type in offset test for digit"),
        }
        match alphanumeric1::<_, Error<_>>(Partial::new(c)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(c.offset_to(i) + i.len(), c.len());
            }
            _ => panic!("wrong return type in offset test for alphanumeric"),
        }
        match space1::<_, Error<_>>(Partial::new(d)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(d.offset_to(i) + i.len(), d.len());
            }
            _ => panic!("wrong return type in offset test for space"),
        }
        match multispace1::<_, Error<_>>(Partial::new(e)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(e.offset_to(i) + i.len(), e.len());
            }
            _ => panic!("wrong return type in offset test for multispace"),
        }
        match hex_digit1::<_, Error<_>>(Partial::new(f)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(f.offset_to(i) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for hex_digit"),
        }
        match oct_digit1::<_, Error<_>>(Partial::new(f)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(f.offset_to(i) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for oct_digit"),
        }
    }

    #[test]
    fn is_not_line_ending_bytes() {
        let a: &[u8] = b"ab12cd\nefgh";
        assert_eq!(
            not_line_ending::<_, Error<_>>(Partial::new(a)),
            Ok((Partial::new(&b"\nefgh"[..]), &b"ab12cd"[..]))
        );

        let b: &[u8] = b"ab12cd\nefgh\nijkl";
        assert_eq!(
            not_line_ending::<_, Error<_>>(Partial::new(b)),
            Ok((Partial::new(&b"\nefgh\nijkl"[..]), &b"ab12cd"[..]))
        );

        let c: &[u8] = b"ab12cd\r\nefgh\nijkl";
        assert_eq!(
            not_line_ending::<_, Error<_>>(Partial::new(c)),
            Ok((Partial::new(&b"\r\nefgh\nijkl"[..]), &b"ab12cd"[..]))
        );

        let d: &[u8] = b"ab12cd";
        assert_eq!(
            not_line_ending::<_, Error<_>>(Partial::new(d)),
            Err(ErrMode::Incomplete(Needed::Unknown))
        );
    }

    #[test]
    fn is_not_line_ending_str() {
        let f = "βèƒôřè\rÂßÇáƒƭèř";
        assert_eq!(
            not_line_ending(Partial::new(f)),
            Err(ErrMode::Backtrack(Error::new(
                Partial::new(f),
                ErrorKind::Tag
            )))
        );

        let g2: &str = "ab12cd";
        assert_eq!(
            not_line_ending::<_, Error<_>>(Partial::new(g2)),
            Err(ErrMode::Incomplete(Needed::Unknown))
        );
    }

    #[test]
    fn hex_digit_test() {
        let i = &b"0123456789abcdefABCDEF;"[..];
        assert_parse!(
            hex_digit1(Partial::new(i)),
            Ok((Partial::new(&b";"[..]), &i[..i.len() - 1]))
        );

        let i = &b"g"[..];
        assert_parse!(
            hex_digit1(Partial::new(i)),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(i),
                ErrorKind::HexDigit
            )))
        );

        let i = &b"G"[..];
        assert_parse!(
            hex_digit1(Partial::new(i)),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(i),
                ErrorKind::HexDigit
            )))
        );

        assert!(AsChar::is_hex_digit(b'0'));
        assert!(AsChar::is_hex_digit(b'9'));
        assert!(AsChar::is_hex_digit(b'a'));
        assert!(AsChar::is_hex_digit(b'f'));
        assert!(AsChar::is_hex_digit(b'A'));
        assert!(AsChar::is_hex_digit(b'F'));
        assert!(!AsChar::is_hex_digit(b'g'));
        assert!(!AsChar::is_hex_digit(b'G'));
        assert!(!AsChar::is_hex_digit(b'/'));
        assert!(!AsChar::is_hex_digit(b':'));
        assert!(!AsChar::is_hex_digit(b'@'));
        assert!(!AsChar::is_hex_digit(b'\x60'));
    }

    #[test]
    fn oct_digit_test() {
        let i = &b"01234567;"[..];
        assert_parse!(
            oct_digit1(Partial::new(i)),
            Ok((Partial::new(&b";"[..]), &i[..i.len() - 1]))
        );

        let i = &b"8"[..];
        assert_parse!(
            oct_digit1(Partial::new(i)),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(i),
                ErrorKind::OctDigit
            )))
        );

        assert!(AsChar::is_oct_digit(b'0'));
        assert!(AsChar::is_oct_digit(b'7'));
        assert!(!AsChar::is_oct_digit(b'8'));
        assert!(!AsChar::is_oct_digit(b'9'));
        assert!(!AsChar::is_oct_digit(b'a'));
        assert!(!AsChar::is_oct_digit(b'A'));
        assert!(!AsChar::is_oct_digit(b'/'));
        assert!(!AsChar::is_oct_digit(b':'));
        assert!(!AsChar::is_oct_digit(b'@'));
        assert!(!AsChar::is_oct_digit(b'\x60'));
    }

    #[test]
    fn full_line_windows() {
        #[allow(clippy::type_complexity)]
        fn take_full_line(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, (&[u8], &[u8])> {
            pair(not_line_ending, line_ending)(i)
        }
        let input = b"abc\r\n";
        let output = take_full_line(Partial::new(input));
        assert_eq!(
            output,
            Ok((Partial::new(&b""[..]), (&b"abc"[..], &b"\r\n"[..])))
        );
    }

    #[test]
    fn full_line_unix() {
        #[allow(clippy::type_complexity)]
        fn take_full_line(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, (&[u8], &[u8])> {
            pair(not_line_ending, line_ending)(i)
        }
        let input = b"abc\n";
        let output = take_full_line(Partial::new(input));
        assert_eq!(
            output,
            Ok((Partial::new(&b""[..]), (&b"abc"[..], &b"\n"[..])))
        );
    }

    #[test]
    fn check_windows_lineending() {
        let input = b"\r\n";
        let output = line_ending(Partial::new(&input[..]));
        assert_parse!(output, Ok((Partial::new(&b""[..]), &b"\r\n"[..])));
    }

    #[test]
    fn check_unix_lineending() {
        let input = b"\n";
        let output = line_ending(Partial::new(&input[..]));
        assert_parse!(output, Ok((Partial::new(&b""[..]), &b"\n"[..])));
    }

    #[test]
    fn cr_lf() {
        assert_parse!(
            crlf(Partial::new(&b"\r\na"[..])),
            Ok((Partial::new(&b"a"[..]), &b"\r\n"[..]))
        );
        assert_parse!(
            crlf(Partial::new(&b"\r"[..])),
            Err(ErrMode::Incomplete(Needed::new(2)))
        );
        assert_parse!(
            crlf(Partial::new(&b"\ra"[..])),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(&b"\ra"[..]),
                ErrorKind::CrLf
            )))
        );

        assert_parse!(crlf(Partial::new("\r\na")), Ok((Partial::new("a"), "\r\n")));
        assert_parse!(
            crlf(Partial::new("\r")),
            Err(ErrMode::Incomplete(Needed::new(2)))
        );
        assert_parse!(
            crlf(Partial::new("\ra")),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new("\ra"),
                ErrorKind::CrLf
            )))
        );
    }

    #[test]
    fn end_of_line() {
        assert_parse!(
            line_ending(Partial::new(&b"\na"[..])),
            Ok((Partial::new(&b"a"[..]), &b"\n"[..]))
        );
        assert_parse!(
            line_ending(Partial::new(&b"\r\na"[..])),
            Ok((Partial::new(&b"a"[..]), &b"\r\n"[..]))
        );
        assert_parse!(
            line_ending(Partial::new(&b"\r"[..])),
            Err(ErrMode::Incomplete(Needed::new(2)))
        );
        assert_parse!(
            line_ending(Partial::new(&b"\ra"[..])),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(&b"\ra"[..]),
                ErrorKind::CrLf
            )))
        );

        assert_parse!(
            line_ending(Partial::new("\na")),
            Ok((Partial::new("a"), "\n"))
        );
        assert_parse!(
            line_ending(Partial::new("\r\na")),
            Ok((Partial::new("a"), "\r\n"))
        );
        assert_parse!(
            line_ending(Partial::new("\r")),
            Err(ErrMode::Incomplete(Needed::new(2)))
        );
        assert_parse!(
            line_ending(Partial::new("\ra")),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new("\ra"),
                ErrorKind::CrLf
            )))
        );
    }

    fn digit_to_i16(input: Partial<&str>) -> IResult<Partial<&str>, i16> {
        use crate::bytes::one_of;

        let i = input;
        let (i, opt_sign) = opt(one_of("+-"))(i)?;
        let sign = match opt_sign {
            Some('+') | None => true,
            Some('-') => false,
            _ => unreachable!(),
        };

        let (i, s) = match digit1::<_, crate::error::Error<_>>(i) {
            Ok((i, s)) => (i, s),
            Err(ErrMode::Incomplete(i)) => return Err(ErrMode::Incomplete(i)),
            Err(_) => return Err(ErrMode::from_error_kind(input, ErrorKind::Digit)),
        };
        match s.parse_slice() {
            Some(n) => {
                if sign {
                    Ok((i, n))
                } else {
                    Ok((i, -n))
                }
            }
            None => Err(ErrMode::from_error_kind(i, ErrorKind::Digit)),
        }
    }

    fn digit_to_u32(i: Partial<&str>) -> IResult<Partial<&str>, u32> {
        let (i, s) = digit1(i)?;
        match s.parse_slice() {
            Some(n) => Ok((i, n)),
            None => Err(ErrMode::from_error_kind(i, ErrorKind::Digit)),
        }
    }

    proptest! {
      #[test]
      #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
      fn ints(s in "\\PC*") {
          let res1 = digit_to_i16(Partial::new(&s));
          let res2 = dec_int(Partial::new(s.as_str()));
          assert_eq!(res1, res2);
      }

      #[test]
      #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
      fn uints(s in "\\PC*") {
          let res1 = digit_to_u32(Partial::new(&s));
          let res2 = dec_uint(Partial::new(s.as_str()));
          assert_eq!(res1, res2);
      }
    }

    #[test]
    fn hex_uint_tests() {
        fn hex_u32(input: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u32> {
            hex_uint(input)
        }

        assert_parse!(
            hex_u32(Partial::new(&b";"[..])),
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(&b";"[..]),
                ErrorKind::IsA
            )))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"ff;"[..])),
            Ok((Partial::new(&b";"[..]), 255))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"1be2;"[..])),
            Ok((Partial::new(&b";"[..]), 7138))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"c5a31be2;"[..])),
            Ok((Partial::new(&b";"[..]), 3_315_801_058))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"C5A31be2;"[..])),
            Ok((Partial::new(&b";"[..]), 3_315_801_058))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"00c5a31be2;"[..])), // overflow
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(&b"00c5a31be2;"[..]),
                ErrorKind::IsA
            )))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"c5a31be201;"[..])), // overflow
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(&b"c5a31be201;"[..]),
                ErrorKind::IsA
            )))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"ffffffff;"[..])),
            Ok((Partial::new(&b";"[..]), 4_294_967_295))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"ffffffffffffffff;"[..])), // overflow
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(&b"ffffffffffffffff;"[..]),
                ErrorKind::IsA
            )))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"ffffffffffffffff"[..])), // overflow
            Err(ErrMode::Backtrack(error_position!(
                Partial::new(&b"ffffffffffffffff"[..]),
                ErrorKind::IsA
            )))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"0x1be2;"[..])),
            Ok((Partial::new(&b"x1be2;"[..]), 0))
        );
        assert_parse!(
            hex_u32(Partial::new(&b"12af"[..])),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
    }
}
