# HIP-0001: hyper Improvement Proposals

- Authors: seanmonstar

## Summary

HIPs establish a predictable, written‑first process for proposing and evaluating significant changes to hyper. They provide an authoritative place to articulate the problem, the design rationale, and the tradeoffs so contributors can make informed, durable decisions.

## Tenets

- Open: the decision-making process should be out in the open, for all to see and any to participate.
- Understandable: Code and changelogs are not the complete theory. Others should be able to understand what we meant.
- Writing is thinking: while the written artifact is extremely valuable, the process of creating is thinking itself. It refines our thought, sharpens it, makes it better. 
- Document the why: just as important is understanding why we thought that way. This allows evaluating when the decision should be reconsidered.

## Motivation

- Much of the shape of hyper can be seen in the VISION and 1.0 ROADMAP.
- Adding new small features is very easy.
- However, designing large new systems in hyper must be handled with care.
    - hyper is deployed in massive scale.
    - The stability promise requires we be deliberate with what API to expose.
- The current process is to ask people to write up larger designs in an issue.
    - GitHub issues are terrible places to discuss multiple points at once.
    - Or people submit massive pull requests, and the discussion is sprinkled through the code reviews.
- It's extremely difficult for reviewers to be sure they understand the design, and to ensure that all critical aspects have been considered.
    - If an implementation is denied for a desirable feature, new pull requests tend to be submitted which don't address the previous concerns.
    - The barrier for someone to contribute goes up due to the lack of structure.
    - And the review process slows down as reviewers struggle to continuously rebuild mental context.

## Recommendation

Introduce a structured, written process for proposing, discussing, and deciding large design changes. We call these **hyper Improvement Proposals** (HIPs). They are similar in spirit to [RFCs](https://en.wikipedia.org/wiki/Request_for_Comments).

### Scope

When should HIPs be used?

HIPs are intended for changes that significantly affect hyper's architecture, public API, performance characteristics, or long‑term maintenance burden. They are useful when decision is difficult to reverse, when there appear to be several strong options, or as a foundation for future proposals.

When are they not needed?

Small, localized changes that do not alter core design or guarantees generally do not require a HIP. 

### Content

What should be included in a HIP?

This defines at a high level the purpose of what content is included. The exact sections might change over time, and so are defined in a separate `0000-template.md` file.

But in general, HIPs should provide the following information:

- Objective - the problem, the motivation, the what are we even doing here.
- Tenets - how to measure solutions
    - A meta point here: spend more time on the objective, tenets, and alternatives. The recommendation will fall into place.
- A single recommendation
    - The job of a proposal is to propose a solution, not to propose a problem.
    - There will often be several ways to solve anything.
    - The proposal should research and measure the options using the facts and tenets, and decide which is the best.
        - The discussion during the proposal period may change what is the recommended solution.
        - The essence of those discussions should be recaptured into the proposal itself. Both in the recommendation section, and also potentially in an FAQ appendix.
        - Future readers should not need to reconstruct the design from the review comments.
- The whys (and why nots) of the solution
    - A proposal should always include why the recommendation is the best option, and why the other options were not chosen
- References
    - Links to all the relevant research that was done
    - Links to previous discussions
- Appendices
    - FAQ

#### What not to include

- Minor details that do not need to be discussed to accept the proposal.

### Lifecycle

- Start with an issue.
    - (A HIP should almost never be started without a previous discussion)
- If the feature request seems large enough, a collaborator may ask for a HIP.
- In a new branch, copy the template file to `0000-your-feature-name.md`.
- Fill out the sections based on the instructions in the template.
    - You don't need to have fully polished prose in every section to submit for discussion.
    - It can even be beneficial to submit initially with bullet points.
    - Balance giving proper thought to the sections, with moving quickly to get appropriate feedback.
- Consider sharing an early draft with someone else first.
- Submit a pull request, and include a big reminder to prefer line comments over top-level PR comments.
- As discussion occurs, revise the sections.
    - Consider adding to an FAQ appendix.
- Eventually, a maintainer will decide whether the proposal is accepted or rejected.

Once a proposal is accepted, a number will be assigned. The file in the PR can be renamed with the assigned number, and then merged into trunk.

#### Mutability and History

HIPs are mutable, even after being accepted.

Significant changes can be added to a `## History` section of the document, inserted before any `## References` or appendices. Besides explaining what changed, the reasoning for _why_ should be included.

## Alternatives

- Ad-hoc design docs
    - While design documents are an ideal artifact, without defining a process behind them, it's hard for contributors to know when to use one or how.
- Module documentation
    - Readers of module documentation is not the right audience.
    - The purpose of module documentation is to explain how to use the library.
    - It is not a great place to document the why.
- Issues and Pull Requests
    - This is the status quo.
    - Discussion in issues is a horrible experience.
    - Spreads out the full design across disconnected areas.
    - Silos the knowledge directly in GitHub.
- A chat application, such as Zulip
    - Chat-style messages are extremely poor for having thoughtful discussion and decisions.
    - They become littered with messages that have no longer-term value, and it makes it hard to piece back together what a decision is.
    - No contributor should ever _have_ to participate in chat-style messaging.

## Unresolved Questions

- Is there a point when significant changes to an accepted HIP should itself be a new HIP?
    - Possibly, but I'm not sure it's worth deciding yet.

## References

- https://rust-lang.github.io/rfcs/0002-rfc-process.html
- https://rfd.shared.oxide.computer/rfd/0001
- https://peps.python.org/pep-0001/
- https://docs.jj-vcs.dev/latest/design_docs/
- https://blog.ceejbot.com/posts/design-docs/
- https://adr.github.io/
