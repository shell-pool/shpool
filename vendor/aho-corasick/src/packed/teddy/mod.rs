#[cfg(not(all(feature = "std", target_arch = "x86_64")))]
pub use crate::packed::teddy::fallback::{Builder, Teddy};
#[cfg(all(feature = "std", target_arch = "x86_64"))]
pub use crate::packed::teddy::{compile::Builder, runtime::Teddy};

#[cfg(all(feature = "std", target_arch = "x86_64"))]
mod compile;
#[cfg(all(feature = "std", target_arch = "x86_64"))]
mod runtime;

#[cfg(not(all(feature = "std", target_arch = "x86_64")))]
mod fallback {
    use crate::{packed::pattern::Patterns, Match};

    #[derive(Clone, Debug, Default)]
    pub struct Builder(());

    impl Builder {
        pub fn new() -> Builder {
            Builder(())
        }

        pub fn build(&self, _: &Patterns) -> Option<Teddy> {
            None
        }

        pub fn fat(&mut self, _: Option<bool>) -> &mut Builder {
            self
        }

        pub fn avx(&mut self, _: Option<bool>) -> &mut Builder {
            self
        }
    }

    #[derive(Clone, Debug)]
    pub struct Teddy(());

    impl Teddy {
        pub fn find_at(
            &self,
            _: &Patterns,
            _: &[u8],
            _: usize,
        ) -> Option<Match> {
            None
        }

        pub fn minimum_len(&self) -> usize {
            0
        }

        pub fn memory_usage(&self) -> usize {
            0
        }
    }
}
