//! Versioned schema migrations. Each entry runs once, inside its own transaction, and is
//! recorded in `schema_migrations`. Never edit a released migration: add a new one.

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial schema",
    sql: r#"
-- Media files live on disk; rows only hold paths and metadata. Ids are UUID strings.

CREATE TABLE recordings (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL CHECK (kind IN ('video', 'audio')),
    title         TEXT NOT NULL,
    path          TEXT NOT NULL UNIQUE,
    container     TEXT,
    duration_ms   INTEGER,
    size_bytes    INTEGER NOT NULL DEFAULT 0,
    width         INTEGER,
    height        INTEGER,
    -- ready | recording | interrupted | recovered | missing
    status        TEXT NOT NULL DEFAULT 'ready',
    thumbnail     TEXT,
    session_id    TEXT REFERENCES capture_sessions(id) ON DELETE SET NULL,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
CREATE INDEX recordings_created ON recordings(created_at DESC);
CREATE INDEX recordings_kind ON recordings(kind);

CREATE TABLE screenshots (
    id            TEXT PRIMARY KEY,
    title         TEXT NOT NULL,
    path          TEXT NOT NULL UNIQUE,
    format        TEXT NOT NULL CHECK (format IN ('png', 'jpeg', 'webp')),
    width         INTEGER NOT NULL,
    height        INTEGER NOT NULL,
    size_bytes    INTEGER NOT NULL DEFAULT 0,
    -- Editable annotations (JSON) until the image is flattened.
    annotations   TEXT,
    status        TEXT NOT NULL DEFAULT 'ready',
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
CREATE INDEX screenshots_created ON screenshots(created_at DESC);

CREATE TABLE media_metadata (
    recording_id  TEXT PRIMARY KEY REFERENCES recordings(id) ON DELETE CASCADE,
    container     TEXT,
    video_codec   TEXT,
    audio_codec   TEXT,
    width         INTEGER,
    height        INTEGER,
    fps           REAL,
    duration_ms   INTEGER,
    bitrate       INTEGER,
    video_streams INTEGER NOT NULL DEFAULT 0,
    audio_streams INTEGER NOT NULL DEFAULT 0,
    -- Full ffprobe JSON for the inspector.
    raw           TEXT,
    probed_at     INTEGER NOT NULL
);

CREATE TABLE capture_sessions (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL CHECK (kind IN ('video', 'audio')),
    -- idle | preparing | countdown | recording | paused | finalizing | completed | failed | cancelled | interrupted
    state         TEXT NOT NULL,
    source        TEXT NOT NULL,
    settings      TEXT NOT NULL DEFAULT '{}',
    output_path   TEXT,
    error         TEXT,
    started_at    INTEGER NOT NULL,
    ended_at      INTEGER
);
CREATE INDEX capture_sessions_state ON capture_sessions(state);

CREATE TABLE export_jobs (
    id            TEXT PRIMARY KEY,
    source_id     TEXT REFERENCES recordings(id) ON DELETE SET NULL,
    source_path   TEXT NOT NULL,
    output_path   TEXT NOT NULL,
    preset        TEXT NOT NULL DEFAULT '{}',
    -- queued | initializing | running | finalizing | completed | failed | cancelled
    status        TEXT NOT NULL,
    progress      REAL NOT NULL DEFAULT 0,
    error         TEXT,
    created_at    INTEGER NOT NULL,
    started_at    INTEGER,
    finished_at   INTEGER
);
CREATE INDEX export_jobs_status ON export_jobs(status);
CREATE INDEX export_jobs_created ON export_jobs(created_at DESC);

CREATE TABLE devices (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL CHECK (kind IN ('microphone', 'speaker', 'system-audio', 'webcam', 'display')),
    -- The backend's own id (PipeWire node name, WASAPI endpoint id, …).
    backend_id    TEXT NOT NULL,
    name          TEXT NOT NULL,
    is_default    INTEGER NOT NULL DEFAULT 0,
    connected     INTEGER NOT NULL DEFAULT 1,
    details       TEXT NOT NULL DEFAULT '{}',
    last_seen     INTEGER NOT NULL,
    UNIQUE (kind, backend_id)
);

CREATE TABLE settings (
    key           TEXT PRIMARY KEY,
    value         TEXT NOT NULL
);

CREATE TABLE shortcuts (
    id            TEXT PRIMARY KEY,
    action        TEXT NOT NULL UNIQUE,
    accelerator   TEXT,
    enabled       INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE tags (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color         TEXT,
    created_at    INTEGER NOT NULL
);

-- A tag or favourite points at a recording OR a screenshot: two real foreign keys, exactly one set.
CREATE TABLE media_tags (
    id            TEXT PRIMARY KEY,
    tag_id        TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    recording_id  TEXT REFERENCES recordings(id) ON DELETE CASCADE,
    screenshot_id TEXT REFERENCES screenshots(id) ON DELETE CASCADE,
    CHECK ((recording_id IS NULL) <> (screenshot_id IS NULL)),
    UNIQUE (tag_id, recording_id),
    UNIQUE (tag_id, screenshot_id)
);
CREATE INDEX media_tags_recording ON media_tags(recording_id);
CREATE INDEX media_tags_screenshot ON media_tags(screenshot_id);

CREATE TABLE favorites (
    id            TEXT PRIMARY KEY,
    recording_id  TEXT UNIQUE REFERENCES recordings(id) ON DELETE CASCADE,
    screenshot_id TEXT UNIQUE REFERENCES screenshots(id) ON DELETE CASCADE,
    created_at    INTEGER NOT NULL,
    CHECK ((recording_id IS NULL) <> (screenshot_id IS NULL))
);
"#,
}];
