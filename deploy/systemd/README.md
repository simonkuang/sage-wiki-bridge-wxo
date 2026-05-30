# systemd deployment

Example layout:

```sh
/usr/local/bin/sage-wiki-bridge
/data/workspace/sage-wiki-bridge-wxo/scripts/bridgectl.sh
/data/workspace/sage-wiki-bridge-wxo/data/
/data/workspace/sage-wiki-bridge-wxo/.env
/etc/systemd/system/sage-wiki-bridge.service
```

Install outline:

```sh
sudo useradd --system --home /data/workspace/sage-wiki-bridge-wxo --shell /usr/sbin/nologin sagewiki
sudo mkdir -p /data/workspace/sage-wiki-bridge-wxo/scripts /data/workspace/sage-wiki-bridge-wxo/data
sudo install -m 0755 target/release/sage-wiki-bridge /usr/local/bin/sage-wiki-bridge
sudo install -m 0755 scripts/bridgectl.sh /data/workspace/sage-wiki-bridge-wxo/scripts/bridgectl.sh
sudo install -m 0600 .env.example /data/workspace/sage-wiki-bridge-wxo/.env
sudo install -m 0644 deploy/systemd/sage-wiki-bridge.service /etc/systemd/system/sage-wiki-bridge.service
sudo chown -R sagewiki:sagewiki /data/workspace/sage-wiki-bridge-wxo
sudo systemctl daemon-reload
sudo systemctl enable --now sage-wiki-bridge
```

Before starting, edit `/data/workspace/sage-wiki-bridge-wxo/.env`. It contains two groups:

- `BRIDGE_*`: optional deployment/runtime overrides. Unset values use binary defaults.
- non-`BRIDGE_*`: secrets and environment-bound identifiers loaded by the binary via `--env-file`.

For the current production layout, keep or set at least these:

- `BRIDGE_SAGE_WIKI_SOURCE_DIR`
- `BRIDGE_WECHAT_CALLBACK_PATH`
- `BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED`
- `BRIDGE_BIND_ADDR` if OpenResty does not proxy to `127.0.0.1:8080`
- `WECHAT_TOKEN`
- `WECHAT_APP_ID`
- `WECHAT_APP_SECRET`
- `WECHAT_ENCODING_AES_KEY` when encrypted callback mode is enabled
- `WECHAT_ADMIN_OPENIDS`
- `ADMIN_VIEW_KEY`
- provider keys such as `GEMINI_API_KEY`, `TENCENT_LBS_KEY`, and `JINA_API_KEY`

After editing `/data/workspace/sage-wiki-bridge-wxo/.env`:

```sh
cd /data/workspace/sage-wiki-bridge-wxo
sudo scripts/bridgectl.sh doctor
sudo systemctl restart sage-wiki-bridge
sudo scripts/bridgectl.sh service-status
sudo scripts/bridgectl.sh health
sudo scripts/bridgectl.sh ready
sudo scripts/bridgectl.sh status
sudo scripts/bridgectl.sh tail
```

Use the same binary-backed wrapper for diagnostics; this avoids manually reconstructing runtime config:

```sh
sudo ENV_FILE=/data/workspace/sage-wiki-bridge-wxo/.env /data/workspace/sage-wiki-bridge-wxo/scripts/bridgectl.sh -V
sudo ENV_FILE=/data/workspace/sage-wiki-bridge-wxo/.env /data/workspace/sage-wiki-bridge-wxo/scripts/bridgectl.sh status
```

Command reference:

- `sage-wiki-bridge doctor --env-file /data/workspace/sage-wiki-bridge-wxo/.env`: preflight check for required secrets, writable dirs, and local URLs.
- `sage-wiki-bridge -V --env-file /data/workspace/sage-wiki-bridge-wxo/.env`: print resolved config and sources.
- `sage-wiki-bridge health --env-file /data/workspace/sage-wiki-bridge-wxo/.env`: call local `/healthz`.
- `sage-wiki-bridge ready --env-file /data/workspace/sage-wiki-bridge-wxo/.env`: call local `/readyz`.
- `sage-wiki-bridge status --env-file /data/workspace/sage-wiki-bridge-wxo/.env`: print database-backed process/config/message/job counters.
- `GET /admin/status`: return protected JSON status; authenticate with `?key=` or `Authorization: Bearer`.
- `bridgectl.sh service-status`: run `systemctl status --no-pager`.
- `bridgectl.sh tail`: follow journald logs.
- `bridgectl.sh logs`: print journald logs; pass extra journalctl flags after `logs`.

The binary does not load `.env` implicitly. Config sources must be enabled explicitly:

```sh
/usr/local/bin/sage-wiki-bridge --env-file /data/workspace/sage-wiki-bridge-wxo/.env
```

Every config value also has a CLI flag. CLI flags override values from `--env-file`; use `--use-process-env` only when you intentionally want process environment variables to participate. The packaged systemd unit starts the binary directly, and the binary natively reads secrets plus `BRIDGE_*` runtime overrides from `/data/workspace/sage-wiki-bridge-wxo/.env`.

The unit sets `MemoryMax=256M` to match the target VPS budget. If the configured `BRIDGE_SAGE_WIKI_SOURCE_DIR` differs, update `ReadWritePaths` before starting.
