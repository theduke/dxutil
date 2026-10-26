use dioxus::prelude::*;
use dxterm::{TerminalView, use_terminal_writer};
fn main() {
    dioxus::launch(app);
}
fn app() -> Element {
    let writer = use_terminal_writer();
    use_future(move || async move {
        writer
            .write(b"\x1b[32mRust terminal\x1b[0m\r\nType to echo locally.\r\n")
            .await;
    });
    rsx! {style {"html,body {{ margin:0; }}"} div {style:"width:800px;height:480px;", TerminalView {writer,on_input:move |bytes:Vec<u8>| {spawn(async move {if bytes==b"\r" {writer.write(b"\r\n").await;} else {writer.write(bytes).await;}});},on_resize:move |_|{},on_link:move |_|{},on_selection:move |_|{}}}}
}
