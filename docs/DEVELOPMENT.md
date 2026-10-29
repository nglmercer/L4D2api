# Development

## Layout

```text
addon/                  VScript addon source (packed into the .vpk)
crates/
  l4d2api-protocol/     v1 codec (reference implementation) + fixture tests
  l4d2api-client/       session client + mock-peer tests
  l4d2api-installer/    Steam discovery, install lifecycle, quarantine
  l4d2api-pack/         deterministic VPK packer + validator (lib + CLI)
sdk/
  python/               l4d2api package (stdlib only) + unittest suites
  javascript/           l4d2api package (TypeScript-first) + node:test suites
examples/
  rust/ | python/ | javascript/
tests/fixtures/         canonical protocol records (generated, checked in)
scripts/                gen_fixtures.py, lint_nut.sh
docs/                   this documentation set
```

Rust workspace members include `examples/rust` so `cargo run -p
l4d2api-example` works from the repo root.

## Workflow

- Small, grouped commits; no unrelated rewrites; no new dependencies
  without need (Python SDK is stdlib-only by design).
- Preserve working behavior: the VScript side is the live contract —
  protocol v1 fields, action names, install paths stay compatible or
  migrate safely (see installer legacy paths).
- Never modify unrelated game addons; never retry gameplay mutations
  after ambiguous failures; never touch TikTools-app from this repo.

## Gates (run before finishing)

```bash
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace -- -D warnings
bash scripts/lint_nut.sh
python3 scripts/gen_fixtures.py --check
```

Plus the SDK suites:

```bash
cd sdk/python && PYTHONPATH=src python3 -m unittest discover -s tests
cd sdk/javascript && npm test
```

VScript has no runtime here: `scripts/lint_nut.sh` is a text-pattern
lint, not a parser — say so when reporting. Squirrel parity with the
Rust codec is by careful mirroring plus the documented validation
rules, not by executed Squirrel tests.

## Versioning

Workspace version is `0.1.0` (`Cargo.toml` `[workspace.package]`, SDK
`package.json` / `pyproject.toml` / `__init__.py` track it). Bump all
four together with release notes. Protocol v1 is frozen by the
compatibility rules in [PROTOCOL.md](PROTOCOL.md).
