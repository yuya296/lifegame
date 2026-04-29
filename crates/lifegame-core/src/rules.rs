//! Conway's Game of Life rules: B3/S23.
//!
//! Bit-packed implementation: cells live in `Vec<u64>` words (64 cells per
//! word, LSB = leftmost cell). The hot kernel processes 64 cells per word
//! in parallel using bit-sliced full-adders, so each ~30 logical operations
//! advance 64 cells of generation.
//!
//! Layout reminder (see `Grid` docs): row `y` of width `w` occupies
//! `stride_words = ceil(w / 64)` consecutive `u64` words. Bits past `w`
//! within the trailing word are *invariant zero*. The kernel relies on
//! this to suppress bogus neighbour counts at the right edge.

use crate::grid::{Boundary, Cell, Grid};

/// Compute the next generation of `src` into `dst`.
///
/// Panics if `src` and `dst` differ in dimensions.
pub fn next_generation(src: &Grid, dst: &mut Grid, boundary: Boundary) {
    assert_eq!(src.width(), dst.width());
    assert_eq!(src.height(), dst.height());
    let w = src.width() as usize;
    let h = src.height() as usize;
    // Width of the trailing word's "live" region, in bits (1..=64).
    // Any bit at position >= live_tail_bits in the last word of a row is
    // out-of-grid and must remain zero in the destination.
    let _ = w; // (kept to make the intent above explicit; tail mask derived inside kernels)
    match boundary {
        Boundary::Fixed => {
            if h >= 1 {
                next_fixed(src, dst);
            }
        }
        Boundary::Toroidal => {
            // Same alias-via-wrap caveat as the byte version: 1xN / Nx1 /
            // tiny boards can wrap onto the centre cell. Route those to the
            // generic per-cell path.
            if w >= 3 && h >= 3 {
                next_toroidal(src, dst);
            } else {
                next_generic(src, dst, Boundary::Toroidal);
            }
        }
    }
}

/// Generic fallback for degenerate boards. Same per-cell loop the byte
/// version used; correctness is the only goal here.
#[inline(never)]
fn next_generic(src: &Grid, dst: &mut Grid, boundary: Boundary) {
    let w = src.width() as i32;
    let h = src.height() as i32;
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

// -------------------------------------------------------------------
// SWAR helpers
// -------------------------------------------------------------------

/// Half-adder over 64-bit lanes: returns (sum, carry).
#[inline(always)]
fn ha(a: u64, b: u64) -> (u64, u64) {
    (a ^ b, a & b)
}

/// Full-adder over 64-bit lanes: returns (sum, carry).
#[inline(always)]
fn fa(a: u64, b: u64, c: u64) -> (u64, u64) {
    let ab = a ^ b;
    (ab ^ c, (a & b) | (ab & c))
}

/// Sum of three 1-bit lanes -> (s0, s1) representing 0..3.
#[inline(always)]
fn sum3(a: u64, b: u64, c: u64) -> (u64, u64) {
    fa(a, b, c)
}

/// Sum of three 2-bit lanes (each 0..3) -> (r0, r1, r2, r3) representing 0..9.
/// We carry-propagate the LSBs first, then the MSBs with the LSB carry.
#[inline(always)]
fn sum_three_2bit(a: (u64, u64), b: (u64, u64), c: (u64, u64)) -> (u64, u64, u64, u64) {
    // LSB lane sum: 3 inputs of 1 bit -> 0..3 across (low0, low1).
    let (low0, low1_carry) = sum3(a.0, b.0, c.0);
    // MSB lane sum: 3 inputs of 1 bit -> 0..3 across (mid0, mid1).
    let (mid0, mid1_carry) = sum3(a.1, b.1, c.1);
    // Combine: bit 1 position has contribution `low1_carry` from LSB carry,
    // plus `mid0` (the LSB of the MSB-lane sum). Add them.
    let (r1, c1) = ha(low1_carry, mid0);
    // Bit 2: mid1_carry + c1.
    let (r2, c2) = ha(mid1_carry, c1);
    // Bit 3: c2 (max 1).
    (low0, r1, r2, c2)
}

// -------------------------------------------------------------------
// Toroidal kernel
// -------------------------------------------------------------------

#[inline]
fn next_toroidal(src: &Grid, dst: &mut Grid) {
    let w = src.width() as usize;
    let h = src.height() as usize;
    let stride = src.stride_words();
    debug_assert_eq!(stride, dst.stride_words());

    // Mask of valid bits in the trailing word. If width is a multiple of
    // 64 every bit is valid; otherwise only the low `w & 63` bits are.
    let tail_bits = (w & 63) as u32;
    let tail_mask: u64 = if tail_bits == 0 {
        u64::MAX
    } else {
        (1u64 << tail_bits) - 1
    };

    let s = src.bits();
    let d = dst.bits_mut();

    for y in 0..h {
        let yu = if y == 0 { h - 1 } else { y - 1 };
        let yd = if y + 1 == h { 0 } else { y + 1 };
        let row = y * stride;
        let row_u = yu * stride;
        let row_d = yd * stride;
        for wi in 0..stride {
            // Source words for the three rows at this word position.
            let centre_u = s[row_u + wi];
            let centre_c = s[row + wi];
            let centre_d = s[row_d + wi];

            // Left/right neighbour words for each row, with toroidal wrap
            // both at row-internal word boundaries and at the row's own
            // left/right edge.
            let (lwi, rwi) = if stride == 1 {
                (0usize, 0usize)
            } else if wi == 0 {
                (stride - 1, 1)
            } else if wi + 1 == stride {
                (wi - 1, 0)
            } else {
                (wi - 1, wi + 1)
            };

            // For the leftmost word, the `<< 1` would shift in 0 at bit 0,
            // but bit 0 is x=0 and its left neighbour is x=-1, which on
            // a torus is x = w - 1. That bit lives at:
            //   * the MSB of the trailing word (`tail_bits - 1` if the row
            //     uses a partial trailing word)
            //   * bit 63 of the previous word otherwise (the "stride > 1
            //     and wi == 0" case below already pulls from `lwi`)
            // The MSB-shift for the right edge mirrors this.
            //
            // Build `left(row)` = each source row shifted right by 1 cell
            // (so position p reads cell p-1), with bit 63 of the previous
            // word OR'd into bit 0 of the result. For x=0 of the row we
            // also need to wrap from the row's last live bit.
            let left_u = shift_left_with_carry(centre_u, s[row_u + lwi], wi, stride, tail_bits);
            let left_c = shift_left_with_carry(centre_c, s[row + lwi], wi, stride, tail_bits);
            let left_d = shift_left_with_carry(centre_d, s[row_d + lwi], wi, stride, tail_bits);

            let right_u =
                shift_right_with_carry(centre_u, s[row_u + rwi], wi, stride, tail_bits);
            let right_c = shift_right_with_carry(centre_c, s[row + rwi], wi, stride, tail_bits);
            let right_d =
                shift_right_with_carry(centre_d, s[row_d + rwi], wi, stride, tail_bits);

            // Per-row 3-cell horizontal sum (0..3) for upper / centre / lower.
            let row_sum_u = sum3(left_u, centre_u, right_u);
            let row_sum_c = sum3(left_c, centre_c, right_c);
            let row_sum_d = sum3(left_d, centre_d, right_d);

            // Sum the three row sums -> 3x3 total (0..9) in 4 planes.
            let (t0, t1, t2, t3) = sum_three_2bit(row_sum_u, row_sum_c, row_sum_d);
            // Subtract the centre cell to leave just the eight neighbours.
            // total - centre, with centre being a 1-bit value, lowers t0
            // from 1 to 0 (or borrow). The simplest correct path: compute
            // n = total, then evaluate the rule using `total` and centre
            // explicitly: n == 3 OR (centre & n == 4 means alive with 3
            // alive neighbours after subtracting itself). Easier: derive
            // alive-or-3-neighbour directly.
            //
            // total = neighbours + centre_bit. So:
            //   n == 3 (need)        <=> total == 3 if centre_bit==0,
            //                            total == 4 if centre_bit==1.
            //   alive && n in {2,3}  <=> centre_bit==1 AND total in {3, 4}.
            //
            // Combine: next-alive cell iff
            //   (centre_bit==0 AND total==3) OR (centre_bit==1 AND total in {3,4}).
            //
            // 3 = 0b0011, 4 = 0b0100.
            let is3 = !t3 & !t2 & t1 & t0;
            let is4 = !t3 & t2 & !t1 & !t0;
            let next = is3 | (centre_c & is4);
            // Mask off out-of-grid bits in the trailing word.
            let next = if wi + 1 == stride {
                next & tail_mask
            } else {
                next
            };
            d[row + wi] = next;
        }
    }
}

/// For a centre word `centre` at column-word position `wi`, return the
/// "shift-left by one cell" view: bit at position p of the result is
/// the source cell at column (p - 1). Wraps within the row toroidally.
#[inline(always)]
fn shift_left_with_carry(
    centre: u64,
    left_word: u64,
    wi: usize,
    stride: usize,
    tail_bits: u32,
) -> u64 {
    // Cell at position p of `centre` corresponds to global x = wi*64 + p.
    // The "left neighbour" of bit p in the result represents x-1, which is
    // bit (p-1) of the original. So we shift `centre` left by 1, and need
    // bit 0 of the result to be the highest live bit of `left_word`.
    //
    // For interior words (wi > 0), the highest live bit of `left_word` is
    // bit 63. For wi == 0 (leftmost word in the row), the wrap source is
    // the *last live bit of the row's trailing word*, which is what
    // `left_word` is set to by the caller.
    let carry_in = if wi == 0 {
        // `left_word` here is the row's trailing word; its highest live
        // bit is bit (tail_bits - 1) when the row is not a clean multiple
        // of 64, and bit 63 otherwise.
        let high_bit = if tail_bits == 0 { 63 } else { tail_bits - 1 };
        (left_word >> high_bit) & 1
    } else {
        // Standard intra-row carry: pull bit 63 of the previous word.
        let _ = stride;
        left_word >> 63
    };
    (centre << 1) | carry_in
}

#[inline(always)]
fn shift_right_with_carry(
    centre: u64,
    right_word: u64,
    wi: usize,
    stride: usize,
    tail_bits: u32,
) -> u64 {
    // Bit p of the result is the original bit (p+1). Shift right by 1; the
    // top bit needs to come from bit 0 of `right_word`. For the trailing
    // word the "natural" top bit is the live high bit (tail_bits - 1), and
    // its right-neighbour is bit 0 of the next word in the row (which
    // toroidally wraps to word 0). For non-trailing words the high bit is
    // 63 and its right neighbour is bit 0 of the next word.
    let in_bit = right_word & 1;
    let result = (centre >> 1) | (in_bit << 63);
    // For the trailing word, the cells past `tail_bits` are not real, so
    // the bit we just put at position 63 is only valid if `tail_bits ==
    // 64`. When the row is not a multiple of 64, we instead want the in-
    // bit at position (tail_bits - 1).
    if wi + 1 == stride && tail_bits != 0 {
        // Clear position 63 (already shifted) and place the carry at the
        // proper "end of live bits" position.
        let cleared = result & !(1u64 << 63);
        let placed = cleared | (in_bit << (tail_bits - 1));
        // Also: the bit that would have lived at position `tail_bits`
        // (i.e. just past the live region) is the source's own (tail_bits)
        // bit, which is zero by invariant — so the right-shift never
        // pollutes us. Only the explicit carry-in needed re-routing.
        return placed;
    }
    result
}

// -------------------------------------------------------------------
// Fixed boundary kernel
// -------------------------------------------------------------------

#[inline]
fn next_fixed(src: &Grid, dst: &mut Grid) {
    let w = src.width() as usize;
    let h = src.height() as usize;
    let stride = src.stride_words();
    debug_assert_eq!(stride, dst.stride_words());

    let tail_bits = (w & 63) as u32;
    let tail_mask: u64 = if tail_bits == 0 {
        u64::MAX
    } else {
        (1u64 << tail_bits) - 1
    };

    let s = src.bits();
    let d = dst.bits_mut();

    for y in 0..h {
        let row = y * stride;
        let has_up = y > 0;
        let has_dn = y + 1 < h;
        let row_u = if has_up { row - stride } else { 0 };
        let row_d = if has_dn { row + stride } else { 0 };
        for wi in 0..stride {
            let centre_u = if has_up { s[row_u + wi] } else { 0 };
            let centre_c = s[row + wi];
            let centre_d = if has_dn { s[row_d + wi] } else { 0 };

            // Fixed-boundary horizontal carries: left of leftmost word is
            // 0; right of rightmost word is 0.
            let left_carry_u = if wi > 0 { s[row_u + wi - 1] >> 63 } else { 0 };
            let left_carry_c = if wi > 0 { s[row + wi - 1] >> 63 } else { 0 };
            let left_carry_d = if wi > 0 { s[row_d + wi - 1] >> 63 } else { 0 };
            let right_carry_u = if wi + 1 < stride { s[row_u + wi + 1] & 1 } else { 0 };
            let right_carry_c = if wi + 1 < stride { s[row + wi + 1] & 1 } else { 0 };
            let right_carry_d = if wi + 1 < stride { s[row_d + wi + 1] & 1 } else { 0 };

            let left_u = (centre_u << 1) | left_carry_u;
            let left_c = (centre_c << 1) | left_carry_c;
            let left_d = (centre_d << 1) | left_carry_d;
            let right_u = (centre_u >> 1) | (right_carry_u << 63);
            let right_c = (centre_c >> 1) | (right_carry_c << 63);
            let right_d = (centre_d >> 1) | (right_carry_d << 63);
            // For the trailing word, the right shift introduces the bit
            // that *would* have been at position 63 from `right_carry_*`,
            // but the actual rightmost live cell is at `tail_bits - 1`.
            // For Fixed boundary, the right carry of the trailing word is
            // 0 (no neighbour), so right_*[63] is already 0 and nothing
            // needs adjusting. The right shift of the trailing word's own
            // contents is also fine: position `tail_bits - 1` reads from
            // bit `tail_bits`, which is invariant zero.
            //
            // The left shift of the trailing word might push a live bit
            // past `tail_bits - 1`: for example with width=60, bit 59 is
            // the last live cell, and shifting left puts it at position 60
            // (out of bounds). That bogus bit must not feed neighbours,
            // but we mask the final result with tail_mask anyway.

            let row_sum_u = sum3(left_u, centre_u, right_u);
            let row_sum_c = sum3(left_c, centre_c, right_c);
            let row_sum_d = sum3(left_d, centre_d, right_d);
            let (t0, t1, t2, t3) = sum_three_2bit(row_sum_u, row_sum_c, row_sum_d);
            let is3 = !t3 & !t2 & t1 & t0;
            let is4 = !t3 & t2 & !t1 & !t0;
            let next = is3 | (centre_c & is4);
            let next = if wi + 1 == stride {
                next & tail_mask
            } else {
                next
            };
            d[row + wi] = next;
        }
    }
}
