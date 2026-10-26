use super::*;
#[component]
pub(super) fn EntityPreview(ui: EditorUiState, handle: EngineHandle) -> Element {
    let Some(entity) = ui.entity_preview.clone() else {
        return rsx! {};
    };
    let placement = style(
        &ui,
        ui.geometry.caret,
        Size {
            width: 300.,
            height: 160.,
        },
        Side::Bottom,
    );
    rsx! {div{class:"dxeditor-engine__surface dxeditor-engine__entity-preview",role:"dialog",aria_label:"Entity preview",style:"{placement}",
        if entity.loading{span{role:"status","Loading…"}}
        else if let Some(preview)=entity.preview {
            div{strong{"{preview.label}"}if let Some(detail)=preview.detail{p{"{detail}"}}for field in preview.fields{div{strong{"{field.label}: "}span{"{field.value}"}}}}
            ActionButton{label:"Open".to_string(),handle,command:Command::OpenEntity(entity.entity_id)}
        }else{span{role:"status","This entity was deleted or is unavailable"}}
    }}
}
