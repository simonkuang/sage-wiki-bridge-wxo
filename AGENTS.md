# Project Constitution

This repository is an independent Rust service for bridging WeChat Official Account callbacks into a `sage-wiki` source directory.

## Operating Rules

- Prefer small, focused changes that match the existing Rust module boundaries.
- Keep runtime configuration explicit. Do not add implicit `.env` loading.
- Do not log secrets, access tokens, authorization headers, raw OpenIDs, or full callback payloads in normal logs.
- Preserve raw payloads through the archive layer, not through ad hoc debug logging.
- Keep memory usage suitable for a small VPS target. Avoid unbounded buffering of media, callback bodies, query results, or generated source.
- Add or update tests for behavior changes, especially signature checks, whitelist routing, job state transitions, config parsing, and source writes.

## Versioning And Changelog

- Every `feature` or `fix` change must update the package version in `Cargo.toml`.
- Every `feature` or `fix` change must update `CHANGELOG.md` in the same commit.
- Use semantic versioning:
  - `fix`: patch version bump.
  - backward-compatible `feature`: minor version bump.
  - breaking change: major version bump.
- Documentation-only, test-only, chore, refactor, and build changes do not require a version bump unless they change runtime behavior.
- Keep the changelog human-readable. Group entries under `Added`, `Changed`, `Fixed`, `Security`, or `Documentation` as appropriate.
- If the change adds a new CLI flag, command, environment key, database migration, or operational behavior, mention it in `CHANGELOG.md`.

## Documentation

- Keep English and Chinese README files aligned for user-facing behavior.
- Keep English and Chinese product/technical design docs aligned when the design or operational contract changes.
- Prefer links from README files to detailed design docs instead of duplicating long explanations.

## Verification

- Run `cargo fmt` after Rust edits.
- Run `cargo test` before committing feature or fix changes.
- For CLI behavior changes, manually run the affected command shape when practical.
