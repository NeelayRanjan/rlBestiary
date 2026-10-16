//! Generational physics training of one body for one task, in the manner of
//! Keiwan's Evolution. Deterministic and free of rendering, UI and async.
mod engine;
mod settings;
pub use engine::*;
pub use settings::*;
