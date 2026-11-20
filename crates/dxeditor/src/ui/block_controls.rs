use super::controls::{MenuGroup, MenuRow, MenuSeparator, SwatchRow, turn_into_options};
use super::*;

const GUTTER_HEIGHT: f64 = 24.;
const GUTTER_OFFSET: f64 = 56.;
#[component]
pub(super) fn BlockControls(
    ui: EditorUiState,
    handle: EngineHandle,
    catalog: EditorCatalog,
) -> Element {
    // Graph nodes have their own structural controls outside this nested editor.
    if ui.read_only
        || ui.composing
        || ui.profile == crate::engine::Profile::GraphNode
        || catalog.component_specs().spec_by_id("heading").is_none()
    {
        return rsx! {};
    }
    let Some(uid) = ui
        .hover_block
        .clone()
        .or_else(|| ui.focused.then(|| ui.selection.uid.clone()).flatten())
    else {
        return rsx! {};
    };
    let Some(line) = ui.geometry.block_line.or(ui.geometry.block) else {
        return rsx! {};
    };
    let sx = if ui.geometry.scale_x > 0.0 {
        ui.geometry.scale_x
    } else {
        1.
    };
    let sy = if ui.geometry.scale_y > 0.0 {
        ui.geometry.scale_y
    } else {
        1.
    };
    let placement = format!(
        "left:{}px;top:{}px;transform:scale({},{});transform-origin:top left",
        line.x - GUTTER_OFFSET / sx,
        (line.y + (line.height.min(48. / sy) - GUTTER_HEIGHT / sy) / 2.).max(0.0),
        1.0 / sx,
        1.0 / sy
    );
    let add_uid = uid.clone();
    let add_handle = handle.clone();
    let menu_uid = uid.clone();
    let menu_handle = handle.clone();
    rsx! {div{class:"dxeditor-gutter dxeditor-engine__block-controls",style:"{placement}",
        button{r#type:"button",class:"dxeditor-engine__button",title:"Insert block (Alt: above)",aria_label:"Insert block",onmousedown:move|e|e.prevent_default(),onclick:move|event|{
            let block=crate::engine::model::paragraph();let new_uid=block.id.as_ref().unwrap().0.clone();
            add_handle.run(Command::InsertBlocks{at:crate::InsertAt::AfterBlock{uid:add_uid.clone()},nodes:vec![block]});
            if event.modifiers().alt(){add_handle.run(Command::MoveBlocks{uids:vec![new_uid.clone()],target:add_uid.clone(),placement:crate::engine::commands::BlockPlacement::Before});}
            add_handle.run(Command::SetSelection(EditorSelection::caret(InlineContainerRef::Block(new_uid),0)));add_handle.run(Command::Focus);add_handle.run(Command::OpenMenu("insert".into()));
            },icons::Icon{name:"plus"}}
            // Cancelling mousedown here prevents native dragging in browsers.
            button{r#type:"button",class:"dxeditor-engine__button dxeditor-block-grip",title:"Drag to move · Click for actions",aria_label:"Block actions","aria-description":"Drag to reorder. Click for block actions, including Move up and Move down.",aria_haspopup:"menu",draggable:true,"data-dxeditor-drag":"{uid}","data-uid":"{uid}",onclick:move |_|{menu_handle.run(Command::OpenMenu("block".into()));menu_handle.run(Command::SelectNode(menu_uid.clone()));},
                svg{width:"12",height:"18",view_box:"0 0 12 18",fill:"currentColor","aria-hidden":"true","focusable":"false",
                    for (x,y) in [(3,4),(9,4),(3,9),(9,9),(3,14),(9,14)]{circle{cx:"{x}",cy:"{y}",r:"1.35"}}
                }
            }
    }}
}
#[component]
pub(super) fn BlockMenu(
    ui: EditorUiState,
    handle: EngineHandle,
    catalog: EditorCatalog,
) -> Element {
    let Some(uid) = ui.selection.uid.clone() else {
        return rsx! {};
    };
    let sx = if ui.geometry.scale_x > 0.0 {
        ui.geometry.scale_x
    } else {
        1.0
    };
    let anchor = ui
        .geometry
        .block_line
        .or(ui.geometry.block)
        .map(|line| Rect {
            x: line.x - GUTTER_OFFSET / sx,
            ..line
        });
    let placement = style(
        &ui,
        anchor,
        Size {
            width: 284.,
            height: 300.,
        },
        Side::Bottom,
    );
    let keys = handle.clone();
    let kind = ui.selection.kind.as_deref();
    let textual = kind.is_some_and(crate::engine::model::is_text_kind);
    let run = |command: Command| {
        let handle = handle.clone();
        move |_| {
            handle.run(command.clone());
            handle.close();
        }
    };
    let options = turn_into_options(&catalog);
    rsx! {
        div {
            class: "dxeditor-engine__surface dxeditor-engine__block-menu",
            role: "menu",
            aria_label: "Block actions",
            style: "{placement}",
            onmounted: focus_first,
            onfocusin: roving_focus,
            onkeydown: move |event| menu_keys(event, keys.clone()),
            if textual {
                MenuGroup { label: "Turn into", icon: "turn_into", name: "dxeditor-block-menu",
                    for option in options {
                        MenuRow {
                            label: option.label.clone(),
                            icon: option.icon.clone(),
                            shortcut: option.shortcut,
                            checked: Some(option.matches(kind, &ui.selection.attrs)),
                            radio: true,
                            onclick: run(Command::TurnInto { kind: option.kind, attrs: option.attrs }),
                        }
                    }
                }
                if ui.profile.allows_colors() && catalog.component_specs().spec_by_id("highlight").is_some() {
                    MenuGroup { label: "Colour", icon: "colour", name: "dxeditor-block-menu",
                        for (key, label, filled) in [("text_color", "Text colour", false), ("background_color", "Background", true)] {
                            SwatchRow {
                                label,
                                filled,
                                current: ui.selection.attrs.get(key).and_then(serde_json::Value::as_str).map(str::to_owned),
                                onpick: {
                                    let handle = handle.clone();
                                    move |color| {
                                        handle.run(Command::SetColor { key: key.into(), color });
                                        handle.close();
                                    }
                                },
                            }
                        }
                    }
                }
                MenuSeparator {}
            }
            MenuRow { label: "Duplicate", icon: "duplicate", onclick: run(Command::Duplicate) }
            MenuRow { label: "Copy link to block", icon: "link", onclick: run(Command::Request(EngineEvent::CopyBlockLink { uid: uid.clone() })) }
            MenuRow { label: "Move to…", icon: "move_to", onclick: {
                let handle = handle.clone();
                let uid = uid.clone();
                move |_| handle.run(Command::Request(EngineEvent::MoveToRequest { uids: vec![uid.clone()] }))
            } }
            MenuRow { label: "Move up", icon: "arrow_up", shortcut: "Alt+Shift+ArrowUp", onclick: run(Command::MoveUp) }
            MenuRow { label: "Move down", icon: "arrow_down", shortcut: "Alt+Shift+ArrowDown", onclick: run(Command::MoveDown) }
            if kind == Some("todo") {
                MenuRow { label: "Convert to task", icon: "todo", onclick: {
                    let handle = handle.clone();
                    let state = ui.clone();
                    move |_| handle.create(&state, "task", String::new())
                } }
            }
            MenuSeparator {}
            MenuRow { label: "Delete", icon: "delete", danger: true, onclick: run(Command::DeleteBlocks) }
        }
    }
}
