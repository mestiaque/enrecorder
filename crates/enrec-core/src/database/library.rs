//! Library actions: one item (recording or screenshot), rename, delete, favourites.

use super::{new_id, now_ms, Db, MediaItem};
use serde::Serialize;
use crate::error::{AppError, Result};
use crate::media::probe::Probe;
use rusqlite::{params, OptionalExtension};

impl Db {
    /// One library item by id (recordings and screenshots share the id space: UUIDs).
    pub fn media_item(&self, id: &str) -> Result<MediaItem> {
        let c = self.conn.lock();
        let row = c
            .query_row(
                "SELECT r.id, r.kind, r.title, r.path, r.size_bytes, r.duration_ms, r.width, r.height, r.status,
                        EXISTS(SELECT 1 FROM favorites f WHERE f.recording_id = r.id), r.created_at
                 FROM recordings r WHERE r.id = ?1
                 UNION ALL
                 SELECT s.id, 'screenshot', s.title, s.path, s.size_bytes, NULL, s.width, s.height, s.status,
                        EXISTS(SELECT 1 FROM favorites f WHERE f.screenshot_id = s.id), s.created_at
                 FROM screenshots s WHERE s.id = ?1",
                params![id],
                |r| {
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
                },
            )
            .optional()?;
        row.ok_or_else(|| AppError::NotFound(format!("library item {id}")))
    }

    /// FFprobe facts stored for a recording.
    pub fn media_metadata(&self, id: &str) -> Result<Option<Probe>> {
        let c = self.conn.lock();
        Ok(c.query_row(
            "SELECT container, video_codec, audio_codec, width, height, fps, duration_ms, bitrate, video_streams, audio_streams, raw FROM media_metadata WHERE recording_id = ?1",
            params![id],
            |r| {
                Ok(Probe {
                    container: r.get(0)?,
                    video_codec: r.get(1)?,
                    audio_codec: r.get(2)?,
                    width: r.get(3)?,
                    height: r.get(4)?,
                    fps: r.get(5)?,
                    duration_ms: r.get(6)?,
                    bitrate: r.get(7)?,
                    video_streams: r.get(8)?,
                    audio_streams: r.get(9)?,
                    raw: r.get(10)?,
                })
            },
        )
        .optional()?)
    }

    /// New title and path after the file was renamed on disk.
    pub fn rename_media(&self, id: &str, title: &str, path: &str) -> Result<()> {
        let c = self.conn.lock();
        let now = now_ms();
        let n = c.execute("UPDATE recordings SET title = ?2, path = ?3, updated_at = ?4 WHERE id = ?1", params![id, title, path, now])?
            + c.execute("UPDATE screenshots SET title = ?2, path = ?3, updated_at = ?4 WHERE id = ?1", params![id, title, path, now])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("library item {id}")));
        }
        Ok(())
    }

    /// Removes the row (metadata, tags and favourites go with it through foreign keys).
    pub fn delete_media(&self, id: &str) -> Result<()> {
        let c = self.conn.lock();
        let n = c.execute("DELETE FROM recordings WHERE id = ?1", params![id])? + c.execute("DELETE FROM screenshots WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!("library item {id}")));
        }
        Ok(())
    }

    pub fn set_favorite(&self, id: &str, kind: &str, on: bool) -> Result<()> {
        let c = self.conn.lock();
        let col = if kind == "screenshot" { "screenshot_id" } else { "recording_id" };
        if on {
            c.execute(&format!("INSERT INTO favorites(id, {col}, created_at) SELECT ?1, ?2, ?3 WHERE NOT EXISTS (SELECT 1 FROM favorites WHERE {col} = ?2)"), params![new_id(), id, now_ms()])?;
        } else {
            c.execute(&format!("DELETE FROM favorites WHERE {col} = ?1"), params![id])?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- tags

    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT t.id, t.name, t.color, (SELECT COUNT(*) FROM media_tags mt WHERE mt.tag_id = t.id) FROM tags t ORDER BY t.name COLLATE NOCASE")?;
        let rows = st.query_map([], |r| Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, count: r.get(3)? }))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Creates a tag, or returns the existing one with that name (names ignore case).
    pub fn create_tag(&self, name: &str, color: Option<&str>) -> Result<Tag> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
            return Err(AppError::InvalidSetting("a tag name has 1–40 characters".into()));
        }
        let c = self.conn.lock();
        c.execute("INSERT INTO tags(id, name, color, created_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(name) DO NOTHING", params![new_id(), name, color, now_ms()])?;
        Ok(c.query_row("SELECT t.id, t.name, t.color, (SELECT COUNT(*) FROM media_tags mt WHERE mt.tag_id = t.id) FROM tags t WHERE t.name = ?1 COLLATE NOCASE", params![name], |r| {
            Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, count: r.get(3)? })
        })?)
    }

    pub fn delete_tag(&self, id: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM tags WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Replaces an item's tags.
    pub fn set_media_tags(&self, item_id: &str, kind: &str, tag_ids: &[String]) -> Result<()> {
        let col = if kind == "screenshot" { "screenshot_id" } else { "recording_id" };
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        tx.execute(&format!("DELETE FROM media_tags WHERE {col} = ?1"), params![item_id])?;
        for t in tag_ids {
            tx.execute(&format!("INSERT INTO media_tags(id, tag_id, {col}) VALUES (?1, ?2, ?3)"), params![new_id(), t, item_id])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn media_tag_ids(&self, item_id: &str) -> Result<Vec<String>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT tag_id FROM media_tags WHERE recording_id = ?1 OR screenshot_id = ?1")?;
        let rows = st.query_map(params![item_id], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    // ---------------------------------------------------------------- files

    /// Marks items whose file is gone as "missing" (and back to "ready" when it returns).
    /// Returns (now missing, back again).
    pub fn refresh_missing(&self) -> Result<(usize, usize)> {
        let items: Vec<(String, String, String, &'static str)> = {
            let c = self.conn.lock();
            let mut v = Vec::new();
            for (table, kind) in [("recordings", "r"), ("screenshots", "s")] {
                let mut st = c.prepare(&format!("SELECT id, path, status FROM {table} WHERE status IN ('ready', 'missing')"))?;
                let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                for row in rows {
                    let (id, path, status): (String, String, String) = row?;
                    v.push((id, path, status, kind));
                }
            }
            v
        };
        let (mut gone, mut back) = (0, 0);
        let c = self.conn.lock();
        for (id, path, status, kind) in items {
            let exists = std::path::Path::new(&path).exists();
            let table = if kind == "r" { "recordings" } else { "screenshots" };
            if !exists && status == "ready" {
                c.execute(&format!("UPDATE {table} SET status = 'missing', updated_at = ?2 WHERE id = ?1"), params![id, now_ms()])?;
                gone += 1;
            } else if exists && status == "missing" {
                c.execute(&format!("UPDATE {table} SET status = 'ready', updated_at = ?2 WHERE id = ?1"), params![id, now_ms()])?;
                back += 1;
            }
        }
        Ok((gone, back))
    }

    /// New FFprobe facts for a recording (reprocess): size, duration, picture size, metadata.
    pub fn update_probe(&self, id: &str, size: i64, p: &Probe) -> Result<()> {
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        tx.execute(
            "UPDATE recordings SET size_bytes = ?2, duration_ms = COALESCE(?3, duration_ms), width = COALESCE(?4, width), height = COALESCE(?5, height), container = COALESCE(?6, container), status = 'ready', updated_at = ?7 WHERE id = ?1",
            params![id, size, p.duration_ms, p.width, p.height, p.container, now_ms()],
        )?;
        tx.execute(
            "INSERT INTO media_metadata(recording_id, container, video_codec, audio_codec, width, height, fps, duration_ms, bitrate, video_streams, audio_streams, raw, probed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(recording_id) DO UPDATE SET container = excluded.container, video_codec = excluded.video_codec, audio_codec = excluded.audio_codec,
               width = excluded.width, height = excluded.height, fps = excluded.fps, duration_ms = excluded.duration_ms, bitrate = excluded.bitrate,
               video_streams = excluded.video_streams, audio_streams = excluded.audio_streams, raw = excluded.raw, probed_at = excluded.probed_at",
            params![id, p.container, p.video_codec, p.audio_codec, p.width, p.height, p.fps, p.duration_ms, p.bitrate, p.video_streams, p.audio_streams, p.raw, now_ms()],
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// A device as remembered in `devices` (also ones that are unplugged now).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRow {
    pub kind: String,
    pub backend_id: String,
    pub name: String,
    pub is_default: bool,
    pub connected: bool,
    pub details: serde_json::Value,
    pub last_seen: i64,
}

impl Db {
    /// Records the devices of `kinds` seen now: present ones are updated and marked connected,
    /// known ones that are missing are marked disconnected.
    pub fn sync_devices(&self, kinds: &[&str], present: &[(String, String, String, bool, serde_json::Value)]) -> Result<()> {
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        let now = now_ms();
        for k in kinds {
            tx.execute("UPDATE devices SET connected = 0 WHERE kind = ?1", params![k])?;
        }
        for (kind, id, name, default, details) in present {
            tx.execute(
                "INSERT INTO devices(id, kind, backend_id, name, is_default, connected, details, last_seen) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)
                 ON CONFLICT(kind, backend_id) DO UPDATE SET name = excluded.name, is_default = excluded.is_default, connected = 1, details = excluded.details, last_seen = excluded.last_seen",
                params![new_id(), kind, id, name, default, details.to_string(), now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceRow>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT kind, backend_id, name, is_default, connected, details, last_seen FROM devices ORDER BY kind, connected DESC, is_default DESC, name COLLATE NOCASE")?;
        let rows = st.query_map([], |r| {
            let details: String = r.get(5)?;
            Ok(DeviceRow { kind: r.get(0)?, backend_id: r.get(1)?, name: r.get(2)?, is_default: r.get(3)?, connected: r.get(4)?, details: serde_json::from_str(&details).unwrap_or_default(), last_seen: r.get(6)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    /// Items with this tag.
    pub count: i64,
}

#[cfg(test)]
mod tag_tests {
    use super::super::{Db, LibraryQuery};

    #[test]
    fn tags_filters_and_missing_files() {
        let d = Db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.mp4");
        std::fs::write(&file, b"x").unwrap();
        let now = crate::database::now_ms();
        d.execute_raw(&format!("INSERT INTO recordings(id, kind, title, path, size_bytes, duration_ms, height, created_at, updated_at) VALUES ('r1', 'video', 'Demo', '{}', 600000000, 700000, 1080, {now}, {now})", file.display())).unwrap();
        d.execute_raw("INSERT INTO recordings(id, kind, title, path, size_bytes, duration_ms, height, created_at, updated_at) VALUES ('r2', 'video', 'Old', '/nope/b.mp4', 1000, 30000, 480, 1, 1)").unwrap();
        let work = d.create_tag("Work", Some("#ff0000")).unwrap();
        assert_eq!(d.create_tag("work", None).unwrap().id, work.id, "names ignore case");
        assert!(d.create_tag("  ", None).is_err());
        d.set_media_tags("r1", "video", std::slice::from_ref(&work.id)).unwrap();
        let q = |f: &dyn Fn(&mut LibraryQuery)| {
            let mut q = LibraryQuery::default();
            f(&mut q);
            d.list_media(&q).unwrap().into_iter().map(|m| m.id).collect::<Vec<_>>()
        };
        assert_eq!(q(&|q| q.tag = work.id.clone()), ["r1"]);
        assert_eq!(q(&|q| q.length = "long".into()), ["r1"]);
        assert_eq!(q(&|q| q.length = "short".into()), ["r2"]);
        assert_eq!(q(&|q| q.size = "large".into()), ["r1"]);
        assert_eq!(q(&|q| q.resolution = "fhd".into()), ["r1"]);
        assert_eq!(q(&|q| q.resolution = "sd".into()), ["r2"]);
        assert_eq!(q(&|q| q.date = "today".into()), ["r1"]);
        assert_eq!(d.list_media(&LibraryQuery::default()).unwrap()[0].tags, ["Work"]);
        assert_eq!(d.list_tags().unwrap()[0].count, 1);
        assert_eq!(d.refresh_missing().unwrap(), (1, 0));
        assert_eq!(d.media_item("r2").unwrap().status, "missing");
        let mic = |id: &str| ("microphone".to_string(), id.to_string(), format!("Mic {id}"), id == "a", serde_json::json!({"channels": 2}));
        d.sync_devices(&["microphone"], &[mic("a"), mic("b")]).unwrap();
        d.sync_devices(&["microphone"], &[mic("a")]).unwrap();
        let devs = d.list_devices().unwrap();
        assert_eq!(devs.len(), 2);
        assert!(devs[0].connected && devs[0].is_default && !devs[1].connected, "an unplugged device is remembered, not removed");
        d.delete_tag(&work.id).unwrap();
        assert!(d.media_tag_ids("r1").unwrap().is_empty(), "tag links go with the tag");
    }
}
