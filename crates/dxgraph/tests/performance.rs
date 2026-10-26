//! Opt-in native timing reports for the graph block performance targets.
use dxgraph::{LayoutConfig, LayoutEdge, LayoutInput, LayoutNode, NodeId, Size, run_layout};
use std::time::Instant;

fn fixture(count: usize) -> LayoutInput {
    LayoutInput {
        nodes: (0..count)
            .map(|i| LayoutNode {
                id: NodeId::from(format!("node-{i}")),
                size: Size::new(180.0, 80.0),
                fixed: None,
                previous: None,
                layout_parent: (i > 0).then(|| NodeId::from(format!("node-{}", (i - 1) / 4))),
                order_key: Some(format!("{i:04}")),
                branch_side: None,
            })
            .collect(),
        edges: (1..count)
            .map(|i| LayoutEdge {
                source: NodeId::from(format!("node-{}", (i - 1) / 4)),
                target: NodeId::from(format!("node-{i}")),
                weight: 1.0,
            })
            .collect(),
        roots: vec!["node-0".into()],
    }
}

#[test]
#[ignore = "native timing report; run explicitly with --ignored --nocapture"]
fn graph_block_layout_timings() {
    for (name, count, layout) in [
        ("tree", 2_000, LayoutConfig::Tree(Default::default())),
        ("mind-map", 2_000, LayoutConfig::MindMap(Default::default())),
        (
            "organic-default-300",
            500,
            LayoutConfig::Force(Default::default()),
        ),
        (
            "organic-graph-80",
            500,
            LayoutConfig::Force(dxgraph::layout::ForceOptions {
                iterations: 80,
                ..Default::default()
            }),
        ),
    ] {
        let input = fixture(count);
        let start = Instant::now();
        let result = run_layout(&layout, &input);
        let elapsed = start.elapsed();
        assert_eq!(result.positions.len(), count);
        assert!(
            result
                .positions
                .values()
                .all(|p| p.x.is_finite() && p.y.is_finite())
        );
        eprintln!(
            "{name}, {count} nodes: {:.2} ms",
            elapsed.as_secs_f64() * 1000.0
        );
    }
}
