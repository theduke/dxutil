//! Rectangular table operations. A grid edit replaces one table content node,
//! making paste, growth and header repair a single undoable transaction.
use super::{Axis, Command, TableCommand, blocks};
use crate::engine::{
    EditorEngine, EngineOutput, Profile, identity, model,
    selection::{EditorSelection, InlineContainerRef, Uid},
    transaction::TxKind,
};
use crate::{ComponentNode, InsertAt, NodeId};
use serde_json::{Value, json};

pub const MAX_TABLE_ROWS: usize = 100;
pub const MAX_TABLE_COLUMNS: usize = 100;

#[derive(Clone, Debug)]
pub struct TableContext {
    pub uid: Uid,
    pub row: usize,
    pub column: usize,
    /// Inclusive top/left/bottom/right coordinates.
    pub rect: (usize, usize, usize, usize),
}

pub fn context(engine: &EditorEngine) -> Option<TableContext> {
    let uid = blocks::current_uid(engine)?;
    let table = engine.content(&uid).filter(|node| node.kind.0 == "table")?;
    let (row, column, rect) = match &engine.selection {
        EditorSelection::Cells { anchor, head, .. } => {
            let rect = (
                anchor.0.min(head.0),
                anchor.1.min(head.1),
                anchor.0.max(head.0),
                anchor.1.max(head.1),
            );
            (rect.0, rect.1, rect)
        }
        EditorSelection::Text(range) => {
            let InlineContainerRef::Cell { row, col } = &range.head.container else {
                return None;
            };
            let row = table
                .content
                .iter()
                .position(|node| node.id.as_ref().is_some_and(|id| &id.0 == row))?;
            (row, *col, (row, *col, row, *col))
        }
        EditorSelection::Node(_) | EditorSelection::Blocks { .. } => (0, 0, (0, 0, 0, 0)),
    };
    if table.content.get(rect.2)?.content.get(rect.3).is_none() {
        return None;
    }
    Some(TableContext {
        uid,
        row,
        column,
        rect,
    })
}

fn header(table: &ComponentNode, row: usize, column: usize) -> bool {
    (row == 0
        && table
            .attrs
            .get("header_row")
            .and_then(Value::as_bool)
            .unwrap_or(false))
        || (column == 0
            && table
                .attrs
                .get("header_column")
                .and_then(Value::as_bool)
                .unwrap_or(false))
}

fn repair_headers(table: &mut ComponentNode) {
    let row_header = table
        .attrs
        .get("header_row")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let column_header = table
        .attrs
        .get("header_column")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    for (row, node) in table.content.iter_mut().enumerate() {
        for (column, cell) in node.content.iter_mut().enumerate() {
            cell.kind = if (row_header && row == 0) || (column_header && column == 0) {
                "table_header_cell"
            } else {
                "table_cell"
            }
            .into();
        }
    }
}

fn cell(table: &ComponentNode, row: usize, column: usize) -> ComponentNode {
    ComponentNode::new(if header(table, row, column) {
        "table_header_cell"
    } else {
        "table_cell"
    })
}
fn row(table: &ComponentNode, index: usize, width: usize) -> ComponentNode {
    ComponentNode::container(
        "table_row",
        Some(NodeId::new(identity::new_uid())),
        (0..width)
            .map(|column| cell(table, index, column))
            .collect(),
    )
}
fn caret(table: &ComponentNode, row: usize, column: usize) -> Option<EditorSelection> {
    let row = row.min(table.content.len().checked_sub(1)?);
    let node = table.content.get(row)?;
    let column = column.min(node.content.len().checked_sub(1)?);
    Some(EditorSelection::caret(
        InlineContainerRef::Cell {
            row: node.id.as_ref()?.0.clone(),
            col: column,
        },
        0,
    ))
}
fn unmerged(table: &ComponentNode) -> bool {
    table.content.iter().all(|row| {
        row.content.iter().all(|cell| {
            ["colspan", "rowspan"].iter().all(|key| {
                cell.attrs
                    .get(*key)
                    .is_none_or(|value| value.as_u64() == Some(1))
            })
        })
    })
}
fn widths_mut(table: &mut ComponentNode) -> Option<&mut Vec<Value>> {
    table.attrs.get_mut("column_widths")?.as_array_mut()
}

pub fn run(engine: &mut EditorEngine, command: TableCommand) -> Vec<EngineOutput> {
    if engine.options.read_only || !engine.schema.has_kind("table") {
        return Vec::new();
    }
    if let TableCommand::Insert {
        rows,
        columns,
        header_row,
    } = command
    {
        if rows == 0 || columns == 0 || rows > MAX_TABLE_ROWS || columns > MAX_TABLE_COLUMNS {
            return Vec::new();
        }
        let mut table = ComponentNode::new("table");
        table.attrs.insert("header_row".into(), json!(header_row));
        table.attrs.insert("header_column".into(), json!(false));
        for index in 0..rows {
            table.content.push(row(&table, index, columns));
        }
        let selection = caret(&table, 0, 0);
        let mut output =
            blocks::insert_blocks(engine, InsertAt::Selection, vec![model::block_node(table)]);
        if let Some(selection) = selection {
            output.extend(blocks::select(engine, selection));
        }
        return output;
    }
    let Some(target) = context(engine) else {
        return Vec::new();
    };
    let Some(mut table) = engine.content(&target.uid).cloned() else {
        return Vec::new();
    };
    if !unmerged(&table) {
        return Vec::new();
    }
    let rows = table.content.len();
    let columns = table.content.first().map_or(0, |row| row.content.len());
    if rows == 0 || columns == 0 || table.content.iter().any(|row| row.content.len() != columns) {
        return Vec::new();
    }
    let mut selection = engine.selection.clone();
    let mut kind = TxKind::Structural;
    match command {
        TableCommand::Insert { .. } => unreachable!(),
        TableCommand::Delete => return engine.run(Command::RemoveNode(target.uid)),
        TableCommand::MoveRow { from, to } => {
            if from >= rows || to >= rows || from == to {
                return Vec::new();
            }
            let moved = table.content.remove(from);
            table.content.insert(to, moved);
            repair_headers(&mut table);
            selection = EditorSelection::Cells {
                table: target.uid.clone(),
                anchor: (to, 0),
                head: (to, columns - 1),
            };
        }
        TableCommand::InsertAxis { axis, index, after } => {
            let count = if axis == Axis::Row { rows } else { columns };
            if index >= count
                || count
                    >= if axis == Axis::Row {
                        MAX_TABLE_ROWS
                    } else {
                        MAX_TABLE_COLUMNS
                    }
            {
                return Vec::new();
            }
            let at = index + usize::from(after);
            if axis == Axis::Row {
                let new_row = row(&table, at, columns);
                table.content.insert(at, new_row);
            } else {
                for (row_index, row) in table.content.iter_mut().enumerate() {
                    row.content.insert(
                        at,
                        ComponentNode::new(
                            if row_index == 0
                                && table
                                    .attrs
                                    .get("header_row")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false)
                            {
                                "table_header_cell"
                            } else {
                                "table_cell"
                            },
                        ),
                    );
                }
                if let Some(widths) = widths_mut(&mut table) {
                    widths.resize(columns, json!(100));
                    widths.insert(at, json!(100));
                }
            }
            repair_headers(&mut table);
            selection = caret(
                &table,
                if axis == Axis::Row { at } else { target.row },
                if axis == Axis::Column {
                    at
                } else {
                    target.column
                },
            )
            .unwrap_or(selection);
        }
        TableCommand::DeleteAxis { axis, index } => {
            let count = if axis == Axis::Row { rows } else { columns };
            if index >= count {
                return Vec::new();
            }
            if count == 1 {
                return engine.run(Command::RemoveNode(target.uid));
            }
            if axis == Axis::Row {
                table.content.remove(index);
            } else {
                for row in &mut table.content {
                    row.content.remove(index);
                }
                if let Some(widths) = widths_mut(&mut table) {
                    if index < widths.len() {
                        widths.remove(index);
                    }
                }
            }
            repair_headers(&mut table);
            selection = caret(
                &table,
                if axis == Axis::Row { index } else { target.row },
                if axis == Axis::Column {
                    index
                } else {
                    target.column
                },
            )
            .unwrap_or(selection);
        }
        TableCommand::DuplicateAxis { axis, index } => {
            let count = if axis == Axis::Row { rows } else { columns };
            if index >= count
                || count
                    >= if axis == Axis::Row {
                        MAX_TABLE_ROWS
                    } else {
                        MAX_TABLE_COLUMNS
                    }
            {
                return Vec::new();
            }
            if axis == Axis::Row {
                let mut new_row = table.content[index].clone();
                new_row.id = Some(NodeId::new(identity::new_uid()));
                table.content.insert(index + 1, new_row);
            } else {
                for row in &mut table.content {
                    let copy = row.content[index].clone();
                    row.content.insert(index + 1, copy);
                }
                if let Some(widths) = widths_mut(&mut table) {
                    widths.resize(columns, json!(100));
                    let width = widths[index].clone();
                    widths.insert(index + 1, width);
                }
            }
            repair_headers(&mut table);
            selection = caret(&table, target.row, target.column).unwrap_or(selection);
        }
        TableCommand::ToggleHeader(axis) => {
            if engine.profile == Profile::MarkdownNotes && axis == Axis::Column {
                return Vec::new();
            }
            let key = if axis == Axis::Row {
                "header_row"
            } else {
                "header_column"
            };
            let value = table
                .attrs
                .get(key)
                .and_then(Value::as_bool)
                .unwrap_or(false);
            table.attrs.insert(key.into(), json!(!value));
            repair_headers(&mut table);
        }
        TableCommand::SetAlignment(alignment) => {
            if !matches!(alignment.as_str(), "left" | "center" | "right") {
                return Vec::new();
            }
            for_selected(&mut table, target.rect, |cell| {
                cell.attrs.insert("alignment".into(), json!(alignment));
            });
        }
        TableCommand::SetBackground(color) => {
            if !engine.profile.allows_colors()
                || color.as_ref().is_some_and(|color| {
                    !crate::profiles::block_document::COLOR_TOKENS.contains(&color.as_str())
                })
            {
                return Vec::new();
            }
            for_selected(&mut table, target.rect, |cell| {
                if let Some(color) = &color {
                    cell.attrs.insert("background_color".into(), json!(color));
                } else {
                    cell.attrs.remove("background_color");
                }
            });
        }
        TableCommand::Clear => for_selected(&mut table, target.rect, |cell| cell.content.clear()),
        TableCommand::Navigate { backwards } => {
            let current = target.row * columns + target.column;
            if backwards {
                if current == 0 {
                    return Vec::new();
                }
                let at = current - 1;
                return blocks::select(
                    engine,
                    caret(&table, at / columns, at % columns).unwrap_or(selection),
                );
            }
            if current + 1 < rows * columns {
                let at = current + 1;
                return blocks::select(
                    engine,
                    caret(&table, at / columns, at % columns).unwrap_or(selection),
                );
            }
            if rows >= MAX_TABLE_ROWS {
                return Vec::new();
            }
            let new_row = row(&table, rows, columns);
            table.content.push(new_row);
            selection = caret(&table, rows, 0).unwrap_or(selection);
        }
        TableCommand::PasteGrid(grid) => {
            return run(
                engine,
                TableCommand::PasteCells(
                    grid.into_iter()
                        .map(|row| row.into_iter().map(|text| text_inline(&text)).collect())
                        .collect(),
                ),
            );
        }
        TableCommand::PasteCells(grid) => {
            let height = grid.len();
            let width = grid.first().map_or(0, Vec::len);
            if height == 0
                || width == 0
                || height > MAX_TABLE_ROWS
                || width > MAX_TABLE_COLUMNS
                || grid.iter().any(|row| {
                    row.len() != width
                        || row.iter().any(|nodes| {
                            nodes.iter().any(|node| {
                                !matches!(
                                    node.kind.0.as_str(),
                                    "text" | "hard_break" | "mention" | "opaque_markdown_inline"
                                )
                            }) || super::super::text::len(nodes)
                                > super::super::clipboard::MAX_CLIPBOARD_TEXT
                        })
                })
            {
                return Vec::new();
            }
            let height = height.max(target.rect.2 - target.rect.0 + 1);
            let width = width.max(target.rect.3 - target.rect.1 + 1);
            let Some(required_rows) = target.row.checked_add(height) else {
                return Vec::new();
            };
            let Some(required_columns) = target.column.checked_add(width) else {
                return Vec::new();
            };
            if required_rows > MAX_TABLE_ROWS || required_columns > MAX_TABLE_COLUMNS {
                return Vec::new();
            }
            let row_count = rows.max(required_rows);
            let column_count = columns.max(required_columns);
            for row_index in 0..row_count {
                if row_index >= table.content.len() {
                    let new_row = row(&table, row_index, column_count);
                    table.content.push(new_row);
                } else {
                    while table.content[row_index].content.len() < column_count {
                        let column = table.content[row_index].content.len();
                        let new_cell = cell(&table, row_index, column);
                        table.content[row_index].content.push(new_cell);
                    }
                }
                for column in target.column..target.column + width {
                    if row_index >= target.row && row_index < target.row + height {
                        let value = &grid[(row_index - target.row) % grid.len()]
                            [(column - target.column) % grid[0].len()];
                        table.content[row_index].content[column].content = value.clone();
                    }
                }
            }
            if let Some(widths) = widths_mut(&mut table) {
                widths.resize(column_count, json!(100));
            }
            repair_headers(&mut table);
            selection = caret(&table, target.row, target.column).unwrap_or(selection);
            kind = TxKind::Paste;
        }
    }
    blocks::set_content(engine, &target.uid, table, selection, kind)
}

fn for_selected(
    table: &mut ComponentNode,
    rect: (usize, usize, usize, usize),
    mut f: impl FnMut(&mut ComponentNode),
) {
    for row in table.content.iter_mut().take(rect.2 + 1).skip(rect.0) {
        for cell in row.content.iter_mut().take(rect.3 + 1).skip(rect.1) {
            f(cell);
        }
    }
}

pub fn text_inline(text: &str) -> Vec<ComponentNode> {
    let mut nodes = Vec::new();
    for (index, line) in crate::engine::clipboard::normalize_line_endings(text)
        .split('\n')
        .enumerate()
    {
        if index > 0 {
            nodes.push(ComponentNode::new("hard_break"));
        }
        if !line.is_empty() {
            nodes.push(ComponentNode::text(line));
        }
    }
    nodes
}
