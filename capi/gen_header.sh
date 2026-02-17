#!/usr/bin/env bash
#
# This script regenerates hyper.h.
# nightly build of Rust.
#
# Requirements:
#
# cargo install cbindgen
# cargo install cargo-expand

set -e

CAPI_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
header_file="$CAPI_DIR/include/hyper.h"
header_file_backup="$CAPI_DIR/include/hyper.h.backup"
verify_flag=$1
function cleanup {
    rm -rf "$WORK_DIR" || true
    if [[ "--verify" == "$verify_flag" ]]; then
        rm "$header_file_backup" || true
    fi
}

trap cleanup EXIT

WORK_DIR=$(mktemp -d)

# check if tmp dir was created
if [[ ! "$WORK_DIR" || ! -d "$WORK_DIR" ]]; then
    echo "Could not create temp dir"
    exit 1
fi

# backup hyper.h
if [[ "--verify" == "$verify_flag" ]]; then
    cp "$header_file" "$header_file_backup"
fi

# Expand just the ffi module
if ! RUSTFLAGS='--cfg hyper_unstable_ffi' cargo expand --features ffi,server,client,http1,http2 ::ffi 2> $WORK_DIR/expand_stderr.err > $WORK_DIR/expanded.rs; then
    cat $WORK_DIR/expand_stderr.err
fi

# Bindgen!
if ! cbindgen \
    --config "$CAPI_DIR/cbindgen.toml" \
    --lockfile "$CAPI_DIR/../Cargo.lock" \
    --output "$header_file" \
    "${@}"\
    $WORK_DIR/expanded.rs 2> $WORK_DIR/cbindgen_stderr.err; then
    bindgen_exit_code=$?
    if [[ "--verify" == "$verify_flag" ]]; then
        echo "Changes from previous header (old < > new)"
        diff -u "$header_file_backup" "$header_file"
    else
        echo "cbindgen failed:"
        cat $WORK_DIR/cbindgen_stderr.err
    fi
    exit $bindgen_exit_code
fi

exit 0
