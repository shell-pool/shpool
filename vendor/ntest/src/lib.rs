//! The ntest lib enhances the rust test framework with some useful functions.

// Reexport procedural macros
extern crate ntest_test_cases;
extern crate ntest_timeout;

#[doc(inline)]
pub use ntest_test_cases::test_case;

#[doc(inline)]
pub use ntest_timeout::timeout;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// Reexport traits
mod traits;
#[doc(inline)]
pub use crate::traits::MaxDifference;

#[doc(hidden)]
/// Timeout helper for proc macro timeout
pub fn execute_with_timeout<T: Send>(
    code: &'static (dyn Fn() -> T + Sync + 'static),
    timeout_ms: u64,
) -> Option<T> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || if let Ok(()) = sender.send(code()) {});
    match receiver.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(t) => Some(t),
        Err(_) => None,
    }
}

#[doc(hidden)]
/// Difference helper for proc macro about equal
pub fn about_eq<T: MaxDifference>(a: T, b: T, eps: f64) -> bool {
    a.max_diff(b) < eps
}

/// Compare floating point values or vectors of floating points wether they are approximately equal.
/// The default value for epsilon is `1.0e-6`.
///
/// # Examples
///
/// Compare two floating point values which are about equal:
/// ```
/// # use ntest::assert_about_eq;
/// # fn main() {
/// assert_about_eq!(42.00000001f32, 42.0f32);
/// # }
/// ```
///
/// Explicitly set an epsilon value. This test should fail:
/// ``` should_panic
/// # use ntest::assert_about_eq;
/// # fn main() {
/// assert_about_eq!(42.001f32, 42.0f32, 1.0e-4);
/// # }
/// ```
///
/// Compare two vectors or arrays of floats which are about equal:
/// ```
/// # use ntest::assert_about_eq;
/// # fn main() {
/// assert_about_eq!(vec![1.100000001, 2.1], vec![1.1, 2.1], 0.001f64);
/// # }
/// ```
///
/// Arrays can be compared to a length of up to `32`. See the [MaxDifference](trait.MaxDifference.html) implementation for more details:
/// ```
/// # use ntest::assert_about_eq;
/// # fn main() {
///# // Test double usage
///# assert_about_eq!([1.100000001, 2.1], [1.1, 2.1], 0.001f64);
/// assert_about_eq!([1.100000001, 2.1], [1.1, 2.1], 0.001f64);
/// # }
/// ```
#[macro_export]
macro_rules! assert_about_eq {
    ($a:expr, $b:expr, $eps:expr) => {
        let eps = $eps;
        assert!(
            $crate::about_eq($a, $b, eps),
            "assertion failed: `(left !== right)` \
             (left: `{:?}`, right: `{:?}`, epsilon: `{:?}`)",
            $a,
            $b,
            eps
        );
    };
    ($a:expr, $b:expr,$eps:expr,) => {
        assert_about_eq!($a, $b, $eps);
    };
    ($a:expr, $b:expr) => {
        assert_about_eq!($a, $b, 1.0e-6);
    };
    ($a:expr, $b:expr,) => {
        assert_about_eq!($a, $b, 1.0e-6);
    };
}

/// Expects a true expression. Otherwise panics.
///
/// Is an alias for the [assert! macro](https://doc.rust-lang.org/std/macro.assert.html).
///
/// # Examples
///
/// This call won't panic.
/// ```rust
/// # use ntest::assert_true;
/// # fn main() {
/// assert_true!(true);
/// # }
///```
///
/// This call will panic.
/// ```should_panic
/// # use ntest::assert_true;
/// # fn main() {
/// assert_true!(false);
/// # }
/// ```
#[macro_export]
macro_rules! assert_true {
    ($x:expr) => {
        if !$x {
            panic!("assertion failed: Expected 'true', but was 'false'");
        }
    };
    ($x:expr,) => {
        assert_true!($x);
    };
}

/// Expects a false expression. Otherwise panics.
///
/// # Examples
///
/// This call won't panic.
/// ```rust
/// # use ntest::assert_false;
/// # fn main() {
/// assert_false!(false);
/// # }
/// ```
///
/// This call will panic.
/// ```should_panic
/// # use ntest::assert_false;
/// # fn main() {
/// assert_false!(true);
/// # }
/// ```
#[macro_export]
macro_rules! assert_false {
    ($x:expr) => {{
        if $x {
            panic!("assertion failed: Expected 'false', but was 'true'");
        }
    }};
    ($x:expr,) => {{
        assert_false!($x);
    }};
}

/// A panic in Rust is not always implemented via unwinding, but can be implemented by aborting the
/// process as well. This function only catches unwinding panics, not those that abort the process.
/// See the catch unwind [documentation](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)
/// for more information.
///
/// # Examples
///
/// This call won't panic.
/// ```rust
/// # use ntest::assert_panics;
/// # fn main() {
/// // Other panics can happen before this call.
/// assert_panics!({panic!("I am panicing")});
/// # }
/// ```
///
/// This call will panic.
/// ```should_panic
/// # use ntest::assert_panics;
/// # fn main() {
/// assert_panics!({println!("I am not panicing")});
/// # }
/// ```
#[macro_export]
macro_rules! assert_panics {
    ($x:block) => {{
        let result = std::panic::catch_unwind(|| $x);
        if !result.is_err() {
            panic!("assertion failed: code in block did not panic");
        }
    }};
    ($x:block,) => {{
        assert_panics!($x);
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assert_true() {
        assert_true!(true);
    }
    #[test]
    #[should_panic]
    fn assert_true_fails() {
        assert_true!(false);
    }
    #[test]
    fn assert_true_trailing_comma() {
        assert_true!(true,);
    }
    #[test]
    fn assert_false() {
        assert_false!(false);
    }
    #[test]
    #[should_panic]
    fn assert_false_fails() {
        assert_false!(true);
    }
    #[test]
    fn assert_false_trailing_comma() {
        assert_false!(false,);
    }
    #[test]
    fn assert_panics() {
        assert_panics!({ panic!("I am panicing!") },);
    }
    #[test]
    #[should_panic]
    fn assert_panics_fails() {
        assert_panics!({ println!("I am not panicing!") },);
    }
    #[test]
    fn assert_panics_trailing_comma() {
        assert_panics!({ panic!("I am panicing!") },);
    }

    #[test]
    fn vector() {
        assert_about_eq!(vec![1.1, 2.1], vec![1.1, 2.1]);
    }

    #[test]
    #[should_panic]
    fn vector_fails() {
        assert_about_eq!(vec![1.2, 2.1], vec![1.1, 2.1]);
    }

    #[test]
    fn vector_trailing_comma() {
        assert_about_eq!(vec![1.2, 2.1], vec![1.2, 2.1],);
    }

    #[test]
    fn vector_trailing_comma_with_epsilon() {
        assert_about_eq!(vec![1.100000001, 2.1], vec![1.1, 2.1], 0.001f64,);
    }

    #[test]
    fn it_should_not_panic_if_values_are_approx_equal() {
        assert_about_eq!(64f32.sqrt(), 8f32);
    }

    #[test]
    fn about_equal_f32() {
        assert_about_eq!(3f32, 3f32, 1f64);
    }

    #[test]
    fn about_equal_f64() {
        assert_about_eq!(3f64, 3f64);
    }

    #[test]
    fn compare_with_epsilon() {
        assert_about_eq!(42f64, 43f64, 2f64);
    }

    #[test]
    #[should_panic]
    fn fail_with_epsilon() {
        assert_about_eq!(3f64, 4f64, 1e-8f64);
    }
}
