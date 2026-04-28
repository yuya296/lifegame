//! Simulation: front/back grid pair plus history (undo).

use std::collections::VecDeque;

use rand::rngs::SmallRng;
use rand::SeedableRng;

use crate::error::CoreError;
use crate::grid::{Boundary, Cell, Grid};
use crate::patterns::Pattern;
use crate::rules::next_generation;

#[derive(Clone, Debug)]
struct Snapshot {
    width: u32,
    height: u32,
    cells: Vec<u8>,
    generation: u64,
}

#[derive(Clone, Debug)]
pub struct Simulation {
    front: Grid,
    back: Grid,
    boundary: Boundary,
    generation: u64,
    history: VecDeque<Snapshot>,
    history_capacity: usize,
}

impl Simulation {
    pub const DEFAULT_HISTORY_CAPACITY: usize = 64;

    pub fn new(width: u32, height: u32, boundary: Boundary) -> Result<Self, CoreError> {
        let front = Grid::new(width, height)?;
        let back = Grid::new(width, height)?;
        Ok(Self {
            front,
            back,
            boundary,
            generation: 0,
            history: VecDeque::new(),
            history_capacity: Self::DEFAULT_HISTORY_CAPACITY,
        })
    }

    pub fn width(&self) -> u32 {
        self.front.width()
    }

    pub fn height(&self) -> u32 {
        self.front.height()
    }

    pub fn boundary(&self) -> Boundary {
        self.boundary
    }

    pub fn set_boundary(&mut self, b: Boundary) {
        self.boundary = b;
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn cells(&self) -> &[u8] {
        self.front.cells()
    }

    pub fn count_alive(&self) -> u32 {
        self.front.count_alive()
    }

    pub fn step(&mut self) {
        self.push_history();
        next_generation(&self.front, &mut self.back, self.boundary);
        std::mem::swap(&mut self.front, &mut self.back);
        self.generation += 1;
    }

    #[must_use = "step_back returns false when no history is available"]
    pub fn step_back(&mut self) -> bool {
        let Some(snap) = self.history.pop_back() else {
            return false;
        };
        // Restore front from snapshot. Resize front if dimensions changed.
        if snap.width != self.front.width() || snap.height != self.front.height() {
            self.front = Grid::new(snap.width, snap.height).expect("snapshot dims must be valid");
            self.back = Grid::new(snap.width, snap.height).expect("snapshot dims must be valid");
        }
        self.front.cells_mut().copy_from_slice(&snap.cells);
        self.generation = snap.generation;
        true
    }

    pub fn set_cell(&mut self, x: i32, y: i32, c: Cell) {
        // Out-of-bounds writes are silently ignored by `Grid::set`, so we must
        // mirror that early-return here too — otherwise we'd waste a history
        // slot on a no-op.
        if !self.in_bounds(x, y) {
            return;
        }
        self.push_history();
        self.front.set(x, y, c);
    }

    pub fn toggle_cell(&mut self, x: i32, y: i32) {
        if !self.in_bounds(x, y) {
            return;
        }
        self.push_history();
        self.front.toggle(x, y);
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.front.width() && (y as u32) < self.front.height()
    }

    pub fn clear(&mut self) {
        self.push_history();
        self.front.clear();
    }

    pub fn randomize(&mut self, density: f32, seed: Option<u64>) -> Result<(), CoreError> {
        if !(0.0..=1.0).contains(&density) || density.is_nan() {
            return Err(CoreError::InvalidDensity(density));
        }
        self.push_history();
        let mut rng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => SmallRng::from_entropy(),
        };
        self.front.fill_random(density, &mut rng)?;
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), CoreError> {
        if width == 0 || height == 0 {
            return Err(CoreError::InvalidDimensions { width, height });
        }
        // Allocate new front/back FIRST so that any failure (e.g. dimensions
        // exceeding `i32::MAX` or causing `usize` overflow) bubbles up before
        // we touch undo history. Otherwise an error would silently consume a
        // history slot even though the simulation state is unchanged.
        let mut new_front = Grid::new(width, height)?;
        let new_back = Grid::new(width, height)?;
        let copy_w = self.front.width().min(width);
        let copy_h = self.front.height().min(height);
        let old_w = self.front.width();
        let old_cells = self.front.cells();
        for y in 0..copy_h {
            for x in 0..copy_w {
                let v = old_cells[(y * old_w + x) as usize];
                new_front.cells_mut()[(y * width + x) as usize] = v;
            }
        }
        // Success is now certain — commit history then swap in the new grids.
        self.push_history();
        self.front = new_front;
        self.back = new_back;
        Ok(())
    }

    pub fn place_pattern(
        &mut self,
        pattern: &Pattern,
        ox: i32,
        oy: i32,
    ) -> Result<(), CoreError> {
        // Try the placement on a clone first so that a failure (e.g. extreme
        // offsets that overflow `i32` inside `Grid::place_pattern`) does NOT
        // consume an undo-history slot. Cloning the front grid is fine here
        // because `place_pattern` is not on the hot simulation path.
        let mut tentative = self.front.clone();
        tentative.place_pattern(pattern, ox, oy, self.boundary)?;
        // Success: only now do we push history and commit the new front.
        self.push_history();
        self.front = tentative;
        Ok(())
    }

    pub fn history_capacity(&self) -> usize {
        self.history_capacity
    }

    pub fn set_history_capacity(&mut self, cap: usize) {
        self.history_capacity = cap;
        if cap == 0 {
            self.history.clear();
        } else {
            while self.history.len() > cap {
                self.history.pop_front();
            }
        }
    }

    fn push_history(&mut self) {
        if self.history_capacity == 0 {
            return;
        }
        let snap = Snapshot {
            width: self.front.width(),
            height: self.front.height(),
            cells: self.front.cells().to_vec(),
            generation: self.generation,
        };
        self.history.push_back(snap);
        while self.history.len() > self.history_capacity {
            self.history.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{all_builtins, builtin, builtin_or_err};

    fn rows_to_cells(rows: &[&str]) -> (u32, u32, Vec<u8>) {
        let h = rows.len() as u32;
        let w = rows[0].len() as u32;
        let mut cells = Vec::with_capacity((w * h) as usize);
        for r in rows {
            assert_eq!(r.len() as u32, w);
            for ch in r.chars() {
                cells.push(if ch == '#' || ch == '1' { 1 } else { 0 });
            }
        }
        (w, h, cells)
    }

    fn place_rows(sim: &mut Simulation, ox: i32, oy: i32, rows: &[&str]) {
        let (w, h, cells) = rows_to_cells(rows);
        for y in 0..h {
            for x in 0..w {
                if cells[(y * w + x) as usize] == 1 {
                    sim.front.set(ox + x as i32, oy + y as i32, Cell::Alive);
                }
            }
        }
    }

    #[test]
    fn blinker_period_2() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        // horizontal blinker at (1,2)..(3,2)
        place_rows(&mut sim, 1, 2, &["###"]);
        assert_eq!(sim.count_alive(), 3);
        sim.step();
        // should be vertical at (2,1)..(2,3)
        assert_eq!(sim.front.get(2, 1, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(2, 2, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(2, 3, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.count_alive(), 3);
        sim.step();
        // back to horizontal
        assert_eq!(sim.front.get(1, 2, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(2, 2, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(3, 2, Boundary::Fixed), Cell::Alive);
    }

    #[test]
    fn block_still_life() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        place_rows(&mut sim, 1, 1, &["##", "##"]);
        let before = sim.cells().to_vec();
        sim.step();
        assert_eq!(sim.cells(), &before[..]);
    }

    #[test]
    fn toad_period_2() {
        let mut sim = Simulation::new(6, 6, Boundary::Fixed).unwrap();
        place_rows(&mut sim, 1, 2, &[".###", "###."]);
        let before = sim.cells().to_vec();
        sim.step();
        sim.step();
        assert_eq!(sim.cells(), &before[..]);
    }

    #[test]
    fn beacon_period_2() {
        let mut sim = Simulation::new(6, 6, Boundary::Fixed).unwrap();
        place_rows(&mut sim, 1, 1, &["##..", "##..", "..##", "..##"]);
        let before = sim.cells().to_vec();
        sim.step();
        sim.step();
        assert_eq!(sim.cells(), &before[..]);
    }

    #[test]
    fn glider_moves_diagonally() {
        let mut sim = Simulation::new(20, 20, Boundary::Toroidal).unwrap();
        let glider = builtin("glider").unwrap();
        sim.place_pattern(glider, 5, 5).unwrap();
        // After 4 generations, the glider moves by (+1, +1).
        for _ in 0..4 {
            sim.step();
        }
        // Reconstruct expected: same glider shape at (6,6).
        let mut expected = Simulation::new(20, 20, Boundary::Toroidal).unwrap();
        expected.place_pattern(glider, 6, 6).unwrap();
        assert_eq!(sim.cells(), expected.cells());
    }

    #[test]
    fn fixed_boundary_glider_dies_at_corner() {
        // A glider near the right/bottom edge with Fixed boundary should not
        // produce a new glider on the opposite side. We just check that the
        // population evolves differently from the toroidal version.
        let mut fixed = Simulation::new(6, 6, Boundary::Fixed).unwrap();
        let glider = builtin("glider").unwrap();
        fixed.place_pattern(glider, 3, 3).unwrap();
        for _ in 0..10 {
            fixed.step();
        }

        let mut tor = Simulation::new(6, 6, Boundary::Toroidal).unwrap();
        tor.place_pattern(glider, 3, 3).unwrap();
        for _ in 0..10 {
            tor.step();
        }
        // They should diverge.
        assert_ne!(fixed.cells(), tor.cells());
    }

    #[test]
    fn toroidal_glider_wraps() {
        let mut sim = Simulation::new(10, 10, Boundary::Toroidal).unwrap();
        let glider = builtin("glider").unwrap();
        // Place glider so that after enough steps it wraps to the opposite side.
        sim.place_pattern(glider, 8, 8).unwrap();
        // 4 steps moves +1,+1 -> (9,9) on a 10-wide toroidal grid.
        for _ in 0..4 {
            sim.step();
        }
        let mut expected = Simulation::new(10, 10, Boundary::Toroidal).unwrap();
        expected.place_pattern(glider, 9, 9).unwrap();
        assert_eq!(sim.cells(), expected.cells());

        // 4 more steps -> (10,10) which wraps to (0,0). Pattern wraps around.
        for _ in 0..4 {
            sim.step();
        }
        let mut expected2 = Simulation::new(10, 10, Boundary::Toroidal).unwrap();
        expected2.place_pattern(glider, 0, 0).unwrap();
        assert_eq!(sim.cells(), expected2.cells());
    }

    #[test]
    fn resize_grow_preserves_top_left() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        sim.set_cell(0, 0, Cell::Alive);
        sim.set_cell(4, 4, Cell::Alive);
        sim.resize(10, 10).unwrap();
        assert_eq!(sim.width(), 10);
        assert_eq!(sim.height(), 10);
        assert_eq!(sim.front.get(0, 0, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(4, 4, Boundary::Fixed), Cell::Alive);
        // New region is dead.
        assert_eq!(sim.front.get(5, 5, Boundary::Fixed), Cell::Dead);
        assert_eq!(sim.front.get(9, 9, Boundary::Fixed), Cell::Dead);
        assert_eq!(sim.count_alive(), 2);
    }

    #[test]
    fn resize_shrink_truncates() {
        let mut sim = Simulation::new(10, 10, Boundary::Fixed).unwrap();
        sim.set_cell(1, 1, Cell::Alive);
        sim.set_cell(9, 9, Cell::Alive);
        sim.resize(5, 5).unwrap();
        assert_eq!(sim.width(), 5);
        assert_eq!(sim.front.get(1, 1, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.count_alive(), 1);
    }

    #[test]
    fn history_undo_returns_to_initial() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        sim.set_history_capacity(10);
        place_rows(&mut sim, 1, 2, &["###"]);
        let initial = sim.cells().to_vec();
        let initial_gen = sim.generation();
        sim.step();
        sim.step();
        assert!(sim.step_back());
        assert!(sim.step_back());
        assert_eq!(sim.cells(), &initial[..]);
        assert_eq!(sim.generation(), initial_gen);
    }

    #[test]
    fn history_capacity_overflow() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        sim.set_history_capacity(2);
        place_rows(&mut sim, 1, 2, &["###"]);
        sim.step();
        sim.step();
        sim.step();
        assert!(sim.step_back());
        assert!(sim.step_back());
        assert!(!sim.step_back());
    }

    #[test]
    fn place_pattern_toroidal_wraps() {
        let mut sim = Simulation::new(10, 10, Boundary::Toroidal).unwrap();
        let glider = builtin("glider").unwrap();
        // Place at right edge so it wraps.
        sim.place_pattern(glider, 9, 9).unwrap();
        // The 3x3 glider's cells should appear wrapped around.
        // Glider pattern:
        //   . # .
        //   . . #
        //   # # #
        // at (9,9), positions are:
        //  (10,9)->(0,9): #
        //  (11,10)->(1,0): #
        //  (9,11)->(9,1): #
        //  (10,11)->(0,1): #
        //  (11,11)->(1,1): #
        assert_eq!(sim.front.get(0, 9, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(1, 0, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(9, 1, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(0, 1, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(1, 1, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.count_alive(), 5);
    }

    #[test]
    fn place_pattern_fixed_out_of_bounds_errors() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        let glider = builtin("glider").unwrap();
        let err = sim.place_pattern(glider, 4, 4).unwrap_err();
        match err {
            CoreError::PatternOutOfBounds { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn builtin_and_all_builtins_consistent() {
        let pats = all_builtins();
        // 13 distinct patterns are bundled (5 oscillators + 3 still lifes +
        // 4 spaceships + 1 fancy).
        assert_eq!(pats.len(), 13);
        for p in pats {
            assert_eq!(
                p.cells.len() as u32,
                p.width * p.height,
                "pattern {} has inconsistent cells.len() vs width*height",
                p.name
            );
            assert!(builtin(p.name).is_some(), "{} not retrievable", p.name);
        }
        // Spot-check a known name.
        assert!(builtin("glider").is_some());
        assert!(builtin("gosper-glider-gun").is_some());
        assert!(builtin("does-not-exist").is_none());
    }

    #[test]
    fn randomize_seed_is_deterministic() {
        let mut a = Simulation::new(20, 20, Boundary::Fixed).unwrap();
        let mut b = Simulation::new(20, 20, Boundary::Fixed).unwrap();
        a.randomize(0.4, Some(12345)).unwrap();
        b.randomize(0.4, Some(12345)).unwrap();
        assert_eq!(a.cells(), b.cells());
    }

    #[test]
    fn count_alive_basic() {
        let mut sim = Simulation::new(10, 10, Boundary::Fixed).unwrap();
        assert_eq!(sim.count_alive(), 0);
        let block = builtin("block").unwrap();
        sim.place_pattern(block, 1, 1).unwrap();
        assert_eq!(sim.count_alive(), 4);
        sim.clear();
        assert_eq!(sim.count_alive(), 0);
        sim.randomize(0.5, Some(42)).unwrap();
        assert!(sim.count_alive() > 0);
    }

    #[test]
    fn invalid_dimensions_error() {
        assert!(Simulation::new(0, 5, Boundary::Fixed).is_err());
        assert!(Simulation::new(5, 0, Boundary::Fixed).is_err());
    }

    #[test]
    fn invalid_density_error() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        assert!(sim.randomize(-0.1, Some(0)).is_err());
        assert!(sim.randomize(1.1, Some(0)).is_err());
    }

    #[test]
    fn density_nan_returns_error() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        match sim.randomize(f32::NAN, None) {
            Err(CoreError::InvalidDensity(v)) => assert!(v.is_nan()),
            other => panic!("expected InvalidDensity(NaN), got {other:?}"),
        }
    }

    #[test]
    fn history_capacity_zero_disables_undo() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        sim.set_history_capacity(0);
        place_rows(&mut sim, 1, 2, &["###"]);
        sim.step();
        sim.step();
        assert!(!sim.step_back());
        assert!(!sim.step_back());
    }

    #[test]
    fn resize_undo_roundtrip() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        sim.set_history_capacity(10);
        sim.set_cell(0, 0, Cell::Alive);
        sim.set_cell(4, 4, Cell::Alive);
        let before_w = sim.width();
        let before_h = sim.height();
        let before_cells = sim.cells().to_vec();
        sim.resize(10, 10).unwrap();
        assert_eq!(sim.width(), 10);
        assert_eq!(sim.height(), 10);
        assert!(sim.step_back());
        assert_eq!(sim.width(), before_w);
        assert_eq!(sim.height(), before_h);
        assert_eq!(sim.cells(), &before_cells[..]);
        // The original alive cells should still be there.
        assert_eq!(sim.front.get(0, 0, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.front.get(4, 4, Boundary::Fixed), Cell::Alive);
        assert_eq!(sim.count_alive(), 2);
    }

    #[test]
    fn place_pattern_with_extreme_offset_returns_error() {
        let mut sim = Simulation::new(10, 10, Boundary::Fixed).unwrap();
        let glider = builtin("glider").unwrap();
        // ox = i32::MAX would overflow when added to the pattern width.
        let err = sim.place_pattern(glider, i32::MAX, 0).unwrap_err();
        assert!(matches!(err, CoreError::PatternOutOfBounds { .. }));
        // Same for the toroidal case at the Grid level: the inner loop must
        // not panic when adding pattern offsets to i32::MAX.
        let mut grid = Grid::new(10, 10).unwrap();
        let err2 = grid
            .place_pattern(glider, i32::MAX, 0, Boundary::Toroidal)
            .unwrap_err();
        assert!(matches!(err2, CoreError::PatternOutOfBounds { .. }));
    }

    #[test]
    fn set_cell_out_of_bounds_does_not_consume_history() {
        let mut sim = Simulation::new(5, 5, Boundary::Fixed).unwrap();
        sim.set_history_capacity(2);
        sim.step(); // history slot 1 used
        sim.set_cell(-1, 0, Cell::Alive); // OOB: must NOT push history
        sim.set_cell(99, 99, Cell::Alive); // OOB: must NOT push history
        // Only one history entry exists, so exactly one step_back succeeds.
        assert!(sim.step_back());
        assert!(!sim.step_back());
        assert!(!sim.step_back());
    }

    #[test]
    fn builtin_or_err_returns_unknown_pattern() {
        assert!(builtin_or_err("glider").is_ok());
        match builtin_or_err("does-not-exist") {
            Err(CoreError::UnknownPattern(name)) => assert_eq!(name, "does-not-exist"),
            other => panic!("expected UnknownPattern, got {other:?}"),
        }
    }

    #[test]
    fn grid_new_rejects_too_large_dimensions() {
        // u32::MAX exceeds i32::MAX so this must error.
        let err = Grid::new(u32::MAX, 1).unwrap_err();
        assert!(matches!(err, CoreError::InvalidDimensions { .. }));
        let err2 = Grid::new(1, u32::MAX).unwrap_err();
        assert!(matches!(err2, CoreError::InvalidDimensions { .. }));
    }

    /// On 32-bit targets (`usize` is 32 bits), `0x10000 * 0x10000` overflows
    /// the index type. Both factors are well within `i32::MAX`, so only the
    /// `checked_mul` guard inside `Grid::new` can reject this — exercising
    /// the regression test for that guard. Skipped on 64-bit hosts where
    /// such a multiplication does not overflow.
    #[test]
    #[cfg(target_pointer_width = "32")]
    fn grid_new_rejects_usize_mul_overflow_on_32bit() {
        let result = Grid::new(0x10000, 0x10000);
        assert!(matches!(result, Err(CoreError::InvalidDimensions { .. })));
    }

    #[test]
    fn resize_error_does_not_consume_history() {
        let mut sim = Simulation::new(5, 5, Boundary::Toroidal).unwrap();
        sim.set_history_capacity(10);
        sim.set_cell(2, 2, Cell::Alive); // history slot 1
        sim.step(); // history slot 2

        // Invalid dimension: must return Err and NOT consume a history slot.
        let result = sim.resize(0, 5);
        assert!(result.is_err());

        // Two history entries remain → step_back twice succeeds, third fails.
        assert!(sim.step_back());
        assert!(sim.step_back());
        assert!(!sim.step_back());
    }

    #[test]
    fn place_pattern_error_does_not_consume_history_toroidal() {
        let mut sim = Simulation::new(20, 20, Boundary::Toroidal).unwrap();
        sim.set_history_capacity(10);
        let glider = builtin("glider").unwrap();
        sim.place_pattern(glider, 5, 5).unwrap(); // history slot 1
        sim.step(); // history slot 2
        assert_eq!(sim.generation(), 1);

        // Extreme offset triggers an `i32` overflow inside Grid::place_pattern
        // for the Toroidal branch. Must NOT consume a history slot.
        let result = sim.place_pattern(glider, i32::MAX, 0);
        assert!(result.is_err());

        // Two history entries remain → step_back twice succeeds, third fails.
        assert!(sim.step_back());
        assert_eq!(sim.generation(), 0);
        assert!(sim.step_back());
        assert!(!sim.step_back());
    }
}
