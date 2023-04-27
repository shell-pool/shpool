use std::fmt;
use std::fmt::{Debug, Display, Formatter};

use std::str::FromStr;

use winnow::prelude::*;
use winnow::{
    branch::alt,
    character::{digit1 as digit, multispace0 as multispace},
    multi::many0,
    sequence::{delimited, preceded},
    IResult,
};

#[derive(Debug)]
pub enum Expr {
    Value(i64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Paren(Box<Expr>),
}

#[derive(Debug)]
pub enum Oper {
    Add,
    Sub,
    Mul,
    Div,
}

impl Display for Expr {
    fn fmt(&self, format: &mut Formatter<'_>) -> fmt::Result {
        use Expr::{Add, Div, Mul, Paren, Sub, Value};
        match *self {
            Value(val) => write!(format, "{}", val),
            Add(ref left, ref right) => write!(format, "{} + {}", left, right),
            Sub(ref left, ref right) => write!(format, "{} - {}", left, right),
            Mul(ref left, ref right) => write!(format, "{} * {}", left, right),
            Div(ref left, ref right) => write!(format, "{} / {}", left, right),
            Paren(ref expr) => write!(format, "({})", expr),
        }
    }
}

pub fn expr(i: &str) -> IResult<&str, Expr> {
    let (i, initial) = term(i)?;
    let (i, remainder) = many0(alt((
        |i| {
            let (i, add) = preceded("+", term)(i)?;
            Ok((i, (Oper::Add, add)))
        },
        |i| {
            let (i, sub) = preceded("-", term)(i)?;
            Ok((i, (Oper::Sub, sub)))
        },
    )))(i)?;

    Ok((i, fold_exprs(initial, remainder)))
}

fn term(i: &str) -> IResult<&str, Expr> {
    let (i, initial) = factor(i)?;
    let (i, remainder) = many0(alt((
        |i| {
            let (i, mul) = preceded("*", factor)(i)?;
            Ok((i, (Oper::Mul, mul)))
        },
        |i| {
            let (i, div) = preceded("/", factor)(i)?;
            Ok((i, (Oper::Div, div)))
        },
    )))(i)?;

    Ok((i, fold_exprs(initial, remainder)))
}

fn factor(i: &str) -> IResult<&str, Expr> {
    alt((
        delimited(multispace, digit, multispace)
            .map_res(FromStr::from_str)
            .map(Expr::Value),
        parens,
    ))(i)
}

fn parens(i: &str) -> IResult<&str, Expr> {
    delimited(
        multispace,
        delimited("(", expr.map(|e| Expr::Paren(Box::new(e))), ")"),
        multispace,
    )(i)
}

fn fold_exprs(initial: Expr, remainder: Vec<(Oper, Expr)>) -> Expr {
    remainder.into_iter().fold(initial, |acc, pair| {
        let (oper, expr) = pair;
        match oper {
            Oper::Add => Expr::Add(Box::new(acc), Box::new(expr)),
            Oper::Sub => Expr::Sub(Box::new(acc), Box::new(expr)),
            Oper::Mul => Expr::Mul(Box::new(acc), Box::new(expr)),
            Oper::Div => Expr::Div(Box::new(acc), Box::new(expr)),
        }
    })
}

#[test]
fn factor_test() {
    assert_eq!(
        factor("  3  ").map(|(i, x)| (i, format!("{:?}", x))),
        Ok(("", String::from("Value(3)")))
    );
}

#[test]
fn term_test() {
    assert_eq!(
        term(" 3 *  5   ").map(|(i, x)| (i, format!("{:?}", x))),
        Ok(("", String::from("Mul(Value(3), Value(5))")))
    );
}

#[test]
fn expr_test() {
    assert_eq!(
        expr(" 1 + 2 *  3 ").map(|(i, x)| (i, format!("{:?}", x))),
        Ok(("", String::from("Add(Value(1), Mul(Value(2), Value(3)))")))
    );
    assert_eq!(
        expr(" 1 + 2 *  3 / 4 - 5 ").map(|(i, x)| (i, format!("{:?}", x))),
        Ok((
            "",
            String::from("Sub(Add(Value(1), Div(Mul(Value(2), Value(3)), Value(4))), Value(5))")
        ))
    );
    assert_eq!(
        expr(" 72 / 2 / 3 ").map(|(i, x)| (i, format!("{:?}", x))),
        Ok(("", String::from("Div(Div(Value(72), Value(2)), Value(3))")))
    );
}

#[test]
fn parens_test() {
    assert_eq!(
        expr(" ( 1 + 2 ) *  3 ").map(|(i, x)| (i, format!("{:?}", x))),
        Ok((
            "",
            String::from("Mul(Paren(Add(Value(1), Value(2))), Value(3))")
        ))
    );
}
