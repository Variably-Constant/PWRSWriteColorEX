//! Write-ColorError, Write-ColorWarning, Write-ColorInfo, Write-ColorSuccess, Write-ColorCritical
//! and Write-ColorDebug: Write-ColorEX with a built-in profile.

use pwrs::prelude::*;

use crate::log::resolve_log_folder;
use crate::style::{helper_profile, write_color_params};
use crate::write::{caller_script_root, render, WriteOptions};

/// What every helper takes.
struct HelperCall<'a> {
    text: Option<&'a [String]>,
    no_new_line: bool,
    log_file: &'a str,
    no_console_output: bool,
    pass_thru: bool,
}

/// Writes the text with a profile's settings, reading the profile from [PSColorStyle]::Profiles
/// on each call so a change to it applies to the next.
fn write_with_profile(ps: &Pipeline<'_>, profile: &str, call: HelperCall<'_>) -> PsResult<()> {
    let params = write_color_params(&helper_profile(profile)?)?;
    let mut options = WriteOptions::default();
    options.apply_profile(&params, false)?;
    options.text = call.text.map(<[String]>::to_vec).unwrap_or_default();
    if call.no_new_line {
        options.no_new_line = true;
    }
    if !call.log_file.is_empty() {
        // A bare -LogFile name goes in the folder of the script that called the helper
        options.log_file = call.log_file.to_string();
        options.log_path = resolve_log_folder(ps, &caller_script_root(ps))?;
    }
    if call.no_console_output {
        options.no_console_output = true;
    }
    render(ps, options, &[], None)?;

    if call.pass_thru {
        match call.text {
            Some(text) => {
                for line in text {
                    ps.write(line.as_str())?;
                }
            }
            None => ps.write_object(&PsObject::null())?,
        }
    }
    Ok(())
}

/// Writes an error message with the Error profile (red, bold).
///
/// Calls Write-ColorEX with the built-in Error profile, which starts as red, bold text. A change to
/// [PSColorStyle]::Profiles['Error'] applies to the next call.
///
/// # Examples
/// Write-ColorError 'Operation failed'
/// 'first', 'second' | Write-ColorError -PassThru
#[cmdlet(verb = "Write", noun = "ColorError", alias = ["WCE", "Write-ErrorColor", "Write-ErrorColour", "Write-ColourError", "WError", "wcerror"])]
#[derive(Default)]
pub struct WriteColorError {
    /// The message. Several strings are written on one line; strings piped in are written one
    /// line each.
    #[param(position = 0, value_from_pipeline)]
    pub text: Option<Vec<String>>,
    /// Leaves the line open, so the next output continues it.
    #[param]
    pub no_new_line: bool,
    /// Writes the message to this log file as well. A file name alone goes in the folder of the
    /// calling script, or the current location when called from the prompt.
    #[param]
    pub log_file: Option<String>,
    /// Writes nothing to the host, only to the log file.
    #[param]
    pub no_console_output: bool,
    /// Writes the text to the pipeline after writing it to the host.
    #[param]
    pub pass_thru: bool,
}

impl Cmdlet for WriteColorError {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        write_with_profile(ps, "Error", self.call())
    }
}

impl WriteColorError {
    fn call(&self) -> HelperCall<'_> {
        HelperCall {
            text: self.text.as_deref(),
            no_new_line: self.no_new_line,
            log_file: self.log_file.as_deref().unwrap_or(""),
            no_console_output: self.no_console_output,
            pass_thru: self.pass_thru,
        }
    }
}

/// Writes a warning message with the Warning profile (yellow).
///
/// Calls Write-ColorEX with the built-in Warning profile, which starts as yellow text. A change to
/// [PSColorStyle]::Profiles['Warning'] applies to the next call.
///
/// # Examples
/// Write-ColorWarning 'This action may cause data loss'
/// 'first', 'second' | Write-ColorWarning -PassThru
#[cmdlet(verb = "Write", noun = "ColorWarning", alias = ["WCW", "Write-WarningColor", "Write-WarningColour", "Write-ColourWarning", "WWarning", "WCWarn", "wcwarning"])]
#[derive(Default)]
pub struct WriteColorWarning {
    /// The message. Several strings are written on one line; strings piped in are written one
    /// line each.
    #[param(position = 0, value_from_pipeline)]
    pub text: Option<Vec<String>>,
    /// Leaves the line open, so the next output continues it.
    #[param]
    pub no_new_line: bool,
    /// Writes the message to this log file as well. A file name alone goes in the folder of the
    /// calling script, or the current location when called from the prompt.
    #[param]
    pub log_file: Option<String>,
    /// Writes nothing to the host, only to the log file.
    #[param]
    pub no_console_output: bool,
    /// Writes the text to the pipeline after writing it to the host.
    #[param]
    pub pass_thru: bool,
}

impl Cmdlet for WriteColorWarning {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        write_with_profile(ps, "Warning", self.call())
    }
}

impl WriteColorWarning {
    fn call(&self) -> HelperCall<'_> {
        HelperCall {
            text: self.text.as_deref(),
            no_new_line: self.no_new_line,
            log_file: self.log_file.as_deref().unwrap_or(""),
            no_console_output: self.no_console_output,
            pass_thru: self.pass_thru,
        }
    }
}

/// Writes an information message with the Info profile (cyan).
///
/// Calls Write-ColorEX with the built-in Info profile, which starts as cyan text. A change to
/// [PSColorStyle]::Profiles['Info'] applies to the next call.
///
/// # Examples
/// Write-ColorInfo 'Processing 150 files'
/// 'first', 'second' | Write-ColorInfo -PassThru
#[cmdlet(verb = "Write", noun = "ColorInfo", alias = ["WCI", "Write-InfoColor", "Write-InfoColour", "Write-ColourInfo", "WInfo", "wcinfo"])]
#[derive(Default)]
pub struct WriteColorInfo {
    /// The message. Several strings are written on one line; strings piped in are written one
    /// line each.
    #[param(position = 0, value_from_pipeline)]
    pub text: Option<Vec<String>>,
    /// Leaves the line open, so the next output continues it.
    #[param]
    pub no_new_line: bool,
    /// Writes the message to this log file as well. A file name alone goes in the folder of the
    /// calling script, or the current location when called from the prompt.
    #[param]
    pub log_file: Option<String>,
    /// Writes nothing to the host, only to the log file.
    #[param]
    pub no_console_output: bool,
    /// Writes the text to the pipeline after writing it to the host.
    #[param]
    pub pass_thru: bool,
}

impl Cmdlet for WriteColorInfo {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        write_with_profile(ps, "Info", self.call())
    }
}

impl WriteColorInfo {
    fn call(&self) -> HelperCall<'_> {
        HelperCall {
            text: self.text.as_deref(),
            no_new_line: self.no_new_line,
            log_file: self.log_file.as_deref().unwrap_or(""),
            no_console_output: self.no_console_output,
            pass_thru: self.pass_thru,
        }
    }
}

/// Writes a success message with the Success profile (green).
///
/// Calls Write-ColorEX with the built-in Success profile, which starts as green text. A change to
/// [PSColorStyle]::Profiles['Success'] applies to the next call.
///
/// # Examples
/// Write-ColorSuccess 'Deployment completed'
/// 'first', 'second' | Write-ColorSuccess -PassThru
#[cmdlet(verb = "Write", noun = "ColorSuccess", alias = ["WCS", "Write-SuccessColor", "Write-SuccessColour", "Write-ColourSuccess", "WSuccess", "wcok", "wcsuccess"])]
#[derive(Default)]
pub struct WriteColorSuccess {
    /// The message. Several strings are written on one line; strings piped in are written one
    /// line each.
    #[param(position = 0, value_from_pipeline)]
    pub text: Option<Vec<String>>,
    /// Leaves the line open, so the next output continues it.
    #[param]
    pub no_new_line: bool,
    /// Writes the message to this log file as well. A file name alone goes in the folder of the
    /// calling script, or the current location when called from the prompt.
    #[param]
    pub log_file: Option<String>,
    /// Writes nothing to the host, only to the log file.
    #[param]
    pub no_console_output: bool,
    /// Writes the text to the pipeline after writing it to the host.
    #[param]
    pub pass_thru: bool,
}

impl Cmdlet for WriteColorSuccess {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        write_with_profile(ps, "Success", self.call())
    }
}

impl WriteColorSuccess {
    fn call(&self) -> HelperCall<'_> {
        HelperCall {
            text: self.text.as_deref(),
            no_new_line: self.no_new_line,
            log_file: self.log_file.as_deref().unwrap_or(""),
            no_console_output: self.no_console_output,
            pass_thru: self.pass_thru,
        }
    }
}

/// Writes a critical message with the Critical profile (white on dark red, bold, blinking).
///
/// Calls Write-ColorEX with the built-in Critical profile, which starts as white, bold, blinking text on dark red. A change to
/// [PSColorStyle]::Profiles['Critical'] applies to the next call.
///
/// # Examples
/// Write-ColorCritical 'Database connection lost'
/// 'first', 'second' | Write-ColorCritical -PassThru
#[cmdlet(verb = "Write", noun = "ColorCritical", alias = ["WCC", "Write-CriticalColor", "Write-CriticalColour", "Write-ColourCritical", "WCritical", "wccritical"])]
#[derive(Default)]
pub struct WriteColorCritical {
    /// The message. Several strings are written on one line; strings piped in are written one
    /// line each.
    #[param(position = 0, value_from_pipeline)]
    pub text: Option<Vec<String>>,
    /// Leaves the line open, so the next output continues it.
    #[param]
    pub no_new_line: bool,
    /// Writes the message to this log file as well. A file name alone goes in the folder of the
    /// calling script, or the current location when called from the prompt.
    #[param]
    pub log_file: Option<String>,
    /// Writes nothing to the host, only to the log file.
    #[param]
    pub no_console_output: bool,
    /// Writes the text to the pipeline after writing it to the host.
    #[param]
    pub pass_thru: bool,
}

impl Cmdlet for WriteColorCritical {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        write_with_profile(ps, "Critical", self.call())
    }
}

impl WriteColorCritical {
    fn call(&self) -> HelperCall<'_> {
        HelperCall {
            text: self.text.as_deref(),
            no_new_line: self.no_new_line,
            log_file: self.log_file.as_deref().unwrap_or(""),
            no_console_output: self.no_console_output,
            pass_thru: self.pass_thru,
        }
    }
}

/// Writes a debug message with the Debug profile (dark gray, italic).
///
/// Calls Write-ColorEX with the built-in Debug profile, which starts as dark gray, italic text. A change to
/// [PSColorStyle]::Profiles['Debug'] applies to the next call.
///
/// # Examples
/// Write-ColorDebug 'Variable value: 42'
/// 'first', 'second' | Write-ColorDebug -PassThru
#[cmdlet(verb = "Write", noun = "ColorDebug", alias = ["WCD", "Write-DebugColor", "Write-DebugColour", "Write-ColourDebug", "WDebug", "wcdebug"])]
#[derive(Default)]
pub struct WriteColorDebug {
    /// The message. Several strings are written on one line; strings piped in are written one
    /// line each.
    #[param(position = 0, value_from_pipeline)]
    pub text: Option<Vec<String>>,
    /// Leaves the line open, so the next output continues it.
    #[param]
    pub no_new_line: bool,
    /// Writes the message to this log file as well. A file name alone goes in the folder of the
    /// calling script, or the current location when called from the prompt.
    #[param]
    pub log_file: Option<String>,
    /// Writes nothing to the host, only to the log file.
    #[param]
    pub no_console_output: bool,
    /// Writes the text to the pipeline after writing it to the host.
    #[param]
    pub pass_thru: bool,
}

impl Cmdlet for WriteColorDebug {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        write_with_profile(ps, "Debug", self.call())
    }
}

impl WriteColorDebug {
    fn call(&self) -> HelperCall<'_> {
        HelperCall {
            text: self.text.as_deref(),
            no_new_line: self.no_new_line,
            log_file: self.log_file.as_deref().unwrap_or(""),
            no_console_output: self.no_console_output,
            pass_thru: self.pass_thru,
        }
    }
}
