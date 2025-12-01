//! Criterion benchmark comparing typg-core codepoint parsing with fontgrep (made by FontLab https://www.fontlab.com/)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fontgrep::cli::parse_codepoints;
use typg_core::query::parse_codepoint_list;

fn bench_codepoint_parsers(c: &mut Criterion) {
    let sample = "U+0041-U+005A,U+0061-U+007A,00E9,1F600";

    c.bench_function("typg-core parse_codepoint_list", |b| {
        b.iter(|| parse_codepoint_list(black_box(sample)).unwrap())
    });

    c.bench_function("fontgrep parse_codepoints", |b| {
        b.iter(|| parse_codepoints(black_box(sample)).unwrap())
    });
}

criterion_group!(benches, bench_codepoint_parsers);
criterion_main!(benches);
