# C API for hyper

This provides a C API to use the hyper library.

## Testing

```
cargo build

gcc -o client examples/client.c -I./include -L./target/debug -lhyper_c

LD_LIBRARY_PATH=./target/debug ./client
```
