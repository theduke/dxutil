//! DOM points use UTF-16. Everything past this boundary uses scalar/atom units.
use crate::engine::{InlineContainerRef, TextPoint};
use wasm_bindgen::JsCast;
use web_sys::{Element, Node};
pub const ZW: &str = "\u{feff}";
/// Islands, including any nested editors, own their descendants' selection.
pub(crate) fn owns_node(root: &Element, node: &Node) -> bool {
    if !root.contains(Some(node)) {
        return false;
    }
    let element = node
        .dyn_ref::<Element>()
        .cloned()
        .or_else(|| node.parent_element());
    !element
        .and_then(|element| element.closest(".dxeditor-island").ok().flatten())
        .is_some_and(|island| root.contains(Some(&island)))
}
pub fn container(node: &Node, root: &Element) -> Option<(Element, InlineContainerRef)> {
    let mut at = if node.node_type() == Node::ELEMENT_NODE {
        node.dyn_ref::<Element>().cloned()
    } else {
        node.parent_element()
    };
    while let Some(el) = at {
        if let Some(raw) = el.get_attribute("data-inline-container") {
            return serde_json::from_str(&raw).ok().map(|c| (el, c));
        }
        if el.get_attribute("data-content-kind").as_deref() == Some("code_block") {
            if let Some(inline) = el.query_selector("[data-inline-container]").ok().flatten() {
                if let Some(container) = inline
                    .get_attribute("data-inline-container")
                    .and_then(|raw| serde_json::from_str(&raw).ok())
                {
                    return Some((inline, container));
                }
            }
        }
        if el == *root {
            return None;
        }
        at = el.parent_element();
    }
    None
}
fn atom(n: &Node) -> bool {
    n.dyn_ref::<Element>()
        .is_some_and(|e| e.has_attribute("data-inline-atom"))
}
fn length(n: &Node) -> usize {
    if atom(n) {
        1
    } else if n.node_type() == Node::TEXT_NODE {
        n.node_value()
            .unwrap_or_default()
            .chars()
            .filter(|&c| c != '\u{feff}')
            .count()
    } else {
        let kids = n.child_nodes();
        (0..kids.length())
            .filter_map(|i| kids.item(i))
            .map(|n| length(&n))
            .sum()
    }
}
fn before(root: &Node, target: &Node, offset: u32) -> Option<usize> {
    if root == target {
        if root.node_type() == Node::TEXT_NODE {
            let s = root.node_value().unwrap_or_default();
            let mut utf16 = 0;
            return Some(
                s.chars()
                    .take_while(|c| {
                        utf16 += c.len_utf16() as u32;
                        utf16 <= offset
                    })
                    .filter(|&c| c != '\u{feff}')
                    .count(),
            );
        }
        let kids = root.child_nodes();
        return Some(
            (0..offset.min(kids.length()))
                .filter_map(|i| kids.item(i))
                .map(|n| length(&n))
                .sum(),
        );
    }
    if atom(root) {
        return None;
    }
    let mut sum = 0;
    let kids = root.child_nodes();
    for i in 0..kids.length() {
        let n = kids.item(i)?;
        if let Some(found) = before(&n, target, offset) {
            return Some(sum + found);
        }
        sum += length(&n);
    }
    None
}
pub fn from_dom(root: &Element, node: &Node, offset: u32) -> Option<TextPoint> {
    if !owns_node(root, node) {
        return None;
    }
    let (element, container) = container(node, root)?;
    Some(TextPoint::new(
        container,
        before(element.as_ref(), node, offset).or_else(|| {
            node.contains(Some(element.as_ref())).then(|| {
                if offset == 0 {
                    0
                } else {
                    length(element.as_ref())
                }
            })
        })?,
    ))
}
pub(crate) fn locate(root: &Node, offset: usize) -> Option<(Node, u32)> {
    if root.node_type() == Node::TEXT_NODE {
        let text = root.node_value().unwrap_or_default();
        let mut count = 0;
        let mut utf16 = 0;
        for c in text.chars() {
            if c != '\u{feff}' {
                if count == offset {
                    break;
                }
                count += 1;
            }
            utf16 += c.len_utf16();
        }
        return Some((root.clone(), utf16 as u32));
    }
    let kids = root.child_nodes();
    let mut remain = offset;
    for i in 0..kids.length() {
        let n = kids.item(i)?;
        let len = length(&n);
        if atom(&n) {
            if remain == 0 {
                return Some((root.clone(), i));
            }
            remain = remain.saturating_sub(1);
            if remain == 0 {
                return Some((root.clone(), i + 1));
            }
        } else if remain <= len {
            return locate(&n, remain);
        } else {
            remain -= len;
        }
    }
    Some((root.clone(), kids.length()))
}
pub fn to_dom(root: &Element, point: &TextPoint) -> Option<(Node, u32)> {
    let el = match &point.container {
        InlineContainerRef::Block(uid) | InlineContainerRef::Caption(uid) => root
            .query_selector(&format!("[data-uid='{uid}'] [data-inline-container]"))
            .ok()
            .flatten()?,
        InlineContainerRef::Cell { row, col } => root
            .query_selector(&format!("tr[data-uid='{row}']"))
            .ok()
            .flatten()?
            .children()
            .item(*col as u32)?,
    };
    locate(el.as_ref(), point.offset)
}
pub fn read_text(element: &Element) -> String {
    fn walk(n: &Node, out: &mut String) {
        if atom(n) {
            if n.dyn_ref::<Element>().is_some_and(|e| e.tag_name() == "BR") {
                out.push('\n');
            } else {
                out.push('\u{fffc}');
            }
            return;
        }
        if n.node_type() == Node::TEXT_NODE {
            out.extend(
                n.node_value()
                    .unwrap_or_default()
                    .chars()
                    .filter(|&c| c != '\u{feff}'),
            );
            return;
        }
        let kids = n.child_nodes();
        for i in 0..kids.length() {
            if let Some(n) = kids.item(i) {
                walk(&n, out);
            }
        }
    }
    let mut out = String::new();
    walk(element.as_ref(), &mut out);
    out
}
pub fn units_to_utf16(text: &str, units: usize) -> u32 {
    text.chars().take(units).map(|c| c.len_utf16() as u32).sum()
}
pub fn utf16_to_units(text: &str, offset: u32) -> usize {
    let mut total = 0;
    text.chars()
        .take_while(|c| {
            total += c.len_utf16() as u32;
            total <= offset
        })
        .count()
}
