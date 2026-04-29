//! Conway's Game of Life rules: B3/S23.
//!
//! `next_generation` is split into two boundary-specialised inner loops so
//! that the per-cell work has no `match boundary`, no `Cell` enum conversion,
//! and no `rem_euclid` (Fixed) / no bounds check (Toroidal). Row-base indices
//! (`y * width`) are precomputed once per row.

use crate::grid::{Boundary, Cell, Grid};

/// Compute the next generation of `src` into `dst`.
///
/// Panics if `src` and `dst` differ in dimensions.
pub fn next_generation(src: &Grid, dst: &mut Grid, boundary: Boundary) {
    assert_eq!(src.width(), dst.width());
    assert_eq!(src.height(), dst.height());
    match boundary {
        Boundary::Fixed => next_fixed(src, dst),
        Boundary::Toroidal => {
            // The fast Toroidal kernel assumes `w >= 2 && h >= 2` so that
            // wrapping the x±1 / y±1 offsets lands on a *different* cell than
            // (x, y). In degenerate 1×N or N×1 boards a wrapped neighbour can
            // alias the centre cell itself, which would then be miscounted.
            // Fall back to the generic per-cell path in that case.
            if src.width() < 2 || src.height() < 2 {
                next_generic(src, dst, Boundary::Toroidal);
            } else {
                next_toroidal(src, dst);
            }
        }
    }
}

/// Generic fallback used only for degenerate Toroidal boards. Routes through
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

#[inline]
fn next_fixed(src: &Grid, dst: &mut Grid) {
    let w = src.width() as usize;
    let h = src.height() as usize;
    let s = src.cells();
    let d = dst.cells_mut();

    for y in 0..h {
        let row = y * w;
        // Row-base indices for the three rows we read from. When out of
        // bounds we use `usize::MAX` as a sentinel and skip those reads.
        let row_up = if y == 0 { usize::MAX } else { row - w };
        let row_dn = if y + 1 == h { usize::MAX } else { row + w };
        for x in 0..w {
            // Three columns: x-1, x, x+1, with edges suppressed.
            let xl_ok = x > 0;
            let xr_ok = x + 1 < w;
            let mut n: u32 = 0;
            if row_up != usize::MAX {
                if xl_ok { n += s[row_up + x - 1] as u32; }
                n += s[row_up + x] as u32;
                if xr_ok { n += s[row_up + x + 1] as u32; }
            }
            if xl_ok { n += s[row + x - 1] as u32; }
            if xr_ok { n += s[row + x + 1] as u32; }
            if row_dn != usize::MAX {
                if xl_ok { n += s[row_dn + x - 1] as u32; }
                n += s[row_dn + x] as u32;
                if xr_ok { n += s[row_dn + x + 1] as u32; }
            }
            let alive = s[row + x] != 0;
            // B3/S23
            d[row + x] = ((alive && (n == 2 || n == 3)) || (!alive && n == 3)) as u8;
        }
    }
}

#[inline]
fn next_toroidal(src: &Grid, dst: &mut Grid) {
    let w = src.width() as usize;
    let h = src.height() as usize;
    let s = src.cells();
    let d = dst.cells_mut();

    for y in 0..h {
        let yu = if y == 0 { h - 1 } else { y - 1 };
        let yd = if y + 1 == h { 0 } else { y + 1 };
        let row = y * w;
        let row_u = yu * w;
        let row_d = yd * w;
        for x in 0..w {
            let xl = if x == 0 { w - 1 } else { x - 1 };
            let xr = if x + 1 == w { 0 } else { x + 1 };
            let n = s[row_u + xl] as u32
                + s[row_u + x] as u32
                + s[row_u + xr] as u32
                + s[row + xl] as u32
                + s[row + xr] as u32
                + s[row_d + xl] as u32
                + s[row_d + x] as u32
                + s[row_d + xr] as u32;
            let alive = s[row + x] != 0;
            d[row + x] = ((alive && (n == 2 || n == 3)) || (!alive && n == 3)) as u8;
        }
    }
}
