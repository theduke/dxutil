#!/usr/bin/env bash
# Linux x86_64 CI tools, using upstream release archives and pinned checksums.
set -euo pipefail

if [[ "$(uname -s)-$(uname -m)" != Linux-x86_64 ]]; then
  echo "This installer supports Linux x86_64 only; see README.md for other platforms." >&2
  exit 1
fi
install_dir="${DXUTIL_TOOL_DIR:-$HOME/.local/bin}"
mkdir -p "$install_dir"
tool_tmp=$(mktemp -d)
trap 'rm -rf "$tool_tmp"' EXIT
cd "$tool_tmp"

curl --fail --silent --show-error --location --retry 3   https://github.com/DioxusLabs/dioxus/releases/download/v0.7.9/dx-x86_64-unknown-linux-gnu.tar.gz   --output dx.tar.gz
printf '%s  %s\n' 3b132551b480bc96f938f9f0d37936ee1190f994977539dcc347eaf38540d005 dx.tar.gz | sha256sum --check
mkdir dx
tar -xzf dx.tar.gz -C dx
find dx -type f -name dx -exec install -m 755 '{}' "$install_dir/dx" \;

curl --fail --silent --show-error --location --retry 3   https://github.com/rustwasm/wasm-bindgen/releases/download/0.2.121/wasm-bindgen-0.2.121-x86_64-unknown-linux-musl.tar.gz   --output wasm-bindgen.tar.gz
printf '%s  %s\n' 3039f38f65fe237b640cf06a140c919ca8d717ec5012146d145d3f27bb4d6b28 wasm-bindgen.tar.gz | sha256sum --check
mkdir wasm-bindgen
tar -xzf wasm-bindgen.tar.gz -C wasm-bindgen
for tool in wasm-bindgen wasm-bindgen-test-runner; do
  find wasm-bindgen -type f -name "$tool" -exec install -m 755 '{}' "$install_dir/$tool" \;
done
if [[ -n "${GITHUB_PATH:-}" ]]; then
  echo "$install_dir" >> "$GITHUB_PATH"
fi
"$install_dir/dx" --version
"$install_dir/wasm-bindgen" --version
"$install_dir/wasm-bindgen-test-runner" --version
