//! The log file Write-ColorEX writes with -LogFile: where it goes, the line written, and the text
//! encoding, the same as PSWriteColorEX's.

use std::time::Duration;

use pwrs::prelude::*;

use crate::host::this_cmdlet;
use crate::write::{now_text, WriteOptions};

/// The session's path methods, $ExecutionContext.SessionState.Path.
fn session_path(ps: &Pipeline<'_>) -> PsResult<PsObject> {
    this_cmdlet(ps).get("SessionState")?.get("Path")
}

/// The folder a log file named without a folder goes in: the calling script's folder, or the
/// current file system location when the caller is the prompt and has no script folder.
pub fn resolve_log_folder(ps: &Pipeline<'_>, script_root: &str) -> PsResult<String> {
    if !script_root.is_empty() {
        return Ok(script_root.to_string());
    }
    let location = session_path(ps)?.get("CurrentFileSystemLocation")?;
    String::from_ps(&location.get("ProviderPath")?)
}

/// Whether a file name ends with an extension: a dot and one or more word characters, as the
/// pattern `\.\w+$` matches.
fn has_extension(name: &str) -> bool {
    let name = name.strip_suffix('\n').unwrap_or(name);
    match name.rfind('.') {
        Some(dot) => {
            let extension = &name[dot + 1..];
            !extension.is_empty() && extension.chars().all(|c| c.is_alphanumeric() || c == '_')
        }
        None => false,
    }
}

/// The text encoding a log file is written in, by the name -Encoding takes. Each name gives the
/// same bytes on Windows PowerShell 5.1 and PowerShell 7: utf8, utf8NoBOM and default write UTF-8
/// without a byte order mark, utf8BOM with one, and unicode, string and unknown write UTF-16
/// little-endian with one. ansi and oem write the system's code pages on Windows and UTF-8
/// elsewhere, where the system's text is UTF-8.
fn log_encoding(name: &str) -> PsResult<PsObject> {
    let new = |type_name: &str, args: &[bool]| -> PsResult<PsObject> {
        let args = args.iter().map(|&b| b.into_ps()).collect::<PsResult<Vec<_>>>()?;
        PsType::from_name(type_name).new(&args)
    };
    let code_page = |property: &str| -> PsResult<PsObject> {
        if !cfg!(windows) {
            return new("System.Text.UTF8Encoding", &[false]);
        }
        let culture = PsType::from_name("System.Globalization.CultureInfo").call_static("get_CurrentCulture", &[])?;
        let page = culture.get("TextInfo")?.get(property)?;
        PsType::from_name("System.Text.Encoding").call_static("GetEncoding", &[page])
    };
    match name.to_ascii_lowercase().as_str() {
        "utf8bom" => new("System.Text.UTF8Encoding", &[true]),
        "unicode" | "string" | "unknown" => new("System.Text.UnicodeEncoding", &[false, true]),
        "bigendianunicode" => new("System.Text.UnicodeEncoding", &[true, true]),
        "utf32" => new("System.Text.UTF32Encoding", &[false, true]),
        "bigendianutf32" => new("System.Text.UTF32Encoding", &[true, true]),
        "ascii" => new("System.Text.ASCIIEncoding", &[]),
        "utf7" => new("System.Text.UTF7Encoding", &[]),
        "ansi" => code_page("ANSICodePage"),
        "oem" => code_page("OEMCodePage"),
        _ => new("System.Text.UTF8Encoding", &[false]),
    }
}

/// A failed write as PowerShell words the .NET call failing in a script, with .NET's text for
/// the common causes.
fn call_failure(method: &str, arguments: usize, error: &std::io::Error, path: &str) -> String {
    let text = if cfg!(windows) && error.raw_os_error() == Some(32) {
        format!("The process cannot access the file '{path}' because it is being used by another process.")
    } else {
        match error.kind() {
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::IsADirectory => format!("Access to the path '{path}' is denied."),
            std::io::ErrorKind::NotADirectory => format!("Could not find a part of the path '{path}'."),
            std::io::ErrorKind::NotFound if method == "CreateDirectory" && !cfg!(windows) => format!("Could not find file '{path}'."),
            std::io::ErrorKind::NotFound => format!("Could not find a part of the path '{path}'."),
            _ => error.to_string(),
        }
    };
    format!("Exception calling \"{method}\" with \"{arguments}\" argument(s): \"{text}\"")
}

/// Appends bytes to a file, with the encoding's byte order mark first when the file is new or
/// empty, as File.AppendAllText does, creating the file's folder first.
fn append(path: &str, preamble: &[u8], bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    let file_path = std::path::Path::new(path);
    if let Some(folder) = file_path.parent()
        && !folder.as_os_str().is_empty() && !folder.is_dir() {
            let folder_text = folder.to_string_lossy();
            std::fs::create_dir_all(folder).map_err(|e| call_failure("CreateDirectory", 1, &e, &folder_text))?;
        }
    let failure = |e: std::io::Error| call_failure("AppendAllText", 3, &e, path);
    let mut file = std::fs::OpenOptions::new().append(true).create(true).open(file_path).map_err(failure)?;
    let empty = file.metadata().map_err(failure)?.len() == 0;
    let mut data = Vec::with_capacity(preamble.len() + bytes.len());
    if empty {
        data.extend_from_slice(preamble);
    }
    data.extend_from_slice(bytes);
    file.write_all(&data).map_err(failure)
}

/// Appends the text, without markup tags, to the log file, creating its folder, with up to
/// -LogRetry tries 50 ms apart. A file name alone goes in -LogPath, or the calling script's
/// folder, or the current location, with .log added when it has no extension.
pub fn write_log(ps: &Pipeline<'_>, o: &WriteOptions, text: &str, mut debug: impl FnMut(&str) -> PsResult<()>) -> PsResult<()> {
    debug(&format!("Writing to log file: {}", o.log_file))?;

    let paths = session_path(ps)?;
    let mut log_name = o.log_file.clone();
    if !log_name.contains(['\\', '/']) {
        if !has_extension(&log_name) {
            log_name.push_str(".log");
        }
        let folder = if o.log_path.is_empty() { resolve_log_folder(ps, &o.caller_script_root)? } else { o.log_path.clone() };
        log_name = String::from_ps(&paths.call("Combine", &[folder.into_ps()?, log_name.into_ps()?])?)?;
    }
    let file_path = paths.call("GetUnresolvedProviderPathFromPSPath", &[log_name.into_ps()?])?;

    let mut log_info = String::new();
    if o.log_time {
        log_info.push_str(&format!("[{}]", now_text(&o.date_time_format)?));
    }
    if !o.log_level.is_empty() {
        log_info.push_str(&format!("[{}]", o.log_level));
    }
    let mut entry = if log_info.is_empty() { text.to_string() } else { format!("{log_info} {text}") };
    if !o.no_new_line {
        entry.push_str(if cfg!(windows) { "\r\n" } else { "\n" });
    }
    let encoding = log_encoding(&o.encoding)?;
    let preamble = Vec::<u8>::from_ps(&encoding.call("GetPreamble", &[])?)?;
    let bytes = Vec::<u8>::from_ps(&encoding.call("GetBytes", &[entry.into_ps()?])?)?;
    let file_path = String::from_ps(&file_path)?;

    let attempts = o.log_retry.max(1);
    for attempt in 1..=attempts {
        match append(&file_path, &preamble, &bytes) {
            Ok(()) => {
                debug("Successfully wrote to log file")?;
                break;
            }
            Err(message) if attempt >= attempts => {
                ps.warning(&format!("Write-ColorEX - Couldn't write to log file {message}. Tried ({attempt}/{attempts})"))?;
            }
            Err(_) => {
                debug(&format!("Log write failed, retrying... ({attempt}/{attempts})"))?;
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::has_extension;

    #[test]
    fn extensions_match_the_powershell_pattern() {
        assert!(has_extension("app.log"));
        assert!(has_extension("app.txt"));
        assert!(has_extension("a.b_c"));
        assert!(!has_extension("app"));
        assert!(!has_extension("app."));
        assert!(!has_extension("app.lo-g"));
    }
}
