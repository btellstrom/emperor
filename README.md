# emperor

A CI tool that detects when the evidence behind a requirement changes.

Requirements are stored and managed with [Doorstop](https://github.com/doorstop-dev/doorstop).
Doorstop already notices when a requirement, its links or its list of references change.
It does not notice when the *content* of a referenced test or file changes. emperor fills that gap.

## How it works

1. A requirement names its justifications in its Doorstop item:
   - `references` (Doorstop's own field, `type: file` only): the whole file is the justification.
   - `justified_by` (a custom attribute): finer-grained justifications such as a single Rust function.
2. `emperor accept REQ001` records a fingerprint of each justification in `emperor.lock`.
   Run it after a human has reviewed the evidence; commit the lockfile.
3. `emperor check` fails when a justification no longer matches its fingerprint.

```yaml
# reqs/REQ001.yml
text: |
  Login shall reject a wrong password.
references:
- path: docs/threat-model.md
  type: file
justified_by:
- type: rust-fn
  path: tests/login.rs
  item: rejects_bad_password
```

emperor only reads Doorstop files; it never writes them.

## Recommended Doorstop configuration

Add `justified_by` to the review stamp of every document, so that Doorstop reports
`unreviewed changes` when someone edits which justifications a requirement has:

```yaml
# reqs/.doorstop.yml
settings:
  digits: 3
  itemformat: yaml
  prefix: REQ
  sep: ''
attributes:
  reviewed:
  - justified_by
```

Doorstop already does this for `references`.

## CI

```yaml
- run: uv run doorstop --error-all   # requirement text, links, references and review status
- run: emperor check                 # content of the justifications
```

`--error-all` matters: without it, Doorstop only warns about unreviewed changes and exits 0.

Exit codes of `emperor check`: `0` everything accepted, `1` problems found, `2` error.

## Development

Doorstop is managed with [uv](https://docs.astral.sh/uv/) as a development dependency: run `uv sync` once,
then use `uv run doorstop …`.

emperor's own requirements live in a Doorstop tree under `reqs/`: `NEED` items state what emperor is
for, and `SPEC` items refine them. Every scenario in `features/` names the `SPEC` items it justifies
with `@justifies:SPECxxx` tags.

Behaviour is specified in Gherkin. `features/` holds approved scenarios, which `cargo test` runs;
approved scenarios that are not implemented yet are tagged `@pending` and skipped.
`proposed/` holds drafts awaiting approval.
