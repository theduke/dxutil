//! Isolates engine costs from browser rendering and host persistence callbacks.
use dxeditor::engine::{
    Command, EditorEngine, EditorSelection, EngineInput, EngineOptions, InlineContainerRef,
    Profile, model,
};
use dxeditor::{ComponentNode, EditorCatalog};
#[test]
#[ignore = "manual engine profiling; browser performance is measured by Playwright"]
fn large_document_last_block_typing() {
    for count in [2000, 10000] {
        let mut blocks = Vec::with_capacity(count);
        for _ in 0..count {
            let mut block = model::paragraph();
            block.content[0].content = vec![ComponentNode::text(
                "A short paragraph for measuring editing latency.",
            )];
            blocks.push(block);
        }
        let uid = blocks.last().unwrap().id.as_ref().unwrap().0.clone();
        let doc = model::document(blocks);
        let start = std::time::Instant::now();
        let mut engine = EditorEngine::new(
            doc,
            &EditorCatalog::block_document(),
            Profile::BlockDocument,
            EngineOptions::default(),
        )
        .unwrap();
        let mount = start.elapsed().as_secs_f64() * 1000.;
        engine.run(Command::SetSelection(EditorSelection::caret(
            InlineContainerRef::Block(uid),
            47,
        )));
        let mut samples = Vec::new();
        for i in 0..100 {
            engine.set_time_ms(i * 50);
            let start = std::time::Instant::now();
            engine.handle(EngineInput::InsertText { text: "a".into() });
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        eprintln!(
            "engine blocks={count} mount_ms={mount:.3} typing_p50_ms={:.3} typing_p95_ms={:.3}",
            samples[50], samples[95]
        );
    }
}
