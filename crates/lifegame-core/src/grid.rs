//! Grid type and cell-level operations.
//!
//! Cells are stored bit-packed: 64 cells per `u64` word, with cell `(x, y)`
//! at bit `x & 63` of word `y * stride_words + (x >> 6)`. The lowest bit of
//! a word holds the leftmost cell (x=0 of that word).
//!
//! `width` does not need to be a multiple of 64. The unused high bits in the
//! last word of each row are *invariant zero* — every operation must preserve
//! that, otherwise neighbour counting at the right edge would read garbage.

use crate::error::CoreError;
use crate::patterns::Pattern;
use rand::Rng;

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Cell {
    Dead = 0,
    Alive = 1,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Boundary {
    Toroidal,
    Fixed,
}

#[derive(Clone, Debug)]
pub struct Grid {
    width: u32,
    height: u32,
    stride_words: usize, // ceil(width / 64), in u64 units
    cells: Vec<u64>,     // length = stride_words * height
}

impl Grid {
    pub fn new(width: u32, height: u32) -> Result<Self, CoreError> {
        if width == 0 || height == 0 {
            return Err(CoreError::InvalidDimensions { width, height });
        }
        if width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(CoreError::InvalidDimensions { width, height });
        }
        let stride_words = ((width as usize) + 63) / 64;
        let len = stride_words
            .checked_mul(height as usize)
            .ok_or(CoreError::InvalidDimensions { width, height })?;
        Ok(Self {
            width,
            height,
            stride_words,
            cells: vec![0u64; len],
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Words per row in the underlying `u64` storage.
    pub fn stride_words(&self) -> usize {
        self.stride_words
    }

    /// Bytes per row in the underlying storage (always a multiple of 8).
    pub fn stride_bytes(&self) -> usize {
        self.stride_words * 8
    }

    /// Underlying `u64` words. Bit layout: cell `(x, y)` lives at
    /// `bits()[y * stride_words() + (x >> 6)]` bit `x & 63`.
    pub fn bits(&self) -> &[u64] {
        &self.cells
    }

    /// Mutable view of the underlying `u64` words. Callers writing to this
    /// slice MUST keep the high (out-of-row) bits zero in the last word of
    /// each row.
    pub fn bits_mut(&mut self) -> &mut [u64] {
        &mut self.cells
    }

    /// Byte view of the bit-packed cells. Layout is little-endian within
    /// each `u64` word (LSB of byte 0 is cell x=0). Provided for FFI / wasm
    /// consumers; pure-Rust callers should prefer `bits()`.
    pub fn cells(&self) -> &[u8] {
        bytemuck::cast_slice(&self.cells)
    }

    /// Bit-level mask helper.
    #[inline]
    fn word_index(&self, x: u32, y: u32) -> (usize, u64) {
        let xi = x as usize;
        let yi = y as usize;
        let widx = yi * self.stride_words + (xi >> 6);
        let mask = 1u64 << (xi & 63);
        (widx, mask)
    }

    pub fn get(&self, x: i32, y: i32, boundary: Boundary) -> Cell {
        let w = self.width as i32;
        let h = self.height as i32;
        let (xi, yi) = match boundary {
            Boundary::Toroidal => {
                let xi = x.rem_euclid(w);
                let yi = y.rem_euclid(h);
                (xi as u32, yi as u32)
            }
            Boundary::Fixed => {
                if x < 0 || x >= w || y < 0 || y >= h {
                    return Cell::Dead;
                }
                (x as u32, y as u32)
            }
        };
        let (widx, mask) = self.word_index(xi, yi);
        if (self.cells[widx] & mask) != 0 {
            Cell::Alive
        } else {
            Cell::Dead
        }
    }

    pub fn set(&mut self, x: i32, y: i32, cell: Cell) {
        if !self.in_bounds(x, y) {
            return;
        }
        let (widx, mask) = self.word_index(x as u32, y as u32);
        match cell {
            Cell::Alive => self.cells[widx] |= mask,
            Cell::Dead => self.cells[widx] &= !mask,
        }
    }

    pub fn toggle(&mut self, x: i32, y: i32) {
        if !self.in_bounds(x, y) {
            return;
        }
        let (widx, mask) = self.word_index(x as u32, y as u32);
        self.cells[widx] ^= mask;
    }

    #[inline]
    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    pub fn clear(&mut self) {
        for w in self.cells.iter_mut() {
            *w = 0;
        }
    }

    pub fn fill_random(
        &mut self,
        density: f32,
        rng: &mut impl rand::RngCore,
    ) -> Result<(), CoreError> {
        if !(0.0..=1.0).contains(&density) || density.is_nan() {
            return Err(CoreError::InvalidDensity(density));
        }
        // Per-cell Bernoulli draws keep the existing semantics (and golden
        // hash) deterministic for a given seed: the RNG sequence is the
        // same as before, just packed into bits afterwards.
        let stride = self.stride_words;
        let w = self.width as usize;
        for y in 0..self.height as usize {
            let row_base = y * stride;
            // Walk each whole word, plus the partial trailing word if
            // width is not a multiple of 64.
            for word_idx in 0..stride {
                let bit_start = word_idx * 64;
                let bits_in_word = (w - bit_start).min(64);
                let mut word: u64 = 0;
                for b in 0..bits_in_word {
                    let r: f32 = rng.gen();
                    if r < density {
                        word |= 1u64 << b;
                    }
                }
                self.cells[row_base + word_idx] = word;
            }
        }
        Ok(())
    }

    pub fn count_alive(&self) -> u32 {
        // High bits of the trailing word in each row are kept at zero by
        // every mutator, so a flat popcount over the whole storage gives
        // the live cell total directly.
        let mut total: u32 = 0;
        for w in &self.cells {
            total += w.count_ones();
        }
        total
    }

    pub fn place_pattern(
        &mut self,
        pattern: &Pattern,
        ox: i32,
        oy: i32,
        boundary: Boundary,
    ) -> Result<(), CoreError> {
        let w = self.width as i32;
        let h = self.height as i32;
        let oob = || CoreError::PatternOutOfBounds {
            name: pattern.name.to_string(),
            ox,
            oy,
            gw: self.width,
            gh: self.height,
        };
        if pattern.width > i32::MAX as u32 || pattern.height > i32::MAX as u32 {
            return Err(oob());
        }
        let pw = pattern.width as i32;
        let ph = pattern.height as i32;
        if boundary == Boundary::Fixed {
            if ox < 0 || oy < 0 {
                return Err(oob());
            }
            let end_x = ox.checked_add(pw).ok_or_else(oob)?;
            let end_y = oy.checked_add(ph).ok_or_else(oob)?;
            if end_x > w || end_y > h {
                return Err(oob());
            }
        }
        for py in 0..pattern.height {
            for px in 0..pattern.width {
                let v = pattern.cells[(py * pattern.width + px) as usize];
                if v == 0 {
                    continue;
                }
                let gx = ox.checked_add(px as i32).ok_or_else(oob)?;
                let gy = oy.checked_add(py as i32).ok_or_else(oob)?;
                let (xi, yi) = match boundary {
                    Boundary::Toroidal => {
                        (gx.rem_euclid(w) as u32, gy.rem_euclid(h) as u32)
                    }
                    Boundary::Fixed => (gx as u32, gy as u32),
                };
                let (widx, mask) = self.word_index(xi, yi);
                self.cells[widx] |= mask;
            }
        }
        Ok(())
    }
}
