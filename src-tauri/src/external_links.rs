use std::io;
use std::process::Command;

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    let normalized = url.trim();
    if normalized.is_empty() {
        return Err("URL is empty.".to_string());
    }

    let lower = normalized.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("Only HTTP(S) links are allowed.".to_string());
    }

    open_in_default_browser(normalized)
        .map_err(|error| format!("Failed to open link in browser: {error}"))
}

fn open_in_default_browser(url: &str) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let status = Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()?;

        if status.success() {
            return Ok(());
        }

        return Err(io::Error::other(format!(
            "cmd returned non-zero status: {status}"
        )));
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open").arg(url).status()?;
        if status.success() {
            return Ok(());
        }

        return Err(io::Error::other(format!(
            "open returned non-zero status: {status}"
        )));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = Command::new("xdg-open").arg(url).status()?;
        if status.success() {
            return Ok(());
        }

        return Err(io::Error::other(format!(
            "xdg-open returned non-zero status: {status}"
        )));
    }

    #[allow(unreachable_code)]
    Err(io::Error::other(
        "Opening links is not supported on this platform.",
    ))
}

