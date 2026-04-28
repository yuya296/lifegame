//! Conway's Game of Life rules: B3/S23.

use crate::grid::{Boundary, Cell, Grid};

/// Compute the next generation of `src` into `dst`.
///
/// Panics if `src` and `dst` differ in dimensions.
pub fn next_generation(src: &Grid, dst: &mut Grid, boundary: Boundary) {
    assert_eq!(src.width(), dst.width());
    assert_eq!(src.height(), dst.height());
    let w = src.width() as i32;
    let h = src.height() as i32;
    for y in 0..h {
        for x in 0..w {
            let mut n = 0u8;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    if src.get(x + dx, y + dy, boundary) == Cell::Alive {
                        n += 1;
                    }
                }
            }
            let alive = src.get(x, y, boundary) == Cell::Alive;
            let next = matches!((alive, n), (true, 2) | (true, 3) | (false, 3));
            let idx = ((y as u32) * src.width() + (x as u32)) as usize;
            dst.cells_mut()[idx] = if next { 1 } else { 0 };
        }
    }
}
