use super::{length_data, length_value, many0, many1};
use crate::Parser;
use crate::Partial;
use crate::{
    character::digit1 as digit,
    error::{ErrMode, ErrorKind, Needed, ParseError},
    lib::std::str::{self, FromStr},
    number::{be_u16, be_u8},
    IResult,
};
#[cfg(feature = "alloc")]
use crate::{
    lib::std::vec::Vec,
    multi::{
        count, fold_many0, fold_many1, fold_many_m_n, length_count, many_m_n, many_till0,
        separated0, separated1,
    },
};

#[test]
#[cfg(feature = "alloc")]
fn separated0_test() {
    fn multi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        separated0("abcd", ",").parse_next(i)
    }
    fn multi_empty(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        separated0("", ",").parse_next(i)
    }
    fn multi_longsep(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        separated0("abcd", "..").parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"abcd,abcdef"[..];
    let c = &b"azerty"[..];
    let d = &b",,abc"[..];
    let e = &b"abcd,abcd,ef"[..];
    let f = &b"abc"[..];
    let g = &b"abcd."[..];
    let h = &b"abcd,abc"[..];

    let res1 = vec![&b"abcd"[..]];
    assert_eq!(multi(Partial::new(a)), Ok((Partial::new(&b"ef"[..]), res1)));
    let res2 = vec![&b"abcd"[..], &b"abcd"[..]];
    assert_eq!(multi(Partial::new(b)), Ok((Partial::new(&b"ef"[..]), res2)));
    assert_eq!(
        multi(Partial::new(c)),
        Ok((Partial::new(&b"azerty"[..]), Vec::new()))
    );
    let res3 = vec![&b""[..], &b""[..], &b""[..]];
    assert_eq!(
        multi_empty(Partial::new(d)),
        Ok((Partial::new(&b"abc"[..]), res3))
    );
    let res4 = vec![&b"abcd"[..], &b"abcd"[..]];
    assert_eq!(
        multi(Partial::new(e)),
        Ok((Partial::new(&b",ef"[..]), res4))
    );

    assert_eq!(
        multi(Partial::new(f)),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        multi_longsep(Partial::new(g)),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        multi(Partial::new(h)),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
}

#[test]
#[cfg(feature = "alloc")]
#[cfg_attr(debug_assertions, should_panic)]
fn separated0_empty_sep_test() {
    fn empty_sep(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        separated0("abc", "").parse_next(i)
    }

    let i = &b"abcabc"[..];

    let i_err_pos = &i[3..];
    assert_eq!(
        empty_sep(Partial::new(i)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(i_err_pos),
            ErrorKind::Assert
        )))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn separated1_test() {
    fn multi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        separated1("abcd", ",").parse_next(i)
    }
    fn multi_longsep(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        separated1("abcd", "..").parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"abcd,abcdef"[..];
    let c = &b"azerty"[..];
    let d = &b"abcd,abcd,ef"[..];

    let f = &b"abc"[..];
    let g = &b"abcd."[..];
    let h = &b"abcd,abc"[..];

    let res1 = vec![&b"abcd"[..]];
    assert_eq!(multi(Partial::new(a)), Ok((Partial::new(&b"ef"[..]), res1)));
    let res2 = vec![&b"abcd"[..], &b"abcd"[..]];
    assert_eq!(multi(Partial::new(b)), Ok((Partial::new(&b"ef"[..]), res2)));
    assert_eq!(
        multi(Partial::new(c)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(c),
            ErrorKind::Tag
        )))
    );
    let res3 = vec![&b"abcd"[..], &b"abcd"[..]];
    assert_eq!(
        multi(Partial::new(d)),
        Ok((Partial::new(&b",ef"[..]), res3))
    );

    assert_eq!(
        multi(Partial::new(f)),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        multi_longsep(Partial::new(g)),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        multi(Partial::new(h)),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn many0_test() {
    fn multi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        many0("abcd").parse_next(i)
    }

    assert_eq!(
        multi(Partial::new(&b"abcdef"[..])),
        Ok((Partial::new(&b"ef"[..]), vec![&b"abcd"[..]]))
    );
    assert_eq!(
        multi(Partial::new(&b"abcdabcdefgh"[..])),
        Ok((Partial::new(&b"efgh"[..]), vec![&b"abcd"[..], &b"abcd"[..]]))
    );
    assert_eq!(
        multi(Partial::new(&b"azerty"[..])),
        Ok((Partial::new(&b"azerty"[..]), Vec::new()))
    );
    assert_eq!(
        multi(Partial::new(&b"abcdab"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        multi(Partial::new(&b"abcd"[..])),
        Err(ErrMode::Incomplete(Needed::new(4)))
    );
    assert_eq!(
        multi(Partial::new(&b""[..])),
        Err(ErrMode::Incomplete(Needed::new(4)))
    );
}

#[test]
#[cfg(feature = "alloc")]
#[cfg_attr(debug_assertions, should_panic)]
fn many0_empty_test() {
    fn multi_empty(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        many0("").parse_next(i)
    }

    assert_eq!(
        multi_empty(Partial::new(&b"abcdef"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"abcdef"[..]),
            ErrorKind::Assert
        )))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn many1_test() {
    fn multi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        many1("abcd").parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"abcdabcdefgh"[..];
    let c = &b"azerty"[..];
    let d = &b"abcdab"[..];

    let res1 = vec![&b"abcd"[..]];
    assert_eq!(multi(Partial::new(a)), Ok((Partial::new(&b"ef"[..]), res1)));
    let res2 = vec![&b"abcd"[..], &b"abcd"[..]];
    assert_eq!(
        multi(Partial::new(b)),
        Ok((Partial::new(&b"efgh"[..]), res2))
    );
    assert_eq!(
        multi(Partial::new(c)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(c),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        multi(Partial::new(d)),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn many_till_test() {
    #[allow(clippy::type_complexity)]
    fn multi(i: &[u8]) -> IResult<&[u8], (Vec<&[u8]>, &[u8])> {
        many_till0("abcd", "efgh").parse_next(i)
    }

    let a = b"abcdabcdefghabcd";
    let b = b"efghabcd";
    let c = b"azerty";

    let res_a = (vec![&b"abcd"[..], &b"abcd"[..]], &b"efgh"[..]);
    let res_b: (Vec<&[u8]>, &[u8]) = (Vec::new(), &b"efgh"[..]);
    assert_eq!(multi(&a[..]), Ok((&b"abcd"[..], res_a)));
    assert_eq!(multi(&b[..]), Ok((&b"abcd"[..], res_b)));
    assert_eq!(
        multi(&c[..]),
        Err(ErrMode::Backtrack(error_node_position!(
            &c[..],
            ErrorKind::Many,
            error_position!(&c[..], ErrorKind::Tag)
        )))
    );
}

#[test]
#[cfg(feature = "std")]
fn infinite_many() {
    fn tst(input: &[u8]) -> IResult<&[u8], &[u8]> {
        println!("input: {:?}", input);
        Err(ErrMode::Backtrack(error_position!(input, ErrorKind::Tag)))
    }

    // should not go into an infinite loop
    fn multi0(i: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
        many0(tst).parse_next(i)
    }
    let a = &b"abcdef"[..];
    assert_eq!(multi0(a), Ok((a, Vec::new())));

    fn multi1(i: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
        many1(tst).parse_next(i)
    }
    let a = &b"abcdef"[..];
    assert_eq!(
        multi1(a),
        Err(ErrMode::Backtrack(error_position!(a, ErrorKind::Tag)))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn many_m_n_test() {
    fn multi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        many_m_n(2, 4, "Abcd").parse_next(i)
    }

    let a = &b"Abcdef"[..];
    let b = &b"AbcdAbcdefgh"[..];
    let c = &b"AbcdAbcdAbcdAbcdefgh"[..];
    let d = &b"AbcdAbcdAbcdAbcdAbcdefgh"[..];
    let e = &b"AbcdAb"[..];

    assert_eq!(
        multi(Partial::new(a)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"ef"[..]),
            ErrorKind::Tag
        )))
    );
    let res1 = vec![&b"Abcd"[..], &b"Abcd"[..]];
    assert_eq!(
        multi(Partial::new(b)),
        Ok((Partial::new(&b"efgh"[..]), res1))
    );
    let res2 = vec![&b"Abcd"[..], &b"Abcd"[..], &b"Abcd"[..], &b"Abcd"[..]];
    assert_eq!(
        multi(Partial::new(c)),
        Ok((Partial::new(&b"efgh"[..]), res2))
    );
    let res3 = vec![&b"Abcd"[..], &b"Abcd"[..], &b"Abcd"[..], &b"Abcd"[..]];
    assert_eq!(
        multi(Partial::new(d)),
        Ok((Partial::new(&b"Abcdefgh"[..]), res3))
    );
    assert_eq!(
        multi(Partial::new(e)),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn count_test() {
    const TIMES: usize = 2;
    fn cnt_2(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        count("abc", TIMES).parse_next(i)
    }

    assert_eq!(
        cnt_2(Partial::new(&b"abcabcabcdef"[..])),
        Ok((Partial::new(&b"abcdef"[..]), vec![&b"abc"[..], &b"abc"[..]]))
    );
    assert_eq!(
        cnt_2(Partial::new(&b"ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        cnt_2(Partial::new(&b"abcab"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        cnt_2(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        cnt_2(Partial::new(&b"xxxabcabcdef"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxxabcabcdef"[..]),
            ErrorKind::Tag
        )))
    );
    assert_eq!(
        cnt_2(Partial::new(&b"abcxxxabcdef"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxxabcdef"[..]),
            ErrorKind::Tag
        )))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn count_zero() {
    const TIMES: usize = 0;
    fn counter_2(i: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
        count("abc", TIMES).parse_next(i)
    }

    let done = &b"abcabcabcdef"[..];
    let parsed_done = Vec::new();
    let rest = done;
    let incomplete_1 = &b"ab"[..];
    let parsed_incompl_1 = Vec::new();
    let incomplete_2 = &b"abcab"[..];
    let parsed_incompl_2 = Vec::new();
    let error = &b"xxx"[..];
    let error_remain = &b"xxx"[..];
    let parsed_err = Vec::new();
    let error_1 = &b"xxxabcabcdef"[..];
    let parsed_err_1 = Vec::new();
    let error_1_remain = &b"xxxabcabcdef"[..];
    let error_2 = &b"abcxxxabcdef"[..];
    let parsed_err_2 = Vec::new();
    let error_2_remain = &b"abcxxxabcdef"[..];

    assert_eq!(counter_2(done), Ok((rest, parsed_done)));
    assert_eq!(
        counter_2(incomplete_1),
        Ok((incomplete_1, parsed_incompl_1))
    );
    assert_eq!(
        counter_2(incomplete_2),
        Ok((incomplete_2, parsed_incompl_2))
    );
    assert_eq!(counter_2(error), Ok((error_remain, parsed_err)));
    assert_eq!(counter_2(error_1), Ok((error_1_remain, parsed_err_1)));
    assert_eq!(counter_2(error_2), Ok((error_2_remain, parsed_err_2)));
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NilError;

impl<I> From<(I, ErrorKind)> for NilError {
    fn from(_: (I, ErrorKind)) -> Self {
        NilError
    }
}

impl<I> ParseError<I> for NilError {
    fn from_error_kind(_: I, _: ErrorKind) -> NilError {
        NilError
    }
    fn append(self, _: I, _: ErrorKind) -> NilError {
        NilError
    }
}

#[test]
#[cfg(feature = "alloc")]
fn length_count_test() {
    fn number(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u32> {
        digit
            .map_res(str::from_utf8)
            .map_res(FromStr::from_str)
            .parse_next(i)
    }

    fn cnt(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        length_count(number, "abc").parse_next(i)
    }

    assert_eq!(
        cnt(Partial::new(&b"2abcabcabcdef"[..])),
        Ok((Partial::new(&b"abcdef"[..]), vec![&b"abc"[..], &b"abc"[..]]))
    );
    assert_eq!(
        cnt(Partial::new(&b"2ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        cnt(Partial::new(&b"3abcab"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        cnt(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Slice
        )))
    );
    assert_eq!(
        cnt(Partial::new(&b"2abcxxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Tag
        )))
    );
}

#[test]
fn length_data_test() {
    fn number(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u32> {
        digit
            .map_res(str::from_utf8)
            .map_res(FromStr::from_str)
            .parse_next(i)
    }

    fn take(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, &[u8]> {
        length_data(number).parse_next(i)
    }

    assert_eq!(
        take(Partial::new(&b"6abcabcabcdef"[..])),
        Ok((Partial::new(&b"abcdef"[..]), &b"abcabc"[..]))
    );
    assert_eq!(
        take(Partial::new(&b"3ab"[..])),
        Err(ErrMode::Incomplete(Needed::new(1)))
    );
    assert_eq!(
        take(Partial::new(&b"xxx"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"xxx"[..]),
            ErrorKind::Slice
        )))
    );
    assert_eq!(
        take(Partial::new(&b"2abcxxx"[..])),
        Ok((Partial::new(&b"cxxx"[..]), &b"ab"[..]))
    );
}

#[test]
fn length_value_test() {
    use crate::stream::StreamIsPartial;

    fn length_value_1(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, u16> {
        length_value(be_u8, be_u16).parse_next(i)
    }
    fn length_value_2(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, (u8, u8)> {
        length_value(be_u8, (be_u8, be_u8)).parse_next(i)
    }

    let mut empty_complete = Partial::new(&b""[..]);
    let _ = empty_complete.complete();

    let i1 = [0, 5, 6];
    assert_eq!(
        length_value_1(Partial::new(&i1)),
        Err(ErrMode::Backtrack(error_position!(
            empty_complete,
            ErrorKind::Slice
        )))
    );
    assert_eq!(
        length_value_2(Partial::new(&i1)),
        Err(ErrMode::Backtrack(error_position!(
            empty_complete,
            ErrorKind::Token
        )))
    );

    let i2 = [1, 5, 6, 3];
    {
        let mut middle_complete = Partial::new(&i2[1..2]);
        let _ = middle_complete.complete();
        assert_eq!(
            length_value_1(Partial::new(&i2)),
            Err(ErrMode::Backtrack(error_position!(
                middle_complete,
                ErrorKind::Slice
            )))
        );
        assert_eq!(
            length_value_2(Partial::new(&i2)),
            Err(ErrMode::Backtrack(error_position!(
                empty_complete,
                ErrorKind::Token
            )))
        );
    }

    let i3 = [2, 5, 6, 3, 4, 5, 7];
    assert_eq!(
        length_value_1(Partial::new(&i3)),
        Ok((Partial::new(&i3[3..]), 1286))
    );
    assert_eq!(
        length_value_2(Partial::new(&i3)),
        Ok((Partial::new(&i3[3..]), (5, 6)))
    );

    let i4 = [3, 5, 6, 3, 4, 5];
    assert_eq!(
        length_value_1(Partial::new(&i4)),
        Ok((Partial::new(&i4[4..]), 1286))
    );
    assert_eq!(
        length_value_2(Partial::new(&i4)),
        Ok((Partial::new(&i4[4..]), (5, 6)))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn fold_many0_test() {
    fn fold_into_vec<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
        acc.push(item);
        acc
    }
    fn multi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        fold_many0("abcd", Vec::new, fold_into_vec).parse_next(i)
    }

    assert_eq!(
        multi(Partial::new(&b"abcdef"[..])),
        Ok((Partial::new(&b"ef"[..]), vec![&b"abcd"[..]]))
    );
    assert_eq!(
        multi(Partial::new(&b"abcdabcdefgh"[..])),
        Ok((Partial::new(&b"efgh"[..]), vec![&b"abcd"[..], &b"abcd"[..]]))
    );
    assert_eq!(
        multi(Partial::new(&b"azerty"[..])),
        Ok((Partial::new(&b"azerty"[..]), Vec::new()))
    );
    assert_eq!(
        multi(Partial::new(&b"abcdab"[..])),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
    assert_eq!(
        multi(Partial::new(&b"abcd"[..])),
        Err(ErrMode::Incomplete(Needed::new(4)))
    );
    assert_eq!(
        multi(Partial::new(&b""[..])),
        Err(ErrMode::Incomplete(Needed::new(4)))
    );
}

#[test]
#[cfg(feature = "alloc")]
#[cfg_attr(debug_assertions, should_panic)]
fn fold_many0_empty_test() {
    fn fold_into_vec<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
        acc.push(item);
        acc
    }
    fn multi_empty(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        fold_many0("", Vec::new, fold_into_vec).parse_next(i)
    }

    assert_eq!(
        multi_empty(Partial::new(&b"abcdef"[..])),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"abcdef"[..]),
            ErrorKind::Assert
        )))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn fold_many1_test() {
    fn fold_into_vec<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
        acc.push(item);
        acc
    }
    fn multi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        fold_many1("abcd", Vec::new, fold_into_vec).parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"abcdabcdefgh"[..];
    let c = &b"azerty"[..];
    let d = &b"abcdab"[..];

    let res1 = vec![&b"abcd"[..]];
    assert_eq!(multi(Partial::new(a)), Ok((Partial::new(&b"ef"[..]), res1)));
    let res2 = vec![&b"abcd"[..], &b"abcd"[..]];
    assert_eq!(
        multi(Partial::new(b)),
        Ok((Partial::new(&b"efgh"[..]), res2))
    );
    assert_eq!(
        multi(Partial::new(c)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(c),
            ErrorKind::Many
        )))
    );
    assert_eq!(
        multi(Partial::new(d)),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
}

#[test]
#[cfg(feature = "alloc")]
fn fold_many_m_n_test() {
    fn fold_into_vec<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
        acc.push(item);
        acc
    }
    fn multi(i: Partial<&[u8]>) -> IResult<Partial<&[u8]>, Vec<&[u8]>> {
        fold_many_m_n(2, 4, "Abcd", Vec::new, fold_into_vec).parse_next(i)
    }

    let a = &b"Abcdef"[..];
    let b = &b"AbcdAbcdefgh"[..];
    let c = &b"AbcdAbcdAbcdAbcdefgh"[..];
    let d = &b"AbcdAbcdAbcdAbcdAbcdefgh"[..];
    let e = &b"AbcdAb"[..];

    assert_eq!(
        multi(Partial::new(a)),
        Err(ErrMode::Backtrack(error_position!(
            Partial::new(&b"ef"[..]),
            ErrorKind::Tag
        )))
    );
    let res1 = vec![&b"Abcd"[..], &b"Abcd"[..]];
    assert_eq!(
        multi(Partial::new(b)),
        Ok((Partial::new(&b"efgh"[..]), res1))
    );
    let res2 = vec![&b"Abcd"[..], &b"Abcd"[..], &b"Abcd"[..], &b"Abcd"[..]];
    assert_eq!(
        multi(Partial::new(c)),
        Ok((Partial::new(&b"efgh"[..]), res2))
    );
    let res3 = vec![&b"Abcd"[..], &b"Abcd"[..], &b"Abcd"[..], &b"Abcd"[..]];
    assert_eq!(
        multi(Partial::new(d)),
        Ok((Partial::new(&b"Abcdefgh"[..]), res3))
    );
    assert_eq!(
        multi(Partial::new(e)),
        Err(ErrMode::Incomplete(Needed::new(2)))
    );
}

#[test]
fn many0_count_test() {
    fn count0_nums(i: &[u8]) -> IResult<&[u8], usize> {
        many0((digit, ",")).parse_next(i)
    }

    assert_eq!(count0_nums(&b"123,junk"[..]), Ok((&b"junk"[..], 1)));

    assert_eq!(count0_nums(&b"123,45,junk"[..]), Ok((&b"junk"[..], 2)));

    assert_eq!(
        count0_nums(&b"1,2,3,4,5,6,7,8,9,0,junk"[..]),
        Ok((&b"junk"[..], 10))
    );

    assert_eq!(count0_nums(&b"hello"[..]), Ok((&b"hello"[..], 0)));
}

#[test]
fn many1_count_test() {
    fn count1_nums(i: &[u8]) -> IResult<&[u8], usize> {
        many1((digit, ",")).parse_next(i)
    }

    assert_eq!(count1_nums(&b"123,45,junk"[..]), Ok((&b"junk"[..], 2)));

    assert_eq!(
        count1_nums(&b"1,2,3,4,5,6,7,8,9,0,junk"[..]),
        Ok((&b"junk"[..], 10))
    );

    assert_eq!(
        count1_nums(&b"hello"[..]),
        Err(ErrMode::Backtrack(error_position!(
            &b"hello"[..],
            ErrorKind::Slice
        )))
    );
}
