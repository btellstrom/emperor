# emperor

A CI tool that detects when the evidence behind a requirement changes.

Requirements are stored and managed with [Doorstop](https://github.com/doorstop-dev/doorstop).
Doorstop already notices when a requirement, its links or its list of references change.
It does not notice when the *content* of a referenced test or file changes. emperor fills that gap.

## How it works

1. Requirements name their **justifications**, the evidence that they are fulfilled (see below).
2. `emperor accept REQ001` records a fingerprint of each justification in `emperor.lock`.
   Run it after a person has reviewed the evidence, and commit the lockfile.
3. `emperor check` fails when a justification no longer matches its fingerprint, has never been
   accepted, can no longer be found, or was dropped since it was accepted.

emperor only reads Doorstop files; it never writes them.

## Naming justifications

There are four ways, which can be combined:

| Where | Example | Fingerprinted by |
|---|---|---|
| Doorstop's `references` | `- path: docs/threat-model.md`<br>`  type: file` | the file's bytes |
| The custom `justified_by` attribute | `- type: rust-fn`<br>`  path: tests/login.rs`<br>`  item: tests::rejects_bad_password` | the function's tokens |
| A Rust attribute | `#[justifies("REQ001", "REQ002")]` on a function | the function's tokens |
| A Gherkin tag | `@justifies:REQ001` on a Scenario, Rule or Feature | the scenario's steps |

`justified_by` also accepts `type: file`. Rust functions are addressed by their module path within
the file. Their fingerprint ignores formatting and comments, but not code or attributes. A
scenario's fingerprint covers its steps, Background, doc strings, tables and Examples, but not
comments, whitespace or tags.

The attribute comes from the `emperor-tags` crate and leaves the function unchanged:

```rust
use emperor_tags::justifies;

#[test]
#[justifies("REQ001")]
fn rejects_bad_password() { /* ... */ }
```

emperor scans the project's `.rs` and `.feature` files and skips everything git ignores.

## Commands

```text
emperor [--root DIR] check [--require-justification] [--test-results JUNIT.xml] [--format human|json|github]
emperor [--root DIR] accept (REQ001 ... | --all)
emperor [--root DIR] judge REQ001 ... [--fail-on-unfulfilled]
```

- `--require-justification` also fails for active requirements without any justification.
- `--test-results` reads JUnit XML (for example from `cargo nextest run --profile ci`) and fails
  for every test that justifies a requirement and failed, was skipped or did not run.
  Only functions with a test attribute such as `#[test]` count as tests.
- `--format github` prints GitHub Actions annotations on the justification's file and line.

Exit codes: `0` no problems, `1` problems found, `2` emperor could not complete the command.

## Judge

`emperor judge` asks an external command, for example a script that calls an LLM, whether a
requirement's justifications fulfil it:

```toml
# emperor.toml
[judge]
command = ["python3", "scripts/judge.py"]
```

The command receives the requirement's text and the content of each justification as JSON on stdin,
and must print `{"verdict": "fulfilled" | "unfulfilled", "reason": "..."}`. Verdicts are advisory
unless `--fail-on-unfulfilled` is given, because a judge can be wrong or manipulated by the code it
reads. They are cached in `.emperor/verdicts.json` until the request or the command changes.

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

Doorstop already does this for `references`. Markdown items (`itemformat: markdown`) are not
supported yet and are reported as an error.

## CI

```yaml
- run: uv run doorstop --error-all   # requirement text, links, references and review status
- run: emperor check --format github # content of the justifications
```

`--error-all` matters: without it, Doorstop only warns about unreviewed changes and exits 0.

## Development

The workspace has two crates: `emperor` (library and CLI) and `emperor-tags` (the attribute macro).

Doorstop is managed with [uv](https://docs.astral.sh/uv/) as a development dependency: run `uv sync`
once, then use `uv run doorstop …`.

emperor's own requirements live in a Doorstop tree under `reqs/`: `NEED` items state what emperor is
for, and `SPEC` items refine them. Every scenario in `features/` names the `SPEC` items it justifies
with `@justifies:SPECxxx` tags, so `emperor check` on this repository checks its own specification.

Behaviour is specified in Gherkin. `features/` holds approved scenarios, which `cargo test` runs;
approved scenarios that are not implemented yet are tagged `@pending` and skipped.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
