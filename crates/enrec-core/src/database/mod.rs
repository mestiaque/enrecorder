//! SQLite store. WAL journal (a crash never leaves a half-written page), foreign keys on,
//! one connection behind a mutex (every statement is short). Schema changes go through
//! [`migrations::MIGRATIONS`].

mod library;
pub mod migrations;

use crate::error::{AppError, Result};
use migrations::MIGRATIONS;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub struct Db {
    pub(crate) conn: Mutex<Connection>,
}

/// One item of the media library (a recording or a screenshot).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    /// "video" | "audio" | "screenshot"
    pub kind: String,
    pub title: String,
    pub path: String,
    pub size_bytes: i64,
    pub duration_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub status: String,
    pub favorite: bool,
    pub created_at: i64,
    /// Tag names (filled by `list_media`).
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub videos: i64,
    pub audio: i64,
    pub screenshots: i64,
    /// Sum of recorded durations (video + audio).
    pub total_duration_ms: i64,
    /// Sum of file sizes known to the library.
    pub total_bytes: i64,
    /// Recordings this calendar week is computed in the UI; this is the last 7 days.
    pub last7_days: i64,
    pub interrupted: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportJobRow {
    pub id: String,
    pub source_path: String,
    pub output_path: String,
    pub status: String,
    pub progress: f64,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

pub struct NewSession {
    pub id: String,
    /// "video" | "audio"
    pub kind: String,
    pub state: String,
    /// JSON of the capture source.
    pub source: String,
    /// JSON snapshot of the options used.
    pub settings: String,
    pub output_path: Option<String>,
}

pub struct NewRecording {
    pub kind: String,
    pub title: String,
    pub path: String,
    pub container: Option<String>,
    pub duration_ms: Option<i64>,
    pub size_bytes: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub session_id: Option<String>,
}

/// Library query from the Recordings page. Only these fixed sort keys exist, so no SQL is
/// ever built from user text (the search term is a bound parameter).
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryQuery {
    /// "all" | "video" | "audio" | "screenshot" | "favorites"
    pub filter: String,
    pub search: String,
    /// "newest" | "oldest" | "name" | "size" | "duration"
    pub sort: String,
    pub limit: u32,
    /// "" | "today" | "week" | "month"
    pub date: String,
    /// "" | "short" (< 1 min) | "medium" (1–10 min) | "long" (> 10 min)
    pub length: String,
    /// "" | "small" (< 50 MB) | "medium" (50–500 MB) | "large" (> 500 MB)
    pub size: String,
    /// "" | "uhd" (≥ 2160) | "fhd" (≥ 1080) | "hd" (≥ 720) | "sd" (< 720)
    pub resolution: String,
    /// A tag id ("" = any).
    pub tag: String,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self.conn.lock().query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?)
    }

    // ---------------------------------------------------------------- settings

    pub fn settings_map(&self) -> Result<HashMap<String, Value>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT key, value FROM settings")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut m = HashMap::new();
        for row in rows {
            let (k, v) = row?;
            match serde_json::from_str(&v) {
                Ok(v) => {
                    m.insert(k, v);
                }
                Err(_) => log::warn!("ignoring unreadable stored setting {k}"),
            }
        }
        Ok(m)
    }

    pub fn set_setting(&self, key: &str, value: &Value) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    /// Replace every setting at once (reset / import), in one transaction.
    pub fn replace_settings(&self, values: &serde_json::Map<String, Value>) -> Result<()> {
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        tx.execute("DELETE FROM settings", [])?;
        for (k, v) in values {
            tx.execute("INSERT INTO settings(key, value) VALUES (?1, ?2)", params![k, serde_json::to_string(v)?])?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---------------------------------------------------------------- recovery

    /// Sessions a crash left open are marked `interrupted` (their files are kept for
    /// recovery). Returns how many were found.
    pub fn mark_interrupted_sessions(&self) -> Result<usize> {
        let c = self.conn.lock();
        let n = c.execute(
            "UPDATE capture_sessions SET state = 'interrupted', ended_at = ?1, error = COALESCE(error, 'The app stopped while recording')
             WHERE state IN ('preparing', 'countdown', 'recording', 'paused', 'finalizing')",
            params![now_ms()],
        )?;
        c.execute("UPDATE recordings SET status = 'interrupted', updated_at = ?1 WHERE status = 'recording'", params![now_ms()])?;
        // An export cannot continue after a restart; it is shown as failed and can be retried.
        c.execute(
            "UPDATE export_jobs SET status = 'failed', error = 'Interrupted when the app closed', finished_at = ?1
             WHERE status IN ('queued', 'initializing', 'running', 'finalizing')",
            params![now_ms()],
        )?;
        Ok(n)
    }

    // ---------------------------------------------------------------- library

    pub fn library_stats(&self) -> Result<LibraryStats> {
        let c = self.conn.lock();
        let week_ago = now_ms() - 7 * 24 * 3600 * 1000;
        let (videos, audio, dur, rec_bytes, rec_week, interrupted): (i64, i64, i64, i64, i64, i64) = c.query_row(
            "SELECT
                COALESCE(SUM(kind = 'video'), 0), COALESCE(SUM(kind = 'audio'), 0),
                COALESCE(SUM(duration_ms), 0), COALESCE(SUM(size_bytes), 0),
                COALESCE(SUM(created_at >= ?1), 0), COALESCE(SUM(status = 'interrupted'), 0)
             FROM recordings",
            params![week_ago],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )?;
        let (shots, shot_bytes, shot_week): (i64, i64, i64) = c.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0), COALESCE(SUM(created_at >= ?1), 0) FROM screenshots",
            params![week_ago],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        Ok(LibraryStats {
            videos,
            audio,
            screenshots: shots,
            total_duration_ms: dur,
            total_bytes: rec_bytes + shot_bytes,
            last7_days: rec_week + shot_week,
            interrupted,
        })
    }

    pub fn list_media(&self, q: &LibraryQuery) -> Result<Vec<MediaItem>> {
        let order = match q.sort.as_str() {
            "oldest" => "created_at ASC",
            "name" => "title COLLATE NOCASE ASC",
            "size" => "size_bytes DESC",
            "duration" => "duration_ms DESC",
            _ => "created_at DESC",
        };
        let kind_filter = match q.filter.as_str() {
            "video" => "WHERE kind = 'video'",
            "audio" => "WHERE kind = 'audio'",
            "screenshot" => "WHERE kind = 'screenshot'",
            "favorites" => "WHERE favorite = 1",
            _ => "WHERE 1",
        };
        let limit = if q.limit == 0 { 500 } else { q.limit.min(5000) };
        // Fixed clauses only; the one user value (the tag id) is a bound parameter.
        let day = 24 * 3600 * 1000;
        let since = match q.date.as_str() {
            "today" => Some(now_ms() - now_ms().rem_euclid(day)),
            "week" => Some(now_ms() - 7 * day),
            "month" => Some(now_ms() - 30 * day),
            _ => None,
        };
        let mut extra = String::new();
        if let Some(t) = since {
            extra.push_str(&format!(" AND created_at >= {t}"));
        }
        extra.push_str(match q.length.as_str() {
            "short" => " AND duration_ms < 60000",
            "medium" => " AND duration_ms BETWEEN 60000 AND 600000",
            "long" => " AND duration_ms > 600000",
            _ => "",
        });
        extra.push_str(match q.size.as_str() {
            "small" => " AND size_bytes < 52428800",
            "medium" => " AND size_bytes BETWEEN 52428800 AND 524288000",
            "large" => " AND size_bytes > 524288000",
            _ => "",
        });
        extra.push_str(match q.resolution.as_str() {
            "uhd" => " AND height >= 2160",
            "fhd" => " AND height >= 1080",
            "hd" => " AND height >= 720",
            "sd" => " AND height < 720",
            _ => "",
        });
        if !q.tag.is_empty() {
            extra.push_str(" AND EXISTS(SELECT 1 FROM media_tags mt WHERE mt.tag_id = ?3 AND (mt.recording_id = m.id OR mt.screenshot_id = m.id))");
        }
        let sql = format!(
            "SELECT * FROM (
                SELECT r.id, r.kind, r.title, r.path, r.size_bytes, r.duration_ms, r.width, r.height, r.status,
                       EXISTS(SELECT 1 FROM favorites f WHERE f.recording_id = r.id) AS favorite, r.created_at
                FROM recordings r
                UNION ALL
                SELECT s.id, 'screenshot', s.title, s.path, s.size_bytes, NULL, s.width, s.height, s.status,
                       EXISTS(SELECT 1 FROM favorites f WHERE f.screenshot_id = s.id), s.created_at
                FROM screenshots s
             ) AS m {kind_filter} AND (?1 = '' OR title LIKE '%' || ?1 || '%' ESCAPE '\\') {extra}
             ORDER BY {order} LIMIT ?2"
        );
        let search = escape_like(q.search.trim());
        let c = self.conn.lock();
        let mut st = c.prepare(&sql)?;
        let tag = q.tag.clone();
        let bind: Vec<&dyn rusqlite::ToSql> = if tag.is_empty() { vec![&search, &limit] } else { vec![&search, &limit, &tag] };
        let rows = st.query_map(bind.as_slice(), |r| {
            Ok(MediaItem {
                id: r.get(0)?,
                kind: r.get(1)?,
                title: r.get(2)?,
                path: r.get(3)?,
                size_bytes: r.get(4)?,
                duration_ms: r.get(5)?,
                width: r.get(6)?,
                height: r.get(7)?,
                status: r.get(8)?,
                favorite: r.get(9)?,
                created_at: r.get(10)?,
                tags: Vec::new(),
            })
        })?;
        let mut items = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(AppError::from)?;
        drop(st);
        // Tags of the listed items in one query.
        let mut tags: HashMap<String, Vec<String>> = HashMap::new();
        let mut st = c.prepare("SELECT COALESCE(mt.recording_id, mt.screenshot_id), t.name FROM media_tags mt JOIN tags t ON t.id = mt.tag_id ORDER BY t.name COLLATE NOCASE")?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = row?;
            tags.entry(id).or_default().push(name);
        }
        for it in &mut items {
            if let Some(t) = tags.remove(&it.id) {
                it.tags = t;
            }
        }
        Ok(items)
    }

    pub fn list_export_jobs(&self, limit: u32) -> Result<Vec<ExportJobRow>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT id, source_path, output_path, status, progress, error, created_at, finished_at FROM export_jobs ORDER BY created_at DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit.clamp(1, 1000)], |r| {
            Ok(ExportJobRow {
                id: r.get(0)?,
                source_path: r.get(1)?,
                output_path: r.get(2)?,
                status: r.get(3)?,
                progress: r.get(4)?,
                error: r.get(5)?,
                created_at: r.get(6)?,
                finished_at: r.get(7)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(AppError::from)
    }

    // ---------------------------------------------------------------- capture sessions

    pub fn insert_session(&self, s: &NewSession) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO capture_sessions(id, kind, state, source, settings, output_path, started_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![s.id, s.kind, s.state, s.source, s.settings, s.output_path, now_ms()],
        )?;
        Ok(())
    }

    /// Records a state change; final states also set `ended_at`.
    pub fn set_session_state(&self, id: &str, state: &str, error: Option<&str>) -> Result<()> {
        let ended = matches!(state, "completed" | "failed" | "cancelled" | "interrupted").then(now_ms);
        self.conn.lock().execute(
            "UPDATE capture_sessions SET state = ?2, error = COALESCE(?3, error), ended_at = COALESCE(?4, ended_at) WHERE id = ?1",
            params![id, state, error, ended],
        )?;
        Ok(())
    }

    pub fn set_session_output(&self, id: &str, path: &str) -> Result<()> {
        self.conn.lock().execute("UPDATE capture_sessions SET output_path = ?2 WHERE id = ?1", params![id, path])?;
        Ok(())
    }

    /// Adds a finished recording and its FFprobe metadata in one transaction. Returns its id.
    pub fn insert_recording(&self, r: &NewRecording, probe: Option<&crate::media::probe::Probe>) -> Result<String> {
        let id = new_id();
        let now = now_ms();
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        tx.execute(
            "INSERT INTO recordings(id, kind, title, path, container, duration_ms, size_bytes, width, height, status, session_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'ready', ?10, ?11, ?11)",
            params![id, r.kind, r.title, r.path, r.container, r.duration_ms, r.size_bytes, r.width, r.height, r.session_id, now],
        )?;
        if let Some(p) = probe {
            tx.execute(
                "INSERT INTO media_metadata(recording_id, container, video_codec, audio_codec, width, height, fps, duration_ms, bitrate, video_streams, audio_streams, raw, probed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![id, p.container, p.video_codec, p.audio_codec, p.width, p.height, p.fps, p.duration_ms, p.bitrate, p.video_streams, p.audio_streams, p.raw, now],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// Test and recovery helper: run one statement (never called with user input).
    #[doc(hidden)]
    pub fn execute_raw(&self, sql: &str) -> Result<usize> {
        Ok(self.conn.lock().execute(sql, [])?)
    }

    #[doc(hidden)]
    pub fn query_i64(&self, sql: &str) -> Result<Option<i64>> {
        Ok(self.conn.lock().query_row(sql, [], |r| r.get(0)).optional()?)
    }
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Applies the migrations that are missing, each in its own transaction.
fn migrate(conn: &mut Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at INTEGER NOT NULL)")?;
    let current: i64 = conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?;
    let latest = MIGRATIONS.last().map_or(0, |m| m.version);
    if current > latest {
        return Err(AppError::Db(format!("the database was created by a newer ENRecorder (schema {current}); update ENRecorder")));
    }
    for m in MIGRATIONS.iter().filter(|m| m.version > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql).map_err(|e| AppError::Db(format!("migration {} ({}) failed: {e}", m.version, m.name)))?;
        tx.execute("INSERT INTO schema_migrations(version, name, applied_at) VALUES (?1, ?2, ?3)", params![m.version, m.name, now_ms()])?;
        tx.commit()?;
        log::info!("database migrated to schema {} ({})", m.version, m.name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn db() -> Db {
        Db::open_in_memory().unwrap()
    }

    #[test]
    fn migrates_to_latest_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let d = Db::open(&path).unwrap();
        assert_eq!(d.schema_version().unwrap(), MIGRATIONS.last().unwrap().version);
        drop(d);
        // Opening again applies nothing new and keeps one row per migration.
        let d = Db::open(&path).unwrap();
        assert_eq!(d.query_i64("SELECT COUNT(*) FROM schema_migrations").unwrap(), Some(MIGRATIONS.len() as i64));
        for t in ["recordings", "screenshots", "media_metadata", "capture_sessions", "export_jobs", "devices", "settings", "shortcuts", "tags", "media_tags", "favorites"] {
            assert_eq!(d.query_i64(&format!("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '{t}'")).unwrap(), Some(1), "{t}");
        }
    }

    #[test]
    fn refuses_a_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.db");
        Db::open(&path).unwrap().execute_raw("INSERT INTO schema_migrations VALUES (999, 'future', 0)").unwrap();
        assert!(matches!(Db::open(&path), Err(AppError::Db(m)) if m.contains("newer")));
    }

    #[test]
    fn foreign_keys_and_exactly_one_target() {
        let d = db();
        // Unknown recording → foreign key error.
        assert!(d.execute_raw("INSERT INTO favorites(id, recording_id, created_at) VALUES ('f1', 'nope', 0)").is_err());
        d.execute_raw("INSERT INTO screenshots(id, title, path, format, width, height, created_at, updated_at) VALUES ('s1', 'Shot', '/tmp/s1.png', 'png', 10, 10, 0, 0)").unwrap();
        // Both targets set, or none → CHECK fails.
        assert!(d.execute_raw("INSERT INTO favorites(id, created_at) VALUES ('f2', 0)").is_err());
        d.execute_raw("INSERT INTO favorites(id, screenshot_id, created_at) VALUES ('f3', 's1', 0)").unwrap();
        // Deleting the screenshot removes its favourite.
        d.execute_raw("DELETE FROM screenshots WHERE id = 's1'").unwrap();
        assert_eq!(d.query_i64("SELECT COUNT(*) FROM favorites").unwrap(), Some(0));
    }

    #[test]
    fn settings_roundtrip_and_replace() {
        let d = db();
        d.set_setting("theme", &json!("light")).unwrap();
        d.set_setting("theme", &json!("dark")).unwrap();
        assert_eq!(d.settings_map().unwrap()["theme"], json!("dark"));
        let mut m = serde_json::Map::new();
        m.insert("accent".into(), json!("#6366f1"));
        d.replace_settings(&m).unwrap();
        let s = d.settings_map().unwrap();
        assert!(!s.contains_key("theme"));
        assert_eq!(s["accent"], json!("#6366f1"));
    }

    #[test]
    fn interrupted_sessions_are_marked_not_deleted() {
        let d = db();
        d.execute_raw("INSERT INTO capture_sessions(id, kind, state, source, started_at) VALUES ('c1', 'video', 'recording', 'screen', 1)").unwrap();
        d.execute_raw("INSERT INTO capture_sessions(id, kind, state, source, started_at) VALUES ('c2', 'video', 'completed', 'screen', 1)").unwrap();
        d.execute_raw("INSERT INTO recordings(id, kind, title, path, status, session_id, created_at, updated_at) VALUES ('r1', 'video', 'R', '/tmp/r1.mkv', 'recording', 'c1', 1, 1)").unwrap();
        assert_eq!(d.mark_interrupted_sessions().unwrap(), 1);
        assert_eq!(d.query_i64("SELECT COUNT(*) FROM capture_sessions WHERE state = 'interrupted'").unwrap(), Some(1));
        assert_eq!(d.query_i64("SELECT COUNT(*) FROM recordings WHERE status = 'interrupted'").unwrap(), Some(1));
        assert_eq!(d.library_stats().unwrap().interrupted, 1);
    }

    #[test]
    fn sessions_and_recordings() {
        let d = db();
        d.insert_session(&NewSession { id: "s1".into(), kind: "video".into(), state: "preparing".into(), source: "{}".into(), settings: "{}".into(), output_path: None }).unwrap();
        d.set_session_state("s1", "recording", None).unwrap();
        assert_eq!(d.query_i64("SELECT COUNT(*) FROM capture_sessions WHERE state = 'recording' AND ended_at IS NULL").unwrap(), Some(1));
        let probe = crate::media::probe::Probe { video_codec: Some("h264".into()), duration_ms: Some(3000), video_streams: 1, ..Default::default() };
        let id = d
            .insert_recording(
                &NewRecording { kind: "video".into(), title: "Demo".into(), path: "/v/demo.mp4".into(), container: Some("mp4".into()), duration_ms: Some(3000), size_bytes: 10, width: Some(2), height: Some(2), session_id: Some("s1".into()) },
                Some(&probe),
            )
            .unwrap();
        d.set_session_state("s1", "completed", None).unwrap();
        assert_eq!(d.query_i64("SELECT COUNT(*) FROM capture_sessions WHERE state = 'completed' AND ended_at IS NOT NULL").unwrap(), Some(1));
        assert_eq!(d.query_i64(&format!("SELECT duration_ms FROM media_metadata WHERE recording_id = '{id}'")).unwrap(), Some(3000));
        assert_eq!(d.list_media(&LibraryQuery::default()).unwrap()[0].title, "Demo");
        // The same path cannot be added twice.
        assert!(d.insert_recording(&NewRecording { kind: "video".into(), title: "x".into(), path: "/v/demo.mp4".into(), container: None, duration_ms: None, size_bytes: 0, width: None, height: None, session_id: None }, None).is_err());
    }

    #[test]
    fn library_list_filters_sorts_and_escapes_search() {
        let d = db();
        let now = now_ms();
        d.execute_raw(&format!("INSERT INTO recordings(id, kind, title, path, size_bytes, duration_ms, created_at, updated_at) VALUES ('r1', 'video', 'Demo 100%', '/v/1.mp4', 300, 5000, {now}, {now})")).unwrap();
        d.execute_raw("INSERT INTO recordings(id, kind, title, path, size_bytes, duration_ms, created_at, updated_at) VALUES ('r2', 'audio', 'Voice', '/a/2.wav', 100, 9000, 5, 5)").unwrap();
        d.execute_raw("INSERT INTO screenshots(id, title, path, format, width, height, size_bytes, created_at, updated_at) VALUES ('s1', 'Bug_report', '/s/1.png', 'png', 4, 4, 50, 7, 7)").unwrap();
        d.execute_raw("INSERT INTO favorites(id, recording_id, created_at) VALUES ('f1', 'r2', 1)").unwrap();

        let all = d.list_media(&LibraryQuery::default()).unwrap();
        assert_eq!(all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["r1", "s1", "r2"]);
        let q = |filter: &str, search: &str, sort: &str| d.list_media(&LibraryQuery { filter: filter.into(), search: search.into(), sort: sort.into(), limit: 0, ..Default::default() }).unwrap();
        assert_eq!(q("audio", "", "").len(), 1);
        assert_eq!(q("screenshot", "", "")[0].kind, "screenshot");
        assert_eq!(q("favorites", "", "")[0].id, "r2");
        assert_eq!(q("all", "", "duration")[0].id, "r2");
        // "%" and "_" are literal characters, not wildcards.
        assert_eq!(q("all", "100%", "").len(), 1);
        assert_eq!(q("all", "Vo_ce", "").len(), 0);
        assert_eq!(q("all", "Bug_", "").len(), 1);
        // Unknown sort/filter values fall back to the defaults.
        assert_eq!(q("'; DROP TABLE recordings; --", "", "nonsense").len(), 3);

        let st = d.library_stats().unwrap();
        assert_eq!((st.videos, st.audio, st.screenshots), (1, 1, 1));
        assert_eq!(st.total_duration_ms, 14000);
        assert_eq!(st.total_bytes, 450);
        assert_eq!(st.last7_days, 1);
    }
}
