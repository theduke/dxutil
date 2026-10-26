# dxutil

Shared primitives for Dioxus components:

- `id::unique_id(prefix)` allocates a process-wide unique `"{prefix}-{n}"` ID.
- `id::use_unique_id(prefix)` retains an ID for a component's lifetime.
- `time::now_ms()` returns monotonic milliseconds. The origin is unspecified;
  only differences between readings are meaningful.
- `web::EventListener` registers a browser event callback and removes it on drop.

```toml
[dependencies]
dxutil = { git = "https://github.com/theduke/dxutil", features = ["web"] }
```

There are no default features. The optional `web` feature adds `web-sys` and
`wasm-bindgen`. Browser code requires both `web` and the `wasm32` target.
The browser clock uses `window.performance.now()`, falling back to `0.0` when
unavailable. Other configurations use a process-wide `Instant` clock.

| Platform | IDs | Clock | EventListener |
| --- | --- | --- | --- |
| Dioxus web / wasm32 (`web`) | Yes | Performance | Yes |
| Dioxus Desktop (wry webview) | Yes | Instant | Unavailable |
| Dioxus Native (Blitz) | Yes | Instant | Unavailable |
| Headless / native | Yes | Instant | Unavailable |

`EventListener` needs `web-sys` on wasm32; a native webview does not expose it.
