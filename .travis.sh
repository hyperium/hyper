#!/bin/sh

if [ "$BENCH" != "" ]
then
    echo "cargo bench $FEATURES"
    cargo bench $FEATURES
else
    echo "cargo build $FEATURES"
    cargo build $FEATURES
    echo "cargo test $FEATURES"
    cargo test $FEATURES
fi
