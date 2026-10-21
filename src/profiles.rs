//! Style profile files: Export-ColorProfile saves style profiles and registered color names as
//! JSON, Import-ColorProfile reads them back, and Remove-ColorProfile removes profiles, as
//! PSWriteColorEX's. The JSON is written here, the same bytes as PSWriteColorEX writes, rather
//! than by ConvertTo-Json, whose layout differs between Windows PowerShell 5.1 and PowerShell 7.

use pwrs::prelude::*;

use crate::colors;
use crate::host::this_cmdlet;
use crate::names::{like_any, register_name, upper_order};
use crate::oklab;
use crate::style::new_style;

/// The PSColorStyle properties a profile file holds, in the order it holds them.
const PROPERTIES: [&str; 35] = [
    "ForegroundColor",
    "BackgroundColor",
    "Gradient",
    "BackgroundGradient",
    "GradientSpace",
    "GradientHue",
    "GradientEasing",
    "GradientRepeat",
    "GradientDirection",
    "GradientSteps",
    "Style",
    "Bold",
    "Italic",
    "Underline",
    "Blink",
    "Faint",
    "CrossedOut",
    "DoubleUnderline",
    "Overline",
    "Reverse",
    "UnderlineColor",
    "UnderlineStyle",
    "StartTab",
    "StartSpaces",
    "LinesBefore",
    "LinesAfter",
    "ShowTime",
    "NoNewLine",
    "HorizontalCenter",
    "AutoPad",
    "PadLeft",
    "PadCenter",
    "PadChar",
    "Truncate",
    "Wrap",
];

/// A value written as JSON.
enum Json {
    Null,
    Bool(bool),
    Integer(i64),
    Text(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// A .NET value as JSON: whole numbers of 32 and 64 bits as numbers, strings and characters
    /// as strings, arrays and lists as arrays, and anything else as its text.
    fn of(value: &PsObject) -> PsResult<Json> {
        use pwrs::sys::*;
        if value.is_null() {
            return Ok(Json::Null);
        }
        Ok(match value.type_tag()? {
            PS_TYPE_BOOL => Json::Bool(bool::from_ps(value)?),
            PS_TYPE_I32 | PS_TYPE_I64 => Json::Integer(i64::from_ps(value)?),
            PS_TYPE_STRING | PS_TYPE_CHAR => Json::Text(String::from_ps(value)?),
            PS_TYPE_OBJECT if is_list(value)? => Json::Array(Vec::<PsObject>::from_ps(value)?.iter().map(Json::of).collect::<PsResult<_>>()?),
            _ => Json::Text(String::from_ps(value)?),
        })
    }

    fn write(&self, out: &mut String, depth: usize) {
        let indent = "  ".repeat(depth + 1);
        let close = "  ".repeat(depth);
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Integer(n) => out.push_str(&n.to_string()),
            Json::Text(text) => write_string(out, text),
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Array(items) if items.iter().all(|item| matches!(item, Json::Integer(_))) => {
                // A list of numbers, such as an RGB color, stays on one line
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.write(out, depth);
                }
                out.push(']');
            }
            Json::Array(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    out.push_str(&indent);
                    item.write(out, depth + 1);
                }
                out.push('\n');
                out.push_str(&close);
                out.push(']');
            }
            Json::Object(members) if members.is_empty() => out.push_str("{}"),
            Json::Object(members) => {
                out.push_str("{\n");
                for (i, (key, value)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    out.push_str(&indent);
                    write_string(out, key);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                }
                out.push('\n');
                out.push_str(&close);
                out.push('}');
            }
        }
    }
}

/// A string as JSON, with ", \ and the control characters escaped.
fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Whether an object is an array or a list.
fn is_list(obj: &PsObject) -> PsResult<bool> {
    let name = obj.type_name()?;
    Ok(name.ends_with("[]") || name.starts_with("System.Collections.ArrayList") || name.starts_with("System.Collections.Generic.List"))
}

/// The class the profiles are kept in, from src/csharp/PSColorStyle.cs.
fn style_type() -> PsType {
    PsType::from_name("PSColorStyle")
}

/// The session's path methods, $ExecutionContext.SessionState.Path.
fn session_path(ps: &Pipeline<'_>) -> PsResult<PsObject> {
    this_cmdlet(ps).get("SessionState")?.get("Path")
}

/// Whether two values are one object.
fn same_object(a: &PsObject, b: &PsObject) -> PsResult<bool> {
    bool::from_ps(&PsType::from_name("System.Object").call_static("ReferenceEquals", &[a.clone(), b.clone()])?)
}

/// Whether a property's value is the value a new style has, which a profile file leaves out.
fn is_blank_value(value: &PsObject, blank: &PsObject) -> PsResult<bool> {
    if value.type_name()?.ends_with("[]") {
        return Ok(Vec::<PsObject>::from_ps(value)?.is_empty());
    }
    if blank.is_null() {
        return Ok(false);
    }
    let comparer = PsType::from_name("System.Management.Automation.LanguagePrimitives");
    bool::from_ps(&comparer.call_static("Equals", &[value.clone(), blank.clone(), true.into_ps()?])?)
}

/// Saves style profiles and registered color names to a JSON file.
///
/// Export-ColorProfile writes the profiles named, with every property that differs from a new
/// style's, and every color name Register-ColorName added, as JSON in UTF-8. Import-ColorProfile
/// reads the file back, in this session or another, in Windows PowerShell 5.1 or PowerShell 7.
/// The default style, when it is among the profiles written, is marked as the default.
///
/// # Examples
/// Export-ColorProfile -Path ./styles.json
/// Export-ColorProfile ./alerts.json -Name Alert*
#[cmdlet(verb = "Export", noun = "ColorProfile", alias = ["Export-ColourProfile"])]
#[derive(Default)]
pub struct ExportColorProfile {
    /// The file to write.
    #[param(mandatory, position = 0)]
    pub path: String,
    /// The names of the profiles to write. Wildcards are allowed. Every profile when left out.
    #[param]
    pub name: Option<Vec<String>>,
}

impl Cmdlet for ExportColorProfile {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let patterns = self.name.clone().unwrap_or_else(|| vec!["*".to_string()]);
        let profiles = style_type().call_static("get_Profiles", &[])?;
        let keys = if profiles.is_null() { Vec::new() } else { Vec::<String>::from_ps(&profiles.get("Keys")?)? };
        let names = upper_order(keys.into_iter().filter(|name| like_any(name, &patterns)).collect());
        let blank = style_type().new(&[])?;
        let default = style_type().call_static("get_Default", &[])?;

        let registered = colors::registered()
            .into_iter()
            .map(|(name, entry)| (name, Json::Text(format!("#{:02X}{:02X}{:02X}", entry.rgb[0], entry.rgb[1], entry.rgb[2]))))
            .collect();
        let mut default_name: Option<String> = None;
        let mut entries = Vec::with_capacity(names.len());
        for name in &names {
            let style = profiles.call("get_Item", &[name.as_str().into_ps()?])?;
            if !default.is_null() && same_object(&style, &default)? {
                default_name = Some(String::from_ps(&style.get("Name")?)?);
            }
            let style_name = style.get("Name")?;
            let mut members = vec![("Name".to_string(), Json::of(&style_name)?)];
            for property in PROPERTIES {
                let value = style.get(property)?;
                if property == "ForegroundColor" || property == "BackgroundColor" {
                    members.push((property.to_string(), Json::of(&value)?));
                    continue;
                }
                if !value.is_null() && !is_blank_value(&value, &blank.get(property)?)? {
                    // A list of one item is written as that item, as PSWriteColorEX writes it
                    let items = if is_list(&value)? { Vec::<PsObject>::from_ps(&value)? } else { Vec::new() };
                    let written = if let [item] = items.as_slice() { Json::of(item)? } else { Json::of(&value)? };
                    members.push((property.to_string(), written));
                }
            }
            entries.push(Json::Object(members));
        }
        let mut document = vec![("Colors".to_string(), Json::Object(registered))];
        if let Some(name) = default_name {
            document.push(("Default".to_string(), Json::Text(name)));
        }
        document.push(("Profiles".to_string(), Json::Array(entries)));

        let mut text = String::new();
        Json::Object(document).write(&mut text, 0);
        text.push('\n');
        let file = session_path(ps)?.call("GetUnresolvedProviderPathFromPSPath", &[self.path.as_str().into_ps()?])?;
        let encoding = PsType::from_name("System.Text.UTF8Encoding").new(&[false.into_ps()?])?;
        PsType::from_name("System.IO.File").call_static("WriteAllText", &[file, text.into_ps()?, encoding])?;
        Ok(())
    }
}

/// A value ConvertFrom-Json read, with whole numbers as Int32 as Write-ColorEX takes them, and
/// arrays as object arrays. PowerShell 7 reads whole numbers as Int64.
fn json_value(value: &PsObject) -> PsResult<PsObject> {
    use pwrs::sys::*;
    if value.is_null() {
        return Ok(PsObject::null());
    }
    if value.type_tag()? == PS_TYPE_I64 {
        let n = i64::from_ps(value)?;
        if let Ok(small) = i32::try_from(n) {
            return small.into_ps();
        }
        return Ok(value.clone());
    }
    if value.type_name()?.ends_with("[]") {
        let items = Vec::<PsObject>::from_ps(value)?.iter().map(json_value).collect::<PsResult<Vec<_>>>()?;
        return PsArray(items).into_ps();
    }
    Ok(value.clone())
}

/// The properties of an object ConvertFrom-Json made, by name, in order.
fn json_properties(value: &PsObject) -> PsResult<Vec<(String, PsObject)>> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for property in Vec::<PsObject>::from_ps(&value.get("PSObject")?.get("Properties")?)? {
        out.push((String::from_ps(&property.get("Name")?)?, property.get("Value")?));
    }
    Ok(out)
}

/// The themes Import-ColorProfile -Theme reads, each in Export-ColorProfile's format: copies of the
/// files in PSWriteColorEX's Themes folder, which tests/Parity.Tests.ps1 holds them to byte for
/// byte.
const THEMES: [(&str, &str); 13] = [
    ("CatppuccinFrappe", include_str!("../themes/CatppuccinFrappe.json")),
    ("CatppuccinLatte", include_str!("../themes/CatppuccinLatte.json")),
    ("CatppuccinMacchiato", include_str!("../themes/CatppuccinMacchiato.json")),
    ("CatppuccinMocha", include_str!("../themes/CatppuccinMocha.json")),
    ("CSS", include_str!("../themes/CSS.json")),
    ("Dracula", include_str!("../themes/Dracula.json")),
    ("GruvboxDark", include_str!("../themes/GruvboxDark.json")),
    ("GruvboxLight", include_str!("../themes/GruvboxLight.json")),
    ("Nord", include_str!("../themes/Nord.json")),
    ("OneDark", include_str!("../themes/OneDark.json")),
    ("SolarizedDark", include_str!("../themes/SolarizedDark.json")),
    ("SolarizedLight", include_str!("../themes/SolarizedLight.json")),
    ("TokyoNight", include_str!("../themes/TokyoNight.json")),
];

/// Reads style profiles and color names from a file Export-ColorProfile wrote, or from a theme the
/// module ships.
///
/// Import-ColorProfile registers the color names in the file, replacing names in use, and adds
/// each profile to [PSColorStyle]::Profiles, replacing a profile of the same name. The profile the
/// file marks as the default becomes the default style.
///
/// -Theme reads one of the module's themes instead. A color theme registers its colors under names
/// that start with the theme's name, such as DraculaPurple or Nord8 (SolarizedDark and
/// SolarizedLight both use names that start with Solarized), and restyles the profiles Default,
/// Error, Warning, Info, Success, Critical and Debug in its colors, with Default the default style.
/// CSS adds the CSS color names the color table lacks, such as Tomato, SlateGray and RebeccaPurple;
/// a name already in the table keeps its color, and Grey, DarkGrey and LightGrey take the colors of
/// Gray, DarkGray and LightGray. PowerShell restyles Error, Warning, Info and Debug as
/// PowerShell's own error, warning, verbose and debug output, and in PowerShell 7.2 and later
/// Critical and Default as its error and formatting accents, with Default the default style. Each
/// takes the text color, background and styles of PowerShell's setting and nothing else; a console
/// color in it is written as the terminal's own color, as PowerShell writes it.
///
/// # Examples
/// Import-ColorProfile -Path ./styles.json
/// Import-ColorProfile ./styles.json -PassThru | Format-Table Name, ForegroundColor
/// Import-ColorProfile -Theme Dracula
/// Import-ColorProfile -Theme PowerShell
/// Import-ColorProfile -Theme CSS
#[cmdlet(verb = "Import", noun = "ColorProfile", default_parameter_set = "Path", alias = ["Import-ColourProfile"], output = ["PSColorStyle"])]
#[derive(Default)]
pub struct ImportColorProfile {
    /// The file to read.
    #[param(mandatory, position = 0, set = "Path")]
    pub path: String,
    /// The theme to read: CatppuccinFrappe, CatppuccinLatte, CatppuccinMacchiato, CatppuccinMocha,
    /// CSS, Dracula, GruvboxDark, GruvboxLight, Nord, OneDark, PowerShell, SolarizedDark,
    /// SolarizedLight or TokyoNight.
    #[param(
        mandatory,
        set = "Theme",
        validate_set = [
            "CatppuccinFrappe", "CatppuccinLatte", "CatppuccinMacchiato", "CatppuccinMocha", "CSS", "Dracula", "GruvboxDark",
            "GruvboxLight", "Nord", "OneDark", "PowerShell", "SolarizedDark", "SolarizedLight", "TokyoNight"
        ]
    )]
    pub theme: String,
    /// Writes each profile read to the pipeline.
    #[param]
    pub pass_thru: bool,
}

/// The object ConvertFrom-Json reads from JSON text. Text that holds no value makes it write no
/// object, which PSWriteColorEX reads as $null, a document with no names or profiles.
fn parse_json(ps: &Pipeline<'_>, text: PsObject) -> PsResult<PsObject> {
    let parsed = ps.invoke("ConvertFrom-Json", &[("InputObject", text), ("ErrorAction", "Stop".into_ps()?)])?;
    match parsed.into_iter().next() {
        Some(document) => Ok(document),
        None => Ok(PsObject::null()),
    }
}

impl Cmdlet for ImportColorProfile {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        if self.theme.eq_ignore_ascii_case("PowerShell") {
            return import_powershell_theme(ps, self.pass_thru);
        }
        let (source, read) = if self.theme.is_empty() {
            let file = session_path(ps)?.call("GetUnresolvedProviderPathFromPSPath", &[self.path.as_str().into_ps()?])?;
            let read = (|| -> PsResult<PsObject> {
                let text = PsType::from_name("System.IO.File").call_static("ReadAllText", std::slice::from_ref(&file))?;
                parse_json(ps, text)
            })();
            (self.path.clone(), read)
        } else {
            let Some((name, text)) = THEMES.iter().find(|(name, _)| name.eq_ignore_ascii_case(&self.theme)) else {
                return Err(PsError::new(ErrorCategory::ObjectNotFound, "ThemeFileNotFound", format!("The module has no file for the theme '{}'.", self.theme))
                    .with_target(self.theme.as_str().into_ps()?)
                    .terminating());
            };
            ((*name).to_string(), text.into_ps().and_then(|text| parse_json(ps, text)))
        };
        let document = match read {
            Ok(document) => document,
            Err(e) => {
                return Err(PsError::new(ErrorCategory::InvalidData, "InvalidProfileFile", format!("Cannot read the profiles in '{source}'. {}", e.message))
                    .with_target(source.as_str().into_ps()?)
                    .terminating());
            }
        };
        // The CSS theme adds only the names the color table lacks
        import_document(ps, &document, self.theme.eq_ignore_ascii_case("CSS"), self.pass_thru)
    }
}

/// Registers the color names of a profile document and adds its profiles, as Import-ColorProfile
/// reads a file; with `skip_names_in_use`, a name already in the color table is left as it is.
fn import_document(ps: &Pipeline<'_>, document: &PsObject, skip_names_in_use: bool, pass_thru: bool) -> PsResult<()> {
    let properties = json_properties(document)?;
    let member = |name: &str| properties.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());

    if let Some(registered) = member("Colors").filter(|c| !c.is_null()) {
        for (name, value) in json_properties(&registered)? {
            if skip_names_in_use && colors::lookup(&name).is_some() {
                continue;
            }
            register_name(ps, &name, &json_value(&value)?, true)?;
        }
    }
    let default_name = match member("Default") {
        Some(name) if !name.is_null() => String::from_ps(&name)?,
        _ => String::new(),
    };
    let entries = match member("Profiles") {
        Some(list) if list.is_null() => Vec::new(),
        Some(list) if list.type_name()?.ends_with("[]") => Vec::<PsObject>::from_ps(&list)?,
        Some(single) => vec![single],
        None => Vec::new(),
    };
    for entry in entries {
        if entry.is_null() {
            continue;
        }
        let fields = json_properties(&entry)?;
        let field = |name: &str| fields.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());
        let name = match field("Name") {
            Some(name) if !name.is_null() => String::from_ps(&name)?,
            _ => String::new(),
        };
        if name.is_empty() {
            continue;
        }
        let style = style_type().new(&[name.as_str().into_ps()?, PsObject::null(), PsObject::null()])?;
        for property in PROPERTIES {
            if let Some(value) = field(property) {
                style.set(property, &json_value(&value)?)?;
            }
        }
        style.call("AddToProfiles", &[])?;
        if !default_name.is_empty() && default_name.to_lowercase() == name.to_lowercase() {
            style.call("SetAsDefault", &[])?;
        }
        if pass_thru {
            ps.write_object(&style)?;
        }
    }
    Ok(())
}

/// The names of the console colors by their ANSI color number 0-15.
const CONSOLE_COLORS: [&str; 16] = [
    "Black", "DarkRed", "DarkGreen", "DarkYellow", "DarkBlue", "DarkMagenta", "DarkCyan", "Gray", "DarkGray", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan",
    "White",
];

/// The colors and styles a string of SGR escape codes sets, as PSColorStyle property values, and
/// the codes and text no property holds.
#[derive(Debug, Default, PartialEq)]
struct SgrStyle {
    foreground: Option<String>,
    background: Option<String>,
    underline_color: Option<String>,
    bold: bool,
    faint: bool,
    italic: bool,
    underline: bool,
    double_underline: bool,
    underline_style: String,
    blink: bool,
    reverse: bool,
    crossed_out: bool,
    overline: bool,
    ignored: Vec<String>,
}

/// The parameters of each SGR escape code in a string, ESC [ digits, ';' and ':' m, and the string
/// with the codes taken out.
fn sgr_codes(text: &str) -> (Vec<&str>, String) {
    let bytes = text.as_bytes();
    let mut codes = Vec::new();
    let mut rest = String::new();
    let mut kept_from = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'[') {
            let mut end = i + 2;
            while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b';' || bytes[end] == b':') {
                end += 1;
            }
            if bytes.get(end) == Some(&b'm') {
                rest.push_str(&text[kept_from..i]);
                codes.push(&text[i + 2..end]);
                i = end + 1;
                kept_from = i;
                continue;
            }
        }
        i += 1;
    }
    rest.push_str(&text[kept_from..]);
    (codes, rest)
}

/// The color of a 256-color number (kind 5) or an RGB color (kind 2, the last three values) in an
/// escape code: a console color name for the numbers 0-15, otherwise #RRGGBB. None for values that
/// are not a color, which the caller reports among the codes no profile holds.
fn sgr_color(kind: &str, values: &[&str]) -> Option<String> {
    let channel = |value: &str| value.parse::<i64>().ok().filter(|n| (0..=255).contains(n));
    match kind {
        "5" if !values.is_empty() => {
            let number = channel(values[0])?;
            if number < 16 {
                return Some(CONSOLE_COLORS[number as usize].to_string());
            }
            let rgb = oklab::ansi8_rgb(number);
            Some(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]))
        }
        "2" if values.len() >= 3 => {
            let last = &values[values.len() - 3..];
            Some(format!("#{:02X}{:02X}{:02X}", channel(last[0])?, channel(last[1])?, channel(last[2])?))
        }
        _ => None,
    }
}

/// The colors and styles a string of SGR escape codes sets, as PSWriteColorEX's ConvertFrom-SgrStyle
/// reads them. A 16-color code, or a 256-color number 0-15, is its console color name, which
/// Write-ColorEX writes without a mode switch as that same code; a 256-color number 16-255 or an RGB
/// color is #RRGGBB. The codes apply in order, a later one over an earlier one, and 0 clears them
/// all. A code that is not a number, or not one a profile holds, goes into `ignored`. An underline
/// color is kept only with an underline, since Write-ColorEX underlines a segment that has an
/// underline color.
fn read_sgr(sequence: &str) -> SgrStyle {
    let mut style = SgrStyle::default();
    if sequence.is_empty() {
        return style;
    }
    let (codes, rest) = sgr_codes(sequence);
    for code_text in codes {
        let parameters: Vec<&str> = code_text.split(';').collect();
        let mut i = 0;
        while i < parameters.len() {
            let parameter = parameters[i];
            let parts: Vec<&str> = parameter.split(':').collect();
            let code = if parts[0].is_empty() {
                0
            } else {
                match parts[0].parse::<i32>() {
                    Ok(code) => code,
                    Err(_) => {
                        style.ignored.push(parameter.to_string());
                        i += 1;
                        continue;
                    }
                }
            };
            let mut underline: Option<&str> = None;
            match code {
                0 => {
                    let ignored = std::mem::take(&mut style.ignored);
                    style = SgrStyle { ignored, ..SgrStyle::default() };
                }
                1 => style.bold = true,
                2 => style.faint = true,
                3 => style.italic = true,
                4 => match parts.get(1).copied().unwrap_or("1") {
                    "0" => underline = Some("None"),
                    "1" => underline = Some("Single"),
                    "2" => underline = Some("Double"),
                    "3" => underline = Some("Curly"),
                    "4" => underline = Some("Dotted"),
                    "5" => underline = Some("Dashed"),
                    _ => style.ignored.push(parameter.to_string()),
                },
                5 | 6 => style.blink = true,
                7 => style.reverse = true,
                9 => style.crossed_out = true,
                21 => underline = Some("Double"),
                22 => {
                    style.bold = false;
                    style.faint = false;
                }
                23 => style.italic = false,
                24 => underline = Some("None"),
                25 => style.blink = false,
                27 => style.reverse = false,
                29 => style.crossed_out = false,
                30..=37 => style.foreground = Some(CONSOLE_COLORS[(code - 30) as usize].to_string()),
                90..=97 => style.foreground = Some(CONSOLE_COLORS[(code - 90 + 8) as usize].to_string()),
                39 => style.foreground = None,
                40..=47 => style.background = Some(CONSOLE_COLORS[(code - 40) as usize].to_string()),
                100..=107 => style.background = Some(CONSOLE_COLORS[(code - 100 + 8) as usize].to_string()),
                49 => style.background = None,
                53 => style.overline = true,
                55 => style.overline = false,
                59 => style.underline_color = None,
                38 | 48 | 58 => {
                    // 38;5;N and 38;2;R;G;B take the parameters after them; 38:5:N and 38:2:R:G:B,
                    // with or without a color space before R, hold them in one
                    let mut taken = parameter.to_string();
                    let color = if parts.len() > 1 {
                        sgr_color(parts[1], &parts[2..])
                    } else if i + 2 < parameters.len() && parameters[i + 1] == "5" {
                        taken = parameters[i..=i + 2].join(";");
                        let color = sgr_color("5", &parameters[i + 2..=i + 2]);
                        i += 2;
                        color
                    } else if i + 4 < parameters.len() && parameters[i + 1] == "2" {
                        taken = parameters[i..=i + 4].join(";");
                        let color = sgr_color("2", &parameters[i + 2..=i + 4]);
                        i += 4;
                        color
                    } else {
                        None
                    };
                    match (color, code) {
                        (None, _) => style.ignored.push(taken),
                        (Some(color), 38) => style.foreground = Some(color),
                        (Some(color), 48) => style.background = Some(color),
                        (Some(color), _) => style.underline_color = Some(color),
                    }
                }
                _ => style.ignored.push(parameter.to_string()),
            }
            if let Some(kind) = underline {
                style.underline = kind == "Single";
                style.double_underline = kind == "Double";
                style.underline_style = if matches!(kind, "Curly" | "Dotted" | "Dashed") { kind.to_string() } else { String::new() };
            }
            i += 1;
        }
    }
    if !rest.is_empty() {
        style.ignored.push(format!("'{}'", rest.replace('\u{1b}', "`e")));
    }
    if !(style.underline || style.double_underline || !style.underline_style.is_empty()) {
        style.underline_color = None;
    }
    style
}

/// A property of an object by name, its PSPropertyInfo, or null when it has none.
fn property(object: &PsObject, name: &str) -> PsResult<PsObject> {
    object.get("PSObject")?.get("Properties")?.call("get_Item", &[name.into_ps()?])
}

/// Text as a color value, null for none.
fn color_value(color: Option<String>) -> PsResult<PsObject> {
    match color {
        Some(color) => color.into_ps(),
        None => Ok(PsObject::null()),
    }
}

/// A color setting of the host as a color Write-ColorEX takes: a console color as its name, a
/// color with R, G and B properties, as the ISE's are, as #RRGGBB, and None for a transparent one
/// or none, as PSWriteColorEX's ConvertFrom-HostColor reads it.
fn host_color(ps: &Pipeline<'_>, value: &PsObject) -> PsResult<Option<String>> {
    if value.is_null() {
        return Ok(None);
    }
    if value.type_name()? == "System.ConsoleColor" {
        return Ok(Some(String::from_ps(&value.call("ToString", &[])?)?));
    }
    let (red, green, blue) = (property(value, "R")?, property(value, "G")?, property(value, "B")?);
    if !red.is_null() && !green.is_null() && !blue.is_null() {
        let alpha = property(value, "A")?;
        if !alpha.is_null() && i64::from_ps(&alpha.get("Value")?)? == 0 {
            return Ok(None);
        }
        let channel = |member: &PsObject| -> PsResult<i64> { i64::from_ps(&member.get("Value")?) };
        return Ok(Some(format!("#{:02X}{:02X}{:02X}", channel(&red)?, channel(&green)?, channel(&blue)?)));
    }
    ps.warning(&format!("The host's color '{}' is not a color a profile takes, so it is left out.", String::from_ps(&value.call("ToString", &[])?)?))?;
    Ok(None)
}

/// Import-ColorProfile -Theme PowerShell: each profile it restyles made exactly one of PowerShell's
/// own settings, from $PSStyle.Formatting where there is one (PowerShell 7.2 and later), otherwise
/// from the host's PrivateData, as PSWriteColorEX's Get-PowerShellThemeStyle makes them. A setting
/// the host lacks leaves its profile out, with a warning.
fn import_powershell_theme(ps: &Pipeline<'_>, pass_thru: bool) -> PsResult<()> {
    let cmdlet = this_cmdlet(ps);
    let ps_style = cmdlet.get("SessionState")?.get("PSVariable")?.call("GetValue", &["PSStyle".into_ps()?])?;
    let mut styles = Vec::new();
    if !ps_style.is_null() {
        let formatting = ps_style.get("Formatting")?;
        for (name, setting) in [("Error", "Error"), ("Warning", "Warning"), ("Info", "Verbose"), ("Debug", "Debug"), ("Critical", "ErrorAccent"), ("Default", "FormatAccent")] {
            let member = if formatting.is_null() { PsObject::null() } else { property(&formatting, setting)? };
            if member.is_null() {
                ps.warning(&format!("PowerShell has no {setting} setting here, so the {name} profile keeps its colors."))?;
                continue;
            }
            let value = member.get("Value")?;
            let text = if value.is_null() { String::new() } else { String::from_ps(&value)? };
            let read = read_sgr(&text);
            if !read.ignored.is_empty() {
                ps.warning(&format!("PowerShell's {setting} setting has codes no profile holds, left out of the {name} profile: {}", read.ignored.join(", ")))?;
            }
            let style = new_style(name, color_value(read.foreground)?, color_value(read.background)?)?;
            for (property, on) in [
                ("Bold", read.bold),
                ("Faint", read.faint),
                ("Italic", read.italic),
                ("Underline", read.underline),
                ("DoubleUnderline", read.double_underline),
                ("Blink", read.blink),
                ("Reverse", read.reverse),
                ("CrossedOut", read.crossed_out),
                ("Overline", read.overline),
            ] {
                style.set(property, &on.into_ps()?)?;
            }
            style.set("UnderlineStyle", &read.underline_style.into_ps()?)?;
            style.set("UnderlineColor", &color_value(read.underline_color)?)?;
            styles.push((name, style));
        }
    } else {
        let private_data = cmdlet.get("Host")?.get("PrivateData")?;
        for (name, setting) in [("Error", "Error"), ("Warning", "Warning"), ("Info", "Verbose"), ("Debug", "Debug")] {
            let (foreground, background) = if private_data.is_null() {
                (PsObject::null(), PsObject::null())
            } else {
                (property(&private_data, &format!("{setting}ForegroundColor"))?, property(&private_data, &format!("{setting}BackgroundColor"))?)
            };
            if foreground.is_null() && background.is_null() {
                ps.warning(&format!("The host has no {setting} colors, so the {name} profile keeps its colors."))?;
                continue;
            }
            let foreground_color = if foreground.is_null() { None } else { host_color(ps, &foreground.get("Value")?)? };
            let background_color = if background.is_null() { None } else { host_color(ps, &background.get("Value")?)? };
            styles.push((name, new_style(name, color_value(foreground_color)?, color_value(background_color)?)?));
        }
    }
    for (name, style) in styles {
        style.call("AddToProfiles", &[])?;
        if name == "Default" {
            style.call("SetAsDefault", &[])?;
        }
        if pass_thru {
            ps.write_object(&style)?;
        }
    }
    Ok(())
}

/// Removes style profiles from [PSColorStyle]::Profiles.
///
/// A profile that is not there gives an error. Removing a profile does not change the default
/// style.
///
/// # Examples
/// Remove-ColorProfile -Name Alert
/// Get-ColorProfiles | Where-Object Name -like 'Temp*' | ForEach-Object Name | Remove-ColorProfile
#[cmdlet(verb = "Remove", noun = "ColorProfile", alias = ["Remove-ColourProfile"], supports_should_process)]
#[derive(Default)]
pub struct RemoveColorProfile {
    /// The names of the profiles to remove.
    #[param(mandatory, position = 0, value_from_pipeline)]
    pub name: Vec<String>,
}

impl Cmdlet for RemoveColorProfile {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let profiles = style_type().call_static("get_Profiles", &[])?;
        for name in &self.name {
            let found = !profiles.is_null() && bool::from_ps(&profiles.call("ContainsKey", &[name.as_str().into_ps()?])?)?;
            if !found {
                ps.write_error(
                    &PsError::new(ErrorCategory::ObjectNotFound, "ProfileNotFound", format!("No profile named '{name}'."))
                        .with_target(name.as_str().into_ps()?),
                )?;
                continue;
            }
            if ps.should_process(name, "Remove the style profile")? {
                profiles.call("Remove", &[name.as_str().into_ps()?])?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What read_sgr reads, as one line, in the form PSWriteColorEX's tests write it.
    fn summary(sequence: &str) -> String {
        let read = read_sgr(sequence);
        let mut parts = Vec::new();
        if let Some(color) = &read.foreground {
            parts.push(format!("fg={color}"));
        }
        if let Some(color) = &read.background {
            parts.push(format!("bg={color}"));
        }
        if let Some(color) = &read.underline_color {
            parts.push(format!("ul={color}"));
        }
        for (name, on) in [
            ("Bold", read.bold),
            ("Faint", read.faint),
            ("Italic", read.italic),
            ("Underline", read.underline),
            ("DoubleUnderline", read.double_underline),
            ("Blink", read.blink),
            ("Reverse", read.reverse),
            ("CrossedOut", read.crossed_out),
            ("Overline", read.overline),
        ] {
            if on {
                parts.push(name.to_string());
            }
        }
        if !read.underline_style.is_empty() {
            parts.push(read.underline_style.clone());
        }
        if !read.ignored.is_empty() {
            parts.push(format!("ignored={}", read.ignored.join(",")));
        }
        parts.join(" ")
    }

    #[test]
    fn escape_codes_as_profile_settings() {
        for (sequence, expected) in [
            ("\u{1b}[31;1m", "fg=DarkRed Bold"),
            ("\u{1b}[91m", "fg=Red"),
            ("\u{1b}[38;5;208m", "fg=#FF8700"),
            ("\u{1b}[38;5;3m", "fg=DarkYellow"),
            ("\u{1b}[38:5:11m", "fg=Yellow"),
            ("\u{1b}[38;2;10;20;30m", "fg=#0A141E"),
            ("\u{1b}[38:2::10:20:30m", "fg=#0A141E"),
            ("\u{1b}[44m", "bg=DarkBlue"),
            ("\u{1b}[104m", "bg=Blue"),
            ("\u{1b}[48;5;240m", "bg=#585858"),
            ("\u{1b}[2;3;5;7;9;53m", "Faint Italic Blink Reverse CrossedOut Overline"),
            ("\u{1b}[4:3m", "Curly"),
            ("\u{1b}[21m", "DoubleUnderline"),
            ("\u{1b}[4;58;5;1m", "ul=DarkRed Underline"),
            ("\u{1b}[58;2;1;2;3m", ""),
            ("\u{1b}[1m\u{1b}[22m", ""),
            ("\u{1b}[31m\u{1b}[0m\u{1b}[32m", "fg=DarkGreen"),
            ("\u{1b}[31;39m", ""),
            ("\u{1b}[8m", "ignored=8"),
            ("\u{1b}[38;5;300m", "ignored=38;5;300"),
            ("\u{1b}[31m>> ", "fg=DarkRed ignored='>> '"),
            ("", ""),
        ] {
            assert_eq!(summary(sequence), expected, "{sequence:?}");
        }
    }

    #[test]
    fn every_theme_is_embedded_once() {
        assert_eq!(THEMES.len(), 13);
        for (name, text) in THEMES {
            assert!(text.starts_with("{\n  \"Colors\": {\n"), "{name}");
            assert!(text.ends_with("}\n"), "{name}");
        }
    }
}
