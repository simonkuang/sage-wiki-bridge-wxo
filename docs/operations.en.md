# Operations Runbook

## Principle

Production uses one configuration file:

```sh
/data/workspace/sage-wiki-bridge-wxo/.env
```

Systemd, manual diagnostics, and local checks all go through:

```sh
/data/workspace/sage-wiki-bridge-wxo/scripts/bridgectl.sh
```

Do not manually reconstruct the process argv from the systemd unit.

## Minimal Production Config

Keep secrets and only necessary runtime overrides in `.env`:

```sh
BRIDGE_BIND_ADDR=127.0.0.1:8087
BRIDGE_WECHAT_CALLBACK_PATH=/wechat
BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED=true
BRIDGE_SAGE_WIKI_SOURCE_DIR=/data/workspace/sage-wiki/source

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

Use this to inspect argv without exposing secret values:

```sh
sudo scripts/bridgectl.sh argv
```

## Callback Debugging

If OpenResty shows 200 but the app seems silent:

1. Run `sudo scripts/bridgectl.sh -V` and confirm `WECHAT_CALLBACK_PATH` matches the public callback path.
2. Run `sudo scripts/bridgectl.sh tail` and trigger a WeChat callback.
3. Look for `wechat callback message stored` or `wechat callback signature invalid`.
4. Run `sudo scripts/bridgectl.sh status` and check message/job counters.
5. Check raw archive files under `data/raw`.

If there are no receiver logs and no DB counters change, the request did not reach the Rust app route. Check OpenResty `proxy_pass`, path rewriting, and upstream status.
