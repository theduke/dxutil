//! Dioxus owns island roots. The renderer moves whole roots without replacing
//! their descendants, preserving nested editors, dialogs and native input state.
use wasm_bindgen::JsCast;
use web_sys::Element;

pub fn adopt(root: &Element, session: &str, uid: &str) {
    let Some(document) = root.owner_document() else {
        return;
    };
    let Some(island) = document.get_element_by_id(&format!("dxeditor-island-{session}-{uid}"))
    else {
        return;
    };
    let Ok(Some(target)) = root.query_selector(&format!("[data-island-uid='{uid}']")) else {
        return;
    };
    let _ = island.set_attribute(
        "data-island-parking",
        &format!("dxeditor-islands-{session}"),
    );
    if island.parent_element().as_ref() != Some(&target) {
        target.set_text_content(None);
        let _ = target.append_child(&island);
    }
}
pub fn adopt_all(root: &Element, session: &str) {
    if let Ok(nodes) = root.query_selector_all("[data-island-uid]") {
        for index in 0..nodes.length() {
            if let Some(element) = nodes
                .item(index)
                .and_then(|node| node.dyn_into::<Element>().ok())
            {
                if let Some(uid) = element.get_attribute("data-island-uid") {
                    adopt(root, session, &uid);
                }
            }
        }
    }
}
pub fn park_descendants(root: &Element) {
    let mut roots = Vec::new();
    if root.class_list().contains("dxeditor-island-root") {
        roots.push(root.clone());
    } else if let Ok(nodes) = root.query_selector_all(".dxeditor-island-root") {
        for index in 0..nodes.length() {
            let Some(island) = nodes
                .item(index)
                .and_then(|node| node.dyn_into::<Element>().ok())
            else {
                continue;
            };
            // A nested editor and all its islands travel with the outer island.
            if island
                .parent_element()
                .and_then(|parent| parent.closest(".dxeditor-island-root").ok().flatten())
                .is_some_and(|parent| root.contains(Some(&parent)))
            {
                continue;
            }
            roots.push(island);
        }
    }
    for island in roots {
        let Some(document) = island.owner_document() else {
            continue;
        };
        if let Some(parking) = island
            .get_attribute("data-island-parking")
            .and_then(|id| document.get_element_by_id(&id))
        {
            if island.parent_element().as_ref() != Some(&parking) {
                let _ = parking.append_child(&island);
            }
            continue;
        }
        // Roots rendered before the initial adoption still have a stable id.
        // Match the longest session prefix rather than trying to split UUIDs.
        let Some(id) = island.get_attribute("id") else {
            continue;
        };
        let Ok(nodes) = document.query_selector_all("[id^='dxeditor-islands-']") else {
            continue;
        };
        let parking = (0..nodes.length())
            .filter_map(|index| nodes.item(index)?.dyn_into::<Element>().ok())
            .filter(|parking| {
                parking
                    .id()
                    .strip_prefix("dxeditor-islands-")
                    .is_some_and(|session| id.starts_with(&format!("dxeditor-island-{session}-")))
            })
            .max_by_key(|parking| parking.id().len());
        if let Some(parking) = parking {
            let _ = parking.append_child(&island);
        }
    }
}
