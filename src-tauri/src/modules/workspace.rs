use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct WorkspaceRegistry {
    roots: Mutex<HashSet<PathBuf>>,
}

impl WorkspaceRegistry {
    pub fn authorize<P: AsRef<Path>>(&self, path: P) -> std::io::Result<PathBuf> {
        let canonical = std::fs::canonicalize(path.as_ref())?;
        let mut set = self.roots.lock().expect("workspace registry poisoned");
        set.insert(canonical.clone());
        Ok(canonical)
    }

    pub fn is_authorized(&self, target: &Path) -> bool {
        let set = self.roots.lock().expect("workspace registry poisoned");
        set.iter().any(|root| target.starts_with(root))
    }
}

pub fn bootstrap_registry(registry: &WorkspaceRegistry) {
    if let Ok(cwd) = std::env::current_dir() {
        let _ = registry.authorize(cwd);
    }
    if let Some(home) = dirs::home_dir() {
        let _ = registry.authorize(home);
    }
}

#[tauri::command]
pub async fn workspace_authorize(
    path: String,
    registry: tauri::State<'_, WorkspaceRegistry>,
) -> Result<String, String> {
    let canonical = registry.authorize(&path).map_err(|e| e.to_string())?;
    Ok(canonical.to_string_lossy().replace('\\', "/"))
}

#[tauri::command]
pub async fn workspace_current_dir(
    registry: tauri::State<'_, WorkspaceRegistry>,
) -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let canonical = registry.authorize(&cwd).map_err(|e| e.to_string())?;
    Ok(canonical.to_string_lossy().replace('\\', "/"))
}



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

// Globally active workspace env. The frontend has a single ambient
// `useWorkspaceEnvStore` value (Local or WSL: <distro>) that gates whether
// the file tree, source control, etc. operate on Windows paths or POSIX
// WSL paths. Mirror that into Rust so subsystems that can't easily plumb
// the env through their signatures (e.g. modules::git::process) can route
// through wsl.exe when appropriate — see #333.
static ACTIVE_ENV: OnceLock<Mutex<WorkspaceEnv>> = OnceLock::new();

fn active_env_cell() -> &'static Mutex<WorkspaceEnv> {
    ACTIVE_ENV.get_or_init(|| Mutex::new(WorkspaceEnv::Local))
}

pub fn active_env() -> WorkspaceEnv {
    active_env_cell()
        .lock()
        .expect("active env poisoned")
        .clone()
}

fn set_active_env_inner(env: WorkspaceEnv) {
    let mut guard = active_env_cell().lock().expect("active env poisoned");
    *guard = env;
}

#[tauri::command]
pub async fn workspace_set_active_env(workspace: WorkspaceEnv) -> Result<(), String> {
    set_active_env_inner(workspace);
    Ok(())
}

// User-configured persistent home directory override (#190). The setting
// lives in terax-settings.json; the frontend mirrors changes here so the
// PTY resolution chain in modules::pty::shell_init can pick it up. None
// means "no override, use OS home_dir() as usual."
static CUSTOM_HOME_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn custom_home_cell() -> &'static Mutex<Option<PathBuf>> {
    CUSTOM_HOME_PATH.get_or_init(|| Mutex::new(None))
}

pub fn custom_home_path() -> Option<PathBuf> {
    custom_home_cell()
        .lock()
        .expect("custom home poisoned")
        .clone()
}

#[tauri::command]
pub async fn workspace_set_custom_home(path: Option<String>) -> Result<(), String> {
    let next = match path {
        Some(s) if !s.is_empty() => {
            let p = PathBuf::from(&s);
            if !p.is_dir() {
                return Err(format!("not a directory: {s}"));
            }
            Some(p)
        }
        _ => None,
    };
    let mut guard = custom_home_cell()
        .lock()
        .expect("custom home poisoned");
    *guard = next;
    Ok(())
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

/// True for WSL distro names safe to splice into a UNC path. Real WSL distros
/// are alphanumeric with `.`, `_`, `-` separators (e.g. `Ubuntu-22.04`). Reject
/// anything that could traverse out of the `\\wsl.localhost\<distro>\` prefix
/// (`..`, `\`, `/`, `:`, `?`, `*`, control bytes) or empty names.
#[cfg(windows)]
fn is_safe_distro_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    if name == "." || name == ".." || name.starts_with('.') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' '))
        && !name.contains("..")
}

#[cfg(windows)]
pub fn wsl_path_to_unc(distro: &str, path: &str) -> PathBuf {
    // Defense-in-depth: refuse to construct a UNC path with a distro name that
    // could escape the WSL share root via `..`, `\`, or other path metachars.
    // Returns a clearly-invalid path that downstream `is_dir()`/`metadata()`
    // checks will reject. The webview's distro list comes from `wsl.exe --list`
    // and is normally trustworthy, but a locally-registered malicious distro
    // can name itself with traversal characters; this filter blocks that.
    if !is_safe_distro_name(distro) {
        return PathBuf::from(r"\\wsl.localhost\__terax_invalid_distro__");
    }
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
    fn distro_validator_accepts_real_names() {
        assert!(is_safe_distro_name("Ubuntu"));
        assert!(is_safe_distro_name("Ubuntu-22.04"));
        assert!(is_safe_distro_name("Debian"));
        assert!(is_safe_distro_name("Alpine_3.18"));
        assert!(is_safe_distro_name("openSUSE-Tumbleweed"));
    }

    #[test]
    fn distro_validator_rejects_path_traversal() {
        assert!(!is_safe_distro_name(".."));
        assert!(!is_safe_distro_name("..\\..\\Windows"));
        assert!(!is_safe_distro_name("../foo"));
        assert!(!is_safe_distro_name("foo/bar"));
        assert!(!is_safe_distro_name("foo\\bar"));
        assert!(!is_safe_distro_name("foo..bar"));
    }

    #[test]
    fn distro_validator_rejects_special_chars() {
        assert!(!is_safe_distro_name("foo:bar"));
        assert!(!is_safe_distro_name("foo?bar"));
        assert!(!is_safe_distro_name("foo*bar"));
        assert!(!is_safe_distro_name("foo\0bar"));
        assert!(!is_safe_distro_name(""));
        assert!(!is_safe_distro_name(".hidden"));
    }

    #[test]
    fn wsl_path_to_unc_blocks_traversal_distro() {
        // Malicious distro name must produce a path that is_dir() will reject,
        // never escape the WSL share root.
        let p = wsl_path_to_unc("..\\..\\..\\Windows", "/etc/passwd");
        let s = p.to_string_lossy();
        assert!(s.contains("__terax_invalid_distro__"), "got: {s}");
        assert!(!s.contains("\\..\\"), "got: {s}");
    }

    #[test]
    fn wsl_path_to_unc_accepts_valid_distro() {
        let p = wsl_path_to_unc("Ubuntu", "/etc/hosts");
        let s = p.to_string_lossy();
        assert!(!s.contains("__terax_invalid_distro__"), "got: {s}");
    }

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
