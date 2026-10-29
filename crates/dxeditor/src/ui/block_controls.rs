use super::*;
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
    let Some(rect) = ui.geometry.block else {
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
    let mobile = ui.geometry.viewport.width * sx <= 640.0 && ui.geometry.viewport.width > 0.0;
    let placement = format!(
        "left:{}px;top:{}px;transform:scale({},{});transform-origin:top left",
        rect.x - if mobile { 36.0 / sx } else { 68.0 / sx },
        rect.y.max(0.0),
        1.0 / sx,
        1.0 / sy
    );
    let add_uid = uid.clone();
    let add_handle = handle.clone();
    let menu_uid = uid.clone();
    let menu_handle = handle.clone();
    rsx! {div{class:"dxeditor-gutter",style:"{placement}",
        button{r#type:"button",class:"dxeditor-engine__button",title:"Insert block (Alt: above)",aria_label:"Insert block",onmousedown:move|e|e.prevent_default(),onclick:move|event|{
            let mut block=crate::engine::model::paragraph();let new_uid=block.id.as_ref().unwrap().0.clone();block.content[0].content=vec![crate::ComponentNode::text("/")];
            add_handle.run(Command::InsertBlocks{at:crate::InsertAt::AfterBlock{uid:add_uid.clone()},nodes:vec![block]});
            if event.modifiers().alt(){add_handle.run(Command::MoveBlocks{uids:vec![new_uid.clone()],target:add_uid.clone(),placement:crate::engine::commands::BlockPlacement::Before});}
            add_handle.run(Command::SetSelection(EditorSelection::caret(InlineContainerRef::Block(new_uid),1)));add_handle.run(Command::Focus);
        },"+"}
        button{r#type:"button",class:"dxeditor-engine__button",title:"Block actions",aria_label:"Block actions",aria_haspopup:"menu",draggable:true,"data-dxeditor-drag":"{uid}","data-uid":"{uid}",onmousedown:move|e|e.prevent_default(),onclick:move |_|{menu_handle.run(Command::SelectNode(menu_uid.clone()));menu_handle.run(Command::OpenMenu("block".into()));},"⠿"}
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
    let placement = style(
        &ui,
        ui.geometry.block,
        Size {
            width: 240.,
            height: 360.,
        },
        Side::Bottom,
    );
    let keys = handle.clone();
    let copy_uid = uid.clone();
    let move_uid = uid.clone();
    rsx! {div{class:"dxeditor-engine__surface dxeditor-engine__block-menu",role:"menu",onmounted:focus_first,onfocusin:roving_focus,aria_label:"Block actions",style:"{placement}",onkeydown:move|event|menu_keys(event,keys.clone()),
        details{summary{"Turn into"}div{role:"menu",aria_label:"Turn into",for spec in catalog.component_specs().specs().filter(|s|crate::engine::model::is_text_kind(&s.id.0)){MenuAction{label:spec.label.clone(),handle:handle.clone(),command:Command::TurnInto{kind:spec.id.0.clone(),attrs:AttributeMap::new()}}}}}
        if catalog.component_specs().spec_by_id("highlight").is_some(){details{summary{"Colour"}Colors{handle:handle.clone(),block:true}Colors{handle:handle.clone(),block:true,background:true}}}
        MenuAction{label:"Duplicate".to_string(),handle:handle.clone(),command:Command::Duplicate}
        if catalog.component_specs().spec_by_id("todo").is_some(){MenuAction{label:"Turn into to-do".to_string(),handle:handle.clone(),command:Command::TurnInto{kind:"todo".into(),attrs:attr(serde_json::json!({"checked":false}))}}}
        MenuAction{label:"Copy link to block".to_string(),handle:handle.clone(),command:Command::Request(EngineEvent::CopyBlockLink{uid:copy_uid})}
        MenuAction{label:"Move to…".to_string(),handle:handle.clone(),command:Command::Request(EngineEvent::MoveToRequest{uids:vec![move_uid]})}
        MenuAction{label:"Move up".to_string(),handle:handle.clone(),command:Command::MoveUp}
        MenuAction{label:"Move down".to_string(),handle:handle.clone(),command:Command::MoveDown}
        if ui.selection.kind.as_deref()==Some("todo"){button{r#type:"button",role:"menuitem",class:"dxeditor-engine__button",onmousedown:move|e|e.prevent_default(),onclick:{let h=handle.clone();let state=ui.clone();move |_|h.create(&state,"task",String::new())},"Convert to task"}}
        MenuAction{label:"Delete".to_string(),handle,command:Command::DeleteBlocks,destructive:true}
    }}
}
