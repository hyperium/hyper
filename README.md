# hyper

[![Build Status](https://travis-ci.org/seanmonstar/hyper.svg?branch=master)](https://travis-ci.org/seanmonstar/hyper)

An HTTP library for Rust.

[Documentation](http://seanmonstar.github.io/hyper)

## Scientific* Benchmarks

[Client bench:](./benches/client.rs)

```
running 3 tests
test bench_curl  ... bench:    234539 ns/iter (+/- 22228)
test bench_http  ... bench:    290370 ns/iter (+/- 69179)
test bench_hyper ... bench:    224482 ns/iter (+/- 95197)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured
```

_* No science was harmed in this benchmark._

## License

[MIT](./LICENSE)
