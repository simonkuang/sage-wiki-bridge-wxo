# systemd deployment

Example layout:

```sh
/opt/sage-wiki-bridge/bin/sage-wiki-bridge
/opt/sage-wiki-bridge/data/
/etc/sage-wiki-bridge.env
/etc/systemd/system/sage-wiki-bridge.service
```

Install outline:

```sh
sudo useradd --system --home /opt/sage-wiki-bridge --shell /usr/sbin/nologin sagewiki
sudo mkdir -p /opt/sage-wiki-bridge/bin /opt/sage-wiki-bridge/data
sudo install -m 0755 target/release/sage-wiki-bridge /opt/sage-wiki-bridge/bin/sage-wiki-bridge
sudo install -m 0600 deploy/systemd/sage-wiki-bridge.env.example /etc/sage-wiki-bridge.env
sudo install -m 0644 deploy/systemd/sage-wiki-bridge.service /etc/systemd/system/sage-wiki-bridge.service
sudo chown -R sagewiki:sagewiki /opt/sage-wiki-bridge
sudo systemctl daemon-reload
sudo systemctl enable --now sage-wiki-bridge
```

Before starting, edit `/etc/sage-wiki-bridge.env`. It contains two groups:

- `BRIDGE_*`: deployment and runtime knobs consumed by systemd and passed to the process as CLI flags.
- non-`BRIDGE_*`: secrets and environment-bound identifiers loaded by the binary via `--env-file`.

All `BRIDGE_*` variables in the example have defaults. Change at least these before production use:

- `BRIDGE_SAGE_WIKI_SOURCE_DIR`
- `BRIDGE_WHITELIST_JOIN_COMMAND`
- `WECHAT_TOKEN`
- `WECHAT_APP_ID`
- `WECHAT_APP_SECRET`
- `WECHAT_ADMIN_OPENIDS`
- `ADMIN_VIEW_KEY`
- provider keys such as `GEMINI_API_KEY`, `TENCENT_LBS_KEY`, and `JINA_API_KEY`

After editing `/etc/sage-wiki-bridge.env`:

```sh
sudo systemctl restart sage-wiki-bridge
sudo journalctl -u sage-wiki-bridge -f
```

The binary does not load `.env` implicitly. Config sources must be enabled explicitly:

```sh
/opt/sage-wiki-bridge/bin/sage-wiki-bridge --env-file /etc/sage-wiki-bridge.env
```

Every config value also has a CLI flag. CLI flags override values from `--env-file`; use `--use-process-env` only when you intentionally want process environment variables to participate. The packaged systemd unit reads `/etc/sage-wiki-bridge.env`, expands `BRIDGE_*` variables into CLI flags, and passes the same file to the binary with `--env-file` for secrets. Avoid defining app config keys that duplicate CLI flags unless you intentionally want the CLI flag to override the env-file value.

The unit sets `MemoryMax=256M` to match the target VPS budget. If the configured `SAGE_WIKI_SOURCE_DIR` differs, update `ReadWritePaths` before starting.
