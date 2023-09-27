// Copyright 2023 Google LLC
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

/*! A parser for the duration format supported by the
  attach --ttl flag.
*/

use anyhow::{anyhow, bail, Context};
use std::time;

pub fn parse(src: &str) -> anyhow::Result<time::Duration> {
    if src.contains(":") {
        parse_colon_duration(src)
    } else if src.chars().last().map(|c| c.is_alphabetic()).unwrap_or(false) {
        parse_suffix_duration(src)
    } else {
        bail!("could not parse '{}' as duration", src);
    }
}

/// Parses dd:hh:mm:ss or any suffix
fn parse_colon_duration(src: &str) -> anyhow::Result<time::Duration> {
    let mut parts = src.split(":").collect::<Vec<_>>();
    parts.reverse();
    if parts.len() == 0 {
        bail!("'{}' must have at least one part", src);
    }
    let mut secs = parts[0].parse::<u64>().context("parsing seconds part")?;
    dbg!(secs);
    if parts.len() == 1 {
        return Ok(time::Duration::from_secs(secs));
    }
    secs += parts[1].parse::<u64>().context("parsing minutes part")? * 60;
    dbg!(secs);
    if parts.len() == 2 {
        return Ok(time::Duration::from_secs(secs));
    }
    secs += parts[2].parse::<u64>().context("parsing hours part")? * 60 * 60;
    dbg!(secs);
    if parts.len() == 3 {
        return Ok(time::Duration::from_secs(secs));
    }
    secs += parts[3].parse::<u64>().context("parsing days part")? * 60 * 60 * 24;
    dbg!(secs);
    if parts.len() != 4 {
        bail!("colon duration cannot have more than 4 parts");
    }

    Ok(time::Duration::from_secs(secs))
}

/// Parses 20d, 3h, 14m ect
fn parse_suffix_duration(src: &str) -> anyhow::Result<time::Duration> {
    let num: String = src.chars().take_while(|c| c.is_numeric()).collect();
    let c = src.chars().last().ok_or(anyhow!("internal error: no suffix"))?;
    make_suffix_duration(num.parse::<u64>().context("parsing num part of duration")?, c)
        .ok_or(anyhow!("unknown time unit '{}'", c))
}

fn make_suffix_duration(n: u64, c: char) -> Option<time::Duration> {
    match c {
        's' => Some(time::Duration::from_secs(n)),
        'm' => Some(time::Duration::from_secs(n * 60)),
        'h' => Some(time::Duration::from_secs(n * 60 * 60)),
        'd' => Some(time::Duration::from_secs(n * 60 * 60 * 24)),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn successes() {
        let cases = vec![
            ("10:30", time::Duration::from_secs(10 * 60 + 30)),
            ("3:10:30", time::Duration::from_secs(3 * 60 * 60 + 10 * 60 + 30)),
            ("1:3:10:30", time::Duration::from_secs(60 * 60 * 24 + 3 * 60 * 60 + 10 * 60 + 30)),
            ("5s", time::Duration::from_secs(5)),
            ("5m", time::Duration::from_secs(5 * 60)),
            ("5h", time::Duration::from_secs(5 * 60 * 60)),
            ("5d", time::Duration::from_secs(5 * 60 * 60 * 24)),
        ];

        for (src, dur) in cases.into_iter() {
            match parse(src) {
                Ok(parsed_dur) => {
                    assert_eq!(dur, parsed_dur);
                }
                Err(e) => {
                    assert_eq!("", e.to_string());
                }
            }
        }
    }

    #[test]
    fn errors() {
        let cases = vec![
            ("12", "could not parse"),
            ("12x", "unknown time unit"),
            (":1", "parsing minutes part"),
            ("1:1:1:1:1", "cannot have more than 4"),
        ];

        for (src, err_substring) in cases.into_iter() {
            if let Err(e) = parse(src) {
                eprintln!("ERR: {}", e.to_string());
                eprintln!("err_substring: {}", err_substring);
                assert!(e.to_string().contains(err_substring));
            } else {
                assert_eq!("", "expected err, but got none");
            }
        }
    }
}
