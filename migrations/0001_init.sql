CREATE TABLE IF NOT EXISTS messages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  request_id TEXT NOT NULL,
  wechat_msg_id TEXT,
  to_user_name TEXT NOT NULL,
  from_openid TEXT NOT NULL,
  from_openid_hash TEXT NOT NULL,
  create_time INTEGER,
  received_at TEXT NOT NULL,
  message_type TEXT NOT NULL,
  content_text TEXT,
  media_id TEXT,
  thumb_media_id TEXT,
  pic_url TEXT,
  voice_format TEXT,
  voice_recognition TEXT,
  location_lat REAL,
  location_lng REAL,
  location_scale INTEGER,
  location_label TEXT,
  link_title TEXT,
  link_description TEXT,
  link_url TEXT,
  authorized INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL,
  raw_dir TEXT NOT NULL,
  source_path TEXT,
  processed_text TEXT,
  enrichment_json_path TEXT,
  reader_content_path TEXT,
  provider TEXT,
  model TEXT,
  error_kind TEXT,
  error_message TEXT,
  processed_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_wechat_msg_id
ON messages(wechat_msg_id)
WHERE wechat_msg_id IS NOT NULL AND wechat_msg_id != '';

CREATE INDEX IF NOT EXISTS idx_messages_received_at ON messages(received_at);
CREATE INDEX IF NOT EXISTS idx_messages_status ON messages(status);
CREATE INDEX IF NOT EXISTS idx_messages_type ON messages(message_type);
CREATE INDEX IF NOT EXISTS idx_messages_openid_hash ON messages(from_openid_hash);
CREATE INDEX IF NOT EXISTS idx_messages_link_url ON messages(link_url);

CREATE TABLE IF NOT EXISTS jobs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  job_type TEXT NOT NULL,
  status TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 3,
  next_run_at TEXT NOT NULL,
  locked_at TEXT,
  locked_by TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_jobs_claim ON jobs(status, next_run_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_one_process_per_message
ON jobs(message_id, job_type);

CREATE TABLE IF NOT EXISTS whitelist_subjects (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  openid TEXT NOT NULL UNIQUE,
  openid_hash TEXT NOT NULL,
  unionid TEXT,
  label TEXT,
  source TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  first_bound_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_seen_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_whitelist_enabled ON whitelist_subjects(enabled);
CREATE INDEX IF NOT EXISTS idx_whitelist_unionid ON whitelist_subjects(unionid);

