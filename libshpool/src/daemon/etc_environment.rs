use std::io::{BufRead, BufReader, Read};

use tracing::{debug, warn};

// The syntax for /etc/environment is ill defined. The
// file is parsed by pam_env, so this is an attempt at
// porting the parsing logic from that
// https://github.com/linux-pam/linux-pam/blob/1fbf123d982b90d41463df7b6b59a4e544263358/modules/pam_env/pam_env.c#L906
//
// N.B. The logic from pam_env is horrifically, traumatically broken. We
// have to be bug compatible, but please, for the love of god, never
// write anything like this if you have a choice.
pub fn parse_compat<R: Read>(file: R) -> anyhow::Result<Vec<(String, String)>> {
    let mut pairs = vec![];
    let mut etc_env = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        match etc_env.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let mut line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    debug!("parsing /etc/environment: blank or comment line");
                    continue;
                }

                // This requires exactly one space after the
                // export, or it doesn't count. Bonkers. Absolutely
                // bonkers.
                line = line.strip_prefix("export ").unwrap_or(line);

                // Scan through the line looking for a # that starts a
                // trailing comment. What if the # is in the middle of
                // quotes? Lol, who cares about edge cases, certainly not
                // us!
                let line: String = line.chars().take_while(|c| *c != '#').collect();

                let parts: Vec<_> = line.splitn(2, "=").collect();
                if parts.len() != 2 {
                    warn!("parsing /etc/environment: split failed (should be impossible)");
                    continue;
                }
                let (key, mut val) = (parts[0], parts[1]);
                if key.is_empty() {
                    warn!("parsing /etc/environment: empty key");
                    continue;
                }
                if !key.chars().all(char::is_alphanumeric) {
                    warn!("parsing /etc/environment: non alphanum key");
                    continue;
                }

                // Strip quotes. Yes, you're reading it right, this will match
                // single quotes with double quotes and strip unmatched leading
                // quotes while doing nothing for unmatched trailing quotes.
                let has_leading_quote = val.starts_with('\'') || val.starts_with('"');
                val = val.strip_prefix("'").unwrap_or(val);
                val = val.strip_prefix("\"").unwrap_or(val);
                if has_leading_quote {
                    val = val.strip_suffix("'").unwrap_or(val);
                    val = val.strip_suffix("\"").unwrap_or(val);
                }
                pairs.push((String::from(key), String::from(val)));
            }
            Err(e) => return Err(e)?,
        }
    }

    Ok(pairs)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_file() -> anyhow::Result<()> {
        let pairs = parse_compat(std::io::Cursor::new(
            r#"
BASIC=foo
    LEADINGWS=foo
QUOTEDCOMMENT='surely a # in the middle of a quoted value won't count as a comment'
LEADINGUNTERM='wut is going on
TRAILINGUNTERM=wut is going on'
export EXPORTED1SPACE=foo
export  EXPORTED2SPACE=foo
MISMATCHQUOTE='wut is going on"
DOUBLEEQUALS=foo=bar
DOUBLEEQUALSQUOTE='foo=bar'
        "#,
        ))?;
        assert_eq!(
            pairs,
            vec![
                (String::from("BASIC"), String::from("foo")),
                (String::from("LEADINGWS"), String::from("foo")),
                (String::from("QUOTEDCOMMENT"), String::from("surely a ")),
                (String::from("LEADINGUNTERM"), String::from("wut is going on")),
                (String::from("TRAILINGUNTERM"), String::from("wut is going on'")),
                (String::from("EXPORTED1SPACE"), String::from("foo")),
                (String::from("MISMATCHQUOTE"), String::from("wut is going on")),
                (String::from("DOUBLEEQUALS"), String::from("foo=bar")),
                (String::from("DOUBLEEQUALSQUOTE"), String::from("foo=bar")),
            ]
        );

        Ok(())
    }
}
