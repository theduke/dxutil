#![cfg(all(feature = "web", target_arch = "wasm32"))]
use dioxus::prelude::*;
use dxterm::{TerminalView, use_terminal_writer};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element as WebElement, HtmlElement, KeyboardEvent, KeyboardEventInit};
wasm_bindgen_test_configure!(run_in_browser);

fn app() -> Element {
    let writer = use_terminal_writer();
    let mut input = use_signal(Vec::<u8>::new);
    let mut dimensions = use_signal(|| (0u16, 0u16));
    use_future(move || async move {
        writer.write(b"\x1b[31mred\x1b[0m ").await;
        writer.write("界e\u{301}".as_bytes()).await;
    });
    rsx! {
        div {id:"terminal-fixture",style:"width:640px;height:240px;",
            TerminalView {writer,on_input:move |bytes:Vec<u8>|input.write().extend(bytes),on_resize:move |size|dimensions.set(size),on_link:move |_|{},on_selection:move |_|{}}
        }
        output {id:"terminal-input", "{input:?}"}
        output {id:"terminal-size", "{dimensions.read().0},{dimensions.read().1}"}
    }
}
fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}
fn query(selector: &str) -> WebElement {
    document().query_selector(selector).unwrap().unwrap()
}
async fn until(mut predicate: impl FnMut() -> bool) {
    for _ in 0..500 {
        if predicate() {
            return;
        }
        futures_timer::Delay::new(std::time::Duration::from_millis(10)).await;
    }
    panic!("terminal browser condition timed out");
}

#[wasm_bindgen_test]
async fn mounts_writes_encodes_keys_and_resizes() {
    let root = document().create_element("div").unwrap();
    root.set_id("terminal-test-root");
    document().body().unwrap().append_child(&root).unwrap();
    let (abort, registration) = futures::future::AbortHandle::new_pair();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = futures::future::Abortable::new(
            dioxus::web::run(
                VirtualDom::new(app),
                dioxus::web::Config::new().rootname("terminal-test-root"),
            ),
            registration,
        )
        .await;
    });
    until(|| {
        document()
            .query_selector(".dxterm-row")
            .unwrap()
            .is_some_and(|row| row.text_content().unwrap().contains("red 界e\u{301}"))
    })
    .await;
    let row = query(".dxterm-row");
    let first = row
        .first_element_child()
        .unwrap()
        .dyn_into::<HtmlElement>()
        .unwrap();
    assert_eq!(first.text_content().unwrap(), "red");
    assert_eq!(
        first.style().get_property_value("color").unwrap(),
        "rgb(205, 0, 0)"
    );
    assert_eq!(
        row.query_selector("[data-cells='2']")
            .unwrap()
            .unwrap()
            .text_content()
            .unwrap(),
        "界"
    );
    let key = KeyboardEventInit::new();
    key.set_key("c");
    key.set_code("KeyC");
    key.set_ctrl_key(true);
    key.set_bubbles(true);
    key.set_cancelable(true);
    query(".dxterm-input")
        .dispatch_event(&KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &key).unwrap())
        .unwrap();
    until(|| query("#terminal-input").text_content().unwrap() == "[3]").await;
    until(|| query("#terminal-size").text_content().unwrap() != "0,0").await;
    let before = query("#terminal-size").text_content().unwrap();
    query("#terminal-fixture")
        .dyn_into::<HtmlElement>()
        .unwrap()
        .style()
        .set_property("width", "320px")
        .unwrap();
    until(|| query("#terminal-size").text_content().unwrap() != before).await;
    let after = query("#terminal-size").text_content().unwrap();
    let cols = |size: &str| size.split(',').next().unwrap().parse::<u16>().unwrap();
    assert!(cols(&after) < cols(&before));
    abort.abort();
    root.remove();
}
