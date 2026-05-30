# Changelog

All notable changes to this project are documented here.

This project follows semantic versioning. Feature and fix changes must update both `Cargo.toml` and this changelog in the same commit.

## [Unreleased]

## [0.3.0] - 2026-05-30

### Added

- `scripts/bridgectl.sh doctor` for deployment preflight checks covering binary, env file, required secrets, writable directories, and local URLs.
- `scripts/bridgectl.sh health`, `ready`, `logs`, `tail`, `service-status`, and `argv` as standard operations and diagnostics commands.

### Changed

- `scripts/status.sh` now delegates to the same `bridgectl.sh status` path used by packaged deployments.
- Operations documentation now presents a fixed deploy/check/debug workflow centered on a single `.env` and `bridgectl.sh`.

## [0.2.2] - 2026-05-30

### Fixed

- Systemd deployment paths now match the production layout: project under `/data/workspace/sage-wiki-bridge-wxo`, binary at `/usr/local/bin/sage-wiki-bridge`, and sage-wiki source at `/data/workspace/sage-wiki/source`.
- Deployment now uses a single dotenv file at `/data/workspace/sage-wiki-bridge-wxo/.env`.
- `bridgectl.sh` now passes only explicitly configured `BRIDGE_*` overrides, letting all other runtime options use binary defaults.

## [0.2.1] - 2026-05-29

### Fixed

- Receiver now logs WeChat callback verification, signature failures, message storage, and job queueing so production callback handling is visible in systemd logs.
- Systemd and manual diagnostics now share `scripts/bridgectl.sh`, allowing `run`, `-V`, `status`, and `version` to use the same env-file-driven argument mapping.

## [0.2.0] - 2026-05-29

### Changed

- Source Markdown output is now grouped into one file per received date, using `YYYY-MM-DD.md` under the configured `sage-wiki` source directory.
- Daily source files upsert message sections by hidden message markers so worker retries do not duplicate the same message entry.

## [0.1.0] - 2026-05-29

### Added

- Initial Rust bridge service for WeChat Official Account callbacks.
- Parsing and routing for text, image, voice, video, short video, location, and link messages.
- OpenID whitelist, honeypot handling, and magic-command whitelist join.
- Raw archive, processed artifact storage, SQLite persistence, worker queue, and atomic Markdown source writes.
- Gemini media processing, Tencent LBS reverse geocoding, and Jina Reader link extraction.
- Read-only admin list and detail views.
- Explicit configuration precedence: CLI flags, explicit env file, optional process env, built-in defaults.
- Runtime inspection commands: `--version`, `version`, `-V`, and `status`.
- Systemd deployment templates and bilingual README/design documentation.
