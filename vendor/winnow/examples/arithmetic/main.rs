use winnow::prelude::*;

mod parser;
mod parser_ast;

fn main() -> Result<(), lexopt::Error> {
    let args = Args::parse()?;

    let input = args.input.as_deref().unwrap_or("1 + 1");

    println!("{} =", input);
    match args.implementation {
        Impl::Eval => match parser::expr.parse(input) {
            Ok(result) => {
                println!("  {}", result);
            }
            Err(err) => {
                println!("  {}", err);
            }
        },
        Impl::Ast => match parser_ast::expr.parse(input) {
            Ok(result) => {
                println!("  {:#?}", result);
            }
            Err(err) => {
                println!("  {}", err);
            }
        },
    }

    Ok(())
}

#[derive(Default)]
struct Args {
    input: Option<String>,
    implementation: Impl,
}

enum Impl {
    Eval,
    Ast,
}

impl Default for Impl {
    fn default() -> Self {
        Self::Eval
    }
}

impl Args {
    fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut res = Args::default();

        let mut args = lexopt::Parser::from_env();
        while let Some(arg) = args.next()? {
            match arg {
                Long("impl") => {
                    res.implementation = args.value()?.parse_with(|s| match s {
                        "eval" => Ok(Impl::Eval),
                        "ast" => Ok(Impl::Ast),
                        _ => Err("expected `eval`, `ast`"),
                    })?;
                }
                Value(input) => {
                    res.input = Some(input.string()?);
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(res)
    }
}
