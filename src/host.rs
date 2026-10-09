//! What the cmdlets need from PowerShell's side: environment variables as the session sees them,
//! the PowerShell version, and the host, which Write-ColorEX writes to the way Write-Host does.

use std::mem::ManuallyDrop;
use std::sync::OnceLock;

use pwrs::prelude::*;

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
    static VERSION: OnceLock<(i64, i64)> = OnceLock::new();
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
    static VT: OnceLock<bool> = OnceLock::new();
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

/// One write to the host, built the way Write-Host builds it: a HostInformationMessage written
/// to the information stream with the PSHOST tag, with each color left out taken from the host's
/// current colors, as Write-Host fills them. Windows PowerShell 5.1 records the information
/// stream in no transcript, so there Write-Host hands its text to the transcript itself, through
/// the host's TranscribeResult, and so does this. 5.1 keeps that method internal, so it is
/// reached by reflection.
pub struct HostWriter {
    cmdlet: PsObject,
    message_type: PsType,
    tags: Option<PsObject>,
    current: Option<(PsObject, PsObject)>,
    current_read: bool,
    transcribes: bool,
    /// The host's user interface and its TranscribeResult(string), found on the first write in
    /// Windows PowerShell 5.1.
    transcript: Option<(PsObject, PsObject)>,
}

impl HostWriter {
    pub fn new(ps: &Pipeline<'_>) -> HostWriter {
        HostWriter {
            cmdlet: this_cmdlet(ps),
            message_type: PsType::from_name("System.Management.Automation.HostInformationMessage"),
            tags: None,
            current: None,
            current_read: false,
            transcribes: ps_version(ps).0 < 6,
            transcript: None,
        }
    }

    /// Hands a write's text to the transcript as Windows PowerShell 5.1's Write-Host does. A host
    /// without the method records nothing, and a failure here never fails the write.
    fn transcribe(&mut self, text: &str) {
        if self.transcript.is_none() {
            let found = (|| -> PsResult<(PsObject, PsObject)> {
                let ui = self.cmdlet.get("Host")?.get("UI")?;
                let string_type = text.into_ps()?.call("GetType", &[])?;
                let parameter_types = PsArray(vec![string_type]).into_ps()?;
                // BindingFlags Instance, Public and NonPublic
                let flags = (4i32 | 16 | 32).into_ps()?;
                let method = ui
                    .call("GetType", &[])?
                    .call("GetMethod", &["TranscribeResult".into_ps()?, flags, PsObject::null(), parameter_types, PsObject::null()])?;
                Ok((ui, method))
            })();
            self.transcript = Some(found.unwrap_or_else(|_| (PsObject::null(), PsObject::null())));
        }
        if let Some((ui, method)) = &self.transcript
            && !method.is_null()
            && let Ok(text) = text.into_ps()
            && let Ok(arguments) = PsArray(vec![text]).into_ps()
        {
            let _ = method.call("Invoke", &[ui.clone(), arguments]);
        }
    }

    /// The host's current foreground and background colors, read once per writer.
    fn current_colors(&mut self) -> Option<(PsObject, PsObject)> {
        if !self.current_read {
            self.current_read = true;
            self.current = raw_ui(&self.cmdlet).and_then(|raw| {
                let fg = raw.get("ForegroundColor").ok()?;
                let bg = raw.get("BackgroundColor").ok()?;
                Some((fg, bg))
            });
        }
        self.current.clone()
    }

    pub fn write(&mut self, text: &str, fg: Option<&str>, bg: Option<&str>, no_newline: bool) -> PsResult<()> {
        let message = self.message_type.new(&[])?;
        message.set("Message", &text.into_ps()?)?;
        message.set("NoNewLine", &no_newline.into_ps()?)?;
        let current = self.current_colors();
        match fg {
            Some(color) => message.set("ForegroundColor", &color.into_ps()?)?,
            None => {
                if let Some((current_fg, _)) = &current {
                    message.set("ForegroundColor", current_fg)?;
                }
            }
        }
        match bg {
            Some(color) => message.set("BackgroundColor", &color.into_ps()?)?,
            None => {
                if let Some((_, current_bg)) = &current {
                    message.set("BackgroundColor", current_bg)?;
                }
            }
        }
        if self.tags.is_none() {
            self.tags = Some(vec!["PSHOST".to_string()].into_ps()?);
        }
        let tags = self.tags.clone().unwrap_or_default();
        self.cmdlet.call("WriteInformation", &[message, tags])?;
        if self.transcribes {
            self.transcribe(text);
        }
        Ok(())
    }
}
