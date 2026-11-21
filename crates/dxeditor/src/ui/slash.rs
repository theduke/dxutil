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
    let empty_manual_slash = slash && suggestion.manual && suggestion.matched.query.is_empty();
    use_effect(use_reactive(
        (&active_id, &generation),
        move |(active_id, _)| {
            if let Some(element) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id(&active_id))
            {
                if let Some(list) = element.parent_element() {
                    let item = element.get_bounding_client_rect();
                    let bounds = list.get_bounding_client_rect();
                    let delta = if item.top() < bounds.top() {
                        item.top() - bounds.top()
                    } else {
                        (item.bottom() - bounds.bottom()).max(0.0)
                    };
                    if delta != 0.0 {
                        list.set_scroll_top(list.scroll_top() + delta.round() as i32);
                    }
                }
            }
        },
    ));
    // Keyboard handling remains on the editable root; options never take focus.
    rsx! {div{class:if slash{"dxeditor-engine__surface dxeditor-engine__slash"}else{"dxeditor-engine__surface dxeditor-engine__mentions"},id:"{id}",role:"listbox",onmounted:reveal_caret,aria_label:if slash{"Insert block"}else{"Mention entity"},style:"{placement}",onkeydown:{let h=handle.clone();move|event:KeyboardEvent|{let command=match event.key().to_string().as_str(){"ArrowDown"=>Some(Command::SuggestionNext(1)),"ArrowUp"=>Some(Command::SuggestionNext(-1)),"Home"=>Some(Command::SuggestionFirst),"End"=>Some(Command::SuggestionLast),"Escape"=>Some(Command::DismissSuggestion),"Backspace" if empty_manual_slash=>Some(Command::DismissSuggestion),"Enter"=>Some(Command::AcceptSuggestion),_=>None};if let Some(command)=command{let restore=matches!(command,Command::AcceptSuggestion|Command::DismissSuggestion);event.prevent_default();event.stop_propagation();h.run(command);if restore{h.run(Command::Focus);}}}},
        if suggestion.manual {input{r#type:"search",autofocus:true,onmounted:focus_input,placeholder:if slash{"Search blocks"}else{"Search entities"},aria_label:if slash{"Search blocks"}else{"Search entities"},value:"{suggestion.matched.query}",oninput:{let h=handle.clone();move|event|h.run(Command::SetSuggestionQuery(event.value()))}}}
        if suggestion.searching{div{role:"status","Searching…"}}
        for (index,item) in suggestion.items.iter().enumerate(){
            {let next_group=item.data.get("group").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();let heading=if next_group!=group&&suggestion.matched.query.is_empty(){group=next_group.clone();next_group}else{String::new()};let h=handle.clone();let disabled=item.data.get("disabled").and_then(serde_json::Value::as_bool).unwrap_or(false);let active=index==suggestion.active_index;let steps=suggestion.items[..index].iter().filter(|item|item.data.get("disabled").and_then(serde_json::Value::as_bool)!=Some(true)).count();let hover=handle.clone();rsx!{
                if !heading.is_empty(){div{class:"dxeditor-menu-group",role:"presentation","{heading}"}}
                button{r#type:"button",id:"{id}-{index}",role:"option",tabindex:"-1",aria_selected:active.to_string(),aria_disabled:disabled.to_string(),class:"dxeditor-engine__button",onmousedown:move|e|e.prevent_default(),
                    // The pointer moves the active option, so one option is highlighted.
                    onmousemove:move|_|{if !active&&!disabled{hover.run(Command::SuggestionFirst);if steps>0{hover.run(Command::SuggestionNext(steps as i32));}}},
                    onclick:move |_|{if !disabled{h.run(Command::SuggestionFirst);if steps>0{h.run(Command::SuggestionNext(steps as i32));}h.run(Command::AcceptSuggestion);h.run(Command::Focus);}},
                    icons::Icon{name:item.icon.clone().unwrap_or_default()}
                    span{class:"dxeditor-slash-copy",span{"{item.label}"}if let Some(description)=&item.description{small{class:"dxeditor-slash-description","{description}"}}}
                    if let Some(shortcut)=item.data.get("shortcut").and_then(serde_json::Value::as_str){kbd{"{shortcut}"}}
                }
            }}
        }
        if suggestion.items.is_empty()&&!suggestion.searching{div{role:"status",class:"dxeditor-slash-empty","No matching results"}}

    }}
}

/// On narrow screens the list is a sheet over the bottom of the viewport;
/// scrolls the line being written so it stays visible above the sheet.
fn reveal_caret(event: MountedEvent) {
    use dioxus::web::WebEventExt;
    let Some(list) = event.data().try_as_web_event() else {
        return;
    };
    let Some(window) = web_sys::window() else {
        return;
    };
    let viewport = window
        .inner_height()
        .ok()
        .and_then(|height| height.as_f64())
        .unwrap_or(0.0);
    let sheet = list.get_bounding_client_rect();
    // Only a sheet docked to the bottom edge covers the caret.
    if sheet.bottom() < viewport - 1. || sheet.top() <= 0. {
        return;
    }
    let Some(line) = window
        .get_selection()
        .ok()
        .flatten()
        .and_then(|selection| selection.focus_node())
        .and_then(|node| {
            node.dyn_ref::<web_sys::Element>()
                .cloned()
                .or_else(|| node.parent_element())
        })
        .and_then(|element| {
            element
                .closest(".dx-block,[data-inline-container]")
                .ok()
                .flatten()
        })
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    else {
        return;
    };
    if line.get_bounding_client_rect().bottom() <= sheet.top() - 8. {
        return;
    }
    let style = line.style();
    let _ = style.set_property(
        "scroll-margin-bottom",
        &format!("{}px", viewport - sheet.top() + 16.),
    );
    let options = web_sys::ScrollIntoViewOptions::new();
    options.set_block(web_sys::ScrollLogicalPosition::Nearest);
    line.scroll_into_view_with_scroll_into_view_options(&options);
    let _ = style.remove_property("scroll-margin-bottom");
}
