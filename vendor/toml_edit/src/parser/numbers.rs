use std::ops::RangeInclusive;

use nom8::branch::alt;
use nom8::bytes::one_of;
use nom8::bytes::tag;
use nom8::bytes::take;
use nom8::combinator::cut;
use nom8::combinator::opt;
use nom8::combinator::peek;
use nom8::combinator::rest;
use nom8::multi::many0_count;
use nom8::sequence::preceded;

use crate::parser::prelude::*;
use crate::parser::trivia::from_utf8_unchecked;

// ;; Boolean

// boolean = true / false
#[allow(dead_code)] // directly define in `fn value`
pub(crate) fn boolean(input: Input<'_>) -> IResult<Input<'_>, bool, ParserError<'_>> {
    alt((true_, false_)).parse(input)
}

pub(crate) fn true_(input: Input<'_>) -> IResult<Input<'_>, bool, ParserError<'_>> {
    (peek(TRUE[0]), cut(TRUE)).value(true).parse(input)
}
const TRUE: &[u8] = b"true";

pub(crate) fn false_(input: Input<'_>) -> IResult<Input<'_>, bool, ParserError<'_>> {
    (peek(FALSE[0]), cut(FALSE)).value(false).parse(input)
}
const FALSE: &[u8] = b"false";

// ;; Integer

// integer = dec-int / hex-int / oct-int / bin-int
pub(crate) fn integer(input: Input<'_>) -> IResult<Input<'_>, i64, ParserError<'_>> {
    dispatch! {peek(opt::<_, &[u8], _, _>(take(2usize)));
        Some(b"0x") => cut(hex_int.map_res(|s| i64::from_str_radix(&s.replace('_', ""), 16))),
        Some(b"0o") => cut(oct_int.map_res(|s| i64::from_str_radix(&s.replace('_', ""), 8))),
        Some(b"0b") => cut(bin_int.map_res(|s| i64::from_str_radix(&s.replace('_', ""), 2))),
        _ => dec_int.and_then(cut(rest
            .map_res(|s: &str| s.replace('_', "").parse())))
    }
    .parse(input)
}

// dec-int = [ minus / plus ] unsigned-dec-int
// unsigned-dec-int = DIGIT / digit1-9 1*( DIGIT / underscore DIGIT )
pub(crate) fn dec_int(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    (
        opt(one_of((b'+', b'-'))),
        alt((
            (
                one_of(DIGIT1_9),
                many0_count(alt((
                    digit.value(()),
                    (
                        one_of(b'_'),
                        cut(digit).context(Context::Expected(ParserValue::Description("digit"))),
                    )
                        .value(()),
                ))),
            )
                .value(()),
            digit.value(()),
        )),
    )
        .recognize()
        .map(|b: &[u8]| unsafe { from_utf8_unchecked(b, "`digit` and `_` filter out non-ASCII") })
        .context(Context::Expression("integer"))
        .parse(input)
}
const DIGIT1_9: RangeInclusive<u8> = b'1'..=b'9';

// hex-prefix = %x30.78               ; 0x
// hex-int = hex-prefix HEXDIG *( HEXDIG / underscore HEXDIG )
pub(crate) fn hex_int(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    preceded(
        HEX_PREFIX,
        cut((
            hexdig,
            many0_count(alt((
                hexdig.value(()),
                (
                    one_of(b'_'),
                    cut(hexdig).context(Context::Expected(ParserValue::Description("digit"))),
                )
                    .value(()),
            ))),
        ))
        .recognize(),
    )
    .map(|b| unsafe { from_utf8_unchecked(b, "`hexdig` and `_` filter out non-ASCII") })
    .context(Context::Expression("hexadecimal integer"))
    .parse(input)
}
const HEX_PREFIX: &[u8] = b"0x";

// oct-prefix = %x30.6F               ; 0o
// oct-int = oct-prefix digit0-7 *( digit0-7 / underscore digit0-7 )
pub(crate) fn oct_int(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    preceded(
        OCT_PREFIX,
        cut((
            one_of(DIGIT0_7),
            many0_count(alt((
                one_of(DIGIT0_7).value(()),
                (
                    one_of(b'_'),
                    cut(one_of(DIGIT0_7))
                        .context(Context::Expected(ParserValue::Description("digit"))),
                )
                    .value(()),
            ))),
        ))
        .recognize(),
    )
    .map(|b| unsafe { from_utf8_unchecked(b, "`DIGIT0_7` and `_` filter out non-ASCII") })
    .context(Context::Expression("octal integer"))
    .parse(input)
}
const OCT_PREFIX: &[u8] = b"0o";
const DIGIT0_7: RangeInclusive<u8> = b'0'..=b'7';

// bin-prefix = %x30.62               ; 0b
// bin-int = bin-prefix digit0-1 *( digit0-1 / underscore digit0-1 )
pub(crate) fn bin_int(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    preceded(
        BIN_PREFIX,
        cut((
            one_of(DIGIT0_1),
            many0_count(alt((
                one_of(DIGIT0_1).value(()),
                (
                    one_of(b'_'),
                    cut(one_of(DIGIT0_1))
                        .context(Context::Expected(ParserValue::Description("digit"))),
                )
                    .value(()),
            ))),
        ))
        .recognize(),
    )
    .map(|b| unsafe { from_utf8_unchecked(b, "`DIGIT0_1` and `_` filter out non-ASCII") })
    .context(Context::Expression("binary integer"))
    .parse(input)
}
const BIN_PREFIX: &[u8] = b"0b";
const DIGIT0_1: RangeInclusive<u8> = b'0'..=b'1';

// ;; Float

// float = float-int-part ( exp / frac [ exp ] )
// float =/ special-float
// float-int-part = dec-int
pub(crate) fn float(input: Input<'_>) -> IResult<Input<'_>, f64, ParserError<'_>> {
    alt((
        float_.and_then(cut(rest
            .map_res(|s: &str| s.replace('_', "").parse())
            .verify(|f: &f64| *f != f64::INFINITY))),
        special_float,
    ))
    .context(Context::Expression("floating-point number"))
    .parse(input)
}

pub(crate) fn float_(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    (dec_int, alt((exp, (frac, opt(exp)).map(|_| ""))))
        .recognize()
        .map(|b: &[u8]| unsafe {
            from_utf8_unchecked(
                b,
                "`dec_int`, `one_of`, `exp`, and `frac` filter out non-ASCII",
            )
        })
        .parse(input)
}

// frac = decimal-point zero-prefixable-int
// decimal-point = %x2E               ; .
pub(crate) fn frac(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    (
        b'.',
        cut(zero_prefixable_int).context(Context::Expected(ParserValue::Description("digit"))),
    )
        .recognize()
        .map(|b: &[u8]| unsafe {
            from_utf8_unchecked(
                b,
                "`.` and `parse_zero_prefixable_int` filter out non-ASCII",
            )
        })
        .parse(input)
}

// zero-prefixable-int = DIGIT *( DIGIT / underscore DIGIT )
pub(crate) fn zero_prefixable_int(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    (
        digit,
        many0_count(alt((
            digit.value(()),
            (
                one_of(b'_'),
                cut(digit).context(Context::Expected(ParserValue::Description("digit"))),
            )
                .value(()),
        ))),
    )
        .recognize()
        .map(|b: &[u8]| unsafe { from_utf8_unchecked(b, "`digit` and `_` filter out non-ASCII") })
        .parse(input)
}

// exp = "e" float-exp-part
// float-exp-part = [ minus / plus ] zero-prefixable-int
pub(crate) fn exp(input: Input<'_>) -> IResult<Input<'_>, &str, ParserError<'_>> {
    (
        one_of((b'e', b'E')),
        opt(one_of([b'+', b'-'])),
        cut(zero_prefixable_int),
    )
        .recognize()
        .map(|b: &[u8]| unsafe {
            from_utf8_unchecked(
                b,
                "`one_of` and `parse_zero_prefixable_int` filter out non-ASCII",
            )
        })
        .parse(input)
}

// special-float = [ minus / plus ] ( inf / nan )
pub(crate) fn special_float(input: Input<'_>) -> IResult<Input<'_>, f64, ParserError<'_>> {
    (opt(one_of((b'+', b'-'))), alt((inf, nan)))
        .map(|(s, f)| match s {
            Some(b'+') | None => f,
            Some(b'-') => -f,
            _ => unreachable!("one_of should prevent this"),
        })
        .parse(input)
}
// inf = %x69.6e.66  ; inf
pub(crate) fn inf(input: Input<'_>) -> IResult<Input<'_>, f64, ParserError<'_>> {
    tag(INF).value(f64::INFINITY).parse(input)
}
const INF: &[u8] = b"inf";
// nan = %x6e.61.6e  ; nan
pub(crate) fn nan(input: Input<'_>) -> IResult<Input<'_>, f64, ParserError<'_>> {
    tag(NAN).value(f64::NAN).parse(input)
}
const NAN: &[u8] = b"nan";

// DIGIT = %x30-39 ; 0-9
pub(crate) fn digit(input: Input<'_>) -> IResult<Input<'_>, u8, ParserError<'_>> {
    one_of(DIGIT).parse(input)
}
const DIGIT: RangeInclusive<u8> = b'0'..=b'9';

// HEXDIG = DIGIT / "A" / "B" / "C" / "D" / "E" / "F"
pub(crate) fn hexdig(input: Input<'_>) -> IResult<Input<'_>, u8, ParserError<'_>> {
    one_of(HEXDIG).parse(input)
}
pub(crate) const HEXDIG: (RangeInclusive<u8>, RangeInclusive<u8>, RangeInclusive<u8>) =
    (DIGIT, b'A'..=b'F', b'a'..=b'f');

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn integers() {
        let cases = [
            ("+99", 99),
            ("42", 42),
            ("0", 0),
            ("-17", -17),
            ("1_000", 1_000),
            ("5_349_221", 5_349_221),
            ("1_2_3_4_5", 1_2_3_4_5),
            ("0xF", 15),
            ("0o0_755", 493),
            ("0b1_0_1", 5),
            (&std::i64::MIN.to_string()[..], std::i64::MIN),
            (&std::i64::MAX.to_string()[..], std::i64::MAX),
        ];
        for &(input, expected) in &cases {
            dbg!(input);
            let parsed = integer.parse(new_input(input)).finish();
            assert_eq!(parsed, Ok(expected), "Parsing {input:?}");
        }

        let overflow = "1000000000000000000000000000000000";
        let parsed = integer.parse(new_input(overflow)).finish();
        assert!(parsed.is_err());
    }

    #[track_caller]
    fn assert_float_eq(actual: f64, expected: f64) {
        if expected.is_nan() {
            assert!(actual.is_nan());
        } else if expected.is_infinite() {
            assert!(actual.is_infinite());
            assert_eq!(expected.is_sign_positive(), actual.is_sign_positive());
        } else {
            dbg!(expected);
            dbg!(actual);
            assert!((expected - actual).abs() < std::f64::EPSILON);
        }
    }

    #[test]
    fn floats() {
        let cases = [
            ("+1.0", 1.0),
            ("3.1419", 3.1419),
            ("-0.01", -0.01),
            ("5e+22", 5e+22),
            ("1e6", 1e6),
            ("-2E-2", -2E-2),
            ("6.626e-34", 6.626e-34),
            ("9_224_617.445_991_228_313", 9_224_617.445_991_227),
            ("-1.7976931348623157e+308", std::f64::MIN),
            ("1.7976931348623157e+308", std::f64::MAX),
            ("nan", f64::NAN),
            ("+nan", f64::NAN),
            ("-nan", f64::NAN),
            ("inf", f64::INFINITY),
            ("+inf", f64::INFINITY),
            ("-inf", f64::NEG_INFINITY),
            // ("1e+400", std::f64::INFINITY),
        ];
        for &(input, expected) in &cases {
            dbg!(input);
            let parsed = float.parse(new_input(input)).finish().unwrap();
            assert_float_eq(parsed, expected);

            let overflow = "9e99999";
            let parsed = float.parse(new_input(overflow)).finish();
            assert!(parsed.is_err(), "{:?}", parsed);
        }
    }
}
