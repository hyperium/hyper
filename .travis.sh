#!/bin/sh

if [ "$BENCH" != "" ]
then
    echo "cargo bench $FEATURES"
    cargo bench --verbose $FEATURES
else
    echo "cargo build $FEATURES"
    cargo build --verbose  $FEATURES
    echo "cargo test $FEATURES"
    cargo test --verbose $FEATURES
fi
