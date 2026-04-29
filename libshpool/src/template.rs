// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;

use anyhow::anyhow;

// A guess at how large the values of variables will be on average.
// This is intended as a slight over-estimate as we use it to compute
// the buffer size we should pre-allocate for instantiation.
const VAR_SIZE_GUESS: usize = 40;

/// A template is a simple variable substitution string template used
/// by the templated session name feature to allow automatic client
/// switching.
///
/// The template syntax is that variable subsitutions look like
/// `#{var_name}`, where var_name must be some alphanumeric string.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Template {
    chunks: Vec<Chunk>,
    instantiated_size_guess: usize,
}

/// A chunk is either a raw hunk of text or a variable substitution.
#[derive(Debug, Clone, Eq, PartialEq)]
enum Chunk {
    Raw(String),
    Var(String),
}

impl Template {
    pub fn new(src: &str) -> anyhow::Result<Template> {
        let mut chunks = vec![];
        let mut rest = src;

        // We could speed this up even further with the memchr
        // crate that uses SIMD, but it's not worth the dep.
        while let Some(start) = rest.find('{') {
            // Push any raw text before the '{'
            if start > 0 {
                chunks.push(Chunk::Raw(rest[..start].to_string()));
            }
            rest = &rest[start + 1..];

            // Find the closing '}'
            let end = rest.find('}').ok_or_else(|| anyhow!("unclosed var substitution"))?;
            let var = &rest[..end];

            // Validate the identifier
            let valid_ident = var.chars().next().is_some_and(|c| !c.is_numeric())
                && var.chars().all(|c| c.is_alphanumeric() || c == '_');
            if !valid_ident {
                return Err(anyhow!("invalid var name: '{}'", var));
            }

            chunks.push(Chunk::Var(var.to_string()));
            rest = &rest[end + 1..];
        }

        // Push any remaining text
        if !rest.is_empty() {
            chunks.push(Chunk::Raw(rest.to_string()));
        }

        let mut instantiated_size_guess = 0;
        for c in chunks.iter() {
            instantiated_size_guess += match c {
                Chunk::Raw(text) => text.len(),
                Chunk::Var(_) => VAR_SIZE_GUESS,
            };
        }

        Ok(Template { chunks, instantiated_size_guess })
    }

    /// Given a variable mapping, instantiate the given template.
    /// Any missing vars resolve to the empty string.
    pub fn apply(&self, vars: &HashMap<String, String>) -> String {
        let mut res = String::with_capacity(self.instantiated_size_guess);
        for c in self.chunks.iter() {
            match c {
                Chunk::Raw(text) => res.push_str(text),
                Chunk::Var(var) => res.push_str(vars.get(var).map_or("", |v| v)),
            }
        }
        res
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_ok() -> anyhow::Result<()> {
        let cases = vec![
            ("just raw", vec![Chunk::Raw(String::from("just raw"))]),
            ("just @ raw", vec![Chunk::Raw(String::from("just @ raw"))]),
            (
                "a {var} in middle",
                vec![
                    Chunk::Raw(String::from("a ")),
                    Chunk::Var(String::from("var")),
                    Chunk::Raw(String::from(" in middle")),
                ],
            ),
            ("end {var}", vec![Chunk::Raw(String::from("end ")), Chunk::Var(String::from("var"))]),
            (
                "{var} start",
                vec![Chunk::Var(String::from("var")), Chunk::Raw(String::from(" start"))],
            ),
            (
                "{var1}{var2} next to one another",
                vec![
                    Chunk::Var(String::from("var1")),
                    Chunk::Var(String::from("var2")),
                    Chunk::Raw(String::from(" next to one another")),
                ],
            ),
            (
                "{var1}blurg{var2}blag{var3}",
                vec![
                    Chunk::Var(String::from("var1")),
                    Chunk::Raw(String::from("blurg")),
                    Chunk::Var(String::from("var2")),
                    Chunk::Raw(String::from("blag")),
                    Chunk::Var(String::from("var3")),
                ],
            ),
        ];

        for (src, want) in cases.into_iter() {
            let tmpl = Template::new(src)?;
            assert_eq!(tmpl.chunks, want);
        }

        Ok(())
    }

    #[test]
    fn parse_err() -> anyhow::Result<()> {
        let cases = vec![
            ("{", "unclosed var substitution"),
            ("{$}", "invalid var name"),
            ("{.}", "invalid var name"),
            ("{1foo}", "invalid var name"),
            ("{foo-bar}", "invalid var name"),
            ("{}", "invalid var name"),
            ("{var name with space}", "invalid var name"),
        ];

        for (src, want_err) in cases.into_iter() {
            match Template::new(src) {
                Ok(_) => panic!("expected err, got none"),
                Err(e) => {
                    let err_msg = format!("{}", e);
                    if !err_msg.contains(want_err) {
                        panic!("got '{}' err, want err with '{}'", err_msg, want_err);
                    }
                }
            }
        }

        Ok(())
    }

    #[test]
    fn apply() -> anyhow::Result<()> {
        let cases = vec![
            ("{var}", vec![("other", "other")], ""),
            ("{var}", vec![("var", "val")], "val"),
            ("{var}-foo", vec![("var", "val")], "val-foo"),
            (
                "{var0}{var1}-foo",
                vec![("var0", "val0"), ("var1", "val1"), ("var", "val")],
                "val0val1-foo",
            ),
            ("{var}-{var}", vec![("var", "val")], "val-val"),
        ];

        for (src, vars, want) in cases.into_iter() {
            let tmpl = Template::new(src)?;
            let vars = vars.into_iter().map(|(k, v)| (String::from(k), String::from(v))).collect();
            let got = tmpl.apply(&vars);
            assert_eq!(got, want);
        }

        Ok(())
    }
}
