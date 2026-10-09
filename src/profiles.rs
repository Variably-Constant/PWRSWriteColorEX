//! Style profile files: Export-ColorProfile saves style profiles and registered color names as
//! JSON, Import-ColorProfile reads them back, and Remove-ColorProfile removes profiles, as
//! PSWriteColorEX's. The JSON is written here, the same bytes as PSWriteColorEX writes, rather
//! than by ConvertTo-Json, whose layout differs between Windows PowerShell 5.1 and PowerShell 7.

use pwrs::prelude::*;

use crate::colors;
use crate::host::this_cmdlet;
use crate::names::{like_any, register_name, upper_order};

/// The PSColorStyle properties a profile file holds, in the order it holds them.
const PROPERTIES: [&str; 30] = [
    "ForegroundColor",
    "BackgroundColor",
    "Gradient",
    "BackgroundGradient",
    "GradientSpace",
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
                    members.push((property.to_string(), Json::of(&value)?));
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

/// Reads style profiles and color names from a file Export-ColorProfile wrote.
///
/// Import-ColorProfile registers the color names in the file, replacing names in use, and adds
/// each profile to [PSColorStyle]::Profiles, replacing a profile of the same name. The profile the
/// file marks as the default becomes the default style.
///
/// # Examples
/// Import-ColorProfile -Path ./styles.json
/// Import-ColorProfile ./styles.json -PassThru | Format-Table Name, ForegroundColor
#[cmdlet(verb = "Import", noun = "ColorProfile", alias = ["Import-ColourProfile"], output = ["PSColorStyle"])]
#[derive(Default)]
pub struct ImportColorProfile {
    /// The file to read.
    #[param(mandatory, position = 0)]
    pub path: String,
    /// Writes each profile read to the pipeline.
    #[param]
    pub pass_thru: bool,
}

impl Cmdlet for ImportColorProfile {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let file = session_path(ps)?.call("GetUnresolvedProviderPathFromPSPath", &[self.path.as_str().into_ps()?])?;
        let read = (|| -> PsResult<PsObject> {
            let text = PsType::from_name("System.IO.File").call_static("ReadAllText", std::slice::from_ref(&file))?;
            let parsed = ps.invoke("ConvertFrom-Json", &[("InputObject", text), ("ErrorAction", "Stop".into_ps()?)])?;
            Ok(parsed.into_iter().next().unwrap_or_default())
        })();
        let document = match read {
            Ok(document) => document,
            Err(e) => {
                return Err(PsError::new(ErrorCategory::InvalidData, "InvalidProfileFile", format!("Cannot read the profiles in '{}'. {}", self.path, e.message))
                    .with_target(self.path.as_str().into_ps()?)
                    .terminating());
            }
        };
        let properties = json_properties(&document)?;
        let member = |name: &str| properties.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());

        if let Some(registered) = member("Colors").filter(|c| !c.is_null()) {
            for (name, value) in json_properties(&registered)? {
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
            if self.pass_thru {
                ps.write_object(&style)?;
            }
        }
        Ok(())
    }
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
