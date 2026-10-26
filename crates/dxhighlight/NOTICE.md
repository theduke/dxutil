# Third-party notices

Grammar crate metadata and upstream license texts were checked: all selected grammars are MIT. Preserve these copyright and permission
notices when distributing linked grammars. Arborium glue and its wasm sysroot
are distributed under MIT; upstream tree-sitter is also MIT.

Markdown comes from upstream `tree-sitter-md` (0.5.4, both block and inline),
because Arborium has no inline crate compatible with upstream tree-sitter.
Only that one Markdown grammar package is linked.

| Component | Source | Revision | Notice |
| --- | --- | --- | --- |
| rust | [upstream](https://codeberg.org/grammar-orchard/tree-sitter-rust-orchard) | `261b20226c04ef601adbdf185a800512a5f66291` | [MIT text](licenses/rust-MIT.txt) |
| javascript | [upstream](https://github.com/tree-sitter/tree-sitter-javascript) | `58404d8cf191d69f2674a8fd507bd5776f46cb11` | [MIT text](licenses/javascript-MIT.txt) |
| typescript | [upstream](https://github.com/tree-sitter/tree-sitter-typescript) | `75b3874edb2dc714fb1fd77a32013d0f8699989f` | [MIT text](licenses/typescript-MIT.txt) |
| tsx | [upstream](https://github.com/tree-sitter/tree-sitter-typescript) | `75b3874edb2dc714fb1fd77a32013d0f8699989f` | [MIT text](licenses/tsx-MIT.txt) |
| json | [upstream](https://github.com/tree-sitter/tree-sitter-json) | `001c28d7a29832b06b0e831ec77845553c89b56d` | [MIT text](licenses/json-MIT.txt) |
| toml | [upstream](https://github.com/tree-sitter-grammars/tree-sitter-toml) | `64b56832c2cffe41758f28e05c756a3a98d16f41` | [MIT text](licenses/toml-MIT.txt) |
| yaml | [upstream](https://github.com/tree-sitter-grammars/tree-sitter-yaml) | `7708026449bed86239b1cd5bce6e3c34dbca6415` | [MIT text](licenses/yaml-MIT.txt) |
| html | [upstream](https://github.com/tree-sitter/tree-sitter-html) | `73a3947324f6efddf9e17c0ea58d454843590cc0` | [MIT text](licenses/html-MIT.txt) |
| css | [upstream](https://github.com/tree-sitter/tree-sitter-css) | `dda5cfc5722c429eaba1c910ca32c2c0c5bb1a3f` | [MIT text](licenses/css-MIT.txt) |
| python | [upstream](https://github.com/tree-sitter/tree-sitter-python) | `26855eabccb19c6abf499fbc5b8dc7cc9ab8bc64` | [MIT text](licenses/python-MIT.txt) |
| go | [upstream](https://github.com/tree-sitter/tree-sitter-go) | `2346a3ab1bb3857b48b29d779a1ef9799a248cd7` | [MIT text](licenses/go-MIT.txt) |
| bash | [upstream](https://github.com/tree-sitter/tree-sitter-bash) | `20f55fb9663568f16ebceaac7673d2aa530add46` | [MIT text](licenses/bash-MIT.txt) |
| sql | [upstream](https://github.com/DerekStride/tree-sitter-sql) | `fe77f6868d6cdea593052a6af390116495093dc1` | [MIT text](licenses/sql-MIT.txt) |
| c | [upstream](https://github.com/tree-sitter/tree-sitter-c) | `ae19b676b13bdcc13b7665397e6d9b14975473dd` | [MIT text](licenses/c-MIT.txt) |
| cpp | [upstream](https://github.com/tree-sitter/tree-sitter-cpp) | `12bd6f7e96080d2e70ec51d4068f2f66120dde35` | [MIT text](licenses/cpp-MIT.txt) |
| markdown | [upstream](https://github.com/tree-sitter-grammars/tree-sitter-markdown) | `41ff24a6c775492c543b09228aeb98d1cd2b3a0f` | [MIT text](licenses/markdown-MIT.txt) |
| arborium | [upstream](https://github.com/bearcove/arborium) | `v2.18.2` | [MIT text](licenses/arborium-MIT.txt) |
| jsdoc (Arborium transitive dependency) | [upstream](https://github.com/tree-sitter/tree-sitter-jsdoc) | `658d18dcdddb75c760363faa4963427a7c6b52db` | [MIT text](licenses/jsdoc-MIT.txt) |
| tree-sitter 0.27.1 | [upstream](https://github.com/tree-sitter/tree-sitter) | `v0.27.1` | [MIT text](licenses/tree-sitter-MIT.txt) |

The MIT notices apply to the generated grammar sources and bundled queries.
Transitive Rust dependencies retain their own licenses; see Cargo.lock and their
crate packages for the complete dependency set.

Rust grammar source is pinned by Arborium. Its commit-specific license URL was
unavailable; the notice was retrieved from its upstream main branch and matches
the original tree-sitter-rust MIT notice (Maxim Sokolov, 2017).
