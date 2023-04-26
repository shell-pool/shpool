use super::*;

use crate::bytes::take;
use crate::error::ErrMode;
use crate::error::Error;
use crate::error::ErrorKind;
use crate::error::Needed;
use crate::error::ParseError;
use crate::multi::count;
use crate::number::u16;
use crate::number::u8;
use crate::number::Endianness;
use crate::IResult;
use crate::Parser;
use crate::Partial;

macro_rules! assert_parse(
  ($left: expr, $right: expr) => {
    let res: $crate::IResult<_, _, Error<_>> = $left;
    assert_eq!(res, $right);
  };
);

#[test]
fn eof_on_slices() {
    let not_over: &[u8] = &b"Hello, world!"[..];
    let is_over: &[u8] = &b""[..];

    let res_not_over = eof(not_over);
    assert_parse!(
        res_not_over,
        Err(ErrMode::Backtrack(error_position!(
            not_over,
            ErrorKind::Eof
        )))
    );

    let res_over = eof(is_over);
    assert_parse!(res_over, Ok((is_over, is_over)));
}

#[test]
fn eof_on_strs() {
    let not_over: &str = "Hello, world!";
    let is_over: &str = "";

    let res_not_over = eof(not_over);
    assert_parse!(
        res_not_over,
        Err(ErrMode::Backtrack(error_position!(
            not_over,
            ErrorKind::Eof
        )))
    );

    let res_over = eof(is_over);
    assert_parse!(res_over, Ok((is_over, is_over)));
}

#[test]
fn rest_on_slices() {
    let input: &[u8] = &b"Hello, world!"[..];
    let empty: &[u8] = &b""[..];
    assert_parse!(rest(input), Ok((empty, input)));
}

#[test]
fn rest_on_strs() {
    let input: &str = "Hello, world!";
    let empty: &str = "";
    assert_parse!(rest(input), Ok((empty, input)));
}

#[test]
fn rest_len_on_slices() {
    let input: &[u8] = &b"Hello, world!"[..];
    assert_parse!(rest_len(input), Ok((input, input.len())));
}

use crate::lib::std::convert::From;
impl From<u32> for CustomError {
    fn from(_: u32) -> Self {
        CustomError
    }
}

impl<I> ParseError<I> for CustomError {
    fn from_error_kind(_: I, _: ErrorKind) -> Self {
        CustomError
    }

    fn append(self, _: I, _: ErrorKind) -> Self {
        CustomError
    }
}

struct CustomError;
#[allow(dead_code)]
fn custom_error(input: &[u8]) -> IResult<&[u8], &[u8], CustomError> {
    //fix_error!(input, CustomError<_>, alphanumeric)
    crate::character::alphanumeric1(input)
}

#[test]
fn test_parser_flat_map() {
    let input: &[u8] = &[3, 100, 101, 102, 103, 104][..];
    assert_parse!(
        u8.flat_map(take).parse_next(input),
        Ok((&[103, 104][..], &[100, 101, 102][..]))
    );
}

#[allow(dead_code)]
fn test_closure_compiles_195(input: &[u8]) -> IResult<&[u8], ()> {
    u8.flat_map(|num| count(u16(Endianness::Big), num as usize))
        .parse_next(input)
}

#[test]
fn test_parser_verify_map() {
    let input: &[u8] = &[50][..];
    assert_parse!(
        u8.verify_map(|u| if u < 20 { Some(u) } else { None })
            .parse_next(input),
        Err(ErrMode::Backtrack(Error {
            input: &[50][..],
            kind: ErrorKind::Verify
        }))
    );
    assert_parse!(
        u8.verify_map(|u| if u > 20 { Some(u) } else { None })
            .parse_next(input),
        Ok((&[][..], 50))
    );
}

#[test]
fn test_parser_map_parser() {
    let input: &[u8] = &[100, 101, 102, 103, 104][..];
    assert_parse!(
        take(4usize).and_then(take(2usize)).parse_next(input),
        Ok((&[104][..], &[100, 101][..]))
    );
}

#[test]
#[cfg(feature = "std")]
fn test_parser_into() {
    use crate::bytes::take;
    use crate::error::Error;

    let mut parser = take::<_, _, Error<_>>(3u8).output_into();
    let result: IResult<&[u8], Vec<u8>> = parser.parse_next(&b"abcdefg"[..]);

    assert_eq!(result, Ok((&b"defg"[..], vec![97, 98, 99])));
}

#[test]
fn opt_test() {
    fn opt_abcd(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Option<&[u8]>> {
        opt("abcd").parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"bcdefg"[..];
    let c = &b"ab"[..];
    assert_eq!(
        opt_abcd(Partial::new(a)),
        Ok((Partial::new(&b"ef"[..]), Some(&b"abcd"[..])))
    );
    assert_eq!(
        opt_abcd(Partial::new(b)),
        Ok((Partial::new(&b"bcdefg"[..]), None))
    );
    assert_eq!(
        opt_abcd(Partial::new(c)),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
}

#[test]
fn peek_test() {
    fn peek_tag(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        peek("abcd").parse_next(i)
    }

    assert_eq!(
        peek_tag(Partial::new(&b"abcdef"[..])),
        Ok((Partial::new(&b"abcdef"[..]), &b"abcd"[..]))
    );
    assert_eq!(
        peek_tag(Partial::new(&b"ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        peek_tag(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
}

#[test]
fn not_test() {
    fn not_aaa(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, ()> {
        not("aaa").parse_next(i)
    }

    assert_eq!(
        not_aaa(Partial::new(&b"aaa"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"aaa"[..]),
            ErrorKind::Not
        )))
    );
    assert_eq!(
        not_aaa(Partial::new(&b"aa"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        not_aaa(Partial::new(&b"abcd"[..])),
        Ok((Partial::new(&b"abcd"[..]), ()))
    );
}

#[test]
fn test_parser_verify() {
    use crate::bytes::take;

    fn test(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        take(5u8)
            .verify(|slice: &[u8]| slice[0] == b'a')
            .parse_next(i)
    }
    assert_eq!(
        test(Partial::new(&b"bcd"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        test(Partial::new(&b"bcdefg"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"bcdefg"[..]),
            ErrorKind::Verify
        )))
    );
    assert_eq!(
        test(Partial::new(&b"abcdefg"[..])),
        Ok((Partial::new(&b"fg"[..]), &b"abcde"[..]))
    );
}

#[test]
#[allow(unused)]
fn test_parser_verify_ref() {
    use crate::bytes::take;

    let mut parser1 = take(3u8).verify(|s: &[u8]| s == &b"abc"[..]);

    assert_eq!(
        parser1.parse_next(&b"abcd"[..]),
        Ok((&b"d"[..], &b"abc"[..]))
    );
    assert_eq!(
        parser1.parse_next(&b"defg"[..]),
        Err(ErrMode::Backtrack(Error {
            input: &b"defg"[..],
            kind: ErrorKind::Verify
        }))
    );

    fn parser2(i: &[u8]) -> IResult<&[u8], u32> {
        crate::number::be_u32
            .verify(|val: &u32| *val < 3)
            .parse_next(i)
    }
}

#[test]
#[cfg(feature = "alloc")]
fn test_parser_verify_alloc() {
    use crate::bytes::take;
    let mut parser1 = take(3u8)
        .map(|s: &[u8]| s.to_vec())
        .verify(|s: &[u8]| s == &b"abc"[..]);

    assert_eq!(
        parser1.parse_next(&b"abcd"[..]),
        Ok((&b"d"[..], b"abc".to_vec()))
    );
    assert_eq!(
        parser1.parse_next(&b"defg"[..]),
        Err(ErrMode::Backtrack(Error {
            input: &b"defg"[..],
            kind: ErrorKind::Verify
        }))
    );
}

#[test]
fn fail_test() {
    let a = "string";
    let b = "another string";

    assert_eq!(
        fail::<_, &str, _>(a),
        Err(ErrMode::Backtrack(Error {
            input: a,
            kind: ErrorKind::Fail
        }))
    );
    assert_eq!(
        fail::<_, &str, _>(b),
        Err(ErrMode::Backtrack(Error {
            input: b,
            kind: ErrorKind::Fail
        }))
    );
}
