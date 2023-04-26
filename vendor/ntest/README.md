# NTest

[![docs](https://docs.rs/ntest/badge.svg)](https://docs.rs/ntest)
[![crates](https://img.shields.io/badge/crates.io-ntest-orange)](https://crates.io/crates/ntest)
[![downloads](https://badgen.net/crates/d/ntest)](https://crates.io/crates/ntest)
[![build status](https://github.com/becheran/ntest/actions/workflows/ci.yml/badge.svg)](https://github.com/becheran/ntest/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Testing framework for rust which enhances the built-in library with some useful features. Inspired by the *.Net* unit-testing framework [NUnit](https://github.com/nunit/nunit).

## Getting Started

Some functions of *NTest* use [procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html) which are stable for rust edition 2018.
If you use the library make sure that you are using the *2018 version* of rust. Update the *Cargo.toml* file:

```toml
[package]
edition = "2018"
# ..
```

Add the *NTest library* to your developer dependencies in the *Cargo.toml* file:

```toml
[dev-dependencies]
ntest = "*"
```

## Content

- `#[timeout()]` Attribute used for timeouts in tests.
- `#[test_case()]` Attribute used to define multiple test cases for a test function.
- `assert_about_equal!()` Compare two floating point values or vectors for equality.
- `assert_false!()` Expects false argument for test case.
- `assert_true!()` Expects true argument for test case.
- `assert_panics!()` Expects block to panic. Otherwise the test fails.

For more information read the [documentation](https://docs.rs/ntest/).

## Examples

### Create test cases

```rust
use ntest::test_case;

#[test_case("https://doc.rust-lang.org.html")]
#[test_case("http://www.website.php", name="important_test")]
fn test_http_link_types(link: &str) {
    test_link(link, &LinkType::HTTP);
}
```

### Timeout for long running functions

```rust
use ntest::timeout;

#[test]
#[timeout(10)]
#[should_panic]
fn timeout() {
    loop {};
}
```

### Combine attributes

```rust
use std::{thread, time};
use ntest::timeout;
use ntest::test_case;

#[test_case(200)]
#[timeout(100)]
#[should_panic]
#[test_case(10)]
#[timeout(100)]
fn test_function(i : u32) {
    let sleep_time = time::Duration::from_millis(i);
    thread::sleep(sleep_time);
}
```
