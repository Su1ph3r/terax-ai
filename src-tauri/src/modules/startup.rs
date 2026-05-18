use std::path::PathBuf;
use std::sync::OnceLock;

// CLI-supplied initial workspace path (#280). One of:
//
//   terax.exe -path D:\projects\foo
//   terax.exe --path D:\projects\foo
//   terax.exe D:\projects\foo
//
// Validated to be an existing directory at parse time; an invalid value
// drops to `None` and the app starts as if nothing was passed.
//
// Consumers:
//   - frontend reads via `get_startup_path` and uses it as the initial
//     explorer / workspace root
//   - modules::pty::shell_init falls back to it before $HOME when spawning
//     fresh PTYs, so the first terminal lands in the same place
static STARTUP_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Parse the process args once at startup. Idempotent.
pub fn init_startup_path() {
    STARTUP_PATH.set(parse_args(std::env::args().skip(1))).ok();
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Option<PathBuf> {
    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        // Tauri / wry pass `--` and webview flags through to argv; ignore
        // anything that looks like our own option but isn't the path flag,
        // and skip the standard `--` separator entirely.
        if arg == "--" {
            continue;
        }
        if arg == "-path" || arg == "--path" {
            return iter.next().and_then(validate_dir);
        }
        if let Some(rest) = arg.strip_prefix("--path=") {
            return validate_dir(rest.to_string());
        }
        if let Some(rest) = arg.strip_prefix("-path=") {
            return validate_dir(rest.to_string());
        }
        // Positional form: `terax.exe D:\proj`. Accept the first non-flag
        // arg that resolves to an existing directory. Flags get filtered
        // out by the leading `-` check so `--devtools` etc. won't be
        // mistaken for a path.
        if !arg.starts_with('-') {
            if let Some(p) = validate_dir(arg) {
                return Some(p);
            }
        }
    }
    None
}

fn validate_dir(raw: String) -> Option<PathBuf> {
    let p = PathBuf::from(raw);
    if p.is_dir() {
        Some(p)
    } else {
        log::warn!("startup path ignored, not a directory: {}", p.display());
        None
    }
}

/// Returns the captured startup path, if one was provided and valid.
pub fn startup_path() -> Option<PathBuf> {
    STARTUP_PATH.get().and_then(|o| o.clone())
}

/// Frontend reads this on mount to bias the initial explorer root. Returns
/// a forward-slash-normalized string so it compares cleanly with the
/// `homeDir()` value the app already keeps in that form.
#[tauri::command]
pub fn get_startup_path() -> Option<String> {
    startup_path().map(|p| p.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    #[test]
    fn empty_args_yield_none() {
        assert_eq!(parse_args(Vec::<String>::new()), None);
    }

    #[test]
    fn ignores_unknown_flags() {
        let args = vec!["--devtools".to_string(), "--verbose".to_string()];
        assert_eq!(parse_args(args), None);
    }

    #[test]
    fn nonexistent_path_is_rejected() {
        let args = vec![
            "--path".to_string(),
            "/definitely/does/not/exist/ever".to_string(),
        ];
        assert_eq!(parse_args(args), None);
    }

    #[test]
    fn accepts_path_flag_for_existing_dir() {
        // Use the project's own temp dir or current dir — guaranteed to
        // exist on whatever machine runs the test.
        let temp = std::env::temp_dir();
        let s = temp.to_string_lossy().to_string();
        let args = vec!["--path".to_string(), s.clone()];
        let got = parse_args(args).expect("temp dir should be parsed");
        assert_eq!(got, temp);
    }

    #[test]
    fn accepts_equals_form() {
        let temp = std::env::temp_dir();
        let s = format!("--path={}", temp.to_string_lossy());
        let got = parse_args(vec![s]).expect("--path= should parse");
        assert_eq!(got, temp);
    }

    #[test]
    fn accepts_single_dash_flag() {
        let temp = std::env::temp_dir();
        let s = temp.to_string_lossy().to_string();
        let args = vec!["-path".to_string(), s.clone()];
        let got = parse_args(args).expect("-path should parse");
        assert_eq!(got, temp);
    }

    #[test]
    fn accepts_positional_for_existing_dir() {
        let temp = std::env::temp_dir();
        let s = temp.to_string_lossy().to_string();
        let got = parse_args(vec![s.clone()]).expect("positional should parse");
        assert_eq!(got, temp);
    }

    #[test]
    fn skips_dash_dash_separator() {
        let temp = std::env::temp_dir();
        let s = temp.to_string_lossy().to_string();
        let got =
            parse_args(vec!["--".to_string(), "--path".to_string(), s.clone()]).expect("parsed");
        assert_eq!(got, temp);
    }
}
