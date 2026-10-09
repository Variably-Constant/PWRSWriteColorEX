//! Test-AnsiSupport: the terminal's ANSI color support and the styles it shows, read from the
//! environment and the host by the same rules as PSWriteColorEX. Write-ColorEX detects once per
//! session and keeps the result.

use std::sync::Mutex;

use pwrs::prelude::*;

use crate::host::{env_var, host_virtual_terminal, ps_version, this_cmdlet};

/// A color mode, from none to 24-bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Support {
    None,
    Ansi4,
    Ansi8,
    TrueColor,
}

impl Support {
    pub fn as_str(self) -> &'static str {
        match self {
            Support::None => "None",
            Support::Ansi4 => "ANSI4",
            Support::Ansi8 => "ANSI8",
            Support::TrueColor => "TrueColor",
        }
    }
}

/// The styles a terminal shows.
#[derive(Clone, Copy, Debug)]
pub struct Styles {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub faint: bool,
    pub blink: bool,
    pub crossed_out: bool,
    pub double_underline: bool,
    pub overline: bool,
    pub reverse: bool,
}

impl Styles {
    /// Marks a style shown, by its name.
    fn set(&mut self, name: &str) {
        match name {
            "DoubleUnderline" => self.double_underline = true,
            "Overline" => self.overline = true,
            "Blink" => self.blink = true,
            _ => {}
        }
    }
}

/// What Test-AnsiSupport answers.
pub struct Detection {
    pub color_support: Support,
    pub bold_fonts: bool,
    pub ps_version: String,
    pub is_console_host: bool,
    pub virtual_terminal: bool,
    pub compatible_env: bool,
    pub is_core: bool,
    pub os: &'static str,
    pub terminal: String,
    pub instructions: String,
    pub styles: Styles,
    pub warnings: Vec<String>,
}

const UNIX_INSTRUCTIONS: &str = "ANSI colors are enabled by default on Unix/Linux/macOS. To ensure 256 colors, add \"export TERM=xterm-256color\" to your shell config file (~/.bashrc, ~/.zshrc, etc.). For TrueColor, add \"export COLORTERM=truecolor\".";
const WINDOWS_INSTRUCTIONS: &str = "To enable ANSI colors permanently on Windows, run in Admin PowerShell: Set-ItemProperty HKCU:\\Console VirtualTerminalLevel -Type DWORD 1";

/// The Windows version, major and build, read from .NET.
fn windows_version() -> (i64, i64) {
    let read = || -> PsResult<(i64, i64)> {
        let version = PsType::from_name("System.Environment").call_static("get_OSVersion", &[])?.get("Version")?;
        Ok((i64::from_ps(&version.get("Major")?)?, i64::from_ps(&version.get("Build")?)?))
    };
    read().unwrap_or((10, 0))
}

/// Switches on virtual terminal processing for the console the process writes to, when it is
/// off, and answers whether it is on: Some(true) when this call switched it on.
#[cfg(windows)]
fn console_virtual_terminal() -> Option<(bool, bool)> {
    type Handle = *mut std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(std_handle: u32) -> Handle;
        fn GetConsoleMode(console: Handle, mode: *mut u32) -> i32;
        fn SetConsoleMode(console: Handle, mode: u32) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    // SAFETY: plain kernel32 calls on the process's own standard output handle.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0u32;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return None;
        }
        if mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0 {
            return Some((true, false));
        }
        if SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0 {
            return Some((true, true));
        }
        Some((false, false))
    }
}

#[cfg(not(windows))]
fn console_virtual_terminal() -> Option<(bool, bool)> {
    None
}

/// Whether the process's console is a window of the Windows console host itself, whose class is
/// ConsoleWindowClass. A terminal that runs PowerShell through a pseudoconsole, such as Windows
/// Terminal or VS Code, gives the console a hidden window of the class PseudoConsoleWindow, and a
/// process with no console window, such as a CI job, has none.
#[cfg(windows)]
fn console_host_window() -> bool {
    type Window = *mut std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleWindow() -> Window;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetClassNameW(window: Window, class_name: *mut u16, max_count: i32) -> i32;
    }
    const CLASS: &str = "ConsoleWindowClass";
    let mut name = [0u16; 64];
    // SAFETY: GetConsoleWindow takes nothing, and GetClassNameW writes at most max_count units,
    // the buffer's length, into it.
    let length = unsafe {
        let window = GetConsoleWindow();
        if window.is_null() {
            return false;
        }
        GetClassNameW(window, name.as_mut_ptr(), name.len() as i32)
    };
    usize::try_from(length).is_ok_and(|length| length <= name.len() && String::from_utf16_lossy(&name[..length]) == CLASS)
}

#[cfg(not(windows))]
fn console_host_window() -> bool {
    false
}

/// The warning for PowerShell 7 in a window of the Windows console host.
const CONSOLE_HOST_BOLD: &str = "The Windows console host (conhost) draws bold as brighter colors rather than a bold font, so Bold makes colors lighter. Windows Terminal draws a bold font.";

/// Detects the terminal's color support, its bold rendering and the styles it shows.
pub fn detect(ps: &Pipeline<'_>, silent: bool) -> PsResult<Detection> {
    let (major, minor) = ps_version(ps);
    let host_name = this_cmdlet(ps)
        .get("Host")
        .and_then(|h| h.get("Name"))
        .and_then(|n| String::from_ps(&n))
        .unwrap_or_default();
    let mut d = Detection {
        color_support: Support::None,
        bold_fonts: false,
        ps_version: ps
            .variable("PSVersionTable")
            .and_then(|table| table.get("PSVersion"))
            .and_then(|version| String::from_ps(&version.call("ToString", &[])?))
            .unwrap_or_else(|_| format!("{major}.{minor}")),
        is_console_host: host_name.eq_ignore_ascii_case("ConsoleHost"),
        virtual_terminal: false,
        compatible_env: false,
        is_core: major >= 6,
        os: if cfg!(windows) { "Win32NT" } else { "Unix" },
        terminal: String::new(),
        instructions: String::new(),
        styles: Styles {
            bold: true,
            italic: false,
            underline: true,
            faint: false,
            blink: false,
            crossed_out: false,
            double_underline: false,
            overline: false,
            reverse: true,
        },
        warnings: Vec::new(),
    };

    // FORCE_COLOR, NO_COLOR and TERM=dumb answer before anything else
    match env_var("FORCE_COLOR")?.as_deref() {
        Some("0") => return Ok(d),
        Some("1") => {
            d.color_support = Support::Ansi4;
            return Ok(d);
        }
        Some("2") => {
            d.color_support = Support::Ansi8;
            return Ok(d);
        }
        Some("3") => {
            d.color_support = Support::TrueColor;
            d.bold_fonts = true;
            return Ok(d);
        }
        _ => {}
    }
    if env_var("NO_COLOR")?.is_some_and(|v| !v.is_empty()) {
        return Ok(d);
    }

    // CLICOLOR_FORCE, set to anything but 0, keeps colors on whatever the terminal, and comes
    // before CLICOLOR=0 and TERM=dumb
    let cli_color_forced = env_var("CLICOLOR_FORCE")?.is_some_and(|v| !v.is_empty() && v != "0");
    let term = env_var("TERM")?.unwrap_or_default();
    if !cli_color_forced {
        if env_var("CLICOLOR")?.as_deref() == Some("0") {
            return Ok(d);
        }
        // TERM=dumb names a terminal that interprets no escape codes
        if term.eq_ignore_ascii_case("dumb") {
            d.terminal = "dumb".to_string();
            return Ok(d);
        }
    }

    let color_term = env_var("COLORTERM")?.unwrap_or_default();
    let con_emu = env_var("ConEmuANSI")?.unwrap_or_default();
    let wt_session = env_var("WT_SESSION")?.unwrap_or_default();
    let term_program = env_var("TERM_PROGRAM")?.unwrap_or_default();
    let vte_version = env_var("VTE_VERSION")?.unwrap_or_default();
    let terminal_emulator = env_var("TERMINAL_EMULATOR")?.unwrap_or_default();
    let term_lower = term.to_ascii_lowercase();
    // PowerShell's -eq and -like compare without regard to case
    let program_is = |name: &str| term_program.eq_ignore_ascii_case(name);
    let set = |name: &str| -> PsResult<bool> { Ok(env_var(name)?.is_some_and(|v| !v.is_empty())) };

    // Terminals that show TrueColor, draw bold as a bold font, and show italic, underline and
    // strikethrough, by the variables each sets, with the styles each shows beyond those
    let modern: Option<(&str, &[&str])> = if program_is("ghostty") || term_lower == "xterm-ghostty" {
        Some(("Ghostty", &["DoubleUnderline", "Overline"]))
    } else if program_is("WezTerm") {
        Some(("WezTerm", &["DoubleUnderline", "Overline", "Blink"]))
    } else if program_is("WarpTerminal") {
        Some(("Warp", &[]))
    } else if term_lower == "xterm-kitty" || set("KITTY_WINDOW_ID")? {
        Some(("Kitty", &["DoubleUnderline"]))
    } else if term_lower == "alacritty" || set("ALACRITTY_WINDOW_ID")? {
        Some(("Alacritty", &[]))
    } else if term_lower.starts_with("foot") {
        Some(("foot", &[]))
    } else if terminal_emulator.eq_ignore_ascii_case("JetBrains-JediTerm") {
        Some(("JetBrains IDE Terminal", &[]))
    } else {
        None
    };
    let modern_styles = |d: &mut Detection, terminal: &str, styles: &[&str]| {
        d.terminal = terminal.to_string();
        d.color_support = Support::TrueColor;
        d.bold_fonts = true;
        d.styles.italic = true;
        d.styles.underline = true;
        d.styles.crossed_out = true;
        for style in styles {
            d.styles.set(style);
        }
    };

    if color_term.eq_ignore_ascii_case("truecolor") || color_term.eq_ignore_ascii_case("24bit") {
        d.color_support = Support::TrueColor;
        d.compatible_env = true;
    } else if term_lower.contains("256") {
        d.color_support = Support::Ansi8;
        d.compatible_env = true;
    } else if !term.is_empty() {
        d.color_support = Support::Ansi4;
        d.compatible_env = true;
    }
    // At least TrueColor, for the terminals that have it whatever TERM says
    let raise_to_truecolor = |d: &mut Detection| {
        if d.color_support < Support::TrueColor {
            d.color_support = Support::TrueColor;
        }
    };

    if d.os == "Unix" {
        d.virtual_terminal = true;
        if program_is("tmux") || set("TMUX")? {
            // tmux shows the colors its TERM and COLORTERM name, 256 at least, and passes bold to
            // the terminal it runs in
            d.terminal = "tmux".to_string();
            if d.color_support < Support::Ansi8 {
                d.color_support = Support::Ansi8;
            }
            d.bold_fonts = true;
            d.styles.italic = true;
            d.styles.underline = true;
            d.styles.crossed_out = true;
        } else if program_is("Apple_Terminal") {
            d.terminal = "macOS Terminal.app".to_string();
            d.color_support = Support::Ansi8;
            d.bold_fonts = false;
            d.warnings.push("macOS Terminal.app does not support TrueColor (24-bit). Maximum 256 colors. For TrueColor support, use iTerm2.".to_string());
            d.styles.italic = true;
            d.styles.underline = true;
        } else if program_is("iTerm.app") {
            d.terminal = "iTerm2".to_string();
            raise_to_truecolor(&mut d);
            d.bold_fonts = true;
            d.styles.italic = true;
            d.styles.underline = true;
            d.styles.crossed_out = true;
        } else if program_is("vscode") {
            d.terminal = "VS Code Integrated Terminal".to_string();
            raise_to_truecolor(&mut d);
            d.bold_fonts = true;
            d.styles.italic = true;
            d.styles.underline = true;
        } else if let Some((terminal, styles)) = modern {
            modern_styles(&mut d, terminal, styles);
        } else if !wt_session.is_empty() {
            // Windows Terminal running a WSL shell, which passes WT_SESSION through
            d.terminal = "Windows Terminal (WSL)".to_string();
            d.color_support = Support::TrueColor;
            d.bold_fonts = true;
            d.styles.italic = true;
            d.styles.underline = true;
            d.styles.crossed_out = true;
        } else if !vte_version.is_empty() {
            let vte = i64::from(vte_version.trim().parse::<i32>().unwrap_or(0));
            if vte >= 5600 {
                d.terminal = "VTE-based Terminal (GNOME Terminal 3.32+)".to_string();
                d.bold_fonts = true;
                d.styles.italic = true;
                d.styles.underline = true;
                d.styles.blink = true;
                d.styles.overline = true;
                if vte >= 7600 {
                    d.styles.double_underline = true;
                }
            } else if vte >= 5200 {
                d.terminal = "VTE-based Terminal (GNOME Terminal 3.28+)".to_string();
                d.bold_fonts = false;
                d.styles.italic = true;
                d.styles.underline = true;
                d.styles.blink = true;
                d.styles.overline = true;
            } else {
                d.terminal = "VTE-based Terminal".to_string();
                d.bold_fonts = false;
                d.styles.italic = true;
                d.styles.underline = true;
                d.warnings.push("Older VTE version detected. Some styles (Blink, Overline) may not be supported. Consider updating to GNOME Terminal 3.28+ (VTE 0.52+).".to_string());
            }
            raise_to_truecolor(&mut d);
        } else if term_lower.contains("konsole") || env_var("KONSOLE_VERSION")?.is_some_and(|v| !v.is_empty()) {
            d.terminal = "Konsole".to_string();
            raise_to_truecolor(&mut d);
            d.bold_fonts = true;
            d.styles.italic = true;
            d.styles.underline = true;
            d.styles.overline = true;
        } else if term_lower == "xterm-256color" {
            d.terminal = "xterm-256color".to_string();
            if d.color_support == Support::None {
                d.color_support = Support::Ansi8;
            }
            d.bold_fonts = false;
            d.styles.italic = true;
            d.styles.underline = true;
        } else if term_lower.starts_with("xterm") {
            d.terminal = "xterm".to_string();
            if d.color_support == Support::None {
                d.color_support = Support::Ansi4;
            }
            d.bold_fonts = false;
            d.styles.underline = true;
        } else if term_lower.starts_with("rxvt") {
            d.terminal = "rxvt/urxvt".to_string();
            d.bold_fonts = true;
            if term_lower.contains("256") {
                d.color_support = Support::Ansi8;
                d.styles.italic = true;
            } else {
                d.color_support = Support::Ansi4;
                d.warnings.push("rxvt detected. Basic rxvt does not support 256 colors. Consider using rxvt-unicode (urxvt) or a modern terminal.".to_string());
            }
            d.styles.underline = true;
        } else if term_lower.contains("256") {
            d.terminal = "Unix Terminal (256 color)".to_string();
            if d.color_support == Support::None {
                d.color_support = Support::Ansi8;
            }
        } else if !term.is_empty() {
            d.terminal = format!("Unix Terminal ({term})");
            if d.color_support == Support::None {
                d.color_support = Support::Ansi4;
            }
        }
        d.instructions = UNIX_INSTRUCTIONS.to_string();
    } else {
        let (os_major, build) = windows_version();
        let win10_plus = os_major > 10 || (os_major == 10 && build >= 10586);
        let truecolor_capable = os_major > 10 || (os_major == 10 && build >= 14931);

        if !wt_session.is_empty() {
            d.terminal = "Windows Terminal".to_string();
            d.virtual_terminal = true;
            d.color_support = Support::TrueColor;
            d.styles.italic = true;
            d.styles.underline = true;
            d.styles.crossed_out = true;
            if d.is_core {
                // PowerShell 7 in Windows Terminal: bold is a bold font
                d.bold_fonts = true;
                d.styles.bold = true;
            } else {
                d.bold_fonts = false;
                d.warnings.push("PowerShell 5.1 in Windows Terminal: Bold style makes colors lighter instead of bolding the font. Use PowerShell 7+ for proper bold support.".to_string());
            }
        } else if !con_emu.is_empty() {
            d.terminal = "ConEmu/Cmder".to_string();
            d.virtual_terminal = true;
            d.color_support = if truecolor_capable { Support::TrueColor } else { Support::Ansi8 };
            d.bold_fonts = true;
            d.warnings.push("ConEmu/Cmder has LIMITED TrueColor support (only in the bottom buffer area, requires scrolling off). For full TrueColor support, use Windows Terminal.".to_string());
            d.styles.italic = true;
            d.styles.underline = true;
        } else if term_lower.contains("mintty") || program_is("mintty") {
            d.terminal = "mintty (Git Bash/Cygwin/MSYS2)".to_string();
            d.virtual_terminal = true;
            d.color_support = Support::TrueColor;
            d.bold_fonts = true;
            d.styles.italic = true;
            d.styles.underline = true;
        } else if program_is("vscode") {
            d.terminal = "VS Code Integrated Terminal".to_string();
            d.virtual_terminal = true;
            d.color_support = Support::TrueColor;
            d.bold_fonts = true;
            d.styles.italic = true;
            d.styles.underline = true;
        } else if let Some((terminal @ ("WezTerm" | "JetBrains IDE Terminal"), styles)) = modern {
            // WezTerm and the JetBrains IDEs' terminal on Windows
            modern_styles(&mut d, terminal, styles);
            d.virtual_terminal = true;
        } else if host_name.eq_ignore_ascii_case("Windows PowerShell ISE Host") {
            d.terminal = "PowerShell ISE".to_string();
            d.virtual_terminal = false;
            d.color_support = Support::None;
            d.bold_fonts = false;
            d.warnings.push("PowerShell ISE does NOT support ANSI escape sequences. Only native PowerShell colors available. Use Windows Terminal or PowerShell 7+ for ANSI support.".to_string());
        } else if d.is_core {
            d.virtual_terminal = true;
            d.color_support = if truecolor_capable {
                Support::TrueColor
            } else if win10_plus {
                Support::Ansi8
            } else {
                Support::Ansi4
            };
            d.terminal = "PowerShell Core Console".to_string();
            if console_host_window() {
                // The console host draws bold as brighter colors, which only the 16 colors have
                d.bold_fonts = false;
                d.warnings.push(CONSOLE_HOST_BOLD.to_string());
            } else {
                d.bold_fonts = true;
            }
            if truecolor_capable {
                d.styles.italic = true;
                d.styles.underline = true;
            }
        } else if !d.is_console_host {
            d.virtual_terminal = false;
            d.color_support = Support::None;
            d.terminal = host_name.clone();
            d.warnings.push(format!("Host '{host_name}' does not support ANSI escape sequences. Only native PowerShell colors available."));
        } else if win10_plus
            && let Some((on, switched_on)) = console_virtual_terminal() {
                d.virtual_terminal = on;
                if switched_on && !silent {
                    ps.warning("ANSI support has been enabled for this session only. To enable permanently, run in Admin PowerShell: Set-ItemProperty HKCU:\\Console VirtualTerminalLevel -Type DWORD 1")?;
                }
                if on {
                    d.terminal = "Windows PowerShell Console (conhost)".to_string();
                    d.color_support = if truecolor_capable {
                        Support::TrueColor
                    } else if win10_plus {
                        Support::Ansi8
                    } else {
                        Support::Ansi4
                    };
                    d.styles.underline = true;
                    d.bold_fonts = false;
                    d.warnings.push("PowerShell 5.1 conhost does NOT support true bold font rendering. Bold style will make colors lighter instead of actually bolding the text. Use PowerShell 7+ or Windows Terminal for proper bold support.".to_string());
                }
            }

        d.instructions = WINDOWS_INSTRUCTIONS.to_string();
        if !win10_plus && !silent {
            ps.warning("Windows versions before Windows 10 build 10586 do not support ANSI colors natively. Consider using ConEmu or Windows Terminal.")?;
        }
    }

    // Virtual terminal processing with no color level found means the 16 colors
    if d.color_support == Support::None && d.virtual_terminal {
        d.color_support = Support::Ansi4;
    }

    // PowerShell 7.2 and later remove escape codes from the output of a host without virtual
    // terminal support
    if d.color_support != Support::None && !host_virtual_terminal(ps) {
        d.color_support = Support::None;
        d.virtual_terminal = false;
        d.warnings.push(format!("Host '{host_name}' has no virtual terminal support, so PowerShell removes ANSI escape codes from its output. Only console colors are available."));
    }

    // CLICOLOR_FORCE keeps colors on: the 16 colors where nothing more was found
    if cli_color_forced && d.color_support == Support::None {
        d.color_support = Support::Ansi4;
    }

    if !silent {
        if d.color_support == Support::None {
            if !d.is_console_host {
                ps.warning("ANSI not supported: Not running in a compatible console host (e.g., ISE does not support ANSI)")?;
            } else if d.os == "Win32NT" {
                ps.warning(&format!("ANSI not supported: Virtual terminal processing is not available. {}", d.instructions))?;
            }
        }
        for warning in &d.warnings {
            ps.warning(warning)?;
        }
    }
    Ok(d)
}

/// The color support and bold rendering Write-ColorEX uses, detected on first use and kept for
/// the session.
static SESSION_SUPPORT: Mutex<Option<(Support, bool)>> = Mutex::new(None);

pub fn session_support(ps: &Pipeline<'_>) -> PsResult<(Support, bool)> {
    if let Some(found) = *SESSION_SUPPORT.lock().unwrap_or_else(|e| e.into_inner()) {
        return Ok(found);
    }
    let d = detect(ps, true)?;
    let found = (d.color_support, d.bold_fonts);
    *SESSION_SUPPORT.lock().unwrap_or_else(|e| e.into_inner()) = Some(found);
    Ok(found)
}

/// Forgets the session's detection, so the next Write-ColorEX call detects again; the module
/// does this on import.
pub fn reset_session_support() {
    *SESSION_SUPPORT.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn hashtable_of(pairs: &[(&str, PsObject)]) -> PsResult<PsHashtable> {
    let table = crate::host::new_table()?;
    for (key, value) in pairs {
        table.set(key, value.clone())?;
    }
    Ok(table)
}

/// Detects the terminal's ANSI color support and the styles it shows.
///
/// Reads the environment and the host to answer which ANSI color mode the terminal supports
/// (TrueColor, 256-color, 16-color or none), whether it draws bold as a bold font, and which text
/// styles it shows. Write-ColorEX falls back to the modes it reports. FORCE_COLOR, NO_COLOR and
/// TERM=dumb answer first; then COLORTERM, TERM and the terminal's own variables. On PowerShell 7.2
/// and later, a host without virtual terminal support means none, since PowerShell removes escape
/// codes from its output. In Windows PowerShell 5.1 in the Windows console host it switches on
/// virtual terminal processing for the session when it is off.
///
/// # Examples
/// Test-AnsiSupport
/// (Test-AnsiSupport -Silent).ColorSupport
#[cmdlet(verb = "Test", noun = "AnsiSupport", alias = ["TAS", "Test-ANSI"], output = ["System.Management.Automation.PSCustomObject"])]
#[derive(Default)]
pub struct TestAnsiSupport {
    /// Suppresses all warning messages and terminal limitation notices.
    #[param]
    pub silent: bool,
}

impl Cmdlet for TestAnsiSupport {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let d = detect(ps, self.silent)?;
        let styles = hashtable_of(&[
            ("Bold", d.styles.bold.into_ps()?),
            ("Italic", d.styles.italic.into_ps()?),
            ("Underline", d.styles.underline.into_ps()?),
            ("Faint", d.styles.faint.into_ps()?),
            ("Blink", d.styles.blink.into_ps()?),
            ("CrossedOut", d.styles.crossed_out.into_ps()?),
            ("DoubleUnderline", d.styles.double_underline.into_ps()?),
            ("Overline", d.styles.overline.into_ps()?),
            ("Reverse", d.styles.reverse.into_ps()?),
        ])?;
        let details = hashtable_of(&[
            ("PowerShellVersion", d.ps_version.into_ps()?),
            ("IsConsoleHost", d.is_console_host.into_ps()?),
            ("HasVirtualTerminalProcessing", d.virtual_terminal.into_ps()?),
            ("HasCompatibleTerminalEnv", d.compatible_env.into_ps()?),
            ("IsPSCore", d.is_core.into_ps()?),
            ("OperatingSystem", PsType::from_name("System.Environment").call_static("get_OSVersion", &[])?.get("Platform")?),
            ("TerminalType", d.terminal.into_ps()?),
            ("EnableInstructions", d.instructions.into_ps()?),
            ("StyleSupport", styles.0),
            ("Warnings", PsArray(d.warnings.into_iter().map(|w| w.into_ps()).collect::<PsResult<Vec<PsObject>>>()?).into_ps()?),
        ])?;
        let result = pwrs::object::new_psobject("System.Management.Automation.PSCustomObject");
        pwrs::object::add_note(&result, "ColorSupport", d.color_support.as_str().into_ps()?)?;
        pwrs::object::add_note(&result, "SupportsBoldFonts", d.bold_fonts.into_ps()?)?;
        pwrs::object::add_note(&result, "Details", details.0)?;
        ps.write_object(&result)
    }
}
