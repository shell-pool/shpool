# NTest  TestCases

Part of the [NTest library](https://crates.io/crates/ntest). Add test cases to the rust test framework using 
[procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html).

## Examples

Example with a single argument:

```rust
#[test_case(13)]
#[test_case(42)]
fn one_arg(x: u32) {
    assert!(x == 13 || x == 42)
}
```

The test cases above will be parsed at compile time and two rust test functions will be generated instead:

```rust
#[test]
fn one_arg_13() {
    x = 13;
    assert!(x == 13 || x == 42)
}

#[test]
fn one_arg_42() {
    x = 42;
    assert!(x == 13 || x == 42)
}
```

For more examples and information read the [documentation](https://docs.rs/ntest_test_cases/).
