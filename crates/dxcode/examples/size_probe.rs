//! Core-only size probe. An exported entry keeps parsing and querying reachable.
use dxcode::{HighlightSession, languages};

#[unsafe(no_mangle)]
pub extern "C" fn dxcode_size_probe(language: usize, repetitions: usize) -> usize {
    let name = languages().get(language).map_or("unknown", |l| l.name);
    let source = "fn main() { let x = 42; }\n".repeat(repetitions.min(100));
    HighlightSession::new(name, source)
        .and_then(|s| s.highlight_all())
        .map_or(0, |s| s.len())
}

fn main() {
    std::hint::black_box(dxcode_size_probe(0, 1));
}
