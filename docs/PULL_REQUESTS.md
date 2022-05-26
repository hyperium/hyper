# Pull Requests

Pull requests are the way to submit changes to the hyper repository.

## Submitting a Pull Request

In most cases, it a good idea to discuss a potential change in an
[issue](ISSUES.md). This will allow other contributors to provide guidance and
feedback _before_ significant code work is done, and can increase the
likelihood of getting the pull request merged.

### Tests

If the change being proposed alters code (as opposed to only documentation for
example), it is either adding new functionality to hyper or it is fixing
existing, broken functionality. In both of these cases, the pull request should
include one or more tests to ensure that hyper does not regress in the future.

### Commits

Once code, tests, and documentation have been written, a commit needs to be
made. Following the [commit guidelines](COMMITS.md) will help with the review
process by making your change easier to understand, and makes it easier for
hyper to produce a valuable changelog with each release.

However, if your message doesn't perfectly match the guidelines, **do not
worry!** The person that eventually merges can easily fixup the message at that
time.

### Opening the Pull Request

From within GitHub, open a new pull request from your personal branch.

Once opened, pull requests are usually reviewed within a few days.

### Discuss and Update

You will probably get feedback or requests for changes to your Pull Request.
This is a big part of the submission process so don't be discouraged! Some
contributors may sign off on the Pull Request right away, others may have more
detailed comments or feedback. This is a necessary part of the process in order
to evaluate whether the changes are correct and necessary.

Any community member can review a PR and you might get conflicting feedback.
Keep an eye out for comments from code owners to provide guidance on
conflicting feedback.

You don't need to close the PR and create a new one to address feedback. You
may simply push new commits to the existing branch.
