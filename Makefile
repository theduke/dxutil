SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: help check test fmt lint check-wasm check-headless check-desktop docs install-js test-js build-js check-bundles test-editor-native test-editor-wasm build-editor-harness test-editor-e2e audit-js

help:
	@echo "Rust: make check test fmt lint check-headless check-wasm check-desktop docs"
	@echo "JS:   make install-js test-js build-js check-bundles"
	@echo "Web:  make test-editor-native test-editor-wasm test-editor-e2e"

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
	cargo check --workspace --all-targets --target wasm32-unknown-unknown --features dxcomp/web,dxeditor/web,dxgraph/web --locked --quiet --message-format=short

check-desktop:
	cargo check -p dxcomp -p dxgraph --all-targets --no-default-features --features dxcomp/desktop,dxgraph/demo --locked --quiet --message-format=short

docs:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --quiet

install-js:
	npm ci --prefix crates/dxcode/web
	npm ci --prefix crates/dxterm/web
	npm ci --prefix crates/dxeditor/e2e

test-js:
	npm test --prefix crates/dxcode/web
	npm test --prefix crates/dxterm/web
	npm run typecheck --prefix crates/dxeditor/e2e

build-js:
	npm run build --prefix crates/dxcode/web
	npm run build --prefix crates/dxterm/web

check-bundles: build-js
	git diff --exit-code -- crates/dxcode/web/dist crates/dxterm/web/dist

test-editor-native:
	cargo test -p dxeditor --locked --quiet --message-format=short

# The wasm-bindgen test runner rejects Cargo's forwarded --quiet flag.
test-editor-wasm:
	WASM_BINDGEN_USE_BROWSER=1 WASM_BINDGEN_TEST_TIMEOUT=120 cargo test -p dxeditor --test wasm_browser --target wasm32-unknown-unknown --features web --locked --message-format=short

build-editor-harness:
	dx build --web --package dxeditor_harness --locked

test-editor-e2e: build-editor-harness
	cd crates/dxeditor/e2e && npx playwright test

audit-js:
	npm audit --prefix crates/dxcode/web --audit-level=moderate
	npm audit --prefix crates/dxterm/web --audit-level=moderate
	npm audit --prefix crates/dxeditor/e2e --audit-level=moderate
