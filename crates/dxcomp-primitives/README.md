# dxcomp-primitives

Unstyled, accessible Dioxus components **forked from `dioxus-primitives`** in
[DioxusLabs/dioxus-components](https://github.com/DioxusLabs/dioxus-components),
at revision [`02801f27e4b3e30606e5c77e435c9d955c708eb3`](https://github.com/DioxusLabs/dioxus-components/tree/02801f27e4b3e30606e5c77e435c9d955c708eb3/primitives).
The upstream authors are Dioxus Labs and DogeDark.

This temporary fork lets `dxcomp` and its consumers use registry dependencies.
The upstream crates.io release `dioxus-primitives 0.0.0` is a placeholder and
does not contain the component implementation used here. The source was copied
from the revision already used by this workspace; component behavior is
preserved. Package metadata and documentation refer to the fork's new name.

The crate provides dialogs, menus, calendars, selection controls, virtual lists,
and other primitives without imposing styles. Use `dxcomp` for styled wrappers,
or depend on this crate directly:

```toml
[dependencies]
dxcomp-primitives = "0.0.1"
```

Rust imports use `dxcomp_primitives`. The default `router` feature enables
Dioxus routing; `web` enables the browser time integration. The accompanying
`dxcomp-attributes` fork supplies the attribute macros, re-exported under the
existing `dioxus_attributes` module name.

The focus-trap TypeScript source, generated JavaScript, and build script are
retained from upstream. JavaScript regeneration is only needed when the
TypeScript changes.

See the [workspace TODO](https://github.com/theduke/dxutil/blob/main/TODO.md) for
switching back to a compatible published upstream release.

## License

The upstream code retains its **MIT OR Apache-2.0** license. Original license
texts are preserved in [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
