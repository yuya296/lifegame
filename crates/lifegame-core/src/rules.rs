//! Conway's Game of Life rules: B3/S23.
//!
//! `next_generation` is split into two boundary-specialised inner kernels so
//! that the per-cell work has no `match boundary`, no `Cell` enum conversion,
//! and no `rem_euclid` (Fixed) / no bounds check (Toroidal). Row-base indices
//! (`y * width`) are precomputed once per row.
//!
//! Each kernel further splits its grid into:
//!   * the **interior** rectangle `(1..h-1, 1..w-1)` where every cell has all
//!     8 neighbours in-bounds (Fixed) or where wrapping is provably unused
//!     (Toroidal). This region is the hot loop: nine raw `&[u8]` reads, one
//!     write, no branching.
//!   * the **edge band** (top + bottom rows, left + right columns) which
//!     handles wrap / out-of-bounds explicitly. This region's cell count is
//!     `O(w + h)` and so its cost vanishes against the `O(w*h)` interior on
//!     large boards.

use crate::grid::{Boundary, Cell, Grid};

/// Compute the next generation of `src` into `dst`.
///
/// Panics if `src` and `dst` differ in dimensions.
pub fn next_generation(src: &Grid, dst: &mut Grid, boundary: Boundary) {
    assert_eq!(src.width(), dst.width());
    assert_eq!(src.height(), dst.height());
    let w = src.width() as usize;
    let h = src.height() as usize;
    match boundary {
        Boundary::Fixed => {
            // The fast Fixed kernel needs at least a 3x3 board to have any
            // interior at all; for tiny boards just use the slow path so we
            // don't have to special-case the interior split.
            if w >= 3 && h >= 3 {
                next_fixed(src, dst, w, h);
            } else {
                next_generic(src, dst, Boundary::Fixed);
            }
        }
        Boundary::Toroidal => {
            // The fast Toroidal kernel assumes `w >= 3 && h >= 3` so that the
            // edge wrap targets a row/column distinct from BOTH the centre
            // and the opposite edge. In smaller boards a wrapped neighbour
            // can alias the centre cell itself (1×N, N×1) or the opposite
            // edge can collapse onto the same row (1- or 2-cell dimensions).
            // The per-cell `Grid::get` path handles aliasing-via-wrap
            // correctly, so we use it for those degenerate sizes.
            if w >= 3 && h >= 3 {
                next_toroidal(src, dst, w, h);
            } else {
                next_generic(src, dst, Boundary::Toroidal);
            }
        }
    }
}

/// B3/S23 transition table baked into a single expression.
#[inline(always)]
fn b3s23(alive: bool, n: u32) -> u8 {
    ((alive && (n == 2 || n == 3)) || (!alive && n == 3)) as u8
}

/// Generic fallback used only for degenerate boards. Routes through
/// `Grid::get` which already handles aliasing-via-wrap correctly (it skips
/// the centre offset explicitly).
#[inline(never)]
fn next_generic(src: &Grid, dst: &mut Grid, boundary: Boundary) {
    let w = src.width() as i32;
    let h = src.height() as i32;
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
            let idx = ((y as u32) * src.width() + (x as u32)) as usize;
            dst.cells_mut()[idx] = next as u8;
        }
    }
}

// -----------------------------------------------------------------------
// Fixed boundary
// -----------------------------------------------------------------------

#[inline]
fn next_fixed(src: &Grid, dst: &mut Grid, w: usize, h: usize) {
    let s = src.cells();
    let d = dst.cells_mut();

    // Interior: y in 1..h-1, x in 1..w-1. Every cell has all 8 neighbours
    // in-bounds, so we can sum them with no branching.
    for y in 1..h - 1 {
        let row = y * w;
        let row_u = row - w;
        let row_d = row + w;
        // Sub-slices for this row and its two neighbours; keeps indexing
        // explicit so the optimiser sees them as plain pointer arithmetic.
        for x in 1..w - 1 {
            let n = s[row_u + x - 1] as u32
                + s[row_u + x] as u32
                + s[row_u + x + 1] as u32
                + s[row + x - 1] as u32
                + s[row + x + 1] as u32
                + s[row_d + x - 1] as u32
                + s[row_d + x] as u32
                + s[row_d + x + 1] as u32;
            d[row + x] = b3s23(s[row + x] != 0, n);
        }
    }

    // Edge band: top row, bottom row, left column, right column. Each cell
    // is computed via the slow per-cell path. Total cost is O(w + h) so it
    // disappears against the interior on large boards.
    fixed_edge_row(src, dst, 0, w, h);
    if h > 1 {
        fixed_edge_row(src, dst, h - 1, w, h);
    }
    for y in 1..h.saturating_sub(1) {
        fixed_edge_cell(src, dst, 0, y, w, h);
        if w > 1 {
            fixed_edge_cell(src, dst, w - 1, y, w, h);
        }
    }
}

#[inline]
fn fixed_edge_row(src: &Grid, dst: &mut Grid, y: usize, w: usize, h: usize) {
    for x in 0..w {
        fixed_edge_cell(src, dst, x, y, w, h);
    }
}

#[inline]
fn fixed_edge_cell(src: &Grid, dst: &mut Grid, x: usize, y: usize, w: usize, h: usize) {
    let s = src.cells();
    let row = y * w;
    let mut n: u32 = 0;
    let xl_ok = x > 0;
    let xr_ok = x + 1 < w;
    if y > 0 {
        let row_u = row - w;
        if xl_ok {
            n += s[row_u + x - 1] as u32;
        }
        n += s[row_u + x] as u32;
        if xr_ok {
            n += s[row_u + x + 1] as u32;
        }
    }
    if xl_ok {
        n += s[row + x - 1] as u32;
    }
    if xr_ok {
        n += s[row + x + 1] as u32;
    }
    if y + 1 < h {
        let row_d = row + w;
        if xl_ok {
            n += s[row_d + x - 1] as u32;
        }
        n += s[row_d + x] as u32;
        if xr_ok {
            n += s[row_d + x + 1] as u32;
        }
    }
    let alive = s[row + x] != 0;
    dst.cells_mut()[row + x] = b3s23(alive, n);
}

// -----------------------------------------------------------------------
// Toroidal boundary
// -----------------------------------------------------------------------

#[inline]
fn next_toroidal(src: &Grid, dst: &mut Grid, w: usize, h: usize) {
    let s = src.cells();
    let d = dst.cells_mut();

    // Interior: y in 1..h-1, x in 1..w-1. No wrap occurs in this rectangle.
    for y in 1..h - 1 {
        let row = y * w;
        let row_u = row - w;
        let row_d = row + w;
        for x in 1..w - 1 {
            let n = s[row_u + x - 1] as u32
                + s[row_u + x] as u32
                + s[row_u + x + 1] as u32
                + s[row + x - 1] as u32
                + s[row + x + 1] as u32
                + s[row_d + x - 1] as u32
                + s[row_d + x] as u32
                + s[row_d + x + 1] as u32;
            d[row + x] = b3s23(s[row + x] != 0, n);
        }
    }

    // Edge band: top + bottom rows entirely, plus left/right columns of the
    // interior rows. Each call computes its row offsets with wrap.
    toroidal_edge_row(src, dst, 0, w, h);
    toroidal_edge_row(src, dst, h - 1, w, h);
    for y in 1..h - 1 {
        toroidal_edge_cell(src, dst, 0, y, w, h);
        toroidal_edge_cell(src, dst, w - 1, y, w, h);
    }
}

#[inline]
fn toroidal_edge_row(src: &Grid, dst: &mut Grid, y: usize, w: usize, h: usize) {
    for x in 0..w {
        toroidal_edge_cell(src, dst, x, y, w, h);
    }
}

#[inline]
fn toroidal_edge_cell(src: &Grid, dst: &mut Grid, x: usize, y: usize, w: usize, h: usize) {
    let s = src.cells();
    let yu = if y == 0 { h - 1 } else { y - 1 };
    let yd = if y + 1 == h { 0 } else { y + 1 };
    let xl = if x == 0 { w - 1 } else { x - 1 };
    let xr = if x + 1 == w { 0 } else { x + 1 };
    let row = y * w;
    let row_u = yu * w;
    let row_d = yd * w;
    let n = s[row_u + xl] as u32
        + s[row_u + x] as u32
        + s[row_u + xr] as u32
        + s[row + xl] as u32
        + s[row + xr] as u32
        + s[row_d + xl] as u32
        + s[row_d + x] as u32
        + s[row_d + xr] as u32;
    let alive = s[row + x] != 0;
    dst.cells_mut()[row + x] = b3s23(alive, n);
}
