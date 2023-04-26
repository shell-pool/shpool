/// Helper trait for `assert_about_equal` macro. Returns the max difference between
/// two vectors of floats. Can also be used for single floats.  
///
/// # Examples
///
/// Compare two floating numbers:
/// ```
/// # use ntest::MaxDifference;
/// # fn main() {
/// assert!((0.1f64 - 42.1f32.max_diff(42.0f32)) < 1.0e-4f64);
/// # }
/// ```
///
/// Compare two vectors. Returns the maximum difference in the vectors. In this case *~0.1*.:
/// ```
/// # use ntest::MaxDifference;
/// # fn main() {
/// assert!(0.1f64 - vec![42.0, 42.0f32, 1.001f32].max_diff(vec![42.0, 42.1f32, 1.0f32]) < 1.0e-4f64);
/// # }
/// ```
/// Compare two arrays. Trait implemented for arrays of length `0-32`:
/// ```
/// # use ntest::MaxDifference;
/// # fn main() {
/// assert!(0.1f64 - [42.0, 42.0f32, 1.001f32].max_diff([42.0, 42.1f32, 1.0f32]) < 1.0e-4f64);
/// # }
/// ```
pub trait MaxDifference {
    fn max_diff(self, other: Self) -> f64;
}

impl MaxDifference for f32 {
    fn max_diff(self, other: Self) -> f64 {
        f64::from((self - other).abs())
    }
}

impl MaxDifference for f64 {
    fn max_diff(self, other: Self) -> f64 {
        (self - other).abs()
    }
}

impl MaxDifference for Vec<f32> {
    fn max_diff(self, other: Self) -> f64 {
        let mut max: f64 = 0.0;
        for (a, b) in self.iter().zip(other.iter()) {
            let diff = f64::from((*a - *b).abs());
            if diff > max {
                max = diff;
            }
        }
        max
    }
}

impl MaxDifference for Vec<f64> {
    fn max_diff(self, other: Self) -> f64 {
        let mut max: f64 = 0.0;
        for (a, b) in self.iter().zip(other.iter()) {
            let diff = (*a - *b).abs();
            if diff > max {
                max = diff;
            }
        }
        max
    }
}

macro_rules! array_impls {
    ($($N:literal)+) => {
        $(
            impl MaxDifference for [f64; $N] {
                fn max_diff(self, other: Self) -> f64 {
                    let mut max: f64 = 0.0;
                    for (a, b) in self.iter().zip(other.iter()) {
                        let diff = (*a - *b).abs();
                        if diff > max {
                            max = diff;
                        }
                    }
                    max
                }
            }
            impl MaxDifference for [f32; $N] {
                fn max_diff(self, other: Self) -> f64 {
                    let mut max: f64 = 0.0;
                    for (a, b) in self.iter().zip(other.iter()) {
                        let diff = f64::from((*a - *b).abs());
                        if diff > max {
                            max = diff;
                        }
                    }
                    max
                }
            }
        )+
    }
}

array_impls! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}
