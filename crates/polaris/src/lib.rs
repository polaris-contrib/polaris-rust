#[macro_use]
mod cfg;
pub use polaris_core as core;
#[doc(no_inline)]
pub use polaris_core::*;

/// A list of things that automatically imports into application use polaris.
pub mod prelude {
    pub use polaris_core::prelude::*;
}
