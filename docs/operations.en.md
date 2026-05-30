# Operations Runbook

## Principle

Production uses one configuration file:

```sh
/data/workspace/sage-wiki-bridge-wxo/.env
```

Systemd starts the binary directly with the same env file:

```sh
/usr/local/bin/sage-wiki-bridge --env-file /data/workspace/sage-wiki-bridge-wxo/.env
```

Manual diagnostics use the same binary commands. `scripts/bridgectl.sh` remains as a thin compatibility wrapper for the default env-file path plus journald/systemctl helpers. Do not manually reconstruct process argv from the systemd unit.

## Minimal Production Config

Keep secrets and only necessary runtime overrides in `.env`:

```sh
BRIDGE_BIND_ADDR=127.0.0.1:8087
BRIDGE_WECHAT_CALLBACK_PATH=/wechat
BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED=true
BRIDGE_SAGE_WIKI_SOURCE_DIR=/data/workspace/sage-wiki/source
# Optional old verbose source format.
# BRIDGE_SAGE_WIKI_SOURCE_LOG_DIR=/data/workspace/sage-wiki-bridge-wxo/data/source-log

WECHAT_TOKEN=...
WECHAT_APP_ID=...
WECHAT_APP_SECRET=...
WECHAT_ENCODING_AES_KEY=...
WECHAT_ADMIN_OPENIDS=...

ADMIN_VIEW_KEY=...
GEMINI_API_KEY=...
TENCENT_LBS_KEY=...
JINA_API_KEY=...
```

Leave other settings unset unless the default is wrong.

## Deploy Or Update

```sh
cd /data/workspace/sage-wiki-bridge-wxo
sudo install -m 0755 target/release/sage-wiki-bridge /usr/local/bin/sage-wiki-bridge
sudo install -m 0755 scripts/bridgectl.sh /data/workspace/sage-wiki-bridge-wxo/scripts/bridgectl.sh
sudo install -m 0644 deploy/systemd/sage-wiki-bridge.service /etc/systemd/system/sage-wiki-bridge.service
sudo systemctl daemon-reload
sudo scripts/bridgectl.sh doctor
sudo systemctl restart sage-wiki-bridge
```

## Standard Checks

```sh
sudo scripts/bridgectl.sh service-status
sudo scripts/bridgectl.sh health
sudo scripts/bridgectl.sh ready
sudo scripts/bridgectl.sh status
sudo scripts/bridgectl.sh -V
sudo scripts/bridgectl.sh tail
```

The running service also exposes protected JSON status:

```sh
curl -H "Authorization: Bearer $ADMIN_VIEW_KEY" http://127.0.0.1:8087/admin/status
```

## Planned User Commands

The planned first command set stays small:

- `/new`: start a new topic boundary; the next non-command message enters a new AI source thread.
- `/status`: query recent processing summary and failures.
- `/help`: show a short command list.

Ordinary messages do not receive per-message replies by default. See [AI Source Format v1](ai-source-format.en.md) for the target thread format, the default 30-minute grouping window, and implementation status.

## Callback Debugging

If OpenResty shows 200 but the app seems silent:

1. Run `sudo scripts/bridgectl.sh -V` and confirm `WECHAT_CALLBACK_PATH` matches the public callback path.
2. Run `sudo scripts/bridgectl.sh tail` and trigger a WeChat callback.
3. First look for `http request started` / `http request completed`.
4. If HTTP access logs exist, look for `wechat callback message stored` or `wechat callback signature invalid`.
5. Run `sudo scripts/bridgectl.sh status` or request `/admin/status` and check message/job counters.
6. Check raw archive files under `data/raw`.

If there is no `http request started` log, the request did not reach the Rust app. Check OpenResty `proxy_pass`, path rewriting, and upstream status.
