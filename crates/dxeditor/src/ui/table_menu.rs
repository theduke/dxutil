use super::controls::{MenuGroup, MenuRow, MenuSeparator, SwatchRow, mount_menu};
use super::*;

/// Quiet controls on the edges of the focused table: a handle for the
/// current row and column, the outline of the current cell, and rails that
/// append a row or column. They sit on the table's border and in the space the
/// table block reserves below it, never over neighbouring blocks.
#[component]
pub(super) fn TableEdges(ui: EditorUiState, handle: EngineHandle) -> Element {
    let Some(table) = ui.table.clone() else {
        return rsx! {};
    };
    let Some(rect) = ui.geometry.table.or(ui.geometry.block) else {
        return rsx! {};
    };
    let placement = format!(
        "left:{}px;top:{}px;width:{}px;height:{}px",
        rect.x, rect.y, rect.width, rect.height
    );
    let rows = table.rows;
    let cols = table.cols;
    let row_menu = ui.open_menu.as_deref() == Some("table-row");
    let column_menu = ui.open_menu.as_deref() == Some("table-column");
    let (row, col) = if !row_menu && !column_menu && table.anchor == table.head {
        ui.geometry.hovered_cell.unwrap_or(table.head)
    } else {
        table.head
    };
    let row_center = ui
        .geometry
        .table_rows
        .get(row)
        .map(|r| r.y - rect.y + r.height / 2.)
        .unwrap_or((row as f64 + 0.5) * rect.height / rows.max(1) as f64);
    let column_center = ui
        .geometry
        .table_columns
        .get(col)
        .map(|r| r.x - rect.x + r.width / 2.)
        .unwrap_or((col as f64 + 0.5) * rect.width / cols.max(1) as f64);
    let single_cell = table.anchor == table.head;
    // A whole selected row or column keeps only its own handle, so the other
    // one doesn't jump to the selection's far end.
    let (top, bottom) = (
        table.anchor.0.min(table.head.0),
        table.anchor.0.max(table.head.0),
    );
    let (left, right) = (
        table.anchor.1.min(table.head.1),
        table.anchor.1.max(table.head.1),
    );
    let whole_row = top == bottom && left == 0 && right + 1 == cols && cols > 1;
    let whole_column = left == right && top == 0 && bottom + 1 == rows && rows > 1;
    let show_row = !column_menu && !whole_column;
    let show_column = !row_menu && !whole_row;
    let keys = handle.clone();
    let open_row = handle.clone();
    let open_column = handle.clone();
    let add_row = handle.clone();
    let add_column = handle.clone();
    let row_uid = table.uid.clone();
    let column_uid = table.uid.clone();
    rsx! {
        div {
            class: "dxeditor-table-edges",
            role: "toolbar",
            aria_label: "Table editing",
            style: "{placement}",
            onmounted: init_roving,
            onfocusin: roving_focus,
            onkeydown: move |event| menu_keys(event, keys.clone()),
            if let Some(cell) = ui.geometry.cell.filter(|_| single_cell) {
                div {
                    class: "dxeditor-table-cell-outline",
                    style: "left:{cell.x - rect.x}px;top:{cell.y - rect.y}px;width:{cell.width}px;height:{cell.height}px",
                }
            }
            if show_row { button {
                r#type: "button",
                class: "dxeditor-engine__button dxeditor-table-handle dxeditor-table-handle--row",
                aria_label: "Row {row + 1} actions",
                aria_haspopup: "menu",
                aria_expanded: row_menu.to_string(),
                draggable: true,
                "data-table-row-drag": "{row}",
                "data-table-uid": "{table.uid}",
                style: "top:{row_center}px",
                onmousedown: move |event| event.prevent_default(),
                onclick: move |_| {
                    open_row.run(Command::SetSelection(EditorSelection::Cells {
                        table: row_uid.clone(),
                        anchor: (row, 0),
                        head: (row, cols.saturating_sub(1)),
                    }));
                    open_row.run(Command::OpenMenu("table-row".into()));
                },
                GripIcon { vertical: true }
            } }
            if show_column { button {
                r#type: "button",
                class: "dxeditor-engine__button dxeditor-table-handle dxeditor-table-handle--column",
                aria_label: "Column {col + 1} actions",
                aria_haspopup: "menu",
                aria_expanded: column_menu.to_string(),
                style: "left:{column_center}px",
                onmousedown: move |event| event.prevent_default(),
                onclick: move |_| {
                    open_column.run(Command::SetSelection(EditorSelection::Cells {
                        table: column_uid.clone(),
                        anchor: (0, col),
                        head: (rows.saturating_sub(1), col),
                    }));
                    open_column.run(Command::OpenMenu("table-column".into()));
                },
                GripIcon { vertical: false }
            } }
            button {
                r#type: "button",
                class: "dxeditor-engine__button dxeditor-table-rail dxeditor-table-rail--row",
                aria_label: "Add row at bottom",
                title: "Add row",
                onmousedown: move |event| event.prevent_default(),
                onclick: move |_| {
                    add_row.run(Command::Table(TableCommand::InsertAxis {
                        axis: Axis::Row,
                        index: rows.saturating_sub(1),
                        after: true,
                    }))
                },
                icons::Icon { name: "plus" }
            }
            button {
                r#type: "button",
                class: "dxeditor-engine__button dxeditor-table-rail dxeditor-table-rail--column",
                aria_label: "Add column at right",
                title: "Add column",
                onmousedown: move |event| event.prevent_default(),
                onclick: move |_| {
                    add_column.run(Command::Table(TableCommand::InsertAxis {
                        axis: Axis::Column,
                        index: cols.saturating_sub(1),
                        after: true,
                    }))
                },
                icons::Icon { name: "plus" }
            }
        }
    }
}

/// Six dots, laid out along the row or column the handle selects.
#[component]
fn GripIcon(vertical: bool) -> Element {
    let dots: [(f64, f64); 6] = if vertical {
        [(3., 3.), (7., 3.), (3., 8.), (7., 8.), (3., 13.), (7., 13.)]
    } else {
        [(3., 3.), (8., 3.), (13., 3.), (3., 7.), (8., 7.), (13., 7.)]
    };
    let (width, height) = if vertical { (10, 16) } else { (16, 10) };
    rsx! {
        svg {
            width: "{width}",
            height: "{height}",
            view_box: "0 0 {width} {height}",
            fill: "currentColor",
            "aria-hidden": "true",
            "focusable": "false",
            for (x , y) in dots {
                circle { cx: "{x}", cy: "{y}", r: "1.3" }
            }
        }
    }
}

/// The actions of a table row or column, opened from its handle.
#[component]
pub(super) fn TableMenu(ui: EditorUiState, handle: EngineHandle) -> Element {
    let Some(table) = ui.table.clone() else {
        return rsx! {};
    };
    let axis = if ui.open_menu.as_deref() == Some("table-column") {
        Axis::Column
    } else {
        Axis::Row
    };
    let row = axis == Axis::Row;
    let index = if row { table.anchor.0 } else { table.anchor.1 };
    // Anchor in wrapper coordinates while the menu itself renders at screen scale.
    let sx = ui.geometry.scale_x;
    let sy = ui.geometry.scale_y;
    let anchor = ui.geometry.table.map(|rect| {
        let handle = if row {
            ui.geometry.table_rows.get(index).map(|r| Rect {
                x: rect.x - 7. / sx,
                y: r.y + r.height / 2. - 11. / sy,
                width: 14. / sx,
                height: 22. / sy,
            })
        } else {
            ui.geometry.table_columns.get(index).map(|c| Rect {
                x: c.x + c.width / 2. - 11. / sx,
                y: rect.y - 7. / sy,
                width: 22. / sx,
                height: 14. / sy,
            })
        };
        handle.unwrap_or(rect)
    });
    let placement = style(
        &ui,
        anchor.or(ui.geometry.block),
        Size {
            width: 260.,
            height: 420.,
        },
        Side::Bottom,
    );
    let run = move |handle: &EngineHandle, command: TableCommand| {
        let handle = handle.clone();
        move |_| {
            handle.run(Command::Table(command.clone()));
            handle.close();
        }
    };
    let keys = handle.clone();
    rsx! {
        div {
            class: "dxeditor-engine__surface dxeditor-engine__table-menu",
            role: "menu",
            aria_label: if row { "Row actions" } else { "Column actions" },
            style: "{placement}",
            onmounted: mount_menu,
            onfocusin: roving_focus,
            onkeydown: move |event| menu_keys(event, keys.clone()),
            MenuRow {
                label: "Header row",
                icon: "header_row",
                checked: Some(table.header_row),
                onclick: run(&handle, TableCommand::ToggleHeader(Axis::Row)),
            }
            if ui.profile.allows_colors() {
                MenuRow {
                    label: "Header column",
                    icon: "header_column",
                    checked: Some(table.header_column),
                    onclick: run(&handle, TableCommand::ToggleHeader(Axis::Column)),
                }
            }
            MenuSeparator {}
            MenuRow {
                label: if row { "Insert above" } else { "Insert left" },
                icon: if row { "insert_above" } else { "insert_left" },
                onclick: run(&handle, TableCommand::InsertAxis { axis, index, after: false }),
            }
            MenuRow {
                label: if row { "Insert below" } else { "Insert right" },
                icon: if row { "insert_below" } else { "insert_right" },
                onclick: run(&handle, TableCommand::InsertAxis { axis, index, after: true }),
            }
            MenuRow {
                label: "Duplicate",
                icon: "duplicate",
                onclick: run(&handle, TableCommand::DuplicateAxis { axis, index }),
            }
            if row && index > 0 {
                MenuRow {
                    label: "Move up",
                    icon: "arrow_up",
                    onclick: run(&handle, TableCommand::MoveRow { from: index, to: index - 1 }),
                }
            }
            if row && index + 1 < table.rows {
                MenuRow {
                    label: "Move down",
                    icon: "arrow_down",
                    onclick: run(&handle, TableCommand::MoveRow { from: index, to: index + 1 }),
                }
            }
            MenuSeparator {}
            if ui.profile.allows_colors() {
                MenuGroup { label: "Colour", icon: "colour", name: "dxeditor-table-menu",
                    SwatchRow {
                        label: "Background",
                        filled: true,
                        current: None,
                        onpick: {
                            let handle = handle.clone();
                            move |color: Option<String>| {
                                handle.run(Command::Table(TableCommand::SetBackground(color)));
                                handle.close();
                            }
                        },
                    }
                }
            }
            MenuGroup { label: "Alignment", icon: "align_left", name: "dxeditor-table-menu",
                for (alignment , label) in [("left", "Left"), ("center", "Centre"), ("right", "Right")] {
                    MenuRow {
                        label,
                        icon: format!("align_{alignment}"),
                        onclick: run(&handle, TableCommand::SetAlignment(alignment.into())),
                    }
                }
            }
            MenuRow {
                label: "Clear contents",
                icon: "clear",
                onclick: run(&handle, TableCommand::Clear),
            }
            MenuSeparator {}
            MenuRow {
                label: if row { "Delete row" } else { "Delete column" },
                icon: "delete",
                danger: true,
                onclick: run(&handle, TableCommand::DeleteAxis { axis, index }),
            }
            MenuRow {
                label: "Delete table",
                icon: "delete",
                danger: true,
                onclick: run(&handle, TableCommand::Delete),
            }
        }
    }
}
