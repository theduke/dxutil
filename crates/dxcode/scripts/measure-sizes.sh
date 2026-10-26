#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
# Use explicit paths if mise shims are unavailable in this checkout.
: "${WASM_OPT:=wasm-opt}"
: "${CC_wasm32_unknown_unknown:=clang}"
export CC_wasm32_unknown_unknown
output="${CARGO_TARGET_DIR:-target}/dxcode-sizes"
mkdir -p "$output"
printf '| Features | Raw bytes | Optimized bytes | Gzip bytes | Build seconds |\n'
printf '| --- | ---: | ---: | ---: | ---: |\n'
for config in none rust default all-languages; do
    args=(--no-default-features)
    case "$config" in
        none) ;;
        default) args=() ;;
        *) args+=(--features "$config") ;;
    esac
    start=$SECONDS
    CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_PROFILE_RELEASE_LTO=true \
        CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_PANIC=abort \
        cargo build -p dxcode --example size_probe --release --target wasm32-unknown-unknown \
        --locked --quiet --message-format=short "${args[@]}"
    elapsed=$((SECONDS - start))
    source="${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/examples/size_probe.wasm"
    cp "$source" "$output/$config.raw.wasm"
    "$WASM_OPT" --all-features -Oz "$source" -o "$output/$config.opt.wasm"
    gzip -n -9 -c "$output/$config.opt.wasm" > "$output/$config.wasm.gz"
    printf '| %s | %s | %s | %s | %s |\n' "$config" \
        "$(wc -c < "$source")" "$(wc -c < "$output/$config.opt.wasm")" \
        "$(wc -c < "$output/$config.wasm.gz")" "$elapsed"
done
