# AI Source Format v1

本文定义 `sage-wiki-bridge` 写入 `SAGE_WIKI_SOURCE_DIR` 的 AI 友好 source 格式。旧版详细日志格式继续写入 `SAGE_WIKI_SOURCE_LOG_DIR`，用于审计、排错和还原。

## 设计目标

AI 友好不是简单压缩字段，而是让下游 LLM 在较少 tokens 内稳定理解:

- 哪些消息属于同一件事。
- 每条消息的时间、类型和处理后内容。
- 外部处理结果来自哪里，例如 Jina Reader、腾讯 LBS、Gemini。
- 哪些内容是用户原文，哪些内容是 bridge 生成的结构。

同时避免:

- 把 `openid_hash`、`raw_dir`、`bridge_version` 等审计字段塞进主上下文。
- 用太多 Markdown heading 污染用户自己发送的 Markdown。
- 把 location/image/voice/video 与后续解释文本拆散，让下游 LLM 猜不出上下文关系。

## 输出分层

| 输出 | 目录 | 用途 | 内容 |
| --- | --- | --- | --- |
| AI source | `SAGE_WIKI_SOURCE_DIR` | 给 `sage-wiki compile --watch` 和 MCP 检索 | thread 级别、精简字段、处理后内容 |
| Source log | `SAGE_WIKI_SOURCE_LOG_DIR` | 审计和恢复 | 逐消息、完整 metadata、旧版详细格式 |
| Raw archive | `RAW_ARCHIVE_DIR` | 原始证据 | callback XML、message JSON、媒体、外部 payload |
| Processed artifacts | `PROCESSED_ARTIFACT_DIR` | 中间产物 | LLM/Jina/LBS 返回内容、processed.md |

## Thread 边界

AI source 的基本知识单元是 `wechat-thread`，不是单条微信消息。

默认分组规则:

- 同一 OpenID。
- 相邻消息间隔不超过 `AI_SOURCE_THREAD_WINDOW_MINUTES`，默认 30 分钟。
- 用户发送 `/new` 后，下一条非 command 消息必须开启新 thread。
- location/image/voice/video/link 后紧跟 text 时，默认归入同一 thread，因为 text 很可能是在解释前一条消息。
- command 消息只入库和 source log，不写入 AI source。

这个规则故意保守，不在 bridge 里做复杂语义判断。后续如果需要语义归并，应由 sage-wiki 或专门的整理 job 在 thread 基础上继续处理。

## 推荐格式

AI source 仍按天写入 `YYYY-MM-DD.md`，但每天文件内由多个 thread block 组成。

```text
<!-- swb:thread v=1 id=20260531T102000Z_abc123 -->
<<< wechat-thread >>>

<!-- swb:item:start:20260531T102000Z_abc123 -->
[2026-05-31T10:20:00Z location]
广东省广州市天河区...
坐标: 23.134521,113.358803
行政区划: 广东省 / 广州市 / 天河区
adcode: 440106
<!-- swb:item:end:20260531T102000Z_abc123 -->

<!-- swb:item:start:20260531T102213Z_def456 -->
[2026-05-31T10:22:13Z text]
这里就是我刚才说的那个门店，客流很差，但租金还很贵。
<!-- swb:item:end:20260531T102213Z_def456 -->

<<< /wechat-thread >>>
<!-- /swb:thread -->
```

选择 `<<< wechat-thread >>>` 而不是 Markdown heading 的原因:

- 用户文本可能本身就是 Markdown，heading 容易和用户内容混在一起。
- 这种边界对 LLM 清晰，对普通 Markdown 渲染也足够低干扰。
- HTML comment marker 用于程序幂等 upsert，thread block 用于人和 AI 阅读。

## Message Item 格式

每条 message item 只保留必要元信息:

```text
[timestamp message_type]
processed content
```

保留 timestamp 的理由:

- 帮助 LLM 理解先后顺序。
- 支持“刚才那个位置”“上一条语音里说的事”等相对指代。
- 在审计日志和 raw archive 中可反查原消息。

不保留 `msg_id`、`openid_hash`、`raw_dir` 的理由:

- 它们对 AI 回答通常没有语义价值。
- 会增加 token 成本。
- 会把内部实现细节暴露给下游上下文。

## 各消息类型内容规则

### Text

- 无 URL: 写用户文本原文。
- 有 URL: 保留用户上下文，同时追加 Jina Reader 内容。

```text
[2026-05-31T10:22:13Z text]
你看看这个页面，我觉得信息足够判断。

<reader url="https://example.com/article">
...Jina Reader 内容...
</reader>
```

### Link

微信 `link` 消息使用 Jina Reader 内容作为主体，同时保留 URL。

```text
[2026-05-31T10:22:13Z link]
url: https://example.com/article

<reader>
...Jina Reader 内容...
</reader>
```

### Location

写逆地址解析摘要，不把完整 LBS JSON 放进 AI source。完整 JSON 存 processed artifacts/source log。

```text
[2026-05-31T10:20:00Z location]
广东省广州市天河区...
坐标: 23.134521,113.358803
行政区划: 广东省 / 广州市 / 天河区
adcode: 440106
```

### Image / Voice / Video / Short Video

写 LLM/ASR 处理后的自然语言结果，不写媒体 ID、临时 URL、下载路径。

```text
[2026-05-31T10:21:00Z image]
图片中是一张店铺门口照片，门头为...
```

如果后续 text 在 thread window 内出现，应归入同一 thread。

## 用户 Command

第一阶段只保留少量 command，避免行为复杂化。

| Command | 作用 | 是否进入 AI source |
| --- | --- | --- |
| `/new` | 结束当前上下文，下一条非 command 消息开启新 thread | 否 |
| `/status` | 查询最近处理摘要和失败情况 | 否 |
| `/help` | 返回可用 command 简表 | 否 |

白名单加入 command 仍由 `WHITELIST_JOIN_COMMAND` 单独配置，不和上述 command 混用。

## 通知策略

默认不对每条普通消息回复，避免刷屏。

第一阶段推荐:

- 普通消息: 不主动回复。
- `/new`: 回复“已开始新的话题”。
- `/status`: 回复最近处理摘要，例如“今日已接收 5 条，已写入 4 条，失败 1 条”。
- `/help`: 回复 command 列表。
- 处理失败: 暂不主动逐条回复，先通过 `/status` 查询。后续可增加失败摘要或管理员通知。

不做 debounce 汇总的原因:

- 微信公众号被动回复有时限，异步处理结果通常晚于 callback。
- 主动客服消息有平台限制。
- command 查询简单、可控、不扰民。

## 配置

配置项:

| Key | CLI | 默认值 | 说明 |
| --- | --- | --- | --- |
| `AI_SOURCE_THREAD_WINDOW_MINUTES` | `--ai-source-thread-window-minutes` | `30` | 同一用户相邻消息自动归入同一 thread 的时间窗口 |

该参数属于静默配置: 默认值适合生产，通常不需要写入 `.env`。只有当实际使用习惯明显不同，才通过 CLI 或 `BRIDGE_AI_SOURCE_THREAD_WINDOW_MINUTES` 覆盖。

## 实现备注

`0.6.0` 已实现本文定义的第一版运行契约:

- 使用 thread 作为 AI source 的主要知识单元。
- `/new`、`/status`、`/help` 仅对白名单用户生效。
- AI source 按 thread item 幂等 upsert。
- source log 保持逐消息详细格式不变。
