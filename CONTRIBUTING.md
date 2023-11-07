# Contributing to Hyper

You want to contribute? You're awesome! Don't know where to start? Check the [list of easy issues](https://github.com/hyperium/hyper/issues?q=is%3Aopen+is%3Aissue+label%3AE-easy).

[easy tag]: https://github.com/hyperium/hyper/issues?q=label%3AE-easy+is%3Aopen

## [Pull Requests](./docs/PULL_REQUESTS.md)

- [Submitting a Pull Request](./docs/PULL_REQUESTS.md#submitting-a-pull-request)
- [Commit Guidelines](./docs/COMMITS.md)

## Cargo fmt

`cargo fmt --all` does not work in hyper. Please use the following commands:

```txt
# Mac or Linux
rustfmt --check --edition 2018 $(git ls-files '*.rs')

# Powershell
Get-ChildItem . -Filter "*.rs" -Recurse | foreach { rustfmt --check --edition 2018 $_.FullName }
```

> **NOTE**: If you are using `rust-analyzer`, you can add the following two lines in your `settings.json` to make sure the features get taken into account when checking the project:
>
>    ```json
>     "rust-analyzer.cargo.features": ["full"],
>     "rust-analyzer.check.features": ["full"],
>    ```
