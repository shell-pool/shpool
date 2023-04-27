use std::collections::HashMap;

use winnow::prelude::*;
use winnow::{
    bytes::{take_till0, take_while0, take_while1},
    character::{alphanumeric1 as alphanumeric, space0 as space},
    combinator::opt,
    multi::many0,
    sequence::{delimited, terminated},
};

pub type Stream<'i> = &'i str;

pub fn categories(input: Stream<'_>) -> IResult<Stream<'_>, HashMap<&str, HashMap<&str, &str>>> {
    many0(category_and_keys)(input)
}

fn category_and_keys(i: Stream<'_>) -> IResult<Stream<'_>, (&str, HashMap<&str, &str>)> {
    (category, keys_and_values).parse_next(i)
}

fn category(i: Stream<'_>) -> IResult<Stream<'_>, &str> {
    terminated(
        delimited('[', take_while0(|c| c != ']'), ']'),
        opt(take_while1(" \r\n")),
    )(i)
}

fn keys_and_values(input: Stream<'_>) -> IResult<Stream<'_>, HashMap<&str, &str>> {
    many0(key_value)(input)
}

fn key_value(i: Stream<'_>) -> IResult<Stream<'_>, (&str, &str)> {
    let (i, key) = alphanumeric(i)?;
    let (i, _) = (opt(space), "=", opt(space)).parse_next(i)?;
    let (i, val) = take_till0(is_line_ending_or_comment)(i)?;
    let (i, _) = opt(space)(i)?;
    let (i, _) = opt((";", not_line_ending))(i)?;
    let (i, _) = opt(space_or_line_ending)(i)?;

    Ok((i, (key, val)))
}

fn is_line_ending_or_comment(chr: char) -> bool {
    chr == ';' || chr == '\n'
}

fn not_line_ending(i: Stream<'_>) -> IResult<Stream<'_>, &str> {
    take_while0(|c| c != '\r' && c != '\n')(i)
}

fn space_or_line_ending(i: Stream<'_>) -> IResult<Stream<'_>, &str> {
    take_while1(" \r\n")(i)
}

#[test]
fn parse_category_test() {
    let ini_file = "[category]

parameter=value
key = value2";

    let ini_without_category = "parameter=value
key = value2";

    let res = category(ini_file);
    println!("{:?}", res);
    match res {
        Ok((i, o)) => println!("i: {} | o: {:?}", i, o),
        _ => println!("error"),
    }

    assert_eq!(res, Ok((ini_without_category, "category")));
}

#[test]
fn parse_key_value_test() {
    let ini_file = "parameter=value
key = value2";

    let ini_without_key_value = "key = value2";

    let res = key_value(ini_file);
    println!("{:?}", res);
    match res {
        Ok((i, (o1, o2))) => println!("i: {} | o: ({:?},{:?})", i, o1, o2),
        _ => println!("error"),
    }

    assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_key_value_with_space_test() {
    let ini_file = "parameter = value
key = value2";

    let ini_without_key_value = "key = value2";

    let res = key_value(ini_file);
    println!("{:?}", res);
    match res {
        Ok((i, (o1, o2))) => println!("i: {} | o: ({:?},{:?})", i, o1, o2),
        _ => println!("error"),
    }

    assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_key_value_with_comment_test() {
    let ini_file = "parameter=value;abc
key = value2";

    let ini_without_key_value = "key = value2";

    let res = key_value(ini_file);
    println!("{:?}", res);
    match res {
        Ok((i, (o1, o2))) => println!("i: {} | o: ({:?},{:?})", i, o1, o2),
        _ => println!("error"),
    }

    assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_multiple_keys_and_values_test() {
    let ini_file = "parameter=value;abc

key = value2

[category]";

    let ini_without_key_value = "[category]";

    let res = keys_and_values(ini_file);
    println!("{:?}", res);
    match res {
        Ok((i, ref o)) => println!("i: {} | o: {:?}", i, o),
        _ => println!("error"),
    }

    let mut expected: HashMap<&str, &str> = HashMap::new();
    expected.insert("parameter", "value");
    expected.insert("key", "value2");
    assert_eq!(res, Ok((ini_without_key_value, expected)));
}

#[test]
fn parse_category_then_multiple_keys_and_values_test() {
    //FIXME: there can be an empty line or a comment line after a category
    let ini_file = "[abcd]
parameter=value;abc

key = value2

[category]";

    let ini_after_parser = "[category]";

    let res = category_and_keys(ini_file);
    println!("{:?}", res);
    match res {
        Ok((i, ref o)) => println!("i: {} | o: {:?}", i, o),
        _ => println!("error"),
    }

    let mut expected_h: HashMap<&str, &str> = HashMap::new();
    expected_h.insert("parameter", "value");
    expected_h.insert("key", "value2");
    assert_eq!(res, Ok((ini_after_parser, ("abcd", expected_h))));
}

#[test]
fn parse_multiple_categories_test() {
    let ini_file = "[abcd]

parameter=value;abc

key = value2

[category]
parameter3=value3
key4 = value4
";

    let res = categories(ini_file);
    //println!("{:?}", res);
    match res {
        Ok((i, ref o)) => println!("i: {} | o: {:?}", i, o),
        _ => println!("error"),
    }

    let mut expected_1: HashMap<&str, &str> = HashMap::new();
    expected_1.insert("parameter", "value");
    expected_1.insert("key", "value2");
    let mut expected_2: HashMap<&str, &str> = HashMap::new();
    expected_2.insert("parameter3", "value3");
    expected_2.insert("key4", "value4");
    let mut expected_h: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    expected_h.insert("abcd", expected_1);
    expected_h.insert("category", expected_2);
    assert_eq!(res, Ok(("", expected_h)));
}
