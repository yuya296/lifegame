//! Conway's Game of Life rules: B3/S23.
//!
//! NOTE: this stage of the bit-pack migration uses a single generic per-cell
//! kernel routed through `Grid::get` / `Grid::set`. It is correct on every
//! board shape (including 1xN / Nx1 toroidal) but slow. The SWAR-based fast
//! path will be reintroduced in a follow-up commit (Stage 2 of Tier D).

use crate::grid::{Boundary, Cell, Grid};

/// Compute the next generation of `src` into `dst`.
///
/// Panics if `src` and `dst` differ in dimensions.
pub fn next_generation(src: &Grid, dst: &mut Grid, boundary: Boundary) {
    assert_eq!(src.width(), dst.width());
    assert_eq!(src.height(), dst.height());
    let w = src.width() as i32;
    let h = src.height() as i32;
    // Clear the destination first so we can OR live cells into it without
    // having to also write zeros for dead ones.
    for word in dst.bits_mut().iter_mut() {
        *word = 0;
    }
    let stride = dst.stride_words();
    let dst_bits = dst.bits_mut();
    for y in 0..h {
        for x in 0..w {
            let mut n = 0u8;
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
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
            if next {
                let xi = x as usize;
                let yi = y as usize;
                let widx = yi * stride + (xi >> 6);
                dst_bits[widx] |= 1u64 << (xi & 63);
            }
        }
    }
}
