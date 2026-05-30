# AI Source Format v1

This document defines the AI-friendly source format written to `SAGE_WIKI_SOURCE_DIR`. The verbose audit format remains under `SAGE_WIKI_SOURCE_LOG_DIR` for debugging, recovery, and traceability.

## Goals

AI-friendly means more than shorter Markdown. The output should let downstream LLMs understand, with low token cost:

- which messages belong to the same topic;
- the time, type, and processed content of each message;
- whether content came from Jina Reader, Tencent LBS, Gemini, or user text;
- what is user content versus bridge structure.

It should avoid putting audit-only fields such as `openid_hash`, `raw_dir`, and `bridge_version` into the main retrieval context.

## Output Layers

| Output | Directory | Purpose | Content |
| --- | --- | --- | --- |
| AI source | `SAGE_WIKI_SOURCE_DIR` | `sage-wiki compile --watch` and MCP retrieval | thread-level compact content |
| Source log | `SAGE_WIKI_SOURCE_LOG_DIR` | audit and recovery | per-message verbose metadata |
| Raw archive | `RAW_ARCHIVE_DIR` | original evidence | callback XML, message JSON, media, external payloads |
| Processed artifacts | `PROCESSED_ARTIFACT_DIR` | intermediate outputs | LLM/Jina/LBS outputs and `processed.md` |

## Thread Boundaries

The primary AI source unit is a `wechat-thread`, not a single WeChat message.

Default grouping rules:

- same OpenID;
- adjacent message gap no greater than `AI_SOURCE_THREAD_WINDOW_MINUTES`, default 30 minutes;
- after `/new`, the next non-command message starts a new thread;
- text following location/image/voice/video/link within the window is grouped with the prior message because it is often an explanation;
- command messages are recorded in DB/source log but do not enter AI source.

Bridge should keep this conservative and avoid complex semantic merging. Higher-level semantic consolidation can happen later in sage-wiki.

## Recommended Format

AI source remains daily `YYYY-MM-DD.md`, but each file contains thread blocks:

```text
<!-- swb:thread v=1 id=20260531T102000Z_abc123 -->
<<< wechat-thread >>>

<!-- swb:item:start:20260531T102000Z_abc123 -->
[2026-05-31T10:20:00Z location]
Guangzhou Tianhe District...
coordinates: 23.134521,113.358803
adcode: 440106
<!-- swb:item:end:20260531T102000Z_abc123 -->

<!-- swb:item:start:20260531T102213Z_def456 -->
[2026-05-31T10:22:13Z text]
This is the store I mentioned. Traffic is weak, but the rent is high.
<!-- swb:item:end:20260531T102213Z_def456 -->

<<< /wechat-thread >>>
<!-- /swb:thread -->
```

The `<<< wechat-thread >>>` delimiters are intentionally not Markdown headings, because user text may already contain Markdown. HTML comments are for idempotent upsert; the visible block is for humans and LLMs.

## Message Item Rules

Each item keeps only:

```text
[timestamp message_type]
processed content
```

Timestamps stay in AI source because they preserve order and help resolve references such as “the location I just sent”. Audit-only identifiers stay in source log.

## Message Type Handling

Text without URLs is written as user text. Text with URLs keeps the user's surrounding context and appends reader content:

```text
[2026-05-31T10:22:13Z text]
Please look at this page. I think it is enough to judge.

<reader url="https://example.com/article">
...Jina Reader content...
</reader>
```

Link messages use Jina Reader content as the body while keeping the URL. Location messages use a compact reverse-geocode summary, not the full JSON. Media messages use the LLM/ASR result and omit media IDs and local paths.

## User Commands

Keep the first command set small:

| Command | Meaning | Enters AI source |
| --- | --- | --- |
| `/new` | End the current context; the next non-command message starts a new thread | No |
| `/status` | Return recent processing summary and failures | No |
| `/help` | Return a short command list | No |

The whitelist join command remains separately configured by `WHITELIST_JOIN_COMMAND`.

## Notification Strategy

Do not reply to every ordinary message by default.

Recommended first phase:

- ordinary message: no active reply;
- `/new`: reply that a new topic has started;
- `/status`: reply with recent counts, written messages, and failures;
- `/help`: reply with the command list;
- processing failures: query through `/status` first; active failure summaries can be added later.

## Configuration

Setting:

| Key | CLI | Default | Meaning |
| --- | --- | --- | --- |
| `AI_SOURCE_THREAD_WINDOW_MINUTES` | `--ai-source-thread-window-minutes` | `30` | Auto-grouping window for adjacent messages from the same user |

This is a quiet config with a production-safe default. It should usually be omitted from `.env`.

## Implementation Note

Version `0.6.0` implements this first runtime contract: thread-level AI source, `/new`/`/status`/`/help` for whitelisted users, idempotent thread item upsert, and unchanged verbose source logs.
