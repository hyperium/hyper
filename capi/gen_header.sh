#!/usr/bin/env bash

CAPI_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

WORK_DIR=`mktemp -d`


# check if tmp dir was created
if [[ ! "$WORK_DIR" || ! -d "$WORK_DIR" ]]; then
    echo "Could not create temp dir"
    exit 1
fi

header_file_backup="$CAPI_DIR/include/hyper.h.backup"

function cleanup {
    #echo "$WORK_DIR"
    rm -rf "$WORK_DIR"
    rm "$header_file_backup"
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

cd $WORK_DIR

# Expand just the ffi module
cargo rustc -- -Z unstable-options --pretty=expanded > expanded.rs 2>/dev/null

# Replace the previous copy with the single expanded file
rm -rf ./src
mkdir src
mv expanded.rs src/lib.rs


# Bindgen!
cbindgen\
    -c "$CAPI_DIR/cbindgen.toml"\
    --lockfile "$CAPI_DIR/../Cargo.lock"\
    -o "$CAPI_DIR/include/hyper.h"\
    $1

bindgen_exit_code=$?

if [[ "--verify" == "$1" && "$bindgen_exit_code" != 0 ]]; then
    echo "diff generated (<) vs backup (>)"
    diff "$CAPI_DIR/include/hyper.h" "$header_file_backup"
fi

exit $bindgen_exit_code
