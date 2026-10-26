use dxeditor::engine::{
    Command, EditorEngine, EditorSelection, EngineOptions, EngineOutput, InlineContainerRef,
    Profile,
    commands::{
        Axis, TableCommand,
        media::{FileMetadata, MAX_PASTED_FILE_BYTES, file_placeholders},
    },
    model,
};
use dxeditor::{AttributeMap, ComponentNode, EditorCatalog, NodeId};
use serde_json::{Value, json};

fn table_engine(rows: usize, columns: usize) -> (EditorEngine, String) {
    let mut table = ComponentNode::new("table");
    table.attrs.insert("header_row".into(), json!(false));
    table.attrs.insert("header_column".into(), json!(false));
    for row in 0..rows {
        table.content.push(ComponentNode::container(
            "table_row",
            Some(NodeId::new(uuid::Uuid::now_v7().to_string())),
            (0..columns)
                .map(|column| {
                    ComponentNode::container(
                        "table_cell",
                        None,
                        vec![ComponentNode::text(format!("{row}:{column}"))],
                    )
                })
                .collect(),
        ));
    }
    let block = model::block_node(table);
    let uid = block.id.as_ref().unwrap().0.clone();
    let mut engine = EditorEngine::new(
        model::document(vec![block]),
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap();
    select_cell(&mut engine, &uid, 0, 0);
    (engine, uid)
}
fn table<'a>(engine: &'a EditorEngine, uid: &str) -> &'a ComponentNode {
    model::content(engine.document(), uid).unwrap()
}
fn select_cell(engine: &mut EditorEngine, uid: &str, row: usize, col: usize) {
    let row = table(engine, uid).content[row]
        .id
        .as_ref()
        .unwrap()
        .0
        .clone();
    engine.run(Command::SetSelection(EditorSelection::caret(
        InlineContainerRef::Cell { row, col },
        0,
    )));
}
fn attrs(value: Value) -> AttributeMap {
    value.as_object().unwrap().clone()
}
fn changes(output: &[EngineOutput]) -> usize {
    output
        .iter()
        .filter(|output| matches!(output, EngineOutput::Changed { .. }))
        .count()
}

#[test]
fn tsv_paste_grows_preserves_cells_and_undoes_once() {
    let (mut engine, uid) = table_engine(2, 2);
    select_cell(&mut engine, &uid, 1, 1);
    let before = engine.document().clone();
    let output = engine.run(Command::Table(TableCommand::PasteGrid(vec![
        vec!["A".into(), "B".into()],
        vec!["C".into(), "D\nE".into()],
    ])));
    assert_eq!(changes(&output), 1);
    let grid = table(&engine, &uid);
    assert_eq!(grid.content.len(), 3);
    assert_eq!(grid.content[0].content.len(), 3);
    assert_eq!(grid.content[0].content[0].text_content(), "0:0");
    assert_eq!(grid.content[1].content[1].text_content(), "A");
    assert_eq!(
        dxeditor::engine::text::plain(&grid.content[2].content[2].content),
        "D\nE"
    );
    assert_eq!(grid.content[2].content[2].content[1].kind.0, "hard_break");
    let ids: std::collections::BTreeSet<_> =
        grid.content.iter().map(|row| row.id.clone()).collect();
    assert_eq!(ids.len(), 3);
    engine.run(Command::Undo);
    assert_eq!(engine.document(), &before);
    assert!(!engine.can_undo());
    engine.run(Command::Redo);
    assert_eq!(table(&engine, &uid).content.len(), 3);
}

#[test]
fn selected_grid_tiles_and_limits_are_atomic() {
    let (mut engine, uid) = table_engine(3, 3);
    engine.run(Command::SetSelection(EditorSelection::Cells {
        table: uid.clone(),
        anchor: (0, 0),
        head: (2, 2),
    }));
    engine.run(Command::Table(TableCommand::PasteGrid(vec![vec![
        "x".into(),
        "y".into(),
    ]])));
    for row in &table(&engine, &uid).content {
        assert_eq!(
            row.content
                .iter()
                .map(ComponentNode::text_content)
                .collect::<Vec<_>>(),
            ["x", "y", "x"]
        );
    }
    let before = engine.document().clone();
    select_cell(&mut engine, &uid, 2, 2);
    assert!(
        engine
            .run(Command::Table(TableCommand::PasteGrid(vec![vec![
                "x".into(
                );
                100
            ]])))
            .is_empty()
    );
    assert_eq!(engine.document(), &before);
}

#[test]
fn headers_follow_axis_insert_delete_and_duplicate() {
    let (mut engine, uid) = table_engine(2, 2);
    engine.run(Command::Table(TableCommand::ToggleHeader(Axis::Row)));
    engine.run(Command::Table(TableCommand::ToggleHeader(Axis::Column)));
    engine.run(Command::Table(TableCommand::InsertAxis {
        axis: Axis::Row,
        index: 0,
        after: false,
    }));
    let grid = table(&engine, &uid);
    assert_eq!(grid.content[0].content[1].kind.0, "table_header_cell");
    assert_eq!(grid.content[1].content[1].kind.0, "table_cell");
    assert_eq!(grid.content[1].content[0].kind.0, "table_header_cell");
    let first = grid.content[0].id.clone();
    engine.run(Command::Table(TableCommand::DuplicateAxis {
        axis: Axis::Row,
        index: 0,
    }));
    assert_ne!(table(&engine, &uid).content[1].id, first);
    engine.run(Command::Table(TableCommand::DeleteAxis {
        axis: Axis::Row,
        index: 0,
    }));
    assert_eq!(
        table(&engine, &uid).content[0].content[1].kind.0,
        "table_header_cell"
    );
}

#[test]
fn columns_keep_widths_and_selected_cell_settings() {
    let (mut engine, uid) = table_engine(2, 2);
    engine.run(Command::UpdateNodeAttrs {
        uid: uid.clone(),
        attrs: attrs(json!({"column_widths":[70,130]})),
        add_to_history: true,
    });
    engine.run(Command::Table(TableCommand::DuplicateAxis {
        axis: Axis::Column,
        index: 1,
    }));
    assert_eq!(
        table(&engine, &uid).attrs["column_widths"],
        json!([70, 130, 130])
    );
    engine.run(Command::Table(TableCommand::InsertAxis {
        axis: Axis::Column,
        index: 0,
        after: false,
    }));
    assert_eq!(
        table(&engine, &uid).attrs["column_widths"],
        json!([100, 70, 130, 130])
    );
    engine.run(Command::SetSelection(EditorSelection::Cells {
        table: uid.clone(),
        anchor: (0, 1),
        head: (1, 2),
    }));
    engine.run(Command::Table(TableCommand::SetAlignment("right".into())));
    engine.run(Command::Table(TableCommand::SetBackground(Some(
        "blue".into(),
    ))));
    engine.run(Command::Table(TableCommand::Clear));
    let grid = table(&engine, &uid);
    assert_eq!(grid.content[1].content[2].attrs["alignment"], "right");
    assert_eq!(grid.content[1].content[2].attrs["background_color"], "blue");
    assert!(grid.content[1].content[2].content.is_empty());
    assert_eq!(grid.content[0].content[3].text_content(), "0:1");
    engine.run(Command::Table(TableCommand::DeleteAxis {
        axis: Axis::Column,
        index: 1,
    }));
    assert_eq!(
        table(&engine, &uid).attrs["column_widths"],
        json!([100, 130, 130])
    );
}

#[test]
fn tab_navigates_without_history_and_adds_one_row_at_end() {
    let (mut engine, uid) = table_engine(1, 2);
    assert_eq!(
        changes(&engine.run(Command::Table(TableCommand::Navigate { backwards: false }))),
        0
    );
    assert!(!engine.can_undo());
    assert_eq!(
        changes(&engine.run(Command::Table(TableCommand::Navigate { backwards: false }))),
        1
    );
    assert_eq!(table(&engine, &uid).content.len(), 2);
    engine.run(Command::Undo);
    assert_eq!(table(&engine, &uid).content.len(), 1);
}

#[test]
fn deleting_final_axis_removes_table_and_undo_restores_it() {
    let (mut engine, uid) = table_engine(1, 2);
    engine.run(Command::Table(TableCommand::DeleteAxis {
        axis: Axis::Row,
        index: 0,
    }));
    assert!(model::content(engine.document(), &uid).is_none());
    engine.run(Command::Undo);
    assert!(model::content(engine.document(), &uid).is_some());
}

#[test]
fn image_settings_validate_and_transients_never_persist() {
    let placeholders = file_placeholders(&[FileMetadata {
        name: "photo.png".into(),
        mime: "image/png".into(),
        size: 123,
    }]);
    let uid = placeholders[0].uid.clone();
    let mut engine = EditorEngine::new(
        model::document(vec![placeholders[0].node.clone()]),
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap();
    let original = engine.document().clone();
    let output = engine.run(Command::SetTransientAttrs {
        uid: uid.clone(),
        attrs: placeholders[0].transient.clone(),
    });
    assert_eq!(changes(&output), 0);
    assert_eq!(engine.document(), &original);
    assert!(!engine.can_undo());
    assert!(engine.transient(&uid).is_some());
    assert!(
        engine
            .run(Command::Image {
                uid: uid.clone(),
                attrs: attrs(json!({"media":{"type":"url","url":"javascript:bad"}}))
            })
            .is_empty()
    );
    engine.run(Command::Image { uid: uid.clone(), attrs: attrs(json!({"width":50,"alignment":"center","alt":"Photo","media":{"type":"url","url":"https://example.com/a.png"}})) });
    assert_eq!(table(&engine, &uid).attrs["width"], 50);
    assert!(
        !serde_json::to_string(engine.document())
            .unwrap()
            .contains("upload")
    );
    engine.run(Command::Undo);
    assert_eq!(engine.document(), &original);
    assert!(engine.transient(&uid).is_some());
    engine.run(Command::SetTransientAttrs {
        uid: uid.clone(),
        attrs: AttributeMap::new(),
    });
    assert!(engine.transient(&uid).is_none());
    let oversized = file_placeholders(&[FileMetadata {
        name: "large.pdf".into(),
        mime: "application/pdf".into(),
        size: MAX_PASTED_FILE_BYTES + 1,
    }]);
    assert_eq!(oversized[0].node.content[0].kind.0, "file");
    assert_eq!(oversized[0].transient["upload"]["status"], "failed");
}

#[test]
fn host_attr_updates_fold_and_collapse_stays_out_of_history() {
    let block = model::block_node(ComponentNode::new("graph"));
    let uid = block.id.as_ref().unwrap().0.clone();
    let mut engine = EditorEngine::new(
        model::document(vec![block]),
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap();
    let initial = engine.document().clone();
    engine.run(Command::UpdateNodeAttrs {
        uid: uid.clone(),
        attrs: attrs(json!({"title":"first"})),
        add_to_history: true,
    });
    engine.run(Command::UpdateNodeAttrs {
        uid: uid.clone(),
        attrs: attrs(json!({"title":"second","nodes":[]})),
        add_to_history: false,
    });
    engine.run(Command::Undo);
    assert_eq!(engine.document(), &initial);
    assert!(!engine.can_undo());
    engine.run(Command::SetCollapsed(vec![uid.clone()]));
    assert!(engine.is_collapsed(&uid));
    assert!(!engine.can_undo());
    assert_eq!(engine.document(), &initial);
    let output = engine.run(Command::ToggleCollapsed(uid.clone()));
    assert!(!engine.is_collapsed(&uid));
    assert!(output.iter().any(|output| matches!(output, EngineOutput::Request(dxeditor::EngineEvent::ToggleCollapsed { uid: target, collapsed: false, .. }) if target == &uid)));
}
