//! Micro-benchmarks for `next_generation`.
//!
//! Run with `cargo bench -p lifegame-core`. Reports go to
//! `target/criterion/<bench>/report/index.html`.
//!
//! The bench seeds a deterministic random board (density 0.3, seed=42) at
//! each size and measures the cost of one generation step under both
//! boundary conditions. This lets us compare the de-virtualised inner loop
//! (Tier A) against the current implementation directly.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lifegame_core::{next_generation, Boundary, Grid};
use rand::rngs::SmallRng;
use rand::SeedableRng;

const SIZES: &[u32] = &[64, 1024, 4096, 8192];

fn make_board(size: u32) -> Grid {
    let mut g = Grid::new(size, size).expect("valid grid");
    let mut rng = SmallRng::seed_from_u64(42);
    g.fill_random(0.3, &mut rng).expect("valid density");
    g
}

/// Format a byte count as KiB / MiB / GiB.
fn human_bytes(n: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if n >= GIB {
        format!("{:.2} GiB", n as f64 / GIB as f64)
    } else if n >= MIB {
        format!("{:.2} MiB", n as f64 / MIB as f64)
    } else if n >= KIB {
        format!("{:.2} KiB", n as f64 / KIB as f64)
    } else {
        format!("{n} B")
    }
}

fn bench_next_generation(c: &mut Criterion) {
    // Print theoretical working-set sizes once, before any benchmark runs,
    // so we have an algorithmic memory baseline alongside the wall-clock
    // numbers. This is the part of memory that the algorithm itself needs;
    // peak process RSS (which also covers criterion bookkeeping) should be
    // measured externally with `/usr/bin/time -l`.
    eprintln!("--- algorithmic working set per benchmark ---");
    eprintln!("(src + dst, 1 byte/cell — current u8 layout)");
    for &size in SIZES {
        let cells = (size as u64) * (size as u64);
        let bytes = cells * 2;
        eprintln!(
            "  {size}x{size} = {} cells   working set = {}",
            cells,
            human_bytes(bytes)
        );
    }
    eprintln!("---------------------------------------------");

    for &size in SIZES {
        let src = make_board(size);
        let mut dst = Grid::new(size, size).unwrap();
        let cells = (size as u64) * (size as u64);

        let mut group = c.benchmark_group("next_generation");
        group.throughput(Throughput::Elements(cells));
        // Large boards cost ~100ms+ per iteration; cap sample count so a
        // full bench run finishes in a reasonable time without --quick.
        if size >= 4096 {
            group.sample_size(10);
        }

        group.bench_with_input(BenchmarkId::new("toroidal", size), &size, |b, _| {
            b.iter(|| {
                next_generation(&src, &mut dst, Boundary::Toroidal);
            });
        });
        group.bench_with_input(BenchmarkId::new("fixed", size), &size, |b, _| {
            b.iter(|| {
                next_generation(&src, &mut dst, Boundary::Fixed);
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_next_generation);
criterion_main!(benches);
