//! Error types for the core engine.

// Note: `Eq` is not derivable because `InvalidDensity(f32)` makes the
// enum hold a non-`Eq` value. We expose `PartialEq` only.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum CoreError {
    #[error("invalid dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },
    #[error("pattern '{name}' does not fit at offset ({ox}, {oy}) on {gw}x{gh} grid")]
    PatternOutOfBounds {
        name: String,
        ox: i32,
        oy: i32,
        gw: u32,
        gh: u32,
    },
    #[error("unknown pattern: {0}")]
    UnknownPattern(String),
    #[error("invalid density: {0} (must be in [0.0, 1.0])")]
    InvalidDensity(f32),
}
