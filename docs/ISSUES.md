# Issues

The [issue tracker][issues] for hyper is where we track all features, bugs, and discuss proposals.

## Triaging

Once an issue has been opened, it is normal for there to be discussion
around it. Some contributors may have differing opinions about the issue,
including whether the behavior being seen is a bug or a feature. This
discussion is part of the process and should be kept focused, helpful, and
professional.

The objective of helping with triaging issues is to help reduce the issue
backlog and keep the issue tracker healthy, while enabling newcomers another
meaningful way to get engaged and contribute.

### Acknowledge

Acknowledge the human. This is meant actively, such as giving a welcome, or
thanks for a detailed report, or any other greeting that makes the person feel
that their contribution (issues are contributions!) is valued. It also is meant
to be internalized, and be sure to always [treat the person kindly][COC]
throughout the rest of the steps of triaging.

### Ask for more info

Frequently, we need more information than was originally provided to fully
evaluate an issue.

If it is a bug report, ask follow up questions that help us get a [minimum
reproducible example][MRE]. This may take several round-trip questions. Once
all the details are gathered, it may be helpful to edit the original issue text
to include them all.

### Categorize

Once enough information has been gathered, the issue should be categorized
with [labels](#labels). Ideally, most issues should be labelled with an area,
effort, and severity. An issue _can_ have multiple areas, pick what fits. There
should be only one severity, and the descriptions of each should help to pick
the right one. The hardest label to select is "effort". If after reading the
descriptions of each effort level, you're still unsure, you can ping a
maintainer to pick one.

### Adjust the title

An optional step when triaging is to adjust the title once more information is
known. Sometimes an issue starts as a question, and through discussion, it
turns out to be a feature request, or a bug report. In those cases, the title
should be changed from a question, and the title should be a succinct action to
be taken. For example, a question about an non-existent configuration option
may be reworded to "Add option to Client to do Zed".

### Mentoring

The last part of triaging is to try to make the issue a learning experience.
After a discussion with the reporter, it would be good to ask if they are now
interested in submitting the change described in the issue.

Otherwise, it would be best to leave the issue with a series of steps for
anyone else to try to write the change. That could be pointing out that a
design proposal is needed, addressing certain points. Or, if the required
changes are mostly know, a list of links to modules and functions where code
needs to be changed, and to what. That way we mentor newcomers to become
successful contributors of new [pull requests][PRs].

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
[COC]: ./CODE_OF_CONDUCT.md
[PRs]: ./PULL_REQUESTS.md
[MRE]: https://en.wikipedia.org/wiki/Minimal_reproducible_example
