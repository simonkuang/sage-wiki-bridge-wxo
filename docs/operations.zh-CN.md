# 运维 Runbook

## 原则

生产环境只维护一份配置文件:

```sh
/data/workspace/sage-wiki-bridge-wxo/.env
```

systemd 直接用同一份 env file 启动 binary:

```sh
/usr/local/bin/sage-wiki-bridge --env-file /data/workspace/sage-wiki-bridge-wxo/.env
```

手工诊断也走同一套 binary 子命令。`scripts/bridgectl.sh` 只保留生产默认 env-file 路径和 journald/systemctl 辅助命令。不要再从 systemd unit 里手工复制参数拼命令。

## 最小生产配置

`.env` 只保留 secrets 和必要运行覆盖项:

```sh
BRIDGE_BIND_ADDR=127.0.0.1:8087
BRIDGE_WECHAT_CALLBACK_PATH=/wechat
BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED=true
BRIDGE_SAGE_WIKI_SOURCE_DIR=/data/workspace/sage-wiki/source
# 可选：旧版详细 source 格式。
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

其它配置默认不写，除非二进制默认值不符合线上实际。

## 部署或更新

```sh
cd /data/workspace/sage-wiki-bridge-wxo
sudo install -m 0755 target/release/sage-wiki-bridge /usr/local/bin/sage-wiki-bridge
sudo install -m 0755 scripts/bridgectl.sh /data/workspace/sage-wiki-bridge-wxo/scripts/bridgectl.sh
sudo install -m 0644 deploy/systemd/sage-wiki-bridge.service /etc/systemd/system/sage-wiki-bridge.service
sudo systemctl daemon-reload
sudo scripts/bridgectl.sh doctor
sudo systemctl restart sage-wiki-bridge
```

## 标准检查

```sh
sudo scripts/bridgectl.sh service-status
sudo scripts/bridgectl.sh health
sudo scripts/bridgectl.sh ready
sudo scripts/bridgectl.sh status
sudo scripts/bridgectl.sh -V
sudo scripts/bridgectl.sh tail
```

运行中的服务还提供受保护的 JSON 状态接口:

```sh
curl -H "Authorization: Bearer $ADMIN_VIEW_KEY" http://127.0.0.1:8087/admin/status
```

## 用户 Command 规划

用户侧 command 规划第一阶段保持克制:

- `/new`: 开启新话题边界, 下一条非 command 消息进入新的 AI source thread。
- `/status`: 查询最近处理摘要和失败情况。
- `/help`: 查看 command 简表。

普通消息默认不逐条回复。AI source 的目标 thread 格式和 30 分钟默认聚合窗口见 [AI Source Format v1](ai-source-format.zh-CN.md)。当前实现状态以该文档的 “实现备注” 为准。

## Callback 排查

如果 OpenResty 显示 200，但 app 看起来没反应:

1. 执行 `sudo scripts/bridgectl.sh -V`，确认 `WECHAT_CALLBACK_PATH` 等于公网 callback path。
2. 执行 `sudo scripts/bridgectl.sh tail`，再触发一次微信 callback。
3. 先看日志里有没有 `http request started` / `http request completed`。
4. 如果有 HTTP access log，再看有没有 `wechat callback message stored` 或 `wechat callback signature invalid`。
5. 执行 `sudo scripts/bridgectl.sh status` 或请求 `/admin/status`，看 message/job 计数是否变化。
6. 检查 `data/raw` 下是否有新增归档文件。

如果连 `http request started` 都没有，说明请求没有进入 Rust app。重点检查 OpenResty 的 `proxy_pass`、路径重写和 upstream status。
