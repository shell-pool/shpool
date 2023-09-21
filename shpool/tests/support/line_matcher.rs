use std::{io, io::BufRead, time};

use anyhow::{anyhow, Context};
use regex::Regex;

const CMD_READ_TIMEOUT: time::Duration = time::Duration::from_secs(3);
const CMD_READ_SLEEP_DUR: time::Duration = time::Duration::from_millis(20);

pub struct LineMatcher<R> {
    pub out: io::BufReader<R>,
}

impl<R> LineMatcher<R>
where
    R: std::io::Read,
{
    pub fn match_re(&mut self, re: &str) -> anyhow::Result<()> {
        match self.capture_re(re) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn capture_re(&mut self, re: &str) -> anyhow::Result<Vec<Option<String>>> {
        let start = time::Instant::now();
        loop {
            let mut line = String::new();
            match self.out.read_line(&mut line) {
                Ok(0) => {
                    return Err(anyhow!("LineMatcher: EOF"));
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        if start.elapsed() > CMD_READ_TIMEOUT {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "timed out reading line",
                            ))?;
                        }

                        std::thread::sleep(CMD_READ_SLEEP_DUR);
                        continue;
                    }

                    return Err(e).context("reading line from shell output")?;
                }
                Ok(_) => {
                    if line.ends_with('\n') {
                        line.pop();
                        if line.ends_with('\r') {
                            line.pop();
                        }
                    }
                }
            }

            // Don't print the whole line so we don't include any control codes.
            // eprintln!("testing /{}/ against '{}'", re, &line);
            eprintln!("testing /{}/ against line", re);
            return match Regex::new(re)?.captures(&line) {
                Some(caps) => Ok(caps
                    .iter()
                    .map(|maybe_match| maybe_match.map(|m| String::from(m.as_str())))
                    .collect()),
                None => Err(anyhow!("expected /{}/ to match '{}'", re, &line)),
            };
        }
    }
}
