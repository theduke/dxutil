//! Native timings: cargo run -p dxhighlight --example timings --release --no-default-features --features rust
use dxhighlight::{HighlightSession, TextEdit};
use std::time::Instant;

fn main() -> Result<(), dxhighlight::CodeError> {
    let source = "fn main() { let café = 42; }\r\n".repeat(8000);
    let start = Instant::now();
    let mut session = HighlightSession::new("rust", &source)?;
    println!("First query compilation + parse: {:?}", start.elapsed());
    let start = Instant::now();
    let _ = HighlightSession::new("rust", &source)?;
    println!("Cached configuration + parse: {:?}", start.elapsed());
    let start = Instant::now();
    let all = session.highlight_all()?;
    println!(
        "Full highlight ({} spans): {:?}",
        all.len(),
        start.elapsed()
    );
    let start = Instant::now();
    let _ = session.highlight(0..source.find("42").unwrap())?;
    println!("Viewport highlight: {:?}", start.elapsed());
    let start = Instant::now();
    let byte = source.find("42").unwrap();
    let changed = session.apply_edit(TextEdit {
        range: byte..byte + 1,
        replacement: "9".into(),
    })?;
    for range in changed {
        let _ = session.highlight(range)?;
    }
    println!(
        "Incremental edit + changed highlight: {:?}",
        start.elapsed()
    );
    Ok(())
}
