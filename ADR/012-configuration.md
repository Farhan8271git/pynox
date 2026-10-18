# ADR 012: Configuration file for permissions, profiles and pool limits

## Status

Accepted — 2026-10-18.

## Context

With the sandbox as the default, every project needs to state which paths,
variables and network its scripts may use, and servers need to say so once
rather than on every command line or request. Agent integrators asked for
the host to define the envelope and for each call to pick within it.

## Decision

`pynox.toml` holds three things. `[permissions]` are the defaults for
sandboxed command-line runs; `[profiles.NAME]` are named permission sets
selected with `--profile` or a pool request's `profile`; `[pool]` holds
`workers`, `preload`, `package_dirs` and `[pool.limits]`, a ceiling every
request must stay within: paths beneath the listed ones, no network unless
allowed, timeout, memory and output no larger than the limit.

Discovery is deliberately narrow: `--config PATH`, then `PYNOX_CONFIG`, then
`./pynox.toml` in the current directory. A file beside the script is never
consulted, because a program allowed to write its own directory must not be
able to widen its permissions. Relative paths resolve against the file's
directory. A configuration beneath any write grant it defines is rejected.
Unknown keys are errors.

Layering is explicit: configuration, then profile, then the command line, with
lists concatenating and scalars overriding. On the command line flags widen
the configured defaults, because the person typing them is the operator. Pool
requests start from their profile, add their own fields, and are then checked
against `[pool.limits]`; `[permissions]` does not apply to the pool.

## Alternatives considered

- Permissions only on the command line: unworkable once the sandbox is the
  default for a whole project.
- Discover configuration next to the script, like many tools: lets executed
  code configure its own sandbox.
- Flags that narrow configured permissions: safer in theory, confusing for the
  operator who owns both.
- Per-request configuration paths in the pool: lets a requester choose policy.

## Consequences

Pool deployments become `pynox pool --config /etc/pynox/prod.toml`, with
requests carrying `code` and `profile`. The configuration schema is a public
contract; changes need a record. Interactive prompts, environment-specific
overlays and secrets management are not part of this file.
