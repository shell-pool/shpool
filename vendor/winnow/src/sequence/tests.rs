use super::*;

use crate::error::{ErrMode, ErrorKind, Needed};
use crate::IResult;
use crate::Partial;

#[derive(PartialEq, Eq, Debug)]
struct B {
    a: u8,
    b: u8,
}

#[derive(PartialEq, Eq, Debug)]
struct C {
    a: u8,
    b: Option<u8>,
}

#[test]
fn complete() {
    fn err_test(i: &[u8]) -> IResult<&[u8], &[u8]> {
        let (i, _) = "ijkl".parse_next(i)?;
        "mnop".parse_next(i)
    }
    let a = &b"ijklmn"[..];

    let res_a = err_test(a);
    assert_eq!(
        res_a,
        Err(ErrMode::Backtrack(error_position!(
            &b"mn"[..],
            ErrorKind::Tag
        )))
    );
}

#[test]
fn separated_pair_test() {
    #[allow(clippy::type_complexity)]
    fn sep_pair_abc_def(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, (&[u8], &[u8])> {
        separated_pair("abc", ",", "def").parse_next(i)
    }

    assert_eq!(
        sep_pair_abc_def(Partial::new(&b"abc,defghijkl"[..])),
        Ok((Partial::new(&b"ghijkl"[..]), (&b"abc"[..], &b"def"[..])))
    );
    assert_eq!(
        sep_pair_abc_def(Partial::new(&b"ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        sep_pair_abc_def(Partial::new(&b"abc,d"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        sep_pair_abc_def(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        sep_pair_abc_def(Partial::new(&b"xxx,def"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx,def"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        sep_pair_abc_def(Partial::new(&b"abc,xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
}

#[test]
fn preceded_test() {
    fn preceded_abcd_efgh(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        preceded("abcd", "efgh").parse_next(i)
    }

    assert_eq!(
        preceded_abcd_efgh(Partial::new(&b"abcdefghijkl"[..])),
        Ok((Partial::new(&b"ijkl"[..]), &b"efgh"[..]))
    );
    assert_eq!(
        preceded_abcd_efgh(Partial::new(&b"ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        preceded_abcd_efgh(Partial::new(&b"abcde"[..])),
        Err(ErrMode::Incomplete(Needed::new(3)))
    );
    assert_eq!(
        preceded_abcd_efgh(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        preceded_abcd_efgh(Partial::new(&b"xxxxdef"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxxxdef"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        preceded_abcd_efgh(Partial::new(&b"abcdxxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
}

#[test]
fn terminated_test() {
    fn terminated_abcd_efgh(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        terminated("abcd", "efgh").parse_next(i)
    }

    assert_eq!(
        terminated_abcd_efgh(Partial::new(&b"abcdefghijkl"[..])),
        Ok((Partial::new(&b"ijkl"[..]), &b"abcd"[..]))
    );
    assert_eq!(
        terminated_abcd_efgh(Partial::new(&b"ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        terminated_abcd_efgh(Partial::new(&b"abcde"[..])),
        Err(ErrMode::Incomplete(Needed::new(3)))
    );
    assert_eq!(
        terminated_abcd_efgh(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        terminated_abcd_efgh(Partial::new(&b"xxxxdef"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxxxdef"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        terminated_abcd_efgh(Partial::new(&b"abcdxxxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxxx"[..]),
            ErrorKind::Tag
        )))
    );
}

#[test]
fn delimited_test() {
    fn delimited_abc_def_ghi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        delimited("abc", "def", "ghi").parse_next(i)
    }

    assert_eq!(
        delimited_abc_def_ghi(Partial::new(&b"abcdefghijkl"[..])),
        Ok((Partial::new(&b"jkl"[..]), &b"def"[..]))
    );
    assert_eq!(
        delimited_abc_def_ghi(Partial::new(&b"ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        delimited_abc_def_ghi(Partial::new(&b"abcde"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        delimited_abc_def_ghi(Partial::new(&b"abcdefgh"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        delimited_abc_def_ghi(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        delimited_abc_def_ghi(Partial::new(&b"xxxdefghi"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxxdefghi"[..]),
            ErrorKind::Tag
        ),))
    );
    assert_eq!(
        delimited_abc_def_ghi(Partial::new(&b"abcxxxghi"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxxghi"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        delimited_abc_def_ghi(Partial::new(&b"abcdefxxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
}
