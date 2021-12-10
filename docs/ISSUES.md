# Issues

The [issue tracker][issues] for hyper is where we track all features, bugs, and discuss proposals.

## Labels

Issues are organized with a set of labels. Most labels follow a system of being prefixed by a "type".

### Area

The area labels describe what part of hyper is relevant.

- **A-body**: streaming request and response bodies.
- **A-client**: the HTTP client.
- **A-dependencies**: library dependencies.
- **A-docs**: documentation.
- **A-error**: error reporting and types.
- **A-ffi**: the C API.
- **A-http1**: the HTTP/1 specifics.
- **A-http2**: the HTTP/2 specifics.
- **A-server**: the HTTP server.
- **A-tests**: the unit and integration tests.

### Blocked

These labels indicate an issue is "blocked" for some reason.

- **B-breaking-change**: a breaking change that is waiting for the next semver-compatible release.
- **B-rfc**: request for comments. More discussion is needed to explore the design.
- **B-upstream**: waiting on something in a dependency or the compiler.

### Effort

The effort labels are a best-guess at roughly how much effort and knowledge of hyper is needed to accomplish the task.

- **E-easy**: a great starting point for a new contributor.
- **E-medium**: some knowledge of how hyper internals work would be useful.
- **E-hard**: likely requires a deeper understanding of how hyper internals work.

### Severity

The severity marks how _severe_ the issue is. Note this isn't "importance" or "priority".

- **S-bug**: something is wrong, this is bad!
- **S-feature**: this is a new feature request, adding something new.
- **S-performance**: make existing working code go faster.
- **S-refactor**: improve internal code to help readability and maintenance.

[issues]: https://github.com/hyperium/hyper/issues
