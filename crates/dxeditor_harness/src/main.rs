#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
fn main() {
    dioxus::launch(browser::App);
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("Build the editor harness with dx build --web --package dxeditor_harness.");
}
