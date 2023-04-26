extern crate ntest_test_cases;
use ntest_test_cases::test_case;

#[test_case(42)]
fn one_arg(x: u32) {
    assert_eq!(x, 42)
}

#[test_case(1, 42)]
#[test_case(9, 18)]
#[test_case(5, 20)]
fn two_args(x: u8, y: u32) {
    assert!(x < 10);
    assert!(y > 10);
}

#[test_case(42.42)]
fn float(x: f64) {
    assert_eq!(x, 42.42)
}

#[test_case("walter", "white")]
fn test_string(x: &str, y: &str) {
    assert_eq!(x, "walter");
    assert_eq!(y, "white");
}


#[test_case("-390)(#$*Q)")]
fn test_string_special_chars(x: &str) {
    assert_eq!(x, "-390)(#$*Q)");
}

#[test_case(true)]
fn test_bool(x: bool) {
    assert!(x);
}

#[test_case(true, "true", 1)]
fn test_mix(x: bool, y: &str, z: u16) {
    assert!(x);
    assert_eq!(y, "true");
    assert_eq!(z, 1);
}

#[test_case(42, name="my_fancy_test")]
fn with_name(x: u32) {
    assert_eq!(x, 42)
}


#[test_case(42, name="my_snd_fancy_testSPECIALCHARS^$(*")]
fn with_name(x: u32) {
    assert_eq!(x, 42)
}

#[test_case(18)]
#[ignore]
#[test_case(15)]
#[should_panic(expected = "I am panicing")]
fn attributes_test_case(x: u32) {
    panic!("I am panicing {}", x);
}

#[test_case(42)]
fn return_result(x: u32) -> core::result::Result<(), ()> {
    assert_eq!(x, 42);
    Ok(())
}
