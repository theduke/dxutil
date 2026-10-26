# dxcomp-attributes

Attribute macros **forked from `dioxus-attributes`**, the proc-macro dependency
of `dioxus-primitives`, in
[DioxusLabs/dioxus-components](https://github.com/DioxusLabs/dioxus-components)
at revision [`02801f27e4b3e30606e5c77e435c9d955c708eb3`](https://github.com/DioxusLabs/dioxus-components/tree/02801f27e4b3e30606e5c77e435c9d955c708eb3/dioxus-attributes).

This companion to `dxcomp-primitives` preserves the upstream `attributes!`
implementation and publishes with the workspace's shared version. Most callers
can use its existing re-export through
`dxcomp::primitives::dioxus_attributes::attributes`; direct dependencies import
`dxcomp_attributes::attributes`.

This is a temporary fork. See the
[workspace TODO](https://github.com/theduke/dxutil/blob/main/TODO.md) for replacing
both forks with compatible upstream registry releases.

## License

The upstream code retains its **MIT OR Apache-2.0** license. Original license
texts are preserved in [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
