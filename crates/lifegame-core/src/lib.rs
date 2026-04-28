//! Core engine for Conway's Game of Life.
//!
//! Pure Rust, no DOM or wasm dependencies.

mod error;
mod grid;
mod patterns;
mod rules;
mod simulation;

pub use error::CoreError;
pub use grid::{Boundary, Cell, Grid};
pub use patterns::{all_builtins, builtin, builtin_or_err, Pattern, PatternCategory};
pub use rules::next_generation;
pub use simulation::Simulation;
