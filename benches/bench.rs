//! Link throughput benchmarks.
//!
//! Each benchmark builds the objects once, then measures only the link — the work
//! [`linker_lang::link`] does per call: merging sections, resolving symbols, and patching
//! relocations. Two shapes cover the cost drivers: many small objects wired together by a
//! relocation table (symbol resolution and patching dominate), and a few large sections
//! (the byte-merging copy dominates).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use linker_lang::{Object, Width, link};

/// `n` code objects, each a 32-byte `.text` with a symbol at its start, plus a loader whose
/// `.data` table holds one relocated `u64` pointer per function. Stresses the symbol table
/// and the relocation pass at scale.
fn dispatch_table(n: u32) -> Vec<Object> {
    let mut objects = Vec::with_capacity(n as usize + 1);
    for i in 0..n {
        let mut object = Object::new(format!("f{i}"));
        object.section(".text", vec![0u8; 32]);
        object.define(format!("f{i}"), ".text", 0);
        objects.push(object);
    }

    let mut loader = Object::new("loader");
    loader.section(".data", vec![0u8; n as usize * 8]);
    for i in 0..n {
        loader.relocate(".data", u64::from(i) * 8, format!("f{i}"), Width::U64, 0);
    }
    objects.push(loader);
    objects
}

/// Eight objects each contributing `kib` KiB to one shared `.text` section: the link is
/// dominated by concatenating the bytes.
fn merge_sections(kib: usize) -> Vec<Object> {
    (0..8)
        .map(|i| {
            let mut object = Object::new(format!("part{i}"));
            object.section(".text", vec![0u8; kib * 1024]);
            object.define(format!("part{i}"), ".text", 0);
            object
        })
        .collect()
}

fn bench_link(c: &mut Criterion) {
    let mut group = c.benchmark_group("link");

    for &n in &[16u32, 256, 4096] {
        let objects = dispatch_table(n);
        group.throughput(Throughput::Elements(u64::from(n)));
        group.bench_with_input(
            BenchmarkId::new("dispatch_table", n),
            &objects,
            |b, objs| {
                b.iter(|| link(std::hint::black_box(objs)));
            },
        );
    }

    for &kib in &[16usize, 256, 1024] {
        let objects = merge_sections(kib);
        group.throughput(Throughput::Bytes((kib * 1024 * 8) as u64));
        group.bench_with_input(
            BenchmarkId::new("merge_sections_kib", kib),
            &objects,
            |b, objs| {
                b.iter(|| link(std::hint::black_box(objs)));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_link);
criterion_main!(benches);
