# hyper

[![Build Status](https://travis-ci.org/seanmonstar/hyper.svg?branch=master)](https://travis-ci.org/seanmonstar/hyper)

An HTTP library for Rust.

## Scientific* Benchmarks

[Client bench:](./benches/client.rs)

```
running 3 tests
test bench_curl  ... bench:    346762 ns/iter (+/- 16469)
test bench_http  ... bench:    310861 ns/iter (+/- 123168)
test bench_hyper ... bench:    284916 ns/iter (+/- 65935)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured
```

_* No science was harmed in this benchmark._

## License

[MIT](./LICENSE)
