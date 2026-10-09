//! Write-ColorEX: the rendering every Write-Color* command and Format-ColorEX share, step for
//! step the same as PSWriteColorEX's, and the cmdlet itself.

use std::collections::HashSet;
use std::fmt::Write as _;

use pwrs::prelude::*;

use crate::colors::{self, lookup};
use crate::detect::{session_support, Support};
use crate::forms::{is_white_space, read_forms, UnknownNames};
use crate::gradient::gradient_colors;
use crate::host::{env_var, host_ansi, host_width, removes_escape_codes, this_cmdlet, HostWriter};
use crate::log::write_log;
use crate::runs::{
    highlight_segments, items_text, pad_lines, parse_markup, parse_spec, plain_segment, segments_text, single_line, split_segments,
    truncate_segments, wrap_segments, Item, PadSide, Run, Segment, Spec, SplitBy,
};
use crate::style;
use crate::width::{display_width, split_display_characters};

const ESC: char = '\u{1b}';

/// A color as a script gives it: a name or hex code, a number, an array (an RGB value when it
/// holds three numbers), or $null. A fractional number is kept as it is for an RGB array, whose
/// channels PowerShell rounds.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ColorArg {
    #[default]
    Null,
    Name(String),
    Number(i64),
    Float(f64),
    List(Vec<ColorArg>),
}

impl ColorArg {
    /// Reads a color value; None for a type no color form takes, such as a hashtable.
    pub fn from_object(obj: &PsObject) -> PsResult<Option<ColorArg>> {
        use pwrs::sys::*;
        if obj.is_null() {
            return Ok(Some(ColorArg::Null));
        }
        let tag = obj.type_tag()?;
        Ok(match tag {
            PS_TYPE_STRING | PS_TYPE_CHAR => Some(ColorArg::Name(String::from_ps(obj)?)),
            PS_TYPE_I8 | PS_TYPE_I16 | PS_TYPE_I32 | PS_TYPE_I64 | PS_TYPE_U8 | PS_TYPE_U16 | PS_TYPE_U32 => {
                Some(ColorArg::Number(i64::from_ps(obj)?))
            }
            PS_TYPE_F32 | PS_TYPE_F64 | PS_TYPE_DECIMAL => Some(ColorArg::Float(f64::from_ps(obj)?)),
            PS_TYPE_OBJECT if is_list(obj)? => {
                let mut items = Vec::new();
                for item in Vec::<PsObject>::from_ps(obj)? {
                    match ColorArg::from_object(&item)? {
                        Some(value) => items.push(value),
                        None => return Ok(None),
                    }
                }
                Some(ColorArg::List(items))
            }
            _ => None,
        })
    }

    pub fn is_null(&self) -> bool {
        matches!(self, ColorArg::Null)
    }

    /// Whether PowerShell reads the value as true: an array of one element as that element, and
    /// a longer array as true.
    fn is_true(&self) -> bool {
        match self {
            ColorArg::Null => false,
            ColorArg::Name(n) => !n.is_empty(),
            ColorArg::Number(n) => *n != 0,
            ColorArg::Float(f) => *f != 0.0,
            ColorArg::List(items) => list_is_true(items, ColorArg::is_true),
        }
    }

    /// The type a debug message names for the value, as .GetType().Name names it.
    fn type_name(&self) -> &'static str {
        match self {
            ColorArg::Null => "",
            ColorArg::Name(_) => "String",
            ColorArg::Number(_) => "Int32",
            ColorArg::Float(_) => "Double",
            ColorArg::List(_) => "Object[]",
        }
    }

    pub fn is_hex(&self) -> bool {
        matches!(self, ColorArg::Name(n) if colors::is_hex_text(n))
    }

    /// An RGB channel as PowerShell's [int] cast reads it, and whether that equals the value:
    /// a fraction rounds half to even and a string of digits is its number.
    fn channel(&self) -> Option<(i64, bool)> {
        match self {
            ColorArg::Number(n) => Some((*n, true)),
            ColorArg::Float(f) if f.is_finite() => {
                let rounded = colors::round_even(*f);
                Some((rounded, rounded as f64 == *f))
            }
            ColorArg::Name(text) => {
                let text = text.trim();
                if let Ok(n) = text.parse::<i64>() {
                    return Some((n, true));
                }
                let f = text.parse::<f64>().ok().filter(|f| f.is_finite())?;
                let rounded = colors::round_even(f);
                Some((rounded, rounded as f64 == f))
            }
            _ => None,
        }
    }

    /// The RGB value of an array of three numbers.
    pub fn rgb(&self) -> Option<[i64; 3]> {
        self.rgb_exact().map(|(rgb, _)| rgb)
    }

    /// The RGB value of an array of three numbers, and whether each channel is a whole number.
    fn rgb_exact(&self) -> Option<([i64; 3], bool)> {
        match self {
            ColorArg::List(items) if items.len() == 3 => {
                let mut rgb = [0i64; 3];
                let mut exact = true;
                for (slot, item) in rgb.iter_mut().zip(items) {
                    let (value, whole) = item.channel()?;
                    *slot = value;
                    exact &= whole;
                }
                Some((rgb, exact))
            }
            _ => None,
        }
    }

    fn from_rgb(rgb: [i64; 3]) -> ColorArg {
        ColorArg::List(rgb.iter().map(|&v| ColorArg::Number(v)).collect())
    }

    /// The text a warning shows for the value, as PowerShell would print it.
    pub fn display(&self) -> String {
        match self {
            ColorArg::Null => String::new(),
            ColorArg::Name(n) => n.clone(),
            ColorArg::Number(n) => n.to_string(),
            ColorArg::Float(f) => f.to_string(),
            ColorArg::List(items) => items.iter().map(ColorArg::display).collect::<Vec<_>>().join(" "),
        }
    }
}

/// Whether PowerShell reads an array as true: empty is false, one element is that element, and
/// more is true.
fn list_is_true<T>(items: &[T], is_true: impl Fn(&T) -> bool) -> bool {
    match items {
        [] => false,
        [only] => is_true(only),
        _ => true,
    }
}

/// PowerShell's text for a Boolean.
fn ps_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// Whether an object is an array or a list, which a color or style value may hold.
fn is_list(obj: &PsObject) -> PsResult<bool> {
    let name = obj.type_name()?;
    Ok(name.ends_with("[]") || name.starts_with("System.Collections.ArrayList") || name.starts_with("System.Collections.Generic.List"))
}

/// Whether a value passes the check PSWriteColorEX's -Color and -BackGroundColor make on each
/// element: a string, an Int32, or an array of strings, Int32s and arrays.
fn is_color_element(obj: &PsObject, nested: bool) -> PsResult<bool> {
    use pwrs::sys::*;
    if obj.is_null() {
        return Ok(false);
    }
    Ok(match obj.type_tag()? {
        PS_TYPE_STRING | PS_TYPE_I32 => true,
        PS_TYPE_OBJECT if is_list(obj)? => {
            if nested {
                return Ok(true);
            }
            for item in Vec::<PsObject>::from_ps(obj)? {
                if !is_color_element(&item, true)? {
                    return Ok(false);
                }
            }
            true
        }
        _ => false,
    })
}

/// Reads -Color or -BackGroundColor: each entry a color, or $null, which leaves its segment as it
/// is. An entry of any other kind is refused, as PSWriteColorEX refuses it.
pub fn color_list(values: &[PsObject], parameter: &str) -> PsResult<Vec<ColorArg>> {
    let refuse = |message: String| PsError::new(ErrorCategory::InvalidData, "ParameterArgumentValidationError", message).terminating();
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        if value.is_null() {
            out.push(ColorArg::Null);
            continue;
        }
        if !is_color_element(value, false)? {
            // An array shows as its type, as PowerShell shows it in a validation error
            let shown = if is_list(value)? { value.type_name()? } else { String::from_ps(value).unwrap_or_default() };
            return Err(refuse(format!(
                "Cannot validate argument on parameter '{parameter}'. The argument \"{shown}\" is not a color: a string, an integer, or an array of strings, integers and arrays."
            )));
        }
        match ColorArg::from_object(value)? {
            Some(arg) => out.push(arg),
            None => out.push(ColorArg::Null),
        }
    }
    Ok(out)
}

/// Reads -Gradient, which takes any value: one no color form takes is gray, as an unknown name is.
pub fn gradient_list(values: &[PsObject]) -> PsResult<Vec<ColorArg>> {
    values
        .iter()
        .map(|value| Ok(ColorArg::from_object(value)?.unwrap_or(ColorArg::Null)))
        .collect()
}

/// -Style: a style name, or an array of names and arrays of names.
#[derive(Clone, Debug)]
pub enum StyleArg {
    Name(String),
    List(Vec<StyleArg>),
    Other,
}

impl StyleArg {
    pub fn from_object(obj: &PsObject) -> PsResult<Option<StyleArg>> {
        use pwrs::sys::*;
        if obj.is_null() {
            return Ok(None);
        }
        Ok(Some(match obj.type_tag()? {
            PS_TYPE_STRING | PS_TYPE_CHAR => StyleArg::Name(String::from_ps(obj)?),
            PS_TYPE_OBJECT if is_list(obj)? => {
                let mut items = Vec::new();
                for item in Vec::<PsObject>::from_ps(obj)? {
                    items.push(StyleArg::from_object(&item)?.unwrap_or(StyleArg::Other));
                }
                StyleArg::List(items)
            }
            _ => StyleArg::Other,
        }))
    }

    /// Whether PowerShell reads the value as true.
    fn is_true(&self) -> bool {
        match self {
            StyleArg::Name(n) => !n.is_empty(),
            StyleArg::List(items) => list_is_true(items, StyleArg::is_true),
            StyleArg::Other => true,
        }
    }
}

/// The escape code of a style name, compared without regard to case; none for another name.
pub fn style_code(name: &str) -> &'static str {
    const STYLES: [(&str, &str); 14] = [
        ("Reset", "\u{1b}[0m"),
        ("Bold", "\u{1b}[1m"),
        ("Faint", "\u{1b}[2m"),
        ("Italic", "\u{1b}[3m"),
        ("Underline", "\u{1b}[4m"),
        ("Blink", "\u{1b}[5m"),
        ("CrossedOut", "\u{1b}[9m"),
        ("DoubleUnderline", "\u{1b}[21m"),
        ("Overline", "\u{1b}[53m"),
        ("Reverse", "\u{1b}[7m"),
        ("Curly", "\u{1b}[4:3m"),
        ("Dotted", "\u{1b}[4:4m"),
        ("Dashed", "\u{1b}[4:5m"),
        ("None", ""),
    ];
    STYLES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, code)| *code)
        .unwrap_or("")
}

/// The escape code of an -UnderlineStyle, compared without regard to case.
fn underline_style_code(name: &str) -> &'static str {
    const STYLES: [(&str, &str); 5] = [
        ("Single", "\u{1b}[4m"),
        ("Double", "\u{1b}[21m"),
        ("Curly", "\u{1b}[4:3m"),
        ("Dotted", "\u{1b}[4:4m"),
        ("Dashed", "\u{1b}[4:5m"),
    ];
    STYLES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, code)| *code)
        .unwrap_or("")
}

/// Every Write-ColorEX parameter, as the cmdlet, Format-ColorEX or a helper gives them.
#[derive(Clone, Debug)]
pub struct WriteOptions {
    pub text: Vec<String>,
    pub color: Vec<ColorArg>,
    pub background: Vec<ColorArg>,
    pub gradient: Option<Vec<ColorArg>>,
    pub background_gradient: Option<Vec<ColorArg>>,
    pub gradient_space: String,
    pub ansi4: bool,
    pub ansi8: bool,
    pub ansi24: bool,
    pub style: Option<StyleArg>,
    pub style_profile: Option<PsObject>,
    pub default: bool,
    pub bold: bool,
    pub faint: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub crossed_out: bool,
    pub double_underline: bool,
    pub overline: bool,
    pub reverse: bool,
    pub underline_color: Vec<ColorArg>,
    pub underline_style: String,
    pub markup: bool,
    /// -Split and -SplitAround as given, None when left out.
    pub split: Option<Vec<String>>,
    pub split_around: Option<Vec<String>>,
    pub split_evenly: bool,
    /// -Link, one address or $null per segment.
    pub link: Vec<Option<String>>,
    pub start_tab: i32,
    pub lines_before: i32,
    pub lines_after: i32,
    pub start_spaces: i32,
    pub log_file: String,
    pub log_path: String,
    pub log_level: String,
    pub log_time: bool,
    pub date_time_format: String,
    pub log_retry: i32,
    pub encoding: String,
    pub show_time: bool,
    pub no_new_line: bool,
    pub horizontal_center: bool,
    pub blank_line: bool,
    pub no_console_output: bool,
    pub debugging: bool,
    pub silent: bool,
    pub auto_pad: i32,
    pub pad_left: bool,
    pub pad_center: bool,
    pub pad_char: char,
    pub truncate: bool,
    pub wrap: bool,
    /// The parameters given, by lowercase name; a style profile sets the others.
    pub bound: HashSet<String>,
    /// The folder of the script that called the command, for a bare -LogFile name.
    pub caller_script_root: String,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            text: Vec::new(),
            color: Vec::new(),
            background: Vec::new(),
            gradient: None,
            background_gradient: None,
            gradient_space: "OKLab".to_string(),
            ansi4: false,
            ansi8: false,
            ansi24: false,
            style: None,
            style_profile: None,
            default: false,
            bold: false,
            faint: false,
            italic: false,
            underline: false,
            blink: false,
            crossed_out: false,
            double_underline: false,
            overline: false,
            reverse: false,
            underline_color: Vec::new(),
            underline_style: String::new(),
            markup: false,
            split: None,
            split_around: None,
            split_evenly: false,
            link: Vec::new(),
            start_tab: 0,
            lines_before: 0,
            lines_after: 0,
            start_spaces: 0,
            log_file: String::new(),
            log_path: String::new(),
            log_level: String::new(),
            log_time: false,
            date_time_format: "yyyy-MM-dd HH:mm:ss".to_string(),
            log_retry: 2,
            encoding: "utf8".to_string(),
            show_time: false,
            no_new_line: false,
            horizontal_center: false,
            blank_line: false,
            no_console_output: false,
            debugging: false,
            silent: false,
            auto_pad: 0,
            pad_left: false,
            pad_center: false,
            pad_char: ' ',
            truncate: false,
            wrap: false,
            bound: HashSet::new(),
            caller_script_root: String::new(),
        }
    }
}

impl WriteOptions {
    /// Sets the parameters a style profile's hashtable names, other than those given.
    pub fn apply_profile(&mut self, params: &PsHashtable, only_unbound: bool) -> PsResult<()> {
        for key in params.keys()? {
            let lower = key.to_ascii_lowercase();
            if only_unbound && self.bound.contains(&lower) {
                continue;
            }
            let value = params.get(&key)?;
            self.set_parameter(&lower, &value)?;
        }
        Ok(())
    }

    /// Sets one parameter by its lowercase name from a value, as splatting would bind it.
    pub fn set_parameter(&mut self, lower: &str, value: &PsObject) -> PsResult<()> {
        let flag = |v: &PsObject| bool::from_ps(v).unwrap_or(false);
        let number = |v: &PsObject| i64::from_ps(v).map(|n| n as i32).unwrap_or(0);
        let text = |v: &PsObject| if v.is_null() { Ok(String::new()) } else { String::from_ps(v) };
        let list = |v: &PsObject| -> PsResult<Vec<ColorArg>> {
            Ok(match ColorArg::from_object(v)? {
                Some(ColorArg::List(items)) => items,
                Some(ColorArg::Null) | None => Vec::new(),
                Some(single) => vec![single],
            })
        };
        match lower {
            "color" => self.color = list(value)?,
            "backgroundcolor" => self.background = list(value)?,
            "gradient" => self.gradient = Some(list(value)?),
            "backgroundgradient" => self.background_gradient = Some(list(value)?),
            "gradientspace" => self.gradient_space = text(value)?,
            "underlinecolor" => self.underline_color = list(value)?,
            "underlinestyle" => self.underline_style = text(value)?,
            "style" => self.style = StyleArg::from_object(value)?,
            "starttab" => self.start_tab = number(value),
            "startspaces" => self.start_spaces = number(value),
            "linesbefore" => self.lines_before = number(value),
            "linesafter" => self.lines_after = number(value),
            "autopad" => self.auto_pad = number(value),
            "padchar" => {
                if let Some(c) = String::from_ps(value)?.chars().next() {
                    self.pad_char = c;
                }
            }
            "bold" => self.bold = flag(value),
            "italic" => self.italic = flag(value),
            "underline" => self.underline = flag(value),
            "blink" => self.blink = flag(value),
            "faint" => self.faint = flag(value),
            "crossedout" => self.crossed_out = flag(value),
            "doubleunderline" => self.double_underline = flag(value),
            "overline" => self.overline = flag(value),
            "reverse" => self.reverse = flag(value),
            "showtime" => self.show_time = flag(value),
            "nonewline" => self.no_new_line = flag(value),
            "horizontalcenter" => self.horizontal_center = flag(value),
            "padleft" => self.pad_left = flag(value),
            "padcenter" => self.pad_center = flag(value),
            "truncate" => self.truncate = flag(value),
            "wrap" => self.wrap = flag(value),
            _ => {}
        }
        Ok(())
    }
}

/// The color modes of one Write-ColorEX call: the one in use, the one asked for before any
/// fallback, and what the color checks read.
#[derive(Clone, Copy)]
struct Modes {
    ansi4: bool,
    ansi8: bool,
    ansi24: bool,
    original_truecolor: bool,
    original_ansi8: bool,
    original_ansi4: bool,
    implied_truecolor: bool,
    ansi_support: bool,
    /// -Bold where the terminal shows bold as brighter colors, which lightens each color.
    lighten: bool,
}

/// The ANSI escape code for a converted color in the color mode in use, or nothing. Without an
/// ANSI mode the color is the console color as a 16-color code.
fn color_sequence(value: &ColorArg, background: bool, m: &Modes) -> String {
    if value.is_null() {
        return String::new();
    }
    let layer = if background { 48 } else { 38 };
    if m.ansi24
        && let ColorArg::List(items) = value
        && items.len() == 3
    {
        let [r, g, b] = value.rgb().unwrap_or([0, 0, 0]);
        return format!("{ESC}[{layer};2;{r};{g};{b}m");
    }
    if m.ansi8 {
        return match value {
            ColorArg::Name(name) => lookup(name).map(|c| format!("{ESC}[{layer};5;{}m", c.ansi8)).unwrap_or_default(),
            ColorArg::Number(n) => format!("{ESC}[{layer};5;{n}m"),
            _ => String::new(),
        };
    }
    if m.ansi4 {
        return match value {
            ColorArg::Name(name) => lookup(name)
                .map(|c| format!("{ESC}[{}m", if background { c.ansi4_bg } else { c.ansi4_fg }))
                .unwrap_or_default(),
            ColorArg::Number(n) => format!("{ESC}[{n}m"),
            _ => String::new(),
        };
    }
    // A console color name is its own code; any other value goes by its console color
    let console = match value {
        ColorArg::Name(name) => colors::console_color_sgr(name),
        _ => None,
    };
    let code = console.or_else(|| colors::console_color_sgr(native_color_name(value, background))).unwrap_or(37);
    format!("{ESC}[{}m", u32::from(code) + if background { 10 } else { 0 })
}

/// The console color for a converted color value: Gray for text and Black for a background when
/// the value names none.
fn native_color_name(value: &ColorArg, background: bool) -> &'static str {
    let fallback = if background { "Black" } else { "Gray" };
    match value {
        ColorArg::Name(name) => lookup(name).map(|c| c.native).unwrap_or(fallback),
        ColorArg::Number(n) => colors::console_color_name(*n).unwrap_or(fallback),
        _ => fallback,
    }
}

/// Console color numbers 0-15 as their names, so they keep their meaning in another color mode.
fn console_numbers_as_names(values: &mut [ColorArg]) {
    for value in values.iter_mut() {
        if let ColorArg::Number(n) = value
            && let Some(name) = colors::console_color_name(*n)
        {
            *value = ColorArg::Name(name.to_string());
        }
    }
}

/// The ANSI color number 0-15 of a console color, for an underline color in 16 colors.
fn console_ansi_index(native: &str) -> i64 {
    const ORDER: [&str; 16] = [
        "Black", "DarkRed", "DarkGreen", "DarkYellow", "DarkBlue", "DarkMagenta", "DarkCyan", "Gray", "DarkGray", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White",
    ];
    ORDER.iter().position(|n| n.eq_ignore_ascii_case(native)).map_or(0, |i| i as i64)
}

/// A link's address with its control characters removed.
fn clean_link(address: &str) -> String {
    address.chars().filter(|&c| !(c <= '\u{1f}' || c == '\u{7f}')).collect()
}

/// The current time in a .NET date and time format, as [datetime]::Now.ToString() writes it.
pub fn now_text(format: &str) -> PsResult<String> {
    let now = PsType::from_name("System.DateTime").call_static("get_Now", &[])?;
    String::from_ps(&now.call("ToString", &[format.into_ps()?])?)
}

struct Writer<'a, 'ps> {
    ps: &'a Pipeline<'ps>,
    debugging: bool,
    silent: bool,
    host: Option<HostWriter>,
}

impl Writer<'_, '_> {
    fn debug(&mut self, message: impl FnOnce() -> String) -> PsResult<()> {
        if !self.debugging {
            return Ok(());
        }
        let text = format!("[DEBUG] {}", message());
        if self.ps.verbose_enabled() {
            return self.ps.verbose(&text);
        }
        // -Debugging shows its messages without -Verbose, as PSWriteColorEX's Write-Verbose
        // -Verbose does: through the host, the way it shows a verbose record
        let ui = this_cmdlet(self.ps).get("Host")?.get("UI")?;
        ui.call("WriteVerboseLine", &[text.into_ps()?])?;
        Ok(())
    }

    /// A warning that -Silent suppresses.
    fn warn(&mut self, message: &str) -> PsResult<()> {
        if self.silent {
            return Ok(());
        }
        self.ps.warning(message)
    }

    /// The warnings a step collected, in order.
    fn warn_all(&mut self, messages: Vec<String>) -> PsResult<()> {
        for message in messages {
            self.warn(&message)?;
        }
        Ok(())
    }

    fn host(&mut self) -> &mut HostWriter {
        let ps = self.ps;
        self.host.get_or_insert_with(|| HostWriter::new(ps))
    }
}

/// A color converted for the mode in use, with the warnings about its form and range and the
/// debug messages PSWriteColorEX writes for each step: checked against the mode asked for,
/// lightened for -Bold where the terminal shows bold as brighter colors, and converted for the
/// mode in use after any fallback. `index` is the segment the color is for.
fn mode_color(w: &mut Writer<'_, '_>, m: &Modes, value: &ColorArg, index: usize, background: bool) -> PsResult<ColorArg> {
    let original_value = value;
    let mut current = value.clone();
    if current.is_null() || (background && matches!(&current, ColorArg::Name(n) if n.eq_ignore_ascii_case("None"))) {
        return Ok(ColorArg::Null);
    }
    // A name the color table lacks takes no color; the warning came when the colors were read
    if let ColorArg::Name(name) = &current
        && !colors::is_hex_text(name)
        && lookup(name).is_none()
    {
        return Ok(ColorArg::Null);
    }
    // A hex code, which rgb() and hsl() colors have become, is a TrueColor color in any mode
    // asked for, read before -Bold can lighten it into an RGB array
    let hex_color = matches!(&current, ColorArg::Name(name) if colors::is_hex_text(name));
    // The words each message starts with for a background
    let what = if background { "Background " } else { "" };
    let for_background = if background { " for background" } else { "" };
    let rgb_text = |rgb: [i64; 3]| format!("R={} G={} B={}", rgb[0], rgb[1], rgb[2]);
    let rgb_array = |rgb: [i64; 3]| format!("@({},{},{})", rgb[0], rgb[1], rgb[2]);
    if !background {
        w.debug(|| format!("Processing color at index {index}: {} (type: {})", current.display(), current.type_name()))?;
    }

    // Checked against the mode asked for, before any fallback
    if m.original_truecolor {
        if let (Some((rgb, exact)), ColorArg::List(items)) = (current.rgb_exact(), &current) {
            // A fraction counts as out of range, since the rounded value differs from it
            let clamped = rgb.map(|v| v.clamp(0, 255));
            if clamped != rgb || !exact {
                let warning = format!(
                    "{}RGB values out of range (0-255). Original: @({},{},{}). Clamped to: @({},{},{})",
                    what,
                    items[0].display(),
                    items[1].display(),
                    items[2].display(),
                    clamped[0],
                    clamped[1],
                    clamped[2]
                );
                w.warn(&warning)?;
                current = ColorArg::from_rgb(clamped);
            }
        } else if let ColorArg::Number(n) = current
            && !m.implied_truecolor
        {
            w.warn(&format!(
                "TrueColor mode expects RGB array @(R,G,B) or hex color{for_background}, but received integer code {n}. Use -ANSI8 or -ANSI4 for integer codes."
            ))?;
            w.debug(|| format!("Type mismatch: integer {n} provided for TrueColor{}", if background { " background" } else { "" }))?;
        }
    } else if m.original_ansi8 {
        if matches!(&current, ColorArg::List(items) if items.len() == 3) {
            w.warn(&format!("ANSI8 mode expects integer code (0-255) or color name{for_background}, but received RGB array. Use -TrueColor for RGB arrays."))?;
            w.debug(|| format!("Type mismatch: RGB array provided for ANSI8{}", if background { " background" } else { "" }))?;
        } else if let ColorArg::Number(n) = current
            && !(0..=255).contains(&n)
        {
            w.warn(&format!("{what}ANSI8 color code {n} is out of range (0-255). Using Gray (7)."))?;
            current = ColorArg::Number(7);
        }
    }

    // Where the terminal shows bold as brighter colors, the color is made lighter instead
    if m.lighten {
        w.debug(|| format!("Bold enabled but terminal doesn't support bold fonts - auto-lightening {}color", if background { "background " } else { "" }))?;
        let lower_c = if background { "c" } else { "C" };
        if let Some(rgb) = current.rgb() {
            let lighter = colors::lighter_rgb(rgb, 1.4);
            w.debug(|| format!("{what}RGB color lightened to: {}", rgb_text(lighter)))?;
            current = ColorArg::from_rgb(lighter);
        } else if let ColorArg::Number(n) = current {
            // ANSI4 codes are left to the terminal, which brightens them for bold
            if m.ansi8 && (0..=255).contains(&n) {
                let lighter = colors::lighter_ansi8(n, 1.4);
                w.debug(|| format!("{what}ANSI8 code {n} algorithmically lightened to {lighter}"))?;
                current = ColorArg::Number(lighter);
            }
        } else if let ColorArg::Name(name) = &current {
            let name = name.clone();
            if !colors::is_hex_text(&name) {
                let lighter = colors::lighter_name(&name);
                if lighter != name {
                    w.debug(|| format!("{what}{lower_c}olor name lightened from {} to {lighter}", original_value.display()))?;
                    current = ColorArg::Name(lighter);
                } else if let Some(entry) = lookup(&name) {
                    // No lighter name: ANSI8 and TrueColor lighten the color's value instead
                    if m.ansi8 {
                        let lighter = colors::lighter_ansi8(i64::from(entry.ansi8), 1.4);
                        w.debug(|| {
                            format!("{what}{lower_c}olor name {} algorithmically lightened in ANSI8 from code {} to {lighter}", original_value.display(), entry.ansi8)
                        })?;
                        current = ColorArg::Number(lighter);
                    } else if m.ansi24 {
                        let lighter = colors::lighter_rgb(entry.rgb.map(i64::from), 1.4);
                        w.debug(|| format!("{what}{lower_c}olor name {} algorithmically lightened in ANSI24 from RGB to {}", original_value.display(), rgb_text(lighter)))?;
                        current = ColorArg::from_rgb(lighter);
                    }
                }
            } else {
                let rgb = hex_rgb(w.ps, &name)?;
                current = ColorArg::from_rgb(colors::lighter_rgb(rgb, 1.4));
                w.debug(|| format!("{what}{}ex color {} converted to RGB and lightened", if background { "h" } else { "H" }, original_value.display()))?;
            }
        }
    }

    // Converted for the mode in use after any fallback
    let hex_name = |value: &ColorArg| match value {
        ColorArg::Name(name) if colors::is_hex_text(name) => Some(name.clone()),
        _ => None,
    };
    let converted = if m.ansi24 {
        if let Some(rgb) = current.rgb() {
            if !background {
                w.debug(|| format!("RGB array color: {}", rgb_text(rgb)))?;
            }
            current
        } else if let Some(name) = hex_name(&current) {
            let rgb = hex_rgb(w.ps, &name)?;
            if !background {
                w.debug(|| format!("Hex color {name} converted to RGB: {}", rgb_text(rgb)))?;
            }
            ColorArg::from_rgb(rgb)
        } else if let ColorArg::Name(name) = &current {
            match lookup(name) {
                Some(entry) => {
                    if !background {
                        w.debug(|| format!("Named color {name} mapped to RGB"))?;
                    }
                    ColorArg::from_rgb(entry.rgb.map(i64::from))
                }
                None => current,
            }
        } else {
            current
        }
    } else if m.ansi8 && (m.original_truecolor || hex_color) {
        // A TrueColor color in ANSI8, asked for or fallen back to
        if let Some(rgb) = current.rgb() {
            let code = colors::rgb_to_ansi8(rgb);
            w.debug(|| format!("{what}RGB {} converted to ANSI8: {code}", rgb_array(rgb)))?;
            ColorArg::Number(code)
        } else if let Some(name) = hex_name(&current) {
            let code = colors::rgb_to_ansi8(hex_rgb(w.ps, &name)?);
            w.debug(|| format!("{what}{}ex {name} converted to ANSI8: {code}", if background { "h" } else { "H" }))?;
            ColorArg::Number(code)
        } else {
            current
        }
    } else if m.ansi4 && (m.original_truecolor || hex_color) {
        // A TrueColor color in ANSI4, asked for or fallen back to; a background code is the
        // foreground code plus 10
        let offset = if background { 10 } else { 0 };
        if let Some(rgb) = current.rgb() {
            let code = colors::rgb_to_ansi4(rgb) + offset;
            w.debug(|| format!("{what}RGB {} converted to ANSI4: {code}", rgb_array(rgb)))?;
            ColorArg::Number(code)
        } else if let Some(name) = hex_name(&current) {
            let code = colors::rgb_to_ansi4(hex_rgb(w.ps, &name)?) + offset;
            w.debug(|| format!("{what}{}ex {name} converted to ANSI4: {code}", if background { "h" } else { "H" }))?;
            ColorArg::Number(code)
        } else {
            current
        }
    } else if !m.ansi_support && (m.original_truecolor || hex_color) {
        // A TrueColor color fell back to console colors, through the nearest ANSI4 code
        if let Some(rgb) = current.rgb() {
            let native = colors::ansi4_to_native(colors::rgb_to_ansi4(rgb));
            if !background {
                w.debug(|| format!("RGB {} converted to Native: {native}", rgb_array(rgb)))?;
            }
            ColorArg::Name(native.to_string())
        } else if let Some(name) = hex_name(&current) {
            let native = colors::ansi4_to_native(colors::rgb_to_ansi4(hex_rgb(w.ps, &name)?));
            if !background {
                w.debug(|| format!("Hex {name} converted to Native: {native}"))?;
            }
            ColorArg::Name(native.to_string())
        } else {
            current
        }
    } else if let (ColorArg::Number(n), true, true) = (&current, m.ansi4, m.original_ansi8) {
        // ANSI8 fell back to ANSI4
        let offset = if background { 10 } else { 0 };
        let code = colors::ansi8_to_ansi4(*n) + offset;
        w.debug(|| format!("{what}ANSI8 code {n} converted to ANSI4: {code}"))?;
        ColorArg::Number(code)
    } else if let (ColorArg::Number(n), false, true) = (&current, m.ansi_support, m.original_ansi8 || m.original_ansi4) {
        // ANSI8 or ANSI4 fell back to console colors
        let code = if m.original_ansi8 { colors::ansi8_to_ansi4(*n) } else { *n };
        let native = colors::ansi4_to_native(code);
        if !background {
            w.debug(|| format!("Color code {n} converted to Native: {native}"))?;
        }
        ColorArg::Name(native.to_string())
    } else {
        current
    };
    Ok(converted)
}

/// The escape code that sets an underline color in the mode in use, or nothing for none:
/// 58;2;R;G;B in TrueColor, 58;5;N otherwise, with N the 256-color number, or 0-15 in 16 colors.
fn underline_sequence(w: &mut Writer<'_, '_>, m: &Modes, value: &ColorArg) -> PsResult<String> {
    let mut rgb: Option<[i64; 3]> = None;
    let mut number: i64 = 0;
    match value {
        ColorArg::Null => return Ok(String::new()),
        ColorArg::Name(name) if name.eq_ignore_ascii_case("None") => return Ok(String::new()),
        ColorArg::List(items) if items.len() == 3 => match value.rgb() {
            Some(channels) => rgb = Some(channels.map(|c| c.clamp(0, 255))),
            None => return Ok(String::new()),
        },
        ColorArg::Name(name) if colors::is_hex_text(name) => rgb = Some(hex_rgb(w.ps, name)?),
        ColorArg::Name(name) => {
            let Some(entry) = lookup(name) else { return Ok(String::new()) };
            if m.ansi24 {
                rgb = Some(entry.rgb.map(i64::from));
            } else if m.ansi8 {
                number = i64::from(entry.ansi8);
            } else if m.ansi4 {
                let code = i64::from(entry.ansi4_fg);
                number = if code >= 90 { code - 82 } else { code - 30 };
            } else {
                number = console_ansi_index(entry.native);
            }
        }
        ColorArg::Number(n) => number = (*n).clamp(0, 255),
        _ => return Ok(String::new()),
    }
    if let Some(rgb) = rgb {
        if m.ansi24 {
            return Ok(format!("{ESC}[58;2;{};{};{}m", rgb[0], rgb[1], rgb[2]));
        }
        number = if m.ansi8 {
            colors::rgb_to_ansi8(rgb)
        } else {
            let code = colors::rgb_to_ansi4(rgb);
            if code >= 90 { code - 82 } else { code - 30 }
        };
    }
    Ok(format!("{ESC}[58;5;{number}m"))
}

/// The styles -Style gives one segment: a name, or an array of names, at its index.
fn segment_style(style: Option<&StyleArg>, index: usize) -> String {
    let mut codes = String::new();
    if let Some(StyleArg::List(items)) = style {
        match items.get(index) {
            Some(StyleArg::List(names)) => {
                for name in names {
                    if let StyleArg::Name(name) = name {
                        codes.push_str(style_code(name));
                    }
                }
            }
            Some(StyleArg::Name(name)) => codes.push_str(style_code(name)),
            _ => {}
        }
    }
    codes
}

/// What writes each line of one Write-ColorEX call: the colors, styles, underline colors, links
/// and gradients, each per segment.
struct LineWriter<'a> {
    modes: Modes,
    style: Option<&'a StyleArg>,
    /// The escape codes of the styles every segment takes, after each segment's own.
    line_styles: String,
    foregrounds: &'a [ColorArg],
    backgrounds: &'a [ColorArg],
    underlines: &'a [String],
    links: &'a [String],
    links_on: bool,
    gradient: Option<&'a [ColorArg]>,
    background_gradient: Option<&'a [ColorArg]>,
}

/// One piece of a line written with console colors.
struct Piece {
    text: String,
    fg: Option<&'static str>,
    bg: Option<&'static str>,
}

impl LineWriter<'_> {
    /// The text of a line with its colors, styles and links as escape codes. A run's markup or
    /// -Highlight colors and styles go over its segment's. A segment with no text color of its
    /// own takes the gradient, character by character from the run's place in it, and one with no
    /// background color the background gradient. Each run with codes ends with a reset; a link
    /// opens before the codes of the first run it covers and closes after the last.
    fn ansi(&self, items: &[Item]) -> String {
        let m = &self.modes;
        let mut line = String::new();
        let mut open_link = String::new();
        for item in items {
            let i = item.index;
            let segment_styles = if m.ansi_support { format!("{}{}", segment_style(self.style, i), self.line_styles) } else { String::new() };
            let segment_fg = self.foregrounds.get(i).filter(|c| !c.is_null());
            let segment_bg = self.backgrounds.get(i).filter(|c| !c.is_null());
            let segment_underline = self.underlines.get(i).map(String::as_str).unwrap_or("");
            let segment_link = self.links.get(i).map(String::as_str).unwrap_or("");
            for run in &item.runs {
                let run_link = match &run.spec.link {
                    Some(address) => clean_link(address),
                    None => segment_link.to_string(),
                };
                if self.links_on && run_link != open_link {
                    if !open_link.is_empty() {
                        let _ = write!(line, "{ESC}]8;;{ESC}\\");
                    }
                    if !run_link.is_empty() {
                        let _ = write!(line, "{ESC}]8;;{run_link}{ESC}\\");
                    }
                    open_link = run_link;
                }
                let mut codes = format!("{segment_styles}{segment_underline}");
                if m.ansi_support {
                    for style in &run.spec.styles {
                        codes.push_str(style_code(style));
                    }
                }
                let fg = if run.spec.fg.is_some() { Some(&run.mode_fg).filter(|c| !c.is_null()) } else { segment_fg };
                let bg = if run.spec.bg.is_some() { Some(&run.mode_bg).filter(|c| !c.is_null()) } else { segment_bg };
                let fg_gradient = self.gradient.filter(|_| run.spec.fg.is_none() && segment_fg.is_none());
                let bg_gradient = self.background_gradient.filter(|_| run.spec.bg.is_none() && segment_bg.is_none());
                if fg_gradient.is_some() || bg_gradient.is_some() {
                    if fg_gradient.is_none()
                        && let Some(color) = fg
                    {
                        codes.push_str(&color_sequence(color, false, m));
                    }
                    if bg_gradient.is_none()
                        && let Some(color) = bg
                    {
                        codes.push_str(&color_sequence(color, true, m));
                    }
                    line.push_str(&codes);
                    let mut at = run.gradient_index;
                    for character in &run.characters {
                        for (gradient, layer) in [(fg_gradient, 38), (bg_gradient, 48)] {
                            let Some(step) = gradient.and_then(|g| g.get(at)) else { continue };
                            match step {
                                ColorArg::List(_) if m.ansi24 => {
                                    let [r, g, b] = step.rgb().unwrap_or([0, 0, 0]);
                                    let _ = write!(line, "{ESC}[{layer};2;{r};{g};{b}m");
                                }
                                ColorArg::Number(n) if m.ansi8 => {
                                    let _ = write!(line, "{ESC}[{layer};5;{n}m");
                                }
                                _ => {}
                            }
                        }
                        line.push_str(character);
                        at += 1;
                    }
                    let _ = write!(line, "{ESC}[0m");
                    continue;
                }
                if let Some(color) = fg {
                    codes.push_str(&color_sequence(color, false, m));
                }
                if let Some(color) = bg {
                    codes.push_str(&color_sequence(color, true, m));
                }
                line.push_str(&codes);
                line.push_str(&run.text);
                if !codes.is_empty() || (self.gradient.is_some() && segment_fg.is_some()) {
                    let _ = write!(line, "{ESC}[0m");
                }
            }
        }
        if !open_link.is_empty() {
            let _ = write!(line, "{ESC}]8;;{ESC}\\");
        }
        line
    }

    /// The pieces of a line written with console colors, one Write-Host call each: text with the
    /// console colors of its run or segment. White space with no background joins its neighbor,
    /// and pieces of one color pair go out together.
    fn pieces(&self, prefix: &str, time_text: &str, items: &[Item]) -> Vec<Piece> {
        let mut pieces: Vec<Piece> = Vec::new();
        if !prefix.is_empty() {
            pieces.push(Piece { text: prefix.to_string(), fg: None, bg: None });
        }
        if !time_text.is_empty() {
            pieces.push(Piece { text: time_text.to_string(), fg: Some("DarkGray"), bg: None });
        }
        for item in items {
            let i = item.index;
            for run in &item.runs {
                let fg_value = if run.spec.fg.is_some() { Some(&run.mode_fg) } else { self.foregrounds.get(i) };
                let bg_value = if run.spec.bg.is_some() { Some(&run.mode_bg) } else { self.backgrounds.get(i) };
                let fg = fg_value.filter(|c| !c.is_null()).map(|c| native_color_name(c, false));
                let bg = bg_value.filter(|c| !c.is_null()).map(|c| native_color_name(c, true));
                pieces.push(Piece { text: run.text.clone(), fg, bg });
            }
        }

        let blank = |text: &str| text.chars().all(is_white_space);
        let mut merged: Vec<Piece> = Vec::new();
        for piece in pieces {
            if piece.text.is_empty() {
                continue;
            }
            let piece_blank = blank(&piece.text) && piece.bg.is_none();
            if let Some(previous) = merged.last_mut() {
                if previous.bg.is_none() && piece_blank {
                    previous.text.push_str(&piece.text);
                    continue;
                }
                if previous.fg == piece.fg && previous.bg == piece.bg {
                    previous.text.push_str(&piece.text);
                    continue;
                }
                if piece.bg.is_none() && previous.bg.is_none() && blank(&previous.text) {
                    let text = format!("{}{}", previous.text, piece.text);
                    *previous = Piece { text, fg: piece.fg, bg: piece.bg };
                    continue;
                }
            }
            merged.push(piece);
        }
        merged
    }
}

/// A -Highlight pattern compiled, with the style it gives the text it matches.
pub struct HighlightEntry {
    regex: PsObject,
    spec: Spec,
}

/// -Highlight's patterns compiled once for every line, each with its style read as a markup tag
/// is. Patterns are .NET regular expressions matched without regard to case. A pattern that does
/// not compile stops the command; a style that is not one is skipped with a warning unless
/// -Silent.
pub fn compile_highlight(ps: &Pipeline<'_>, highlight: &PsObject, silent: bool) -> PsResult<Vec<HighlightEntry>> {
    let mut entries = Vec::new();
    if highlight.is_null() || i64::from_ps(&highlight.get("Count")?)? <= 0 {
        return Ok(entries);
    }
    let regex_type = PsType::from_name("System.Text.RegularExpressions.Regex");
    // RegexOptions IgnoreCase and CultureInvariant, made from the type of a pattern's options
    let options_type = regex_type.new(&["".into_ps()?])?.get("Options")?.call("GetType", &[])?;
    let options = PsType::from_name("System.Enum").call_static("ToObject", &[options_type, 513i32.into_ps()?])?;
    for key in Vec::<PsObject>::from_ps(&highlight.get("Keys")?)? {
        let pattern = String::from_ps(&key)?;
        let regex = match regex_type.new(&[pattern.as_str().into_ps()?, options.clone()]) {
            Ok(regex) => regex,
            Err(e) => {
                let reason = pattern_error(&pattern).unwrap_or(e.message);
                return Err(PsError::new(
                    ErrorCategory::InvalidData,
                    "ParameterArgumentValidationError",
                    format!("Cannot validate argument on parameter 'Highlight'. {reason}"),
                )
                .with_target(key)
                .terminating());
            }
        };
        let value = highlight.call("get_Item", &[key])?;
        let style_text = if value.is_null() { String::new() } else { String::from_ps(&value)? };
        let mut warnings = Vec::new();
        let spec = parse_spec(&style_text, &mut |message| warnings.push(message));
        if !silent {
            for message in &warnings {
                ps.warning(message)?;
            }
        }
        match spec {
            Some(spec) => entries.push(HighlightEntry { regex, spec }),
            None => {
                if !silent {
                    ps.warning(&format!("Highlight style '{style_text}' for '{pattern}' is not a style; the pattern is skipped."))?;
                }
            }
        }
    }
    Ok(entries)
}

/// Why a pattern does not compile, as .NET's innermost exception says it. The constructor call
/// above reaches the exception only wrapped, so PowerShell compiles the pattern once more here,
/// on this path alone, and answers the message.
fn pattern_error(pattern: &str) -> PsResult<String> {
    const SCRIPT: &str = "param($Pattern) try { $null = [regex]::new($Pattern, 'IgnoreCase, CultureInvariant') } catch { $reason = $_.Exception; while ($reason.InnerException) { $reason = $reason.InnerException }; $reason.Message }";
    let script = PsType::from_name("System.Management.Automation.ScriptBlock").call_static("Create", &[SCRIPT.into_ps()?])?;
    let found = script.call("InvokeReturnAsIs", &[PsArray(vec![pattern.into_ps()?]).into_ps()?])?;
    if found.is_null() {
        return Err(PsError::new(ErrorCategory::InvalidData, "PatternCompiled", "the pattern compiles"));
    }
    String::from_ps(&found)
}

/// The matches of a compiled pattern in a line, as start and end byte offsets.
fn highlight_matches(regex: &PsObject, line: &str) -> PsResult<Vec<(usize, usize)>> {
    // Where each UTF-16 unit of the line starts in its UTF-8 bytes
    let mut byte_at: Vec<usize> = Vec::with_capacity(line.len() + 1);
    for (at, c) in line.char_indices() {
        for _ in 0..c.len_utf16() {
            byte_at.push(at);
        }
    }
    byte_at.push(line.len());
    let mut found = Vec::new();
    for found_match in Vec::<PsObject>::from_ps(&regex.call("Matches", &[line.into_ps()?])?)? {
        let index = i64::from_ps(&found_match.get("Index")?)?.max(0) as usize;
        let length = i64::from_ps(&found_match.get("Length")?)?.max(0) as usize;
        let start = byte_at[index.min(byte_at.len() - 1)];
        let end = byte_at[(index + length).min(byte_at.len() - 1)];
        found.push((start, end));
    }
    Ok(found)
}

/// Writes text to the host, the log file, or `capture` for Format-ColorEX, as Write-ColorEX does.
pub fn render(ps: &Pipeline<'_>, mut o: WriteOptions, highlight: &[HighlightEntry], mut capture: Option<&mut Vec<String>>) -> PsResult<()> {
    let mut w = Writer { ps, debugging: o.debugging, silent: o.silent, host: None };
    w.debug(|| format!("Starting Write-ColorEX with Text count: {}", o.text.len()))?;

    if let Some(gradient) = &o.gradient
        && list_is_true(gradient, ColorArg::is_true)
        && gradient.len() < 2
    {
        let count = gradient.len();
        w.warn(&format!("Gradient requires at least 2 colors (received {count}). Gradient disabled."))?;
        w.debug(|| format!("Gradient validation failed: Only {count} color(s) provided"))?;
        o.gradient = None;
    }
    if let Some(gradient) = &o.background_gradient
        && list_is_true(gradient, ColorArg::is_true)
        && gradient.len() < 2
    {
        let count = gradient.len();
        w.warn(&format!("BackGroundGradient requires at least 2 colors (received {count}). BackGroundGradient disabled."))?;
        o.background_gradient = None;
    }

    // Only one color mode applies: TrueColor, then ANSI8, then ANSI4
    if [o.ansi4, o.ansi8, o.ansi24].iter().filter(|&&m| m).count() > 1 {
        ps.warning("Multiple color modes specified. Only one of -ANSI4, -ANSI8, or -TrueColor should be used.")?;
        if o.ansi24 {
            w.debug(|| "Using TrueColor mode (highest priority)".to_string())?;
            o.ansi4 = false;
            o.ansi8 = false;
        } else {
            w.debug(|| "Using ANSI8 mode".to_string())?;
            o.ansi4 = false;
            o.ansi24 = false;
        }
    }

    if let Some(profile) = o.style_profile.clone() {
        w.debug(|| format!("Applying style profile: {}", profile.get("Name").and_then(|n| String::from_ps(&n)).unwrap_or_default()))?;
        let params = style::write_color_params(&profile)?;
        o.apply_profile(&params, true)?;
    }
    if o.default
        && let Some(default) = style::default_style()?
    {
        w.debug(|| "Applying default style profile".to_string())?;
        if o.style_profile.is_none() {
            let params = style::write_color_params(&default)?;
            o.apply_profile(&params, true)?;
        }
    }

    // Colors written #RGB, 0xRGB, rgb(r, g, b) or hsl(h, s%, l%) read as #RRGGBB, with a warning
    // for each name the color table lacks
    {
        let mut unknown = UnknownNames::default();
        let mut warnings = Vec::new();
        let mut warn = |message: String| warnings.push(message);
        read_forms(&mut o.color, &mut unknown, &mut warn);
        read_forms(&mut o.background, &mut unknown, &mut warn);
        read_forms(&mut o.underline_color, &mut unknown, &mut warn);
        if let Some(gradient) = o.gradient.as_mut() {
            read_forms(gradient, &mut unknown, &mut warn);
        }
        if let Some(gradient) = o.background_gradient.as_mut() {
            read_forms(gradient, &mut unknown, &mut warn);
        }
        w.warn_all(warnings)?;
    }

    // A hex code or an RGB array asks for TrueColor. Without a color mode given, it takes the
    // best mode the terminal has, without the warnings an explicit -TrueColor gives.
    let mut implied_truecolor = false;
    if !(o.ansi4 || o.ansi8 || o.ansi24)
        && o.color.iter().chain(o.background.iter()).chain(o.underline_color.iter()).any(|v| v.is_hex() || matches!(v, ColorArg::List(_)))
    {
        implied_truecolor = true;
        w.debug(|| "Hex or RGB color without a color mode: using TrueColor".to_string())?;
        o.ansi24 = true;
        console_numbers_as_names(&mut o.color);
        console_numbers_as_names(&mut o.background);
        console_numbers_as_names(&mut o.underline_color);
    }

    // Three integers for one segment under -TrueColor are one RGB color, not three colors
    if o.ansi24 && o.text.len() == 1 {
        for (list, what) in [(&mut o.color, Some("")), (&mut o.background, Some(" for background")), (&mut o.underline_color, None)] {
            if list.len() == 3 && list.iter().all(|v| matches!(v, ColorArg::Number(_))) {
                if let Some(what) = what {
                    let shown = list.iter().map(ColorArg::display).collect::<Vec<_>>().join(",");
                    w.debug(|| format!("Detected flattened RGB array{what}, wrapping: @({shown})"))?;
                }
                let rgb = ColorArg::List(std::mem::take(list));
                list.push(rgb);
            }
        }
    }

    // The text as segments, each a list of runs with the colors, styles and link markup and
    // -Highlight give them; -Split, -SplitAround and -SplitEvenly cut the segments
    let mut segments: Vec<Segment> = Vec::with_capacity(o.text.len());
    {
        let mut warnings = Vec::new();
        for item in &o.text {
            if o.markup {
                let mut runs = parse_markup(item, &mut |message| warnings.push(message));
                if runs.is_empty() {
                    runs.push(Run::new(""));
                }
                segments.push(runs);
            } else {
                segments.push(plain_segment(item.as_str()));
            }
        }
        w.warn_all(warnings)?;
    }

    let split_true = |list: &Option<Vec<String>>| list.as_ref().is_some_and(|items| list_is_true(items, |s: &String| !s.is_empty()));
    if split_true(&o.split) || split_true(&o.split_around) {
        segments = if split_true(&o.split) {
            split_segments(segments, SplitBy::After(o.split.as_deref().unwrap_or_default()))
        } else {
            split_segments(segments, SplitBy::Around(o.split_around.as_deref().unwrap_or_default()))
        };
    } else if o.split_evenly {
        let parts = if !o.color.is_empty() { o.color.len() } else { o.background.len() };
        if parts > 1 {
            segments = split_segments(segments, SplitBy::Evenly(parts));
        }
    }

    if !highlight.is_empty() {
        let styles: Vec<Spec> = highlight.iter().map(|entry| entry.spec.clone()).collect();
        let mut failure: Option<PsError> = None;
        segments = highlight_segments(segments, &styles, |entry, line| match highlight_matches(&highlight[entry].regex, line) {
            Ok(found) => found,
            Err(e) => {
                failure.get_or_insert(e);
                Vec::new()
            }
        });
        if let Some(e) = failure {
            return Err(e);
        }
    }

    // Whether markup or -Highlight gives runs colors, or styles or links, which need escape
    // codes. A hex code among their colors asks for TrueColor, as in -Color.
    let mut run_colors = false;
    let mut run_escapes = false;
    if o.markup || !highlight.is_empty() {
        let mut run_hex = false;
        for run in segments.iter().flatten() {
            if run.spec.fg.is_some() || run.spec.bg.is_some() {
                run_colors = true;
                if run.spec.fg.as_deref().is_some_and(colors::is_hex_text) || run.spec.bg.as_deref().is_some_and(colors::is_hex_text) {
                    run_hex = true;
                }
            }
            if run.spec.link.is_some() || !run.spec.styles.is_empty() {
                run_escapes = true;
            }
        }
        if run_hex && !(o.ansi4 || o.ansi8 || o.ansi24) {
            w.debug(|| "Hex or RGB color without a color mode: using TrueColor".to_string())?;
            implied_truecolor = true;
            o.ansi24 = true;
            console_numbers_as_names(&mut o.color);
            console_numbers_as_names(&mut o.background);
            console_numbers_as_names(&mut o.underline_color);
        }
    }

    // The color mode asked for, before any fallback, which the color checks read
    let original_truecolor = o.ansi24;
    let original_ansi8 = o.ansi8;
    let original_ansi4 = o.ansi4;

    // Padding to a display width, measured so wide characters count as 2 cells; with -Wrap, each
    // line is padded when it is written
    let mut pad_width = 1i64;
    if o.auto_pad > 0 {
        w.debug(|| format!("AutoPad processing: Target width = {}, PadLeft = {}, PadChar = '{}'", o.auto_pad, ps_bool(o.pad_left), o.pad_char))?;
        pad_width = display_width(&o.pad_char.to_string(), false);
        if pad_width == 0 {
            w.warn(&format!("PadChar '{}' is a zero-width character and cannot be used for padding. Using space instead.", o.pad_char))?;
            o.pad_char = ' ';
            pad_width = 1;
        }
        if pad_width > 1 {
            w.warn(&format!("PadChar '{}' is a wide character ({pad_width} cells). Padding alignment may be off.", o.pad_char))?;
        }
        if o.truncate {
            segments = truncate_segments(segments, i64::from(o.auto_pad));
        }
        if !o.wrap {
            let current = display_width(&segments_text(&segments), false);
            w.debug(|| format!("Current text display width: {current} cells"))?;
            if current < i64::from(o.auto_pad) {
                let needed = i64::from(o.auto_pad) - current;
                let count = if pad_width > 1 {
                    let remainder = needed % pad_width;
                    if remainder != 0 {
                        w.debug(|| {
                            format!("Padding width ({needed} cells) not evenly divisible by PadChar width ({pad_width} cells). Off by {remainder} cell(s).")
                        })?;
                    }
                    needed / pad_width
                } else {
                    needed
                };
                if count > 0 {
                    w.debug(|| format!("Adding {count} '{}' character(s) = {} cells", o.pad_char, count * pad_width))?;
                    let padding = |count: i64| -> Segment { plain_segment(std::iter::repeat_n(o.pad_char, count as usize).collect::<String>()) };
                    if o.pad_center {
                        let left = count / 2;
                        if left > 0 {
                            segments.insert(0, padding(left));
                        }
                        segments.push(padding(count - left));
                        w.debug(|| "Applied padding on both sides (centered text)".to_string())?;
                    } else if o.pad_left {
                        segments.insert(0, padding(count));
                        w.debug(|| "Applied left padding (right-aligned text)".to_string())?;
                    } else {
                        segments.push(padding(count));
                        w.debug(|| "Applied right padding (left-aligned text)".to_string())?;
                    }
                }
            } else {
                w.debug(|| format!("Text width ({current}) >= Target width ({}). No padding applied.", o.auto_pad))?;
            }
        }
    }

    // The color variables, read on each call since they can change at any time. FORCE_COLOR 1 to
    // 3 keeps colors on in its mode; FORCE_COLOR=0 and NO_COLOR turn them off; CLICOLOR_FORCE, set
    // to anything but 0, keeps them on; CLICOLOR=0 and TERM=dumb turn them off. The first of these
    // set decides.
    let force_color = env_var("FORCE_COLOR")?;
    let forced = matches!(force_color.as_deref(), Some("1" | "2" | "3"));
    let mut cli_color_forced = false;
    let mut color_disabled = false;
    if !forced {
        if force_color.as_deref() == Some("0") || env_var("NO_COLOR")?.is_some_and(|v| !v.is_empty()) {
            color_disabled = true;
        } else if env_var("CLICOLOR_FORCE")?.is_some_and(|v| !v.is_empty() && v != "0") {
            cli_color_forced = true;
        } else if env_var("CLICOLOR")?.as_deref() == Some("0") || env_var("TERM")?.is_some_and(|v| v.eq_ignore_ascii_case("dumb")) {
            color_disabled = true;
        }
    }

    let style_given = o.style.as_ref().is_some_and(StyleArg::is_true);
    let using_ansi = o.ansi4
        || o.ansi8
        || o.ansi24
        || o.bold
        || o.italic
        || o.underline
        || o.blink
        || o.faint
        || o.crossed_out
        || o.double_underline
        || o.overline
        || style_given
        || o.gradient.as_ref().is_some_and(|g| list_is_true(g, ColorArg::is_true))
        || o.reverse
        || !o.underline_style.is_empty()
        || list_is_true(&o.underline_color, ColorArg::is_true)
        || list_is_true(&o.link, |l: &Option<String>| l.as_deref().is_some_and(|l| !l.is_empty()))
        || o.background_gradient.as_ref().is_some_and(|g| list_is_true(g, ColorArg::is_true))
        || run_escapes;

    let mut ansi_support = false;
    let mut compose_line = false;
    let (cached_support, bold_fonts) = session_support(ps)?;
    if color_disabled {
        w.debug(|| "Colors are off: NO_COLOR, FORCE_COLOR=0, CLICOLOR=0 or TERM=dumb".to_string())?;
        o.style = None;
        o.gradient = None;
        o.background_gradient = None;
        o.ansi4 = false;
        o.ansi8 = false;
        o.ansi24 = false;
    } else if !using_ansi {
        // Console colors only: no ANSI detection needed, only whether the line can go out as one call
        compose_line = if forced || cli_color_forced {
            // FORCE_COLOR and CLICOLOR_FORCE keep the colors as escape codes, whatever the host
            capture.is_some() || !o.no_console_output
        } else if capture.is_some() {
            // A string for Format-ColorEX holds escape codes wherever the host shows them
            cached_support != Support::None && host_ansi(ps)
        } else {
            !o.no_console_output && removes_escape_codes(ps) && cached_support != Support::None && host_ansi(ps)
        };
        w.debug(|| format!("Console colors only; one call per line: {}", ps_bool(compose_line)))?;
    } else {
        let support = if forced {
            let value = force_color.as_deref().unwrap_or_default();
            w.debug(|| format!("FORCE_COLOR environment variable detected: {value}"))?;
            let support = match value {
                "1" => Support::Ansi4,
                "2" => Support::Ansi8,
                _ => Support::TrueColor,
            };
            w.debug(|| format!("FORCE_COLOR override: {}", support.as_str()))?;
            support
        } else if cli_color_forced {
            // CLICOLOR_FORCE keeps the support detected, or the 16 colors where none was found
            let support = if cached_support == Support::None { Support::Ansi4 } else { cached_support };
            w.debug(|| format!("CLICOLOR_FORCE override: {}", support.as_str()))?;
            support
        } else {
            let mut found = cached_support;
            if found != Support::None && !host_ansi(ps) {
                // PowerShell would remove the escape codes before they reach the screen
                w.debug(|| "The host renders no escape codes; using console colors".to_string())?;
                found = Support::None;
            }
            w.debug(|| format!("ANSI Color Support: {} (cached)", found.as_str()))?;
            found
        };
        ansi_support = support != Support::None;

        if support == Support::None {
            o.style = None;
            o.ansi4 = false;
            o.ansi8 = false;
            o.ansi24 = false;
            w.debug(|| "ANSI support disabled - using native PowerShell colors".to_string())?;
        } else if o.ansi24 && support != Support::TrueColor {
            o.ansi24 = false;
            if support == Support::Ansi8 {
                if !implied_truecolor {
                    w.warn("TrueColor not supported by terminal. Falling back to ANSI8 (256 colors).")?;
                }
                w.debug(|| "Downgrading from TrueColor to ANSI8".to_string())?;
                o.ansi8 = true;
            } else {
                if !implied_truecolor {
                    w.warn("TrueColor not supported by terminal. Falling back to ANSI4 (16 colors).")?;
                }
                w.debug(|| "Downgrading from TrueColor to ANSI4".to_string())?;
                o.ansi4 = true;
            }
        } else if o.ansi8 && support == Support::Ansi4 {
            w.warn("ANSI8 (256 colors) not supported by terminal. Falling back to ANSI4 (16 colors).")?;
            w.debug(|| "Downgrading from ANSI8 to ANSI4".to_string())?;
            o.ansi8 = false;
            o.ansi4 = true;
        }

        if let Some(count) = o.gradient.as_ref().filter(|g| list_is_true(g, ColorArg::is_true)).map(Vec::len).filter(|&count| count >= 2) {
            w.debug(|| format!("Gradient requested with {count} colors"))?;
            if support == Support::None || support == Support::Ansi4 {
                let found = if support == Support::None { "None" } else { "ANSI4 (16 colors)" };
                w.warn(&format!("Gradient requires ANSI 256-color or TrueColor support. Terminal supports: {found}. Gradient disabled."))?;
                if support == Support::None {
                    w.debug(|| "Gradient disabled: No ANSI support".to_string())?;
                } else {
                    w.debug(|| "Gradient disabled: ANSI4 only".to_string())?;
                }
                o.gradient = None;
            } else {
                if !o.ansi8 && !o.ansi24 {
                    if support == Support::TrueColor {
                        w.debug(|| "Gradient: Auto-enabling TrueColor mode".to_string())?;
                        o.ansi24 = true;
                    } else {
                        w.debug(|| "Gradient: Auto-enabling ANSI8 mode".to_string())?;
                        o.ansi8 = true;
                    }
                }
                w.debug(|| format!("Gradient enabled in {} mode", support.as_str()))?;
            }
        }

        if o.background_gradient.as_ref().is_some_and(|g| list_is_true(g, ColorArg::is_true) && g.len() >= 2) {
            if support == Support::None || support == Support::Ansi4 {
                let found = if support == Support::None { "None" } else { "ANSI4 (16 colors)" };
                w.warn(&format!("BackGroundGradient requires ANSI 256-color or TrueColor support. Terminal supports: {found}. BackGroundGradient disabled."))?;
                o.background_gradient = None;
            } else if !o.ansi8 && !o.ansi24 {
                if support == Support::TrueColor {
                    o.ansi24 = true;
                } else {
                    o.ansi8 = true;
                }
            }
        }
    }

    if !o.no_console_output {
        let mut window_width = 0;
        if o.blank_line || o.horizontal_center || (o.wrap && o.auto_pad <= 0) {
            window_width = host_width(ps);
        }
        if o.blank_line {
            w.debug(|| "Processing blank line".to_string())?;
            o.horizontal_center = false;
            o.start_tab = 0;
            o.start_spaces = 0;
            o.show_time = false;
            o.wrap = false;
            segments = vec![plain_segment(" ".repeat(window_width.max(0) as usize))];
        }

        let time_text = if o.show_time { format!("[{}] ", now_text(&o.date_time_format)?) } else { String::new() };

        // The lines to write, each a list of items: the segment whose colors a piece takes, and
        // its runs. Without -Wrap, one line of every segment.
        let mut segment_count = segments.len();
        let wrap_width = if o.wrap {
            if o.auto_pad > 0 {
                i64::from(o.auto_pad)
            } else {
                window_width - 8 * i64::from(o.start_tab) - i64::from(o.start_spaces) - display_width(&time_text, false)
            }
        } else {
            0
        };
        let mut lines = if wrap_width > 0 {
            let lines = wrap_segments(&segments, wrap_width);
            if o.auto_pad > 0 {
                let side = if o.pad_center {
                    PadSide::Center
                } else if o.pad_left {
                    PadSide::Left
                } else {
                    PadSide::Right
                };
                let (padded, count) = pad_lines(lines, segment_count, i64::from(o.auto_pad), o.pad_char, pad_width, side);
                segment_count = count;
                padded
            } else {
                lines
            }
        } else {
            single_line(segments.clone())
        };

        // Each run's characters, and where it starts in the gradients, which run across every
        // character written, padding included. A color code is never put inside a character.
        let gradient_on = |g: &Option<Vec<ColorArg>>| g.as_ref().is_some_and(|g| list_is_true(g, ColorArg::is_true) && g.len() >= 2);
        let mut total_chars = 0usize;
        if gradient_on(&o.gradient) || gradient_on(&o.background_gradient) {
            for run in lines.iter_mut().flatten().flat_map(|item| item.runs.iter_mut()) {
                if run.characters.is_empty() {
                    run.characters = split_display_characters(&run.text).into_iter().map(str::to_string).collect();
                }
                run.gradient_index = total_chars;
                total_chars += run.characters.len();
            }
        }

        let oklab = !o.gradient_space.eq_ignore_ascii_case("RGB");
        let mut gradient_array: Option<Vec<ColorArg>> = None;
        if let Some(gradient) = o.gradient.clone().filter(|g| list_is_true(g, ColorArg::is_true) && g.len() >= 2) {
            w.debug(|| "Calculating gradient for text".to_string())?;
            w.debug(|| format!("Total characters for gradient: {total_chars}"))?;
            if gradient.len() > total_chars {
                w.warn(&format!(
                    "Gradient has {} colors but text only has {total_chars} characters. Applying standard coloring instead.",
                    gradient.len()
                ))?;
                w.debug(|| format!("Gradient disabled: More colors ({}) than characters ({total_chars})", gradient.len()))?;
                o.gradient = None;
            } else {
                w.debug(|| format!("Generating gradient in {} mode", if o.ansi24 { "TrueColor" } else { "ANSI8" }))?;
                gradient_array = Some(gradient_colors(ps, &gradient, total_chars, o.ansi24, oklab)?);
            }
        }
        let mut background_gradient_array: Option<Vec<ColorArg>> = None;
        if let Some(gradient) = o.background_gradient.clone().filter(|g| list_is_true(g, ColorArg::is_true) && g.len() >= 2) {
            if gradient.len() > total_chars {
                w.warn(&format!(
                    "BackGroundGradient has {} colors but text only has {total_chars} characters. Applying standard coloring instead.",
                    gradient.len()
                ))?;
                o.background_gradient = None;
            } else {
                background_gradient_array = Some(gradient_colors(ps, &gradient, total_chars, o.ansi24, oklab)?);
            }
        }

        let modes = Modes {
            ansi4: o.ansi4,
            ansi8: o.ansi8,
            ansi24: o.ansi24,
            original_truecolor,
            original_ansi8,
            original_ansi4,
            implied_truecolor,
            ansi_support,
            lighten: o.bold && !bold_fonts,
        };

        // Each segment's text and background color, cycling through -Color and -BackGroundColor,
        // converted for the mode in use
        let mut foregrounds: Vec<ColorArg> = Vec::new();
        if !o.color.is_empty() && !color_disabled {
            w.debug(|| format!("Processing {} colors", o.color.len()))?;
            for i in 0..segment_count {
                foregrounds.push(mode_color(&mut w, &modes, &o.color[i % o.color.len()], i, false)?);
            }
        }
        let mut backgrounds: Vec<ColorArg> = Vec::new();
        if !o.background.is_empty() && !color_disabled {
            w.debug(|| format!("Processing {} background colors", o.background.len()))?;
            for i in 0..segment_count {
                backgrounds.push(mode_color(&mut w, &modes, &o.background[i % o.background.len()], i, true)?);
            }
        }

        // The colors markup and -Highlight give runs, converted for the mode in use
        if run_colors && !color_disabled {
            for item in lines.iter_mut().flatten() {
                let index = item.index;
                for run in &mut item.runs {
                    if let Some(fg) = run.spec.fg.clone() {
                        run.mode_fg = mode_color(&mut w, &modes, &ColorArg::Name(fg), index, false)?;
                    }
                    if let Some(bg) = run.spec.bg.clone() {
                        run.mode_bg = mode_color(&mut w, &modes, &ColorArg::Name(bg), index, true)?;
                    }
                }
            }
        }

        // Each segment's underline color, cycling through -UnderlineColor. A segment with no
        // other underline takes a single one.
        let mut underlines: Vec<String> = Vec::new();
        if ansi_support && !o.underline_color.is_empty() {
            let underlined = o.underline || o.double_underline || !o.underline_style.is_empty();
            for i in 0..segment_count {
                let mut code = underline_sequence(&mut w, &modes, &o.underline_color[i % o.underline_color.len()])?;
                if !code.is_empty() && !underlined {
                    let own_underline = o.style.as_ref().is_some_and(StyleArg::is_true)
                        && match &o.style {
                            Some(StyleArg::List(items)) => match items.get(i) {
                                Some(StyleArg::List(names)) => names.iter().any(is_underline_style),
                                Some(single) => is_underline_style(single),
                                None => false,
                            },
                            Some(single) if i == 0 => is_underline_style(single),
                            _ => false,
                        };
                    if !own_underline {
                        code.insert_str(0, underline_style_code("Single"));
                    }
                }
                underlines.push(code);
            }
        }

        // Each segment's link, cycling through -Link, where escape codes reach the terminal
        let links_on = ansi_support;
        let mut links: Vec<String> = Vec::new();
        if links_on && !o.link.is_empty() {
            for i in 0..segment_count {
                links.push(o.link[i % o.link.len()].as_deref().map(clean_link).unwrap_or_default());
            }
        }

        // The styles of every segment, after each segment's own from -Style
        let mut line_styles = String::new();
        if ansi_support {
            for (on, name) in [
                (o.bold, "Bold"),
                (o.faint, "Faint"),
                (o.italic, "Italic"),
                (o.underline, "Underline"),
                (o.blink, "Blink"),
                (o.crossed_out, "CrossedOut"),
                (o.double_underline, "DoubleUnderline"),
                (o.overline, "Overline"),
                (o.reverse, "Reverse"),
            ] {
                if on {
                    line_styles.push_str(style_code(name));
                }
            }
            if !o.underline_style.is_empty() {
                line_styles.push_str(underline_style_code(&o.underline_style));
            }
        }
        let writer = LineWriter {
            modes,
            style: o.style.as_ref(),
            line_styles,
            foregrounds: &foregrounds,
            backgrounds: &backgrounds,
            underlines: &underlines,
            links: &links,
            links_on,
            gradient: gradient_array.as_deref(),
            background_gradient: background_gradient_array.as_deref(),
        };

        w.debug(|| "Starting text output".to_string())?;

        // What comes before the text: centering, tabs and spaces, then the time. Lines after the
        // first take spaces in place of the time.
        let mut indent = String::new();
        if o.start_tab > 0 {
            indent.push_str(&"\t".repeat(o.start_tab as usize));
        }
        if o.start_spaces > 0 {
            indent.push_str(&" ".repeat(o.start_spaces as usize));
        }

        for _ in 0..o.lines_before.max(0) {
            match capture.as_deref_mut() {
                Some(lines) => lines.push(String::new()),
                None => w.host().write("", None, None, false)?,
            }
        }

        let last_line = lines.len().saturating_sub(1);
        for (line_index, items) in lines.iter().enumerate() {
            let line_no_new_line = o.no_new_line && line_index == last_line;
            let mut line_text: Option<String> = None;
            let mut prefix = String::new();
            if o.horizontal_center && window_width > 0 {
                let text = items_text(items);
                let length = display_width(&text, false);
                if window_width >= length {
                    let position = colors::round_even((window_width as f64 / 2.0 - (length / 2) as f64).max(0.0));
                    prefix.push_str(&" ".repeat(position.max(0) as usize));
                }
                line_text = Some(text);
            }
            prefix.push_str(&indent);
            let mut line_time = time_text.as_str();
            if line_index > 0 && !time_text.is_empty() {
                prefix.push_str(&" ".repeat(display_width(&time_text, false).max(0) as usize));
                line_time = "";
            }

            if color_disabled {
                // One call, no colors or styles
                let text = line_text.unwrap_or_else(|| items_text(items));
                let line = format!("{prefix}{line_time}{text}");
                match capture.as_deref_mut() {
                    Some(lines) => lines.push(line),
                    None => {
                        if !line.is_empty() || !line_no_new_line {
                            w.host().write(&line, None, None, line_no_new_line)?;
                        }
                    }
                }
            } else if ansi_support || compose_line {
                // One call, the colors and styles as escape codes
                if gradient_array.is_some() {
                    w.debug(|| "Using gradient mode for output".to_string())?;
                    for item in items {
                        if foregrounds.get(item.index).is_some_and(|c| !c.is_null()) {
                            w.debug(|| format!("Segment {} has explicit color override (skipping gradient)", item.index))?;
                        }
                    }
                }
                let mut line = prefix;
                if !line_time.is_empty() {
                    let _ = write!(line, "{ESC}[90m{line_time}{ESC}[0m");
                }
                line.push_str(&writer.ansi(items));
                match capture.as_deref_mut() {
                    Some(lines) => lines.push(line),
                    None => {
                        if !line.is_empty() || !line_no_new_line {
                            w.host().write(&line, None, None, line_no_new_line)?;
                        }
                    }
                }
            } else {
                // One call per color, each with -ForegroundColor and -BackgroundColor
                let merged = writer.pieces(&prefix, line_time, items);
                if let Some(lines) = capture.as_deref_mut() {
                    // Console colors cannot be held in a string: the text alone
                    lines.push(merged.iter().map(|piece| piece.text.as_str()).collect());
                } else if merged.is_empty() {
                    if !line_no_new_line {
                        w.host().write("", None, None, false)?;
                    }
                } else {
                    let last = merged.len() - 1;
                    for (p, piece) in merged.iter().enumerate() {
                        let no_newline = p < last || line_no_new_line;
                        w.host().write(&piece.text, piece.fg, piece.bg, no_newline)?;
                    }
                }
            }
        }

        for _ in 0..o.lines_after.max(0) {
            match capture.as_deref_mut() {
                Some(lines) => lines.push(String::new()),
                None => w.host().write("", None, None, false)?,
            }
        }
    }

    if !segments.is_empty() && !o.log_file.is_empty() {
        let text = segments_text(&segments);
        write_log(ps, &o, &text, |message| w.debug(|| message.to_string()))?;
    }
    w.debug(|| "Write-ColorEX completed".to_string())?;
    Ok(())
}

/// Whether a -Style entry names an underline: Underline, DoubleUnderline, Curly, Dotted or Dashed.
fn is_underline_style(style: &StyleArg) -> bool {
    matches!(style, StyleArg::Name(name) if ["Underline", "DoubleUnderline", "Curly", "Dotted", "Dashed"].iter().any(|n| n.eq_ignore_ascii_case(name)))
}

/// The RGB value of a hex code as Convert-HexToRGB reads it: gray, with its warning, for a code
/// that is not six hex digits. -Silent leaves the warning, which Convert-HexToRGB writes.
pub fn hex_rgb(ps: &Pipeline<'_>, hex: &str) -> PsResult<[i64; 3]> {
    match colors::hex_to_rgb(hex) {
        Some(rgb) => Ok(rgb.map(i64::from)),
        None => {
            ps.warning(&format!(
                "Invalid hex color format: {}. Expected format: #RRGGBB or RRGGBB",
                colors::strip_hex_prefix(hex)
            ))?;
            Ok([128, 128, 128])
        }
    }
}

/// Writes colored and styled text to the host, with optional padding, alignment and logging to a
/// file.
///
/// Write-ColorEX writes text the way Write-Host does, with more color, style and layout options:
/// the console's 16 colors, ANSI 16 and 256 colors and 24-bit TrueColor; color names, hex codes,
/// rgb() and hsl() forms, RGB arrays and ANSI color numbers; gradients for the text and the
/// background; markup tags such as [bold red]...[/]; text split into parts and colored by
/// pattern; links; padding, truncating and wrapping to a display width that counts wide
/// characters as 2 cells; styles; reusable style profiles; indentation, centering, blank lines and
/// timestamps; and logging to a file. Each line goes to the host in as few Write-Host calls as the
/// colors allow, with the line end on the last call, so a transcript records each line once.
///
/// # Examples
/// Write-ColorEX -Text 'Hello World' -Color Green
/// Write-ColorEX -Text 'Error: ', 'File not found' -Color Red, Yellow -Bold
/// Write-ColorEX -Text '[bold red]Error:[/] file not found' -Markup
/// Write-ColorEX -Text 'RAINBOW' -Gradient @('Red', 'Orange', 'Yellow', 'Green', 'Cyan', 'Blue', 'Magenta')
/// 'first', 'second' | Write-ColorEX -Color Green
#[cmdlet(verb = "Write", noun = "ColorEX", alias = ["Write-ColourEX", "Write-Color", "Write-Colour", "WC", "WCEX", "wcolor", "wcolour"])]
#[derive(Default)]
#[allow(non_snake_case)]
pub struct WriteColorEX {
    /// The text to write. Several strings are written on one line, each with its own colors and
    /// styles. Strings piped in are written one line each.
    #[param(position = 0, value_from_pipeline, alias = ["T"])]
    pub text: Vec<String>,
    /// The foreground color of each text segment: a color name, a hex code, rgb() or hsl(), an
    /// RGB array, or an ANSI color number. With fewer colors than segments the colors repeat.
    #[param(position = 1, alias = ["C", "ForegroundColor", "FGC"], clr = "System.Array")]
    pub color: Option<PsObject>,
    /// The background color of each text segment, in the same forms as -Color.
    #[param(position = 2, alias = ["B", "BGC"], clr = "System.Array")]
    pub back_ground_color: Option<PsObject>,
    /// Two or more colors to blend across the characters of the text.
    #[param(position = 3, alias = ["Grad"])]
    pub gradient: Option<Vec<PsObject>>,
    /// Uses ANSI 4-bit colors (16 colors).
    #[param(alias = ["A4"])]
    pub ANSI4: bool,
    /// Uses ANSI 8-bit colors (256 colors).
    #[param(alias = ["A8"])]
    pub ANSI8: bool,
    /// Uses 24-bit TrueColor (RGB).
    #[param(alias = ["A24", "TrueColor", "TC"])]
    pub ANSI24: bool,
    /// Styles for the text segments in order: one style per segment, or an array of styles for a
    /// segment that takes several.
    #[param(position = 4, alias = ["S"])]
    pub style: Option<PsObject>,
    /// A PSColorStyle object holding colors, styles and layout settings.
    #[param(position = 5, clr = "PSColorStyle")]
    pub style_profile: Option<PsObject>,
    /// Applies the default style set with Set-ColorDefault.
    #[param]
    pub default: bool,
    /// Writes the text in bold.
    #[param]
    pub bold: bool,
    /// Writes the text faint (dimmed).
    #[param]
    pub faint: bool,
    /// Writes the text in italics.
    #[param]
    pub italic: bool,
    /// Underlines the text.
    #[param]
    pub underline: bool,
    /// Makes the text blink, where the terminal supports it.
    #[param]
    pub blink: bool,
    /// Strikes the text through.
    #[param(alias = ["Strikethrough"])]
    pub crossed_out: bool,
    /// Underlines the text twice, where the terminal supports it.
    #[param]
    pub double_underline: bool,
    /// Draws a line above the text, where the terminal supports it.
    #[param]
    pub overline: bool,
    /// The number of tab characters before the text.
    #[param(position = 6, alias = ["Indent"])]
    pub start_tab: i32,
    /// The number of blank lines before the text.
    #[param(position = 7)]
    pub lines_before: i32,
    /// The number of blank lines after the text.
    #[param(position = 8)]
    pub lines_after: i32,
    /// The number of spaces before the text, after any tabs from -StartTab.
    #[param(position = 9)]
    pub start_spaces: i32,
    /// Writes the text to this log file as well.
    #[param(position = 10, alias = ["L"])]
    pub log_file: String,
    /// The folder for a -LogFile given as a file name alone.
    #[param(position = 11, alias = ["LP"])]
    pub log_path: String,
    /// A level written in brackets before the text in the log file.
    #[param(position = 12, alias = ["LL", "LogLvl"])]
    pub log_level: String,
    /// Writes a timestamp in brackets before the text in the log file.
    #[param(alias = ["LT"])]
    pub log_time: bool,
    /// The .NET date and time format for -LogTime and -ShowTime.
    #[param(position = 13, alias = ["DateFormat", "TimeFormat", "Timestamp", "TS"])]
    pub date_time_format: Option<String>,
    /// How many times to try writing the log file, 50 ms apart, when the file is locked.
    #[param(position = 14)]
    pub log_retry: Option<i32>,
    /// The text encoding of the log file.
    #[param(position = 15, validate_set = ["unknown", "string", "unicode", "bigendianunicode", "utf8", "utf8BOM", "utf8NoBOM", "utf7", "utf32", "bigendianutf32", "ascii", "ansi", "default", "oem"])]
    pub encoding: Option<String>,
    /// Writes the time in brackets, in dark gray, before the text on the console.
    #[param]
    pub show_time: bool,
    /// Leaves the line open, so the next output continues it.
    #[param]
    pub no_new_line: bool,
    /// Centers the text in the console window.
    #[param(alias = ["Center"])]
    pub horizontal_center: bool,
    /// Writes a line of spaces as wide as the console window, which -BackGroundColor colors.
    #[param(alias = ["BL", "Empty", "Blank"])]
    pub blank_line: bool,
    /// Writes nothing to the host, only to the log file.
    #[param(alias = ["HideConsole", "NoConsole", "LogOnly", "LO"])]
    pub no_console_output: bool,
    /// Writes [DEBUG] messages about color processing and detection.
    #[param]
    pub debugging: bool,
    /// Suppresses the warnings about colors and fallbacks.
    #[param]
    pub silent: bool,
    /// Pads the text to this display width.
    #[param(position = 16, alias = ["PadWidth", "Pad"])]
    pub auto_pad: i32,
    /// With -AutoPad, pads on the left (right-aligns the text) instead of the right.
    #[param(alias = ["RightAlign"])]
    pub pad_left: bool,
    /// The character -AutoPad pads with.
    #[param(position = 17, alias = ["PaddingChar", "FillChar"])]
    pub pad_char: Option<char>,
    /// Reads tags in the text that color and style part of it: [bold red]text[/]. A tag holds
    /// style names, a color, 'on' and a background color, and link=URL; [/] closes the tag opened
    /// last, and [[ and ]] write [ and ].
    #[param]
    pub markup: bool,
    /// Cuts each string after each of these separators, which stay with the part before them,
    /// so each part takes the next color.
    #[param(position = 18)]
    pub split: Option<Vec<String>>,
    /// Cuts each string before and after each of these separators, which become parts of their
    /// own.
    #[param(position = 19)]
    pub split_around: Option<Vec<String>>,
    /// Cuts each string into as many parts as there are colors, of display characters as equal
    /// as they can be.
    #[param]
    pub split_evenly: bool,
    /// Colors and styles the text regular expressions match: each key a pattern, matched without
    /// regard to case, and each value a style as a markup tag writes it.
    #[param(position = 20, clr = "System.Collections.IDictionary")]
    pub highlight: Option<PsObject>,
    /// The address each text segment links to, where the terminal opens links when clicked.
    #[param(position = 21)]
    pub link: Option<Vec<String>>,
    /// Swaps the text and background colors.
    #[param(alias = ["Invert"])]
    pub reverse: bool,
    /// Two or more colors to blend across the background of the text.
    #[param(position = 22, alias = ["BGGrad"])]
    pub back_ground_gradient: Option<Vec<PsObject>>,
    /// How the gradients blend their colors: OKLab, where equal steps look equally far apart, or
    /// RGB, each channel on its own.
    #[param(position = 23, validate_set = ["OKLab", "RGB"])]
    pub gradient_space: Option<String>,
    /// The color of the underline of each text segment, in the same forms as -Color. A segment
    /// with no other underline is underlined.
    #[param(position = 24, clr = "System.Array")]
    pub underline_color: Option<PsObject>,
    /// The kind of underline: Single, Double, Curly, Dotted or Dashed.
    #[param(position = 25, validate_set = ["Single", "Double", "Curly", "Dotted", "Dashed"])]
    pub underline_style: Option<String>,
    /// With -AutoPad, cuts text wider than -AutoPad, ending it with an ellipsis.
    #[param]
    pub truncate: bool,
    /// With -AutoPad, centers the text.
    #[param]
    pub pad_center: bool,
    /// Breaks text wider than the line into lines at spaces, each with the indentation.
    #[param]
    pub wrap: bool,
    /// -Highlight's patterns, compiled once for every line.
    highlight_entries: Vec<HighlightEntry>,
}

/// The parameter names of Write-ColorEX, for which ones a call gave.
const PARAMETER_NAMES: &[&str] = &[
    "Text", "Color", "BackGroundColor", "Gradient", "ANSI4", "ANSI8", "ANSI24", "Style", "StyleProfile", "Default", "Bold", "Faint", "Italic", "Underline",
    "Blink", "CrossedOut", "DoubleUnderline", "Overline", "StartTab", "LinesBefore", "LinesAfter", "StartSpaces", "LogFile", "LogPath", "LogLevel",
    "LogTime", "DateTimeFormat", "LogRetry", "Encoding", "ShowTime", "NoNewLine", "HorizontalCenter", "BlankLine", "NoConsoleOutput", "Debugging",
    "Silent", "AutoPad", "PadLeft", "PadChar", "Markup", "Split", "SplitAround", "SplitEvenly", "Highlight", "Link", "Reverse", "BackGroundGradient",
    "GradientSpace", "UnderlineColor", "UnderlineStyle", "Truncate", "PadCenter", "Wrap",
];

/// The checks Write-ColorEX makes before its first string: each entry of -Color,
/// -BackGroundColor and -UnderlineColor is a color, and one way of splitting is given. Answers
/// -Highlight's patterns compiled.
pub fn begin_checks(ps: &Pipeline<'_>, colors: [(Option<&PsObject>, &str); 3], split_evenly: bool, highlight: Option<&PsObject>, silent: bool) -> PsResult<Vec<HighlightEntry>> {
    for (value, parameter) in colors {
        color_parameter(value, parameter)?;
    }
    let split_count = usize::from(ps.parameter_is_bound("Split")) + usize::from(ps.parameter_is_bound("SplitAround")) + usize::from(split_evenly);
    if split_count > 1 {
        return Err(PsError::new(ErrorCategory::InvalidArgument, "SplitConflict", "Use only one of -Split, -SplitAround and -SplitEvenly.").terminating());
    }
    match highlight {
        Some(table) if !table.is_null() => compile_highlight(ps, table, silent),
        _ => Ok(Vec::new()),
    }
}

impl Cmdlet for WriteColorEX {
    fn begin(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        self.highlight_entries = begin_checks(
            ps,
            [
                (self.color.as_ref(), "Color"),
                (self.back_ground_color.as_ref(), "BackGroundColor"),
                (self.underline_color.as_ref(), "UnderlineColor"),
            ],
            self.split_evenly,
            self.highlight.as_ref(),
            self.silent,
        )?;
        Ok(())
    }

    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let mut bound = HashSet::new();
        for name in PARAMETER_NAMES {
            if ps.parameter_is_bound(name) {
                bound.insert(name.to_ascii_lowercase());
            }
        }
        let options = WriteOptions {
            text: self.text.clone(),
            color: color_parameter(self.color.as_ref(), "Color")?,
            background: color_parameter(self.back_ground_color.as_ref(), "BackGroundColor")?,
            gradient: match &self.gradient {
                Some(values) => Some(gradient_list(values)?),
                None => None,
            },
            background_gradient: match &self.back_ground_gradient {
                Some(values) => Some(gradient_list(values)?),
                None => None,
            },
            gradient_space: self.gradient_space.clone().unwrap_or_else(|| "OKLab".to_string()),
            ansi4: self.ANSI4,
            ansi8: self.ANSI8,
            ansi24: self.ANSI24,
            style: style_argument(self.style.as_ref())?,
            style_profile: self.style_profile.clone().filter(|p| !p.is_null()),
            default: self.default,
            bold: self.bold,
            faint: self.faint,
            italic: self.italic,
            underline: self.underline,
            blink: self.blink,
            crossed_out: self.crossed_out,
            double_underline: self.double_underline,
            overline: self.overline,
            reverse: self.reverse,
            underline_color: color_parameter(self.underline_color.as_ref(), "UnderlineColor")?,
            underline_style: self.underline_style.clone().unwrap_or_default(),
            markup: self.markup,
            split: self.split.clone(),
            split_around: self.split_around.clone(),
            split_evenly: self.split_evenly,
            link: self.link.clone().unwrap_or_default().into_iter().map(Some).collect(),
            start_tab: self.start_tab,
            lines_before: self.lines_before,
            lines_after: self.lines_after,
            start_spaces: self.start_spaces,
            log_file: self.log_file.clone(),
            log_path: self.log_path.clone(),
            log_level: self.log_level.clone(),
            log_time: self.log_time,
            date_time_format: self.date_time_format.clone().unwrap_or_else(|| "yyyy-MM-dd HH:mm:ss".to_string()),
            log_retry: self.log_retry.unwrap_or(2),
            encoding: self.encoding.clone().unwrap_or_else(|| "utf8".to_string()),
            show_time: self.show_time,
            no_new_line: self.no_new_line,
            horizontal_center: self.horizontal_center,
            blank_line: self.blank_line,
            no_console_output: self.no_console_output,
            debugging: self.debugging,
            silent: self.silent,
            auto_pad: self.auto_pad,
            pad_left: self.pad_left,
            pad_center: self.pad_center,
            pad_char: self.pad_char.unwrap_or(' '),
            truncate: self.truncate,
            wrap: self.wrap,
            bound,
            caller_script_root: caller_script_root(ps),
        };
        render(ps, options, &self.highlight_entries, None)
    }
}

/// -Style read: one style alone is the first segment's, as an array of one.
pub fn style_argument(value: Option<&PsObject>) -> PsResult<Option<StyleArg>> {
    Ok(match value {
        Some(obj) => match StyleArg::from_object(obj)? {
            Some(StyleArg::Name(name)) => Some(StyleArg::List(vec![StyleArg::Name(name)])),
            other => other,
        },
        None => None,
    })
}

/// -Color, -BackGroundColor or -UnderlineColor read and checked: none when left out or given as
/// $null.
pub fn color_parameter(value: Option<&PsObject>, parameter: &str) -> PsResult<Vec<ColorArg>> {
    match value {
        Some(array) if !array.is_null() => color_list(&Vec::<PsObject>::from_ps(array)?, parameter),
        _ => Ok(Vec::new()),
    }
}

/// The folder of the script that called the command, or nothing at the prompt.
pub fn caller_script_root(ps: &Pipeline<'_>) -> String {
    ps.invocation()
        .and_then(|i| i.get("PSScriptRoot"))
        .and_then(|r| if r.is_null() { Ok(String::new()) } else { String::from_ps(&r) })
        .unwrap_or_default()
}
