//! What the cmdlets need from PowerShell's side: environment variables as the session sees them,
//! the PowerShell version, and the host, which Write-ColorEX writes to the way Write-Host does
//! through PWRS's Pipeline::write_host.

use std::mem::ManuallyDrop;

use pwrs::prelude::*;
use pwrs::{ConsoleColor, ErrorCategory, PsError};

use crate::set_once::SetOnce;

/// An environment variable as the session sees it. PowerShell 7 on Linux and macOS keeps
/// `$env:` changes in .NET's copy of the environment, out of reach of the C library, so the
/// variable is read through .NET.
pub fn env_var(name: &str) -> PsResult<Option<String>> {
    let value = PsType::from_name("System.Environment").call_static("GetEnvironmentVariable", &[name.into_ps()?])?;
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(String::from_ps(&value)?))
}

/// An empty hashtable whose keys compare without regard to case, as PowerShell's @{} does.
pub fn new_table() -> PsResult<PsHashtable> {
    let comparer = PsType::from_name("System.StringComparer").call_static("get_CurrentCultureIgnoreCase", &[])?;
    Ok(PsHashtable(PsType::from_name("System.Collections.Hashtable").new(&[comparer])?))
}

/// The PowerShell major and minor version, read once per session.
pub fn ps_version(ps: &Pipeline<'_>) -> (i64, i64) {
    static VERSION: SetOnce<(i64, i64)> = SetOnce::new();
    *VERSION.get_or_init(|| {
        let read = || -> PsResult<(i64, i64)> {
            let version = ps.variable("PSVersionTable")?.get("PSVersion")?;
            Ok((i64::from_ps(&version.get("Major")?)?, i64::from_ps(&version.get("Minor")?)?))
        };
        read().unwrap_or((5, 1))
    })
}

/// Whether PowerShell removes escape codes from transcripts and from the output of a host
/// without virtual terminal support, which it does from 7.2.
pub fn removes_escape_codes(ps: &Pipeline<'_>) -> bool {
    ps_version(ps) >= (7, 2)
}

/// The running cmdlet as an object of its own. The pipeline keeps its handle, so the borrowed
/// wrapper is never dropped and the clone is the handle this call frees.
pub fn this_cmdlet(ps: &Pipeline<'_>) -> PsObject {
    // SAFETY: the handle stays valid for the phase, and ManuallyDrop keeps it from being freed
    // twice; the clone is a handle of its own.
    let borrowed = ManuallyDrop::new(unsafe { PsObject::from_raw(ps.cmdlet_handle()) });
    (*borrowed).clone()
}

/// The host's raw user interface, or None for a host without one.
fn raw_ui(cmdlet: &PsObject) -> Option<PsObject> {
    let raw = cmdlet.get("Host").and_then(|h| h.get("UI")).and_then(|ui| ui.get("RawUI")).ok()?;
    (!raw.is_null()).then_some(raw)
}

/// Whether the host lets escape codes through to the screen. PowerShell 7.2 and later remove
/// them from the output of a host without virtual terminal support, such as a process with no
/// console; earlier versions pass them through. Read once per session.
pub fn host_virtual_terminal(ps: &Pipeline<'_>) -> bool {
    static VT: SetOnce<bool> = SetOnce::new();
    *VT.get_or_init(|| {
        if !removes_escape_codes(ps) {
            return true;
        }
        let cmdlet = this_cmdlet(ps);
        cmdlet
            .get("Host")
            .and_then(|h| h.get("UI"))
            .and_then(|ui| ui.get("SupportsVirtualTerminal"))
            .and_then(|v| bool::from_ps(&v))
            .unwrap_or(false)
    })
}

/// Whether escape codes written to the host reach the screen: the host lets them through and
/// $PSStyle.OutputRendering is not PlainText, which can change at any time.
pub fn host_ansi(ps: &Pipeline<'_>) -> bool {
    if !host_virtual_terminal(ps) {
        return false;
    }
    if !removes_escape_codes(ps) {
        return true;
    }
    let plain = ps
        .variable("PSStyle")
        .and_then(|style| if style.is_null() { Ok(String::new()) } else { String::from_ps(&style.get("OutputRendering")?) })
        .map(|rendering| rendering.eq_ignore_ascii_case("PlainText"))
        .unwrap_or(false);
    !plain
}

/// The width of the console in cells, or 0 when there is none to measure, as with output
/// redirected, where PowerShell reports -1, or a host without a raw user interface.
pub fn host_width(ps: &Pipeline<'_>) -> i64 {
    let Some(raw) = raw_ui(&this_cmdlet(ps)) else { return 0 };
    let width = |property: &str| -> i64 {
        raw.get(property)
            .and_then(|size| size.get("Width"))
            .and_then(|w| i64::from_ps(&w))
            .unwrap_or(0)
    };
    let window = width("WindowSize");
    if window > 0 {
        return window;
    }
    width("BufferSize").max(0)
}

/// The console colors by the names the color table gives them.
const CONSOLE_COLORS: [(&str, ConsoleColor); 16] = [
    ("Black", ConsoleColor::Black),
    ("DarkBlue", ConsoleColor::DarkBlue),
    ("DarkGreen", ConsoleColor::DarkGreen),
    ("DarkCyan", ConsoleColor::DarkCyan),
    ("DarkRed", ConsoleColor::DarkRed),
    ("DarkMagenta", ConsoleColor::DarkMagenta),
    ("DarkYellow", ConsoleColor::DarkYellow),
    ("Gray", ConsoleColor::Gray),
    ("DarkGray", ConsoleColor::DarkGray),
    ("Blue", ConsoleColor::Blue),
    ("Green", ConsoleColor::Green),
    ("Cyan", ConsoleColor::Cyan),
    ("Red", ConsoleColor::Red),
    ("Magenta", ConsoleColor::Magenta),
    ("Yellow", ConsoleColor::Yellow),
    ("White", ConsoleColor::White),
];

/// The console color a name gives, compared without regard to case as PowerShell converts it.
fn console_color(name: &str) -> PsResult<ConsoleColor> {
    CONSOLE_COLORS
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(name))
        .map(|(_, color)| *color)
        .ok_or_else(|| PsError::new(ErrorCategory::InvalidArgument, "UnknownConsoleColor", format!("'{name}' is not a console color")))
}

/// Writes to the host as Write-Host does: PWRS hands the text to the information stream with the
/// PSHOST tag, takes each color left out from the host's current colors, and in Windows
/// PowerShell 5.1 records the write in the transcript.
pub fn write_host(ps: &Pipeline<'_>, text: &str, fg: Option<&str>, bg: Option<&str>, no_newline: bool) -> PsResult<()> {
    let foreground = fg.map(console_color).transpose()?;
    let background = bg.map(console_color).transpose()?;
    ps.write_host(text, foreground, background, no_newline)
}
