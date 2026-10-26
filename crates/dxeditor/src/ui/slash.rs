use super::*;
#[component]
pub(super) fn Suggestions(ui: EditorUiState, handle: EngineHandle) -> Element {
    let Some(suggestion) = ui.suggestion.clone() else {
        return rsx! {};
    };
    let slash = suggestion.matched.trigger == "/";
    let placement = suggestion_style(&ui);
    let id = suggestion.listbox_id.clone();
    let mut group = String::new();
    let active_id = format!("{}-{}", id, suggestion.active_index);
    let generation = suggestion.generation;
    use_effect(use_reactive(
        (&active_id, &generation),
        move |(active_id, _)| {
            if let Some(element) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id(&active_id))
            {
                let options = web_sys::ScrollIntoViewOptions::new();
                options.set_block(web_sys::ScrollLogicalPosition::Nearest);
                element.scroll_into_view_with_scroll_into_view_options(&options);
            }
        },
    ));
    // Keyboard handling remains on the editable root; options never take focus.
    rsx! {div{class:if slash{"dxeditor-engine__surface dxeditor-engine__slash"}else{"dxeditor-engine__surface dxeditor-engine__mentions"},id:"{id}",role:"listbox",aria_label:if slash{"Insert block"}else{"Mention entity"},style:"{placement}",onkeydown:{let h=handle.clone();move|event:KeyboardEvent|{let command=match event.key().to_string().as_str(){"ArrowDown"=>Some(Command::SuggestionNext(1)),"ArrowUp"=>Some(Command::SuggestionNext(-1)),"Home"=>Some(Command::SuggestionFirst),"End"=>Some(Command::SuggestionLast),"Escape"=>Some(Command::DismissSuggestion),"Enter"=>Some(Command::AcceptSuggestion),_=>None};if let Some(command)=command{let restore=matches!(command,Command::AcceptSuggestion|Command::DismissSuggestion);event.prevent_default();event.stop_propagation();h.run(command);if restore{h.run(Command::Focus);}}}},
        if suggestion.manual {input{r#type:"search",autofocus:true,onmounted:focus_input,placeholder:"Search entities",aria_label:"Search entities",value:"{suggestion.matched.query}",oninput:{let h=handle.clone();move|event|h.run(Command::SetSuggestionQuery(event.value()))}}}
        if suggestion.searching{div{role:"status","Searching…"}}
        for (index,item) in suggestion.items.iter().enumerate(){
            {let next_group=item.data.get("group").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();let heading=if next_group!=group{group=next_group.clone();next_group}else{String::new()};let h=handle.clone();let disabled=item.data.get("disabled").and_then(serde_json::Value::as_bool).unwrap_or(false);let active=index==suggestion.active_index;let steps=suggestion.items[..index].iter().filter(|item|item.data.get("disabled").and_then(serde_json::Value::as_bool)!=Some(true)).count();rsx!{
                if !heading.is_empty(){div{class:"dxeditor-menu-group",role:"presentation","{heading}"}}
                button{r#type:"button",id:"{id}-{index}",role:"option",tabindex:"-1",aria_selected:active.to_string(),aria_disabled:disabled.to_string(),class:"dxeditor-engine__button",onmousedown:move|e|e.prevent_default(),onmounted:move|event|{
                    if active {use dioxus::web::WebEventExt;if let Some(element)=event.data().try_as_web_event(){let options=web_sys::ScrollIntoViewOptions::new();options.set_block(web_sys::ScrollLogicalPosition::Nearest);element.scroll_into_view_with_scroll_into_view_options(&options);}}
                },onclick:move |_|{if !disabled{h.run(Command::SuggestionFirst);if steps>0{h.run(Command::SuggestionNext(steps as i32));}h.run(Command::AcceptSuggestion);h.run(Command::Focus);}},
                    icons::Icon{name:item.icon.clone().unwrap_or_default()}
                    span{class:"dxeditor-slash-copy",span{"{item.label}"}if let Some(description)=&item.description{small{class:"dxeditor-slash-description","{description}"}}}
                    if let Some(shortcut)=item.data.get("shortcut").and_then(serde_json::Value::as_str){kbd{"{shortcut}"}}
                }
            }}
        }
        if suggestion.items.is_empty()&&!suggestion.searching{div{role:"status",class:"dxeditor-slash-empty","No matching results"}}

    }}
}
