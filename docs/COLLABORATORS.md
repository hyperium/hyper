# hyper Collaborator Guide

This document describes how collaborators manage hyper.

- First off, a collaborator doesn't need to do all of the following perfectly.
	- It takes time to learn.
	- Instead, focus on growth.
	- Get in the habit of checking this guide with some frequency, and improve rather than stagnate.

## Exemplify the Code of Conduct

- Our [code of conduct](./CODE_OF_CONDUCT.md) is not elaborate. It doesn't require much study, nor explication.
- To be a good collaborator, it does require internalizing it.
- The code of conduct is proactive, positive.
- A collaborator must be a good example of being kind.
	- It should not be possible to find significant fault in a collaborator against the simple principles in the code of conduct.

## Internalize the Vision

- Understand and use the project's [vision](./VISION.md) when helping guide decisions.

## Review Pull Requests

- Review pull requests promptly and kindly. Approve CI for new contributors.
	- Consider telling a maintainer to add you to the automated review queue.
- Collaborators should feel comfortable approving and merging straightforward changes. Such as documentation, chores, styling, simple refactors, and basic bug fixes.
- When reviewing larger changes, keep in mind the points on [being API caretakers](#api-caretakers).
- When approving a PR, allow time for other collaborators to look before merging.
	- Especially if the PR is newer, or has a more significant change.
- When merging, prefer squashing (it adds in the PR number). Touch up the commit message to match our [COMMITS](./COMMITS.md) style.

## Provide Feedback on Features and Designs

Collaborators try to participate early and often when new features are proposed. They also actively consider and recommend new features that users may need.

### API Caretakers

Collaborators are caretakers of hyper's API. When proposing, discussing, or reviewing changes to hyper, keep the following in mind:

- Conservative additions
	- Conservative APIs reduce breaking changes
- Don't expose internal implementation details
	- [Accessors can reveal internal repr](https://seanmonstar.com/micro/20260721-accessors-reveal-internal-repr/)
- hyper-util is a place to explore first
	- [Vision: Not quite stable, but utile](./VISION.md#not-quite-stable-but-utile-useful)
	- [Roadmap: hyper-util](./ROADMAP.md#hyper-util)
- Unstable features
	- Unstable features are not bound by our stability promise.
	- We don't just name a crate feature with unstable.
		- This has the problem of an intermediary crate enabling it, and someone depending on _that_ crate and not realizing they are accessing unstable hyper features.
	- All unstable features require a conditional config flag passed to the Rust compiler.
		- For example, `hyper_unstable_ffi`.
- Breaking changes
	- These would usually be reserved for a new SemVer major release, such as v2.0, and thus would usually be very rare.
	- An exception is fixing a mistake quickly after release of a new feature.
	- All breaking changes require approval by a maintainer.

## Welcome New Contributors

- Help new contributors find their way around the project and its processes. Follow the guidance on [acknowledging people when triaging](./ISSUES.md#acknowledge).

## Answer Questions

- Answer questions in issues or chat when you can, or help find someone who can. See the project's [help channels](../CONTRIBUTING.md#help).

## Mentor Contributors

- Help contributors grow toward becoming project members. Issue triaging often provides opportunities for [mentoring](./ISSUES.md#mentoring), but the ideas can be applied to reviews, or when answering questions.
