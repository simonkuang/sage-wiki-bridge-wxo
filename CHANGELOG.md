# Changelog

All notable changes to this project are documented here.

This project follows semantic versioning. Feature and fix changes must update both `Cargo.toml` and this changelog in the same commit.

## [Unreleased]

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
