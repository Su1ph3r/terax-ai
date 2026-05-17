use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum WorkspaceEnv {
    #[default]
    Local,
    Wsl {
        distro: String,
    },
}

impl WorkspaceEnv {
    pub fn from_option(workspace: Option<Self>) -> Self {
        workspace.unwrap_or_default()
    }

    pub fn is_wsl(&self) -> bool {
        matches!(self, Self::Wsl { .. })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WslDistro {
    pub name: String,
    pub default: bool,
    pub running: bool,
}

#[cfg(windows)]
pub fn resolve_path(path: &str, workspace: &WorkspaceEnv) -> PathBuf {
    match workspace {
        WorkspaceEnv::Local => PathBuf::from(path),
        WorkspaceEnv::Wsl { distro } => wsl_path_to_unc(distro, path),
    }
}

#[cfg(not(windows))]
pub fn resolve_path(path: &str, _workspace: &WorkspaceEnv) -> PathBuf {
    PathBuf::from(path)
}

#[cfg(windows)]
pub fn wsl_path_to_unc(distro: &str, path: &str) -> PathBuf {
    let normalized = path.replace('\\', "/");

    // `/mnt/<drive>[/...]` is WSL's drvfs mount for a Windows drive. Going
    // through `\\wsl.localhost\<distro>\mnt\<drive>` round-trips the request
    // through the WSL VM's 9P proxy back into drvfs, and Windows frequently
    // answers that round-trip with ERROR_ACCESS_DENIED on directory
    // enumeration — even for directories the user can read natively. Short-
    // circuit to the underlying Windows path so the explorer sees the same
    // files as Windows Explorer would, with the user's actual permissions.
    if let Some(win) = drvfs_mount_to_windows(&normalized) {
        return win;
    }

    let trimmed = normalized.trim_start_matches('/');
    let primary = PathBuf::from(format!(
        r"\\wsl.localhost\{}\{}",
        distro,
        trimmed.replace('/', r"\")
    ));
    if primary.exists() {
        return primary;
    }
    PathBuf::from(format!(r"\\wsl$\{}\{}", distro, trimmed.replace('/', r"\")))
}

/// Translate `/mnt/<drive>[/rest]` to a native Windows path like `C:\rest`.
/// Returns `None` for anything that isn't a single-letter drvfs mount —
/// `/mnt/wsl`, `/home/...`, `/etc/...` etc. fall through to the regular UNC
/// resolution. Matches the convention `wsl.conf`'s `automount.root` uses for
/// Windows drives.
#[cfg(windows)]
fn drvfs_mount_to_windows(unix_path: &str) -> Option<PathBuf> {
    let after = unix_path.strip_prefix("/mnt/")?;
    let mut parts = after.splitn(2, '/');
    let drive = parts.next()?;
    if drive.len() != 1 {
        return None;
    }
    let letter = drive.chars().next()?;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    let rest = parts.next().unwrap_or("");
    let win = format!(
        "{}:\\{}",
        letter.to_ascii_uppercase(),
        rest.replace('/', "\\")
    );
    Some(PathBuf::from(win))
}

#[cfg(windows)]
pub fn decode_command_output(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xff, 0xfe]) || looks_utf16le(bytes) {
        let start = if bytes.starts_with(&[0xff, 0xfe]) {
            2
        } else {
            0
        };
        let units: Vec<u16> = bytes[start..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(windows)]
fn looks_utf16le(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || !bytes.len().is_multiple_of(2) {
        return false;
    }
    let nul_odd = bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
    nul_odd * 2 >= bytes.len() / 2
}

#[cfg(windows)]
fn run_wsl(args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("wsl.exe")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = decode_command_output(&out.stderr);
        return Err(stderr.trim().to_string());
    }
    Ok(decode_command_output(&out.stdout))
}

#[cfg(windows)]
fn list_distros_blocking() -> Result<Vec<WslDistro>, String> {
    let out = run_wsl(&["--list", "--verbose"])?;
    let mut distros = Vec::new();
    for raw in out.lines().skip(1) {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let default = line.starts_with('*');
        let line = line.trim_start_matches('*').trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let state_idx = parts.len() - 2;
        let name = parts[..state_idx].join(" ");
        let state = parts[state_idx];
        distros.push(WslDistro {
            name,
            default,
            running: state.eq_ignore_ascii_case("Running"),
        });
    }
    Ok(distros)
}

#[tauri::command]
pub async fn wsl_list_distros() -> Result<Vec<WslDistro>, String> {
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(list_distros_blocking)
            .await
            .map_err(|e| e.to_string())?
    }
}

#[tauri::command]
pub async fn wsl_default_distro() -> Result<Option<String>, String> {
    #[cfg(not(windows))]
    {
        Ok(None)
    }
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(|| {
            let distros = list_distros_blocking()?;
            Ok(distros
                .iter()
                .find(|d| d.default)
                .map(|d| d.name.clone())
                .or_else(|| distros.first().map(|d| d.name.clone())))
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

#[tauri::command]
pub fn wsl_home(distro: String) -> Result<String, String> {
    #[cfg(not(windows))]
    {
        let _ = distro;
        Err("WSL is only available on Windows".into())
    }
    #[cfg(windows)]
    {
        let out = run_wsl(&["-d", &distro, "--exec", "sh", "-lc", "printf %s \"$HOME\""])?;
        let home = out.trim().to_string();
        if home.is_empty() {
            Err(format!("could not resolve WSL home for {distro}"))
        } else {
            Ok(home)
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn drvfs_mount_short_circuits_to_native_windows() {
        // /mnt/<single-letter> is the drvfs convention for Windows drives.
        // We translate to native paths so Windows-side fs ops don't fail
        // with ACCESS_DENIED on the WSL 9P round-trip.
        assert_eq!(
            drvfs_mount_to_windows("/mnt/c"),
            Some(PathBuf::from(r"C:\"))
        );
        assert_eq!(
            drvfs_mount_to_windows("/mnt/c/Users/me"),
            Some(PathBuf::from(r"C:\Users\me"))
        );
        assert_eq!(
            drvfs_mount_to_windows("/mnt/d/projects"),
            Some(PathBuf::from(r"D:\projects"))
        );
        // Multi-character mount names aren't drvfs drives.
        assert_eq!(drvfs_mount_to_windows("/mnt/wsl"), None);
        assert_eq!(drvfs_mount_to_windows("/mnt/wslg"), None);
        // Non-/mnt paths stay in WSL.
        assert_eq!(drvfs_mount_to_windows("/home/paul"), None);
        assert_eq!(drvfs_mount_to_windows("/etc/hosts"), None);
        // Non-alphabetic single chars (numeric, punctuation) don't qualify
        // and shouldn't trick the translation.
        assert_eq!(drvfs_mount_to_windows("/mnt/1"), None);
        assert_eq!(drvfs_mount_to_windows("/mnt/."), None);
    }

    #[test]
    fn wsl_path_to_unc_returns_native_for_drvfs() {
        // Full integration: /mnt/c under any WSL distro should produce a
        // C:\ path, not a \\wsl.localhost\ UNC.
        let p = wsl_path_to_unc("Ubuntu", "/mnt/c/Users");
        let s = p.to_string_lossy();
        assert_eq!(s, r"C:\Users");
        assert!(!s.contains("wsl.localhost"), "got: {s}");
    }

    #[test]
    fn wsl_path_to_unc_keeps_unc_for_non_drvfs() {
        // /home/paul stays as a WSL UNC path so the explorer can read into
        // the WSL filesystem itself.
        let p = wsl_path_to_unc("Ubuntu", "/home/paul");
        let s = p.to_string_lossy();
        assert!(
            s.contains("wsl.localhost") || s.contains("wsl$"),
            "got: {s}"
        );
    }
}
