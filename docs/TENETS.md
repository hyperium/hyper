# Charter

> hyper is a protective and efficient HTTP library for all.

# Tenets

Tenets are guiding principles. They guide how decisions are made for the whole
project. Ideally, we do all of them all the time. In some cases, though, we may
be forced to decide between slightly penalizing one goal or another. In that
case, we tend to support those goals that come earlier in the list over those
that come later (but every case is different).

## 0. Open

hyper is open source, always. The success of hyper depends on the health of the
community building and using it. All contributions are in the open. We don't
maintain private versions, and don't include features that aren't useful to
others.

[We prioritize kindness](./CODE_OF_CONDUCT.md), compassion and empathy towards all
contributors. Technical skill is not a substitute for human decency.

### Examples

It's not usually hard for an open source library to stay open and also meet its
other priorities. Here's some instances where being **Open** would be more
important than **Correct** or **Fast**:

- Say an individual were to bring forward a contribution that makes hyper more
  correct, or faster, perhaps fixing some serious bug. But in doing so, they
  also insulted people, harassed other contributors or users, or shamed
  everyone for the previous code. They felt their contribution was "invaluable".
  We would not accept such a contribution, instead banning the user and
  rewriting the code amongst the kind collaborators of the project.

- Say someone brings a contribution that adds a new feature useful for
  performance or correctness, but their work accomplishes this by integrating
  hyper with a proprietary library. We would not accept such a contribution,
  because we don't want such a feature limited only to those users willing to
  compromise openness, and we don't want to bifurcate the ecosystem between those
  who make that compromise and those who don't.

## 1. Correct

hyper is a memory safe and precise implementation of the HTTP specification.
Memory safety is vital in a core Internet technology. Following the HTTP
specifications correctly protects users. It makes the software durable to the
“real world”. Where feasible, hyper enforces correct usage.

This is more than just "don't write bugs". hyper actively protects the user.

### Examples

- Even though we follow the **HTTP/\*** specs, hyper doesn't blindly implement
  everything without considering if it is safe to do so.

## 2. Fast

A fast experience delights users. A faster network library means a faster
application, resulting in delighting our users’ users. Whether with one request,
or millions.

Being _fast_ means we improve throughput, drive down CPU usage, and improve
sustainability.

Fast _enough_. We don't sacrifice sanity for speed.

## 3. HTTP/*

hyper is specifically focused on HTTP. Supporting new HTTP versions is in scope,
but supporting separate protocols is not.

This also defines what the abstraction layer is: the API is designed around
sending and receiving HTTP messages.

## 4. Flexible

hyper enables as many usecases as possible. It has no opinion on application
structure, and makes few assumptions about its environment. This includes being
portable to different operating systems.

### Examples

- While we choose safer defaults to be **Correct**, hyper includes options to
  _allow_ different behavior, when the user requires them.
- Providing choice usually makes things more complex, so being **Flexible** does
  mean it's less _easy_. That can sometimes conflict with simplest way of making
  hyper **Understandable**.

## 5. Understandable

hyper is [no more complicated than it has to
be](https://en.wikipedia.org/wiki/Occam%27s_razor). HTTP is not simple. It may
not be as "easy" as 1-line to do everything, but it shouldn't be "hard" to find
the answers.

From logical and misuse-resistant APIs, to stellar documentation, to transparent
metrics.
