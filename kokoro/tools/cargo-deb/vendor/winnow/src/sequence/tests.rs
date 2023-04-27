use super::*;
use crate::bytes::{tag, take};
use crate::error::{ErrMode, Error, ErrorKind, Needed};
use crate::number::be_u16;
use crate::IResult;
use crate::Partial;

#[test]
fn single_element_tuples() {
    #![allow(deprecated)]
    use crate::character::alpha1;
    use crate::error::ErrorKind;

    let mut parser = tuple((alpha1,));
    assert_eq!(parser("abc123def"), Ok(("123def", ("abc",))));
    assert_eq!(
        parser("123def"),
        Err(ErrMode::Backtrack(Error {
            input: "123def",
            kind: ErrorKind::Alpha
        }))
    );
}

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
    use crate::bytes::tag;
    fn err_test(i: &[u8]) -> IResult<&[u8], &[u8]> {
        let (i, _) = tag("ijkl")(i)?;
        tag("mnop")(i)
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
fn pair_test() {
    #![allow(deprecated)]
    #[allow(clippy::type_complexity)]
    fn pair_abc_def(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, (&[u8], &[u8])> {
        pair(tag("abc"), tag("def"))(i)
    }

    assert_eq!(
        pair_abc_def(Partial::new(&b"abcdefghijkl"[..])),
        Ok((Partial::new(&b"ghijkl"[..]), (&b"abc"[..], &b"def"[..])))
    );
    assert_eq!(
        pair_abc_def(Partial::new(&b"ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        pair_abc_def(Partial::new(&b"abcd"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        pair_abc_def(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        pair_abc_def(Partial::new(&b"xxxdef"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxxdef"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        pair_abc_def(Partial::new(&b"abcxxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
}

#[test]
fn separated_pair_test() {
    #[allow(clippy::type_complexity)]
    fn sep_pair_abc_def(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, (&[u8], &[u8])> {
        separated_pair(tag("abc"), tag(","), tag("def"))(i)
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
        preceded(tag("abcd"), tag("efgh"))(i)
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
        terminated(tag("abcd"), tag("efgh"))(i)
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
        delimited(tag("abc"), tag("def"), tag("ghi"))(i)
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

#[test]
fn tuple_test() {
    #![allow(deprecated)]
    #[allow(clippy::type_complexity)]
    fn tuple_3(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, (u16, &[u8], &[u8])> {
        tuple((be_u16, take(3u8), tag("fg")))(i)
    }

    assert_eq!(
        tuple_3(Partial::new(&b"abcdefgh"[..])),
        Ok((
            Partial::new(&b"h"[..]),
            (0x6162u16, &b"cde"[..], &b"fg"[..])
        ))
    );
    assert_eq!(
        tuple_3(Partial::new(&b"abcd"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        tuple_3(Partial::new(&b"abcde"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        tuple_3(Partial::new(&b"abcdejk"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"jk"[..]),
            ErrorKind::Tag
        )))
    );
}

#[test]
fn unit_type() {
    #![allow(deprecated)]
    assert_eq!(
        tuple::<&'static str, (), Error<&'static str>, ()>(())("abxsbsh"),
        Ok(("abxsbsh", ()))
    );
    assert_eq!(
        tuple::<&'static str, (), Error<&'static str>, ()>(())("sdfjakdsas"),
        Ok(("sdfjakdsas", ()))
    );
    assert_eq!(
        tuple::<&'static str, (), Error<&'static str>, ()>(())(""),
        Ok(("", ()))
    );
}
