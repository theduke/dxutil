SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: check-native test-terminal-wasm
.PHONY: help check test fmt lint check-wasm check-headless check-desktop docs install-js test-js test-editor-native test-editor-wasm build-editor-harness test-editor-e2e audit-js audit-rust
.PHONY: serve-playground serve-playground-desktop check-playground check-playground-desktop build-playground test-playground

help:
	@echo "Rust: make check test fmt lint check-headless check-wasm check-desktop check-native docs"
	@echo "JS:   make install-js test-js"
	@echo "Web:  make test-editor-native test-editor-wasm test-editor-e2e test-terminal-wasm"
	@echo "Play: make serve-playground build-playground test-playground check-playground-desktop"

check:
	cargo check --workspace --all-targets --locked --quiet --message-format=short

test:
	cargo test --workspace --locked --quiet --message-format=short

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets --locked --quiet --message-format=short

check-headless:
	cargo check --workspace --exclude dxeditor_harness --all-targets --no-default-features --locked --quiet --message-format=short

check-wasm:
	cargo check --workspace --all-targets --target wasm32-unknown-unknown --features dxcomp/web,dxcomp/dxform,dxeditor/web,dxgraph/web,dxutil/web,dxcode/web,dxcode/all-languages,dxterm/web --locked --quiet --message-format=short

check-desktop:
	cargo check -p dxcomp -p dxgraph -p dxcode -p dxterm --all-targets --no-default-features --features dxcomp/desktop,dxcomp/dxform,dxgraph/demo,dxcode/desktop,dxcode/all-languages,dxterm/desktop --locked --quiet --message-format=short
	$(MAKE) check-playground-desktop

check-native:
	cargo check -p dxcomp --no-default-features --features native,dxform --locked --quiet --message-format=short
	cargo check -p dxterm --all-targets --no-default-features --features native --locked --quiet --message-format=short

docs:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --quiet

install-js:
	npm ci --prefix crates/dxeditor/e2e
	npm ci --prefix crates/playground/e2e

test-js:
	npm run typecheck --prefix crates/dxeditor/e2e
	npm run typecheck --prefix crates/playground/e2e

test-editor-native:
	cargo test -p dxeditor --locked --quiet --message-format=short

# The wasm-bindgen test runner rejects Cargo's forwarded --quiet flag.
test-editor-wasm:
	WASM_BINDGEN_USE_BROWSER=1 WASM_BINDGEN_TEST_TIMEOUT=120 cargo test -p dxeditor --test wasm_browser --target wasm32-unknown-unknown --features web --locked --message-format=short

test-terminal-wasm:
	WASM_BINDGEN_USE_BROWSER=1 WASM_BINDGEN_TEST_TIMEOUT=120 cargo test -p dxterm --test wasm_browser --target wasm32-unknown-unknown --features web --locked --message-format=short

build-editor-harness:
	dx build --web --package dxeditor_harness --locked

test-editor-e2e: build-editor-harness
	cd crates/dxeditor/e2e && npx playwright test

serve-playground:
	dx serve --web --package dxplayground --features web --locked

serve-playground-desktop:
	dx serve --desktop --package dxplayground --features desktop --locked

check-playground:
	cargo check -p dxplayground --target wasm32-unknown-unknown --no-default-features --features web --locked --quiet --message-format=short

check-playground-desktop:
	cargo check -p dxplayground --no-default-features --features desktop --locked --quiet --message-format=short

build-playground:
	rm -rf target/dx/dxplayground/release/web/public
	dx build --web --release --debug-symbols=false --keep-names --package dxplayground --features web --locked

test-playground: build-playground
	cd crates/playground/e2e && npm test

audit-js:
	npm audit --prefix crates/dxeditor/e2e --audit-level=moderate
	npm audit --prefix crates/playground/e2e --audit-level=moderate

audit-rust:
	cargo audit --file Cargo.lock

.PHONY: test-code-native test-code-wasm measure-code-sizes

test-code-native:
	cargo test -p dxcode --features all-languages --locked --quiet --message-format=short

# wasm-bindgen-test-runner does not accept Cargo's forwarded --quiet flag.
test-code-wasm:
	cargo test -p dxcode --no-default-features --features viewer,rust,json,markdown --lib --test wasm_runtime --target wasm32-unknown-unknown --locked --message-format=short

measure-code-sizes:
	bash crates/dxcode/scripts/measure-sizes.sh

.PHONY: check-form test-form
check-form:
	cargo check -p dxcomp --all-targets --no-default-features --locked --quiet --message-format=short
	cargo check -p dxcomp --all-targets --no-default-features --features dxform --locked --quiet --message-format=short

test-form:
	cargo test -p dxform --locked --quiet --message-format=short
	cargo test -p dxcomp --no-default-features --locked --quiet --message-format=short
	cargo test -p dxcomp --features dxform --locked --quiet --message-format=short
