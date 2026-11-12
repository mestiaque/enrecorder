//! Global shortcuts: one row per action in `shortcuts`, defaults when the user changed nothing,
//! format checks and conflict detection. The app registers them with the system.

use super::{new_id, Db};
use crate::error::{AppError, Result};
use rusqlite::params;
use serde::Serialize;

/// (action, label, default accelerator)
pub const ACTIONS: &[(&str, &str, &str)] = &[
    ("record", "Start / stop recording", "Ctrl+Shift+R"),
    ("pause", "Pause / resume recording", "Ctrl+Shift+P"),
    ("screenshot", "Take a screenshot", "Ctrl+Shift+S"),
    ("quick", "Quick capture (select an area, draw, copy)", "Ctrl+Alt+A"),
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutRow {
    pub action: String,
    pub label: String,
    /// None = turned off.
    pub accelerator: Option<String>,
    pub default: String,
}

const MODS: &[&str] = &["Ctrl", "Alt", "Shift", "Super"];

/// "ctrl + shift + r" → "Ctrl+Shift+R". Needs Ctrl, Alt or Super (a shortcut without one would
/// take keys away from typing) and one key: A–Z, 0–9, F1–F12.
pub fn normalize(accel: &str) -> Result<String> {
    let mut mods: Vec<&str> = Vec::new();
    let mut key: Option<String> = None;
    for part in accel.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let lower = part.to_ascii_lowercase();
        let m = match lower.as_str() {
            "ctrl" | "control" | "cmdorctrl" => Some("Ctrl"),
            "alt" | "option" => Some("Alt"),
            "shift" => Some("Shift"),
            "super" | "meta" | "win" | "cmd" => Some("Super"),
            _ => None,
        };
        if let Some(m) = m {
            if !mods.contains(&m) {
                mods.push(m);
            }
            continue;
        }
        if key.is_some() {
            return Err(AppError::InvalidSetting(format!("{accel}: only one key besides Ctrl/Alt/Shift/Super")));
        }
        let up = part.to_ascii_uppercase();
        let ok = (up.len() == 1 && up.chars().all(|c| c.is_ascii_alphanumeric())) || (up.starts_with('F') && up[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)));
        if !ok {
            return Err(AppError::InvalidSetting(format!("{part} cannot be used in a shortcut (A–Z, 0–9 or F1–F12)")));
        }
        key = Some(up);
    }
    let key = key.ok_or_else(|| AppError::InvalidSetting("a shortcut needs a key".into()))?;
    if !mods.iter().any(|m| matches!(*m, "Ctrl" | "Alt" | "Super")) {
        return Err(AppError::InvalidSetting("a shortcut needs Ctrl, Alt or Super".into()));
    }
    mods.sort_by_key(|m| MODS.iter().position(|x| x == m));
    Ok(format!("{}+{key}", mods.join("+")))
}

impl Db {
    pub fn shortcuts(&self) -> Result<Vec<ShortcutRow>> {
        let c = self.conn.lock();
        let mut out = Vec::new();
        for (action, label, default) in ACTIONS {
            let stored: Option<(Option<String>, bool)> = c.query_row("SELECT accelerator, enabled FROM shortcuts WHERE action = ?1", params![action], |r| Ok((r.get(0)?, r.get(1)?))).ok();
            let accelerator = match stored {
                Some((a, true)) => a,
                Some((_, false)) => None,
                None => Some(default.to_string()),
            };
            out.push(ShortcutRow { action: action.to_string(), label: label.to_string(), accelerator, default: default.to_string() });
        }
        Ok(out)
    }

    /// Sets (or with `None` turns off) an action's shortcut; refuses one another action uses.
    pub fn set_shortcut(&self, action: &str, accel: Option<&str>) -> Result<String> {
        if !ACTIONS.iter().any(|(a, ..)| *a == action) {
            return Err(AppError::InvalidSetting(format!("unknown action {action}")));
        }
        let value = accel.map(normalize).transpose()?;
        if let Some(v) = &value {
            if let Some(other) = self.shortcuts()?.into_iter().find(|s| s.action != action && s.accelerator.as_deref() == Some(v.as_str())) {
                return Err(AppError::InvalidSetting(format!("{v} is already used for \"{}\"", other.label)));
            }
        }
        self.conn.lock().execute(
            "INSERT INTO shortcuts(id, action, accelerator, enabled) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(action) DO UPDATE SET accelerator = excluded.accelerator, enabled = excluded.enabled",
            params![new_id(), action, value, value.is_some()],
        )?;
        Ok(value.unwrap_or_default())
    }

    pub fn reset_shortcuts(&self) -> Result<()> {
        self.conn.lock().execute("DELETE FROM shortcuts", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(normalize("shift + ctrl + r").unwrap(), "Ctrl+Shift+R");
        assert_eq!(normalize("Alt+F9").unwrap(), "Alt+F9");
        assert!(normalize("Shift+R").is_err(), "Shift alone would take a typed capital");
        assert!(normalize("Ctrl+Shift").is_err());
        assert!(normalize("Ctrl+A+B").is_err());
        assert!(normalize("Ctrl+F13").is_err());
        assert!(normalize("Ctrl+é").is_err());
    }

    #[test]
    fn defaults_conflicts_and_reset() {
        let d = Db::open_in_memory().unwrap();
        assert_eq!(d.shortcuts().unwrap()[0].accelerator.as_deref(), Some("Ctrl+Shift+R"));
        assert!(d.set_shortcut("pause", Some("ctrl+shift+r")).unwrap_err().to_string().contains("Start / stop"));
        d.set_shortcut("record", Some("Alt+F9")).unwrap();
        d.set_shortcut("pause", Some("Ctrl+Shift+R")).unwrap();
        d.set_shortcut("screenshot", None).unwrap();
        let s = d.shortcuts().unwrap();
        assert_eq!((s[0].accelerator.as_deref(), s[1].accelerator.as_deref(), s[2].accelerator.as_deref()), (Some("Alt+F9"), Some("Ctrl+Shift+R"), None));
        assert!(d.set_shortcut("nope", None).is_err());
        d.reset_shortcuts().unwrap();
        assert_eq!(d.shortcuts().unwrap()[2].accelerator.as_deref(), Some("Ctrl+Shift+S"));
    }
}
