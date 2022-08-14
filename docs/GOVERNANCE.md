# Governance

## Making decisions

There's two main pieces to the way decisions are made in hyper:

1. A decision-making framework
2. The people who apply it

The people are described [lower down in this document](#roles).

### Decision-making framework

We start with the users. The project wouldn't exist without them, and it exists
in order to enable users to do amazing things with HTTP. We listen to our
users. Some actively contribute their thoughts, but many others we must seek
out to learn about their usage, joys, and headaches. Those insights allow our
experts to determine the best solutions for the users.

We then define a set of [TENETS](./TENETS.md), which are guiding principles
that can be used to measure aspects of individual decisions. It should be
possible to identify one or more tenets that apply to why a decision is made.
And the set helps us balance which priorities are more important for our users.

We combine the usecases with the tenets to come up with a [VISION](./VISION.md)
that provides a longer-term plan of what hyper _should_ look like.

Finally, we define a [ROADMAP](./ROADMAP.md) that describes what the
short-term, tactical changes to bring hyper closer in line with the vision.

## Roles

These are the roles people can fill when participating in the project. A list
of the people in each role is available in [MAINTAINERS](./MAINTAINERS.md).

### Contributor

A contributor is anyone who contributes their time to provide value for the
project. This could be in the form of code, documentation, filing issues,
discussing designs, or helping others on the issue tracker or in chat.

All contributors MUST follow the [Code of Conduct][coc].

👋  **New here?** [This could be you!][contrib]


### Triager

Triagers assess issues on the issue tracker. They help make sure the work is
well organized, and are critical for making new issue reporters feeling
welcome.

Responsibilities:

- Adhere to the [Code of Conduct][coc]
- Follow the [triager's guide][triage-guide]

Privileges:

- Can edit, label, and close issues
- Member of the organization
- Can be assigned issues and pull requests

How to become:

- Make a few [contributions][contrib] to the project, to show you can follow
  the [Code of Conduct][coc].
- Self-nominate by making a pull request adding yourself to the
  [list](./MAINTAINERS.md#triagers).


### Collaborator

Collaborators are contributors who have been helping out in a consistent basis.

Responsibilities:

- Be exemplars of the [Code of Conduct][coc]
- Internalize the [VISION](./VISION.md)
- Reviewing pull requests from other contributors
- Provide feedback on proposed features and design documents
- [Welcome new contributors][triage-guide]
- Answer questions in issues and/or chat
- Mentor contributors to become members

Privileges:

- Can review and merge pull requests
- Can trigger re-runs of CI, and approve CI for first-time contributors
- Can assign issues and pull requests to other organization members

How to become:

- Work at fulfilling the above responsibilities.
- Any collaborator may nominate a contributor who has been around for some time
  and is already filling the responsibilities.
- Another collaborator must second the nomination.
- If there are no objections, a maintainer will welcome the new collaborator.

Don't be afraid to ask a collaborator for what you could work on to reach this
goal!

### Maintainer

Maintainers are the project admins. Besides being a collaborator, they take care
house-keeping duties, help lead the direction, and have the final authority when
required.

[coc]: ./CODE_OF_CONDUCT.md
[contrib]: ../CONTRIBUTING.md
[triage-guide]: ./ISSUES.md#triaging
