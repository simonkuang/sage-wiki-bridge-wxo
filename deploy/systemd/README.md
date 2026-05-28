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

After editing `/etc/sage-wiki-bridge.env`:

```sh
sudo systemctl restart sage-wiki-bridge
sudo journalctl -u sage-wiki-bridge -f
```

The binary does not load `.env` implicitly. Config sources must be enabled explicitly:

```sh
/opt/sage-wiki-bridge/bin/sage-wiki-bridge --env-file /etc/sage-wiki-bridge.env
```

Every config value also has a CLI flag. CLI flags override values from `--env-file`; use `--use-process-env` only when you intentionally want process environment variables to participate. The packaged systemd unit keeps operational startup settings in `ExecStart` flags and keeps secrets in `/etc/sage-wiki-bridge.env`.

The unit sets `MemoryMax=256M` to match the target VPS budget. If the configured `SAGE_WIKI_SOURCE_DIR` differs, update `ReadWritePaths` before starting.
