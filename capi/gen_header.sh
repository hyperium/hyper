#!/usr/bin/env bash

# This script regenerates hyper.h. As of April 2021, it only works with the
# nightly build of Rust.

set -e

CAPI_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

WORK_DIR=$(mktemp -d)

# check if tmp dir was created
if [[ ! "$WORK_DIR" || ! -d "$WORK_DIR" ]]; then
    echo "Could not create temp dir"
    exit 1
fi

header_file_backup="$CAPI_DIR/include/hyper.h.backup"

function cleanup {
    rm -rf "$WORK_DIR"
    rm "$header_file_backup" || true
}

trap cleanup EXIT

mkdir "$WORK_DIR/src"

# Fake a library
cat > "$WORK_DIR/src/lib.rs" << EOF
#[path = "$CAPI_DIR/../src/ffi/mod.rs"]
pub mod ffi;
EOF

# And its Cargo.toml
cat > "$WORK_DIR/Cargo.toml" << EOF
[package]
name = "hyper"
version = "0.0.0"
edition = "2018"
publish = false

[dependencies]
EOF

cp "$CAPI_DIR/include/hyper.h" "$header_file_backup"

#cargo metadata --no-default-features --features ffi --format-version 1 > "$WORK_DIR/metadata.json"

cd "${WORK_DIR}" || exit 2

# Expand just the ffi module
if ! output=$(cargo rustc -- -Z unstable-options --pretty=expanded 2>&1 > expanded.rs); then
    # As of April 2021 the script above prints a lot of warnings/errors, and
    # exits with a nonzero return code, but hyper.h still gets generated.
    echo "$output"
fi

# Replace the previous copy with the single expanded file
rm -rf ./src
mkdir src
mv expanded.rs src/lib.rs


# Bindgen!
if ! cbindgen \
    --config "$CAPI_DIR/cbindgen.toml" \
    --lockfile "$CAPI_DIR/../Cargo.lock" \
    --output "$CAPI_DIR/include/hyper.h" \
    "${@}"; then
    bindgen_exit_code=$?
    if [[ "--verify" == "$1" ]]; then
        echo "diff generated (<) vs backup (>)"
        diff "$CAPI_DIR/include/hyper.h" "$header_file_backup"
    fi
    exit $bindgen_exit_code
fi

exit 0
