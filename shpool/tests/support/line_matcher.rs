use std::{io, io::BufRead, time};

use anyhow::{anyhow, Context};
use regex::Regex;

const CMD_READ_TIMEOUT: time::Duration = time::Duration::from_secs(3);
const CMD_READ_SLEEP_DUR: time::Duration = time::Duration::from_millis(20);

pub struct LineMatcher<R: std::io::Read> {
    pub out: io::BufReader<R>,
    /// A list of regular expressions which should never match.
    pub never_match_regex: Vec<Regex>,
}

impl<R> LineMatcher<R>
where
    R: std::io::Read,
{
    /// Add a pattern to check to ensure that it never matches.
    ///
    /// NOTE: this will cause the line matcher to consume the whole
    /// output rather than stopping reading at the last match.
    pub fn never_matches(&mut self, re: &str) -> anyhow::Result<()> {
        let compiled_re = Regex::new(re)?;
        self.never_match_regex.push(compiled_re);

        Ok(())
    }

    /// Scan lines until one matches the given regex
    pub fn scan_until_re(&mut self, re: &str) -> anyhow::Result<()> {
        let compiled_re = Regex::new(re)?;
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

            self.check_persistant_assertions(&line)?;

            eprint!("scanning for /{re}/... ");
            if compiled_re.is_match(&line) {
                eprintln!(" match");
                return Ok(());
            } else {
                eprintln!(" no match");
            }
        }
    }

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

            self.check_persistant_assertions(&line)?;

            // Don't print the whole line so we don't include any control codes.
            // eprintln!("testing /{}/ against '{}'", re, &line);
            eprintln!("testing /{re}/ against line");
            return match Regex::new(re)?.captures(&line) {
                Some(caps) => Ok(caps
                    .iter()
                    .map(|maybe_match| maybe_match.map(|m| String::from(m.as_str())))
                    .collect()),
                None => Err(anyhow!("expected /{}/ to match '{}'", re, &line)),
            };
        }
    }

    /// Scan through all the remaining lines and ensure that no persistant
    /// assertions fail (the never match regex).
    pub fn drain(&mut self) -> anyhow::Result<()> {
        let start = time::Instant::now();
        loop {
            let mut line = String::new();
            match self.out.read_line(&mut line) {
                Ok(0) => {
                    return Ok(());
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

            self.check_persistant_assertions(&line)?;
        }
    }

    fn check_persistant_assertions(&self, line: &str) -> anyhow::Result<()> {
        for nomatch_re in self.never_match_regex.iter() {
            if nomatch_re.is_match(line) {
                return Err(anyhow!("expected /{}/ never to match, but it did", nomatch_re));
            }
        }

        Ok(())
    }
}

impl<R> std::ops::Drop for LineMatcher<R>
where
    R: std::io::Read,
{
    fn drop(&mut self) {
        if !self.never_match_regex.is_empty() {
            if let Err(e) = self.drain() {
                panic!("assertion failure during drain: {e:?}");
            }
        }
    }
}
