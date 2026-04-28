//! Built-in patterns for Conway's Game of Life.
//!
//! All patterns are stored in row-major order with the minimal bounding box.
//! References for the canonical layouts: LifeWiki (https://conwaylife.com/wiki/).

use crate::error::CoreError;

/// Category for a builtin pattern. Useful for grouping in UIs.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PatternCategory {
    StillLife,
    Oscillator,
    Spaceship,
    Gun,
}

impl PatternCategory {
    /// Human-readable label (English) for UI display.
    pub fn label(&self) -> &'static str {
        match self {
            PatternCategory::StillLife => "Still life",
            PatternCategory::Oscillator => "Oscillator",
            PatternCategory::Spaceship => "Spaceship",
            PatternCategory::Gun => "Gun",
        }
    }

    /// Machine-readable id (slug).
    pub fn slug(&self) -> &'static str {
        match self {
            PatternCategory::StillLife => "still-life",
            PatternCategory::Oscillator => "oscillator",
            PatternCategory::Spaceship => "spaceship",
            PatternCategory::Gun => "gun",
        }
    }
}

#[derive(Debug)]
pub struct Pattern {
    pub name: &'static str,
    pub category: PatternCategory,
    pub width: u32,
    pub height: u32,
    pub cells: &'static [u8],
}

// ---------- Oscillators ----------

const BLINKER: Pattern = Pattern {
    name: "blinker",
    category: PatternCategory::Oscillator,
    width: 3,
    height: 1,
    cells: &[1, 1, 1],
};

const TOAD: Pattern = Pattern {
    name: "toad",
    category: PatternCategory::Oscillator,
    width: 4,
    height: 2,
    cells: &[
        0, 1, 1, 1, //
        1, 1, 1, 0, //
    ],
};

const BEACON: Pattern = Pattern {
    name: "beacon",
    category: PatternCategory::Oscillator,
    width: 4,
    height: 4,
    cells: &[
        1, 1, 0, 0, //
        1, 1, 0, 0, //
        0, 0, 1, 1, //
        0, 0, 1, 1, //
    ],
};

// Pulsar: 13x13 period-3 oscillator (LifeWiki canonical form).
const PULSAR: Pattern = Pattern {
    name: "pulsar",
    category: PatternCategory::Oscillator,
    width: 13,
    height: 13,
    cells: &[
        0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, //
        1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, //
        1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, //
        0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, //
        1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, //
        1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, //
        1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, //
    ],
};

// Penta-decathlon: 3x10 period-15 oscillator.
const PENTADECATHLON: Pattern = Pattern {
    name: "pentadecathlon",
    category: PatternCategory::Oscillator,
    width: 3,
    height: 10,
    cells: &[
        0, 1, 0, //
        0, 1, 0, //
        1, 0, 1, //
        0, 1, 0, //
        0, 1, 0, //
        0, 1, 0, //
        0, 1, 0, //
        1, 0, 1, //
        0, 1, 0, //
        0, 1, 0, //
    ],
};

// ---------- Still lifes ----------

const BLOCK: Pattern = Pattern {
    name: "block",
    category: PatternCategory::StillLife,
    width: 2,
    height: 2,
    cells: &[
        1, 1, //
        1, 1, //
    ],
};

const BEEHIVE: Pattern = Pattern {
    name: "beehive",
    category: PatternCategory::StillLife,
    width: 4,
    height: 3,
    cells: &[
        0, 1, 1, 0, //
        1, 0, 0, 1, //
        0, 1, 1, 0, //
    ],
};

const LOAF: Pattern = Pattern {
    name: "loaf",
    category: PatternCategory::StillLife,
    width: 4,
    height: 4,
    cells: &[
        0, 1, 1, 0, //
        1, 0, 0, 1, //
        0, 1, 0, 1, //
        0, 0, 1, 0, //
    ],
};

// ---------- Spaceships ----------

const GLIDER: Pattern = Pattern {
    name: "glider",
    category: PatternCategory::Spaceship,
    width: 3,
    height: 3,
    cells: &[
        0, 1, 0, //
        0, 0, 1, //
        1, 1, 1, //
    ],
};

// LWSS (Light-weight spaceship): 5x4
const LWSS: Pattern = Pattern {
    name: "lwss",
    category: PatternCategory::Spaceship,
    width: 5,
    height: 4,
    cells: &[
        0, 1, 1, 1, 1, //
        1, 0, 0, 0, 1, //
        0, 0, 0, 0, 1, //
        1, 0, 0, 1, 0, //
    ],
};

// MWSS (Middle-weight spaceship): 6x5
const MWSS: Pattern = Pattern {
    name: "mwss",
    category: PatternCategory::Spaceship,
    width: 6,
    height: 5,
    cells: &[
        0, 0, 1, 0, 0, 0, //
        1, 0, 0, 0, 1, 0, //
        0, 0, 0, 0, 0, 1, //
        1, 0, 0, 0, 0, 1, //
        0, 1, 1, 1, 1, 1, //
    ],
};

// HWSS (Heavy-weight spaceship): 7x5
const HWSS: Pattern = Pattern {
    name: "hwss",
    category: PatternCategory::Spaceship,
    width: 7,
    height: 5,
    cells: &[
        0, 0, 1, 1, 0, 0, 0, //
        1, 0, 0, 0, 0, 1, 0, //
        0, 0, 0, 0, 0, 0, 1, //
        1, 0, 0, 0, 0, 0, 1, //
        0, 1, 1, 1, 1, 1, 1, //
    ],
};

// ---------- Fancy ----------

// Gosper Glider Gun: 36x9 (LifeWiki canonical form).
const GOSPER_GLIDER_GUN: Pattern = Pattern {
    name: "gosper-glider-gun",
    category: PatternCategory::Gun,
    width: 36,
    height: 9,
    cells: &[
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 1, 1, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 1, 1, 0, 0, //
        1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, //
        1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, //
    ],
};

// 13 distinct patterns are bundled (5 oscillators + 3 still lifes + 4
// spaceships + 1 fancy).
static PATTERNS: [Pattern; 13] = [
    BLINKER,
    TOAD,
    BEACON,
    PULSAR,
    PENTADECATHLON,
    BLOCK,
    BEEHIVE,
    LOAF,
    GLIDER,
    LWSS,
    MWSS,
    HWSS,
    GOSPER_GLIDER_GUN,
];

pub fn all_builtins() -> &'static [Pattern] {
    &PATTERNS
}

pub fn builtin(name: &str) -> Option<&'static Pattern> {
    PATTERNS.iter().find(|p| p.name == name)
}

/// Like [`builtin`] but returns [`CoreError::UnknownPattern`] when the name is
/// not bundled. Convenient for callers (e.g. wasm bindings) that need a
/// `Result`-shaped API.
pub fn builtin_or_err(name: &str) -> Result<&'static Pattern, CoreError> {
    builtin(name).ok_or_else(|| CoreError::UnknownPattern(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_categories_are_assigned() {
        assert_eq!(builtin("block").unwrap().category, PatternCategory::StillLife);
        assert_eq!(builtin("blinker").unwrap().category, PatternCategory::Oscillator);
        assert_eq!(builtin("glider").unwrap().category, PatternCategory::Spaceship);
        assert_eq!(
            builtin("gosper-glider-gun").unwrap().category,
            PatternCategory::Gun
        );
    }

    #[test]
    fn pattern_category_slug_and_label() {
        assert_eq!(PatternCategory::StillLife.slug(), "still-life");
        assert_eq!(PatternCategory::Oscillator.slug(), "oscillator");
        assert_eq!(PatternCategory::Spaceship.slug(), "spaceship");
        assert_eq!(PatternCategory::Gun.slug(), "gun");

        assert_eq!(PatternCategory::StillLife.label(), "Still life");
        assert_eq!(PatternCategory::Oscillator.label(), "Oscillator");
        assert_eq!(PatternCategory::Spaceship.label(), "Spaceship");
        assert_eq!(PatternCategory::Gun.label(), "Gun");
    }

    #[test]
    fn all_builtins_have_expected_count_per_category() {
        let pats = all_builtins();
        let count = |c: PatternCategory| pats.iter().filter(|p| p.category == c).count();
        assert_eq!(count(PatternCategory::StillLife), 3);
        assert_eq!(count(PatternCategory::Oscillator), 5);
        assert_eq!(count(PatternCategory::Spaceship), 4);
        assert_eq!(count(PatternCategory::Gun), 1);
    }
}
