//! Library actions: one item (recording or screenshot), rename, delete, favourites.

use super::{new_id, now_ms, Db, MediaItem};
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
}
