//! In-app updates from GitHub releases (mestiaque/enrecorder). Same flow as ENSave.
//!
//! * `check_update` asks the GitHub API for the latest release. It is the only network
//!   request for updates, and only when the user allows it (Settings).
//! * `install_update` downloads the installer for this kind of install, verifies its SHA-256
//!   (GitHub publishes one per asset) and installs it in place: .deb through
//!   `pkexec apt-get install` (system password prompt), AppImage by replacing the file
//!   atomically, Windows by running the NSIS installer silently after ENRecorder quits.
//!   Not while recording (the UI asks to stop first).
//! * `restart_app` starts the new version once this process has exited.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const API_LATEST: &str = "https://api.github.com/repos/mestiaque/enrecorder/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/mestiaque/enrecorder/releases/latest";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub page: String,
    pub asset_name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub installable: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Deb,
    AppImage(PathBuf),
    Windows,
    Unmanaged,
}

fn parse_version(s: &str) -> (u64, u64, u64) {
    let core = s
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .split('-')
        .next()
        .unwrap_or("");
    let mut it = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

fn install_kind() -> Kind {
    let exe = std::env::current_exe().unwrap_or_default();
    if cfg!(windows) {
        // The NSIS installer puts uninstall.exe next to the program.
        return match exe.parent() {
            Some(dir) if dir.join("uninstall.exe").exists() => Kind::Windows,
            _ => Kind::Unmanaged,
        };
    }
    if let Some(p) = std::env::var_os("APPIMAGE") {
        return Kind::AppImage(PathBuf::from(p));
    }
    if exe.starts_with("/usr/bin") && Path::new("/usr/bin/apt-get").exists() {
        return Kind::Deb;
    }
    Kind::Unmanaged
}

fn pick_asset<'a>(assets: &'a [serde_json::Value], kind: &Kind) -> Option<&'a serde_json::Value> {
    assets.iter().find(|a| {
        let n = a["name"].as_str().unwrap_or("");
        match kind {
            Kind::Deb => n.starts_with("ENRecorder_") && n.ends_with("_amd64.deb"),
            Kind::AppImage(_) => n.starts_with("ENRecorder_") && n.ends_with("_amd64.AppImage"),
            Kind::Windows => n.starts_with("ENRecorder_") && n.ends_with("-setup.exe"),
            Kind::Unmanaged => false,
        }
    })
}

fn update_from_release(rel: &serde_json::Value, current: &str, kind: &Kind) -> Option<UpdateInfo> {
    let tag = rel["tag_name"].as_str().unwrap_or("");
    if tag.is_empty()
        || rel["draft"].as_bool() == Some(true)
        || rel["prerelease"].as_bool() == Some(true)
    {
        return None;
    }
    let v = parse_version(tag);
    if v <= parse_version(current) {
        return None;
    }
    let mut info = UpdateInfo {
        version: format!("{}.{}.{}", v.0, v.1, v.2),
        notes: clean_notes(rel["body"].as_str().unwrap_or("")),
        page: rel["html_url"]
            .as_str()
            .unwrap_or(RELEASES_PAGE)
            .to_string(),
        asset_name: String::new(),
        url: String::new(),
        size: 0,
        sha256: String::new(),
        installable: false,
    };
    let empty = vec![];
    if let Some(a) = pick_asset(rel["assets"].as_array().unwrap_or(&empty), kind) {
        info.asset_name = a["name"].as_str().unwrap_or("").to_string();
        info.url = a["browser_download_url"].as_str().unwrap_or("").to_string();
        info.size = a["size"].as_u64().unwrap_or(0);
        info.sha256 = a["digest"]
            .as_str()
            .and_then(|d| d.strip_prefix("sha256:"))
            .unwrap_or("")
            .to_lowercase();
        // Without a published checksum nothing is installed.
        info.installable = !info.url.is_empty() && info.sha256.len() == 64;
    }
    Some(info)
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(timeout))
        .build()
}

fn user_agent(app: &AppHandle) -> String {
    format!("ENRecorder/{} (update check)", app.package_info().version)
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let current = app.package_info().version.to_string();
    let ua = user_agent(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let rel: serde_json::Value = agent(15)
            .get(API_LATEST)
            .set("User-Agent", &ua)
            .set("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| format!("Could not reach GitHub: {e}"))?
            .into_json()
            .map_err(|e| format!("Unexpected answer from GitHub: {e}"))?;
        Ok(update_from_release(&rel, &current, &install_kind()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Clone)]
struct Progress {
    done: u64,
    total: u64,
}

fn download(app: &AppHandle, info: &UpdateInfo, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let name = Path::new(&info.asset_name)
        .file_name()
        .ok_or("bad asset name")?;
    let out = dir.join(name);
    let part = out.with_extension("part");
    let resp = agent(60)
        .get(&info.url)
        .set("User-Agent", &user_agent(app))
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;
    let total = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(info.size);
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let (mut done, mut last) = (0u64, 0u64);
    loop {
        let n = reader.read(&mut buf).map_err(|e| {
            let _ = std::fs::remove_file(&part);
            format!("Download failed: {e}")
        })?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        done += n as u64;
        if done - last > 512 * 1024 || done == total {
            last = done;
            let _ = app.emit("update-progress", Progress { done, total });
        }
    }
    drop(file);
    let got = format!("{:x}", hasher.finalize());
    if !info.sha256.is_empty() && got != info.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err("The downloaded file is damaged (checksum mismatch). Please try again.".into());
    }
    std::fs::rename(&part, &out).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(unix)]
fn make_executable(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))
}

/// Returns "restart" (installed; call restart_app) or "exiting" (Windows installer takes over).
#[tauri::command]
pub async fn install_update(app: AppHandle, info: UpdateInfo) -> Result<String, String> {
    if app.state::<crate::state::AppState>().recording_active() {
        return Err("Stop the recording before updating.".into());
    }
    let kind = install_kind();
    if !info.installable || kind == Kind::Unmanaged {
        return Err(
            "This copy cannot update itself (it is a development build or a manual install)."
                .into(),
        );
    }
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("updates");
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let file = download(&app2, &info, &dir)?;
        install_file(&app2, kind, file)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn install_file(app2: &AppHandle, kind: Kind, file: PathBuf) -> Result<String, String> {
    match kind {
        Kind::Deb => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644));
            }
            let out = std::process::Command::new("pkexec")
                .args(["apt-get", "install", "-y", "--allow-downgrades"])
                .arg(&file)
                .output()
                .map_err(|e| format!("Could not start the installer (pkexec): {e}"))?;
            match out.status.code() {
                Some(0) => Ok("restart".into()),
                Some(126) | Some(127) => {
                    Err("Update cancelled: the administrator password was not entered.".into())
                }
                _ => {
                    let err = String::from_utf8_lossy(&out.stderr);
                    let tail: Vec<&str> = err.lines().rev().take(3).collect();
                    Err(format!(
                        "Installing the update failed: {}",
                        tail.into_iter().rev().collect::<Vec<_>>().join(" ")
                    ))
                }
            }
        }
        Kind::AppImage(target) => {
            let tmp = target.with_extension("AppImage.new");
            std::fs::copy(&file, &tmp)
                .map_err(|e| format!("Cannot write next to {}: {e}", target.display()))?;
            #[cfg(unix)]
            make_executable(&tmp).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&file);
            Ok("restart".into())
        }
        Kind::Windows => {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let dir = exe.parent().ok_or("The program folder is unknown")?.to_path_buf();
            let script = windows_update_script(std::process::id(), &nsis_step(&file, &dir), &exe, &update_log());
            spawn_update_script(&script).map_err(|e| format!("Could not start the installer: {e}"))?;
            let app3 = app2.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                app3.exit(0);
            });
            Ok("exiting".into())
        }
        Kind::Unmanaged => unreachable!(),
    }
}

/// Start the freshly installed version after this process exits (a direct re-exec would be
/// caught by the single-instance guard while this process is still alive).
#[tauri::command]
pub fn restart_app(app: AppHandle) -> Result<(), String> {
    let target = match install_kind() {
        Kind::AppImage(p) => p,
        _ => std::env::current_exe().map_err(|e| e.to_string())?,
    };
    #[cfg(unix)]
    {
        std::process::Command::new("sh")
            .args(["-c", "sleep 1.5; exec \"$0\"", &target.to_string_lossy()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        let _ = &target;
    }
    app.exit(0);
    Ok(())
}

// ---------------------------------------------------------------- release notes

/// Release notes as plain text for the update dialog: the lines GitHub adds to generated notes
/// ("Full Changelog: https://github.com/.../compare/...", pull-request credits) and Markdown
/// marks are left out.
fn clean_notes(body: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for raw in body.replace('\r', "").lines() {
        let low = raw.to_lowercase();
        let heading = low.trim_start_matches(|c: char| c == '#' || c.is_whitespace()).trim();
        if low.contains("full changelog")
            || low.contains("made their first contribution")
            || heading == "new contributors"
            || (low.contains("github.com/") && (low.contains("/compare/") || low.contains("/commits/")))
        {
            continue;
        }
        let mut line = raw.to_string();
        // "* Fix by @someone in https://github.com/.../pull/3" → "* Fix"
        if let Some(i) = line.find(" by @") {
            if line[i..].contains(" in https://github.com/") {
                line.truncate(i);
            }
        }
        let line = strip_links(&line).replace("**", "").replace("__", "").replace('`', "");
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        let mut rest = line[indent.len()..].to_string();
        if rest.starts_with('#') {
            rest = rest.trim_start_matches('#').trim_start().to_string();
        } else if rest.starts_with("* ") || rest.starts_with("- ") {
            rest = format!("\u{2022} {}", rest[2..].trim_start());
        }
        lines.push(format!("{indent}{rest}").trim_end().to_string());
    }
    let mut text = lines.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    text.trim().to_string()
}

/// `[text](link)` → `text`.
fn strip_links(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        if let Some(mid) = rest[open..].find("](").map(|i| open + i) {
            if let Some(end) = rest[mid + 2..].find(')').map(|i| mid + 2 + i) {
                out.push_str(&rest[..open]);
                out.push_str(&rest[open + 1..mid]);
                rest = &rest[end + 1..];
                continue;
            }
        }
        out.push_str(&rest[..=open]);
        rest = &rest[open + 1..];
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------- Windows: finish the update
//
// After this process quits, a hidden PowerShell script waits for it to exit, installs the update
// and starts the new version; every step is logged to %TEMP%\ENRecorder-update.log. The script travels
// as -EncodedCommand: a `cmd /C "..."` line does not work here, because Windows programs escape
// the quotes around paths as \" (which cmd.exe does not understand) and because cmd's `timeout`
// fails without a console.

/// A PowerShell single-quoted string literal.
#[cfg_attr(not(windows), allow(dead_code))]
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16) | ((*chunk.get(1).unwrap_or(&0) as u32) << 8) | (*chunk.get(2).unwrap_or(&0) as u32);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= chunk.len() { ABC[((n >> shift) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// PowerShell that waits for this process (`pid`) to quit, runs `step`, then starts `exe`.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_update_script(pid: u32, step: &str, exe: &Path, log: &Path) -> String {
    [
        "$ErrorActionPreference = 'Continue'".to_string(),
        format!("$log = {}", ps_quote(&log.to_string_lossy())),
        "function Log($m) { Add-Content -LiteralPath $log -Value ((Get-Date -Format s) + '  ' + $m) -Encoding UTF8 }".to_string(),
        format!("Log 'Update: waiting for ENRecorder (process {pid}) to quit'"),
        format!("Wait-Process -Id {pid} -Timeout 60 -ErrorAction SilentlyContinue"),
        format!("try {{ {step} }} catch {{ Log ('The update was not installed: ' + $_) }}"),
        format!("try {{ Start-Process -FilePath {} }} catch {{ Log ('Could not start ENRecorder: ' + $_) }}", ps_quote(&exe.to_string_lossy())),
    ]
    .join("\n")
}

/// Runs the NSIS installer silently into `dir` (`/D=` must come last and stay unquoted).
#[cfg_attr(not(windows), allow(dead_code))]
fn nsis_step(installer: &Path, dir: &Path) -> String {
    format!(
        "$p = Start-Process -FilePath {} -Wait -PassThru -ArgumentList '/S', {}; Log ('Installer finished with exit code ' + $p.ExitCode)",
        ps_quote(&installer.to_string_lossy()),
        ps_quote(&format!("/D={}", dir.display()))
    )
}

/// Arguments for `powershell.exe` that run `script` hidden (nothing in them needs quoting).
#[cfg_attr(not(windows), allow(dead_code))]
fn powershell_args(script: &str) -> Vec<String> {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-EncodedCommand"]
        .iter()
        .map(|s| s.to_string())
        .chain([base64(&utf16)])
        .collect()
}

/// Starts the update script; this process should quit right after.
#[cfg_attr(not(windows), allow(dead_code))]
fn spawn_update_script(script: &str) -> std::io::Result<()> {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let mut cmd = std::process::Command::new(root.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
    cmd.args(powershell_args(script))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000 | 0x0000_0200); // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
    }
    cmd.spawn().map(|_| ())
}

#[cfg_attr(not(windows), allow(dead_code))]
fn update_log() -> PathBuf {
    std::env::temp_dir().join("ENRecorder-update.log")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release(tag: &str) -> serde_json::Value {
        let sha = format!("sha256:{}", "AB".repeat(32));
        json!({ "tag_name": tag, "body": " Fixes ", "html_url": "https://example.invalid/r", "assets": [
            { "name": "ENRecorder_9.0.0_x64-setup.exe", "browser_download_url": "https://x/w.exe", "size": 3, "digest": sha },
            { "name": "ENRecorder_9.0.0_amd64.AppImage", "browser_download_url": "https://x/a.AppImage", "size": 4, "digest": sha },
            { "name": "ENRecorder_9.0.0_amd64.deb", "browser_download_url": "https://x/a.deb", "size": 5, "digest": sha },
        ]})
    }

    #[test]
    fn versions_compare_numerically() {
        assert_eq!(parse_version("v0.10.2"), (0, 10, 2));
        assert_eq!(parse_version("1.2"), (1, 2, 0));
        assert!(parse_version("v0.10.0") > parse_version("0.9.9"));
    }

    #[test]
    fn picks_the_asset_for_this_install() {
        let deb = update_from_release(&release("v9.0.0"), "0.2.0", &Kind::Deb).unwrap();
        assert_eq!(
            (deb.asset_name.as_str(), deb.notes.as_str(), deb.installable),
            ("ENRecorder_9.0.0_amd64.deb", "Fixes", true)
        );
        assert_eq!(deb.sha256, "ab".repeat(32));
        let img =
            update_from_release(&release("v9.0.0"), "0.2.0", &Kind::AppImage("/x".into())).unwrap();
        assert_eq!(img.asset_name, "ENRecorder_9.0.0_amd64.AppImage");
        let win = update_from_release(&release("v9.0.0"), "0.2.0", &Kind::Windows).unwrap();
        assert_eq!(win.asset_name, "ENRecorder_9.0.0_x64-setup.exe");
        let src = update_from_release(&release("v9.0.0"), "0.2.0", &Kind::Unmanaged).unwrap();
        assert!(!src.installable);
        assert!(update_from_release(&release("v0.2.0"), "0.2.0", &Kind::Deb).is_none());
    }

    #[test]
    fn release_notes_leave_out_github_links() {
        assert_eq!(clean_notes("**Full Changelog**: https://github.com/mestiaque/x/compare/v0.2.9...v0.2.10"), "");
        let notes = clean_notes(
            "## What's Changed\r\n* Faster scans by @me in https://github.com/a/b/pull/3\r\n* **Bold** fix, see [docs](https://x)\r\n\r\n\
             ## New Contributors\r\n* @x made their first contribution in https://github.com/a/b/pull/1\r\n\r\n\
             **Full Changelog**: https://github.com/a/b/compare/v1...v2",
        );
        assert_eq!(notes, "What's Changed\n\u{2022} Faster scans\n\u{2022} Bold fix, see docs");
    }

    #[test]
    fn windows_update_script_waits_quotes_and_encodes() {
        assert_eq!(base64(b"Hi"), "SGk=");
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b"ab"), "YWI=");
        let step = nsis_step(Path::new(r"C:\Users\O'Neil\Cache\ENRecorder_9.0.0_x64-setup.exe"), Path::new(r"C:\Users\O'Neil\AppData\Local\ENRecorder"));
        assert!(step.contains(r"-FilePath 'C:\Users\O''Neil\Cache\ENRecorder_9.0.0_x64-setup.exe'"));
        assert!(step.contains(r"-ArgumentList '/S', '/D=C:\Users\O''Neil\AppData\Local\ENRecorder'"));
        let script = windows_update_script(4242, &step, Path::new(r"C:\Program Files\ENRecorder\app.exe"), Path::new(r"C:\Temp\ENRecorder-update.log"));
        assert!(script.contains("Wait-Process -Id 4242"));
        assert!(script.contains(r"Start-Process -FilePath 'C:\Program Files\ENRecorder\app.exe'"));
        let args = powershell_args("A");
        assert_eq!(args.last().map(String::as_str), Some("QQA=")); // "A" in UTF-16LE
        assert!(args.iter().all(|a| !a.contains('"')));
    }
}
