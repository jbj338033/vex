mod detect;
mod dockerfile;

pub use detect::{ProjectType, detect};
pub use dockerfile::generate;
