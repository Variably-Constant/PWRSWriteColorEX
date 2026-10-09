//! Color names: Register-ColorName and Unregister-ColorName, which add names to the color table
//! and remove them, Show-ColorTable, which writes every name with samples, and the tab completion
//! of color names and profile names, as PSWriteColorEX's.

use pwrs::prelude::*;

use crate::colors::{self, lookup, Entry};
use crate::forms::color_form;
use crate::runs::markup_style;
use crate::write::{render, ColorArg, WriteOptions};

/// Whether a name matches a PowerShell wildcard pattern, without regard to case: * for any run
/// of characters, ? for one, [abc] and [a-c] for one of a set, and ` before a character for the
/// character itself. A pattern with an unclosed [ matches nothing.
pub fn like(name: &str, pattern: &str) -> bool {
    let name: Vec<char> = name.chars().flat_map(char::to_lowercase).collect();
    let pattern: Vec<char> = pattern.chars().flat_map(char::to_lowercase).collect();
    matches_at(&name, &pattern)
}

fn matches_at(name: &[char], pattern: &[char]) -> bool {
    let Some((&first, rest)) = pattern.split_first() else { return name.is_empty() };
    match first {
        '*' => (0..=name.len()).any(|skip| matches_at(&name[skip..], rest)),
        '?' => !name.is_empty() && matches_at(&name[1..], rest),
        '[' => {
            let Some(close) = rest.iter().position(|&c| c == ']') else { return false };
            let (set, after) = (&rest[..close], &rest[close + 1..]);
            let Some((&c, name_rest)) = name.split_first() else { return false };
            let mut found = false;
            let mut i = 0;
            while i < set.len() {
                if i + 2 < set.len() && set[i + 1] == '-' {
                    found |= set[i] <= c && c <= set[i + 2];
                    i += 3;
                } else {
                    found |= set[i] == c;
                    i += 1;
                }
            }
            found && matches_at(name_rest, after)
        }
        '`' if !rest.is_empty() => name.first() == Some(&rest[0]) && matches_at(&name[1..], &rest[1..]),
        c => name.first() == Some(&c) && matches_at(&name[1..], rest),
    }
}

/// Whether a name matches any of the patterns.
pub fn like_any(name: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| like(name, pattern))
}

/// Color names in family order: by the name without Dark or Light, then Dark, the name, Light.
/// Names compare by their letters in upper case, ordinal, so the order is the same everywhere.
pub fn name_order(mut names: Vec<String>) -> Vec<String> {
    let key = |name: &str| -> Vec<u16> {
        let lower = name.to_ascii_lowercase();
        let (variant, family) = if lower.starts_with("dark") && name.len() > 4 {
            (0, &name[4..])
        } else if lower.starts_with("light") && name.len() > 5 {
            (2, &name[5..])
        } else {
            (1, name)
        };
        format!("{}|{}|{}", family.to_uppercase(), variant, name.to_uppercase()).encode_utf16().collect()
    };
    names.sort_by_cached_key(|name| key(name));
    names
}

/// Names in the order of their letters in upper case, ordinal.
pub fn upper_order(mut names: Vec<String>) -> Vec<String> {
    names.sort_by_cached_key(|name| name.to_uppercase().encode_utf16().collect::<Vec<u16>>());
    names
}

/// The RGB color Register-ColorName takes a value as, or None for a value that is not a color: a
/// hex code, rgb(), hsl(), three whole numbers 0-255, or a name in the color table.
pub fn register_rgb(value: &PsObject, warn: &mut dyn FnMut(String)) -> PsResult<Option<[u8; 3]>> {
    use pwrs::sys::*;
    if value.is_null() {
        return Ok(None);
    }
    let name = value.type_name()?;
    if name.ends_with("[]") {
        let items = Vec::<PsObject>::from_ps(value)?;
        if items.len() != 3 {
            return Ok(None);
        }
        let mut rgb = [0u8; 3];
        for (slot, item) in rgb.iter_mut().zip(&items) {
            if item.is_null() || !matches!(item.type_tag()?, PS_TYPE_I32 | PS_TYPE_I64 | PS_TYPE_I16 | PS_TYPE_U8) {
                return Ok(None);
            }
            match u8::try_from(i64::from_ps(item)?) {
                Ok(channel) => *slot = channel,
                Err(_) => return Ok(None),
            }
        }
        return Ok(Some(rgb));
    }
    if value.type_tag()? != PS_TYPE_STRING {
        return Ok(None);
    }
    let text = String::from_ps(value)?;
    if let Some(entry) = lookup(&text) {
        return Ok(Some(entry.rgb));
    }
    let form = color_form(&text, warn);
    if !crate::forms::is_hex6(&form) {
        return Ok(None);
    }
    Ok(colors::hex_to_rgb(&form))
}

/// Whether text can be a color name: letters, digits, '-' and '_', starting with a letter, and
/// not None, on or a style name markup reads.
fn valid_name(name: &str) -> bool {
    let text = name.strip_suffix('\n').unwrap_or(name);
    let mut chars = text.chars();
    let shaped = chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    shaped && !name.eq_ignore_ascii_case("None") && !name.eq_ignore_ascii_case("on") && markup_style(name).is_none()
}

/// Registers a name for a color, with the errors Register-ColorName writes; answers whether it
/// did.
pub fn register_name(ps: &Pipeline<'_>, name: &str, color: &PsObject, force: bool) -> PsResult<bool> {
    if !valid_name(name) {
        ps.write_error(
            &PsError::new(
                ErrorCategory::InvalidArgument,
                "InvalidColorName",
                format!("'{name}' cannot be a color name. A name holds letters, digits, '-' and '_', starts with a letter, and is not None, on or a style name."),
            )
            .with_target(name.into_ps()?),
        )?;
        return Ok(false);
    }
    if lookup(name).is_some() && !force {
        ps.write_error(
            &PsError::new(ErrorCategory::ResourceExists, "ColorNameInUse", format!("The color name '{name}' is in use. Use -Force to replace it."))
                .with_target(name.into_ps()?),
        )?;
        return Ok(false);
    }
    let mut warnings = Vec::new();
    let rgb = register_rgb(color, &mut |message| warnings.push(message))?;
    for message in &warnings {
        ps.warning(message)?;
    }
    let Some(rgb) = rgb else {
        let shown = match ColorArg::from_object(color)? {
            Some(ColorArg::List(items)) => format!("@({})", items.iter().map(ColorArg::display).collect::<Vec<_>>().join(", ")),
            _ if color.is_null() => String::new(),
            _ => String::from_ps(color).unwrap_or_default(),
        };
        ps.write_error(
            &PsError::new(
                ErrorCategory::InvalidArgument,
                "InvalidColor",
                format!("'{shown}' is not a color. A color is a hex code, rgb(), hsl(), an RGB array of three numbers 0-255, or a name in the color table."),
            )
            .with_target(color.clone()),
        )?;
        return Ok(false);
    };
    colors::register(name, Entry::from_rgb(rgb));
    Ok(true)
}

/// Adds a color name to the color table, for every command of the module.
///
/// Register-ColorName names a color, so -Color, -BackGroundColor, -UnderlineColor, the gradients,
/// markup tags and -Highlight styles take the name as they take the built-in names. The name
/// keeps the color in every color mode: the 256-color and 16-color codes and the console color
/// nearest it. A name holds letters, digits, '-' and '_', starting with a letter. It cannot be
/// None, on, or a style name markup reads, such as bold. A name in use, built-in or registered, is
/// replaced only with -Force; Unregister-ColorName brings a replaced built-in name back. The names
/// last for the session. Export-ColorProfile saves them with the style profiles, and
/// Import-ColorProfile registers them again.
///
/// # Examples
/// Register-ColorName -Name Brand -Color '#FF6B35'
/// Register-ColorName Orange 'hsl(30, 100%, 50%)' -Force
#[cmdlet(verb = "Register", noun = "ColorName", alias = ["Register-ColourName"])]
#[derive(Default)]
pub struct RegisterColorName {
    /// The name for the color.
    #[param(mandatory, position = 0)]
    pub name: String,
    /// The color: a hex code ('#FF8000', '#F80'), 'rgb(255, 128, 0)', 'hsl(30, 100%, 50%)', an RGB
    /// array @(255, 128, 0), or a name already in the color table.
    #[param(mandatory, position = 1)]
    pub color: PsObject,
    /// Replaces a name that is in use.
    #[param]
    pub force: bool,
}

impl Cmdlet for RegisterColorName {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        register_name(ps, &self.name, &self.color, self.force)?;
        Ok(())
    }
}

/// Removes color names Register-ColorName added.
///
/// A built-in name that Register-ColorName replaced takes its built-in color again. A name that
/// was not registered gives an error.
///
/// # Examples
/// Unregister-ColorName -Name Brand
#[cmdlet(verb = "Unregister", noun = "ColorName", alias = ["Unregister-ColourName"])]
#[derive(Default)]
pub struct UnregisterColorName {
    /// The names to remove.
    #[param(mandatory, position = 0, value_from_pipeline)]
    pub name: Vec<String>,
}

impl Cmdlet for UnregisterColorName {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        for name in &self.name {
            if !colors::unregister(name) {
                ps.write_error(
                    &PsError::new(ErrorCategory::ObjectNotFound, "ColorNameNotRegistered", format!("No color name '{name}' was registered."))
                        .with_target(name.as_str().into_ps()?),
                )?;
            }
        }
        Ok(())
    }
}

/// Writes the color names with a sample of each in every color mode.
///
/// Show-ColorTable writes one line for each name in the color table, registered names among
/// them: the name, a sample of the color in TrueColor, in 256 colors, in 16 colors and as a
/// console color, then its hex code, its 256-color number and its console color. The names go by
/// family, each family's Dark, normal and Light variants together. A terminal without a mode shows
/// the nearest color it has in that mode's column.
///
/// # Examples
/// Show-ColorTable
/// Show-ColorTable *Blue*, *Cyan* -Background
#[cmdlet(verb = "Show", noun = "ColorTable", alias = ["Show-ColourTable"])]
#[derive(Default)]
pub struct ShowColorTable {
    /// The names to show. Wildcards are allowed. Every name when left out.
    #[param(position = 0)]
    pub name: Option<Vec<String>>,
    /// Shows each sample as a background behind spaces rather than as blocks of text color.
    #[param]
    pub background: bool,
}

/// One Write-ColorEX call of Show-ColorTable: text with a color, a background color or neither,
/// in a mode, warnings off for the samples.
fn show(ps: &Pipeline<'_>, text: &str, color: Option<ColorArg>, background: bool, mode: Option<&str>, no_new_line: bool) -> PsResult<()> {
    let mut options = WriteOptions { text: vec![text.to_string()], no_new_line, ..WriteOptions::default() };
    if let Some(color) = color {
        if background {
            options.background = vec![color];
        } else {
            options.color = vec![color];
        }
    }
    match mode {
        Some("TrueColor") => options.ansi24 = true,
        Some("ANSI8") => options.ansi8 = true,
        Some("ANSI4") => options.ansi4 = true,
        _ => {}
    }
    if mode.is_some() {
        options.silent = true;
    }
    render(ps, options, &[], None)
}

impl Cmdlet for ShowColorTable {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let patterns = self.name.clone().unwrap_or_else(|| vec!["*".to_string()]);
        let table = colors::table();
        let names = name_order(table.iter().filter(|(name, _)| like_any(name, &patterns)).map(|(name, _)| name.clone()).collect());
        if names.is_empty() {
            return Ok(());
        }
        let width = names.iter().map(|name| name.encode_utf16().count()).max().unwrap_or(0).max(4);
        let pad = |text: &str, width: usize| format!("{text}{}", " ".repeat(width.saturating_sub(text.encode_utf16().count())));
        let sample = if self.background { " ".repeat(6) } else { "\u{2588}".repeat(6) };
        let header = format!("{}  TrueColor  256     16      Console  Hex      ANSI8  Console color", pad("Name", width));
        render(ps, WriteOptions { text: vec![header], bold: true, ..WriteOptions::default() }, &[], None)?;
        for name in &names {
            let Some((_, entry)) = table.iter().find(|(n, _)| n == name) else { continue };
            let hex = format!("#{:02X}{:02X}{:02X}", entry.rgb[0], entry.rgb[1], entry.rgb[2]);
            show(ps, &format!("{}  ", pad(name, width)), None, false, None, true)?;
            let bg = self.background;
            show(ps, &sample, Some(ColorArg::Name(hex.clone())), bg, Some("TrueColor"), true)?;
            show(ps, "     ", None, false, None, true)?;
            show(ps, &sample, Some(ColorArg::Number(i64::from(entry.ansi8))), bg, Some("ANSI8"), true)?;
            show(ps, "  ", None, false, None, true)?;
            show(ps, &sample, Some(ColorArg::Name(name.clone())), bg, Some("ANSI4"), true)?;
            show(ps, "  ", None, false, None, true)?;
            show(ps, &sample, Some(ColorArg::Name(entry.native.to_string())), bg, None, true)?;
            let ansi8 = entry.ansi8.to_string();
            show(ps, &format!("   {hex}  {}{ansi8}  {}", " ".repeat(5usize.saturating_sub(ansi8.len())), entry.native), None, false, None, false)?;
        }
        Ok(())
    }
}

/// The text typed so far, without an opening quote.
fn completion_word(word: &str) -> &str {
    word.trim_start_matches(['\'', '"'])
}

/// Every color name that starts with the text typed, in family order, each with its hex code.
pub fn color_name_completions(word: &str) -> Vec<Completion> {
    let pattern = format!("{}*", completion_word(word));
    let table = colors::table();
    let names = name_order(table.iter().filter(|(name, _)| like(name, &pattern)).map(|(name, _)| name.clone()).collect());
    names
        .into_iter()
        .filter_map(|name| {
            let (_, entry) = table.iter().find(|(n, _)| *n == name)?;
            let tip = format!("{name}  #{:02X}{:02X}{:02X}", entry.rgb[0], entry.rgb[1], entry.rgb[2]);
            Some(Completion::value(name).with_tooltip(tip))
        })
        .collect()
}

/// Every registered color name that starts with the text typed.
fn registered_name_completions(word: &str) -> Vec<Completion> {
    let pattern = format!("{}*", completion_word(word));
    let names = name_order(colors::registered().into_iter().map(|(name, _)| name).filter(|name| like(name, &pattern)).collect());
    names.into_iter().map(Completion::value).collect()
}

/// Every profile name that starts with the text typed.
fn profile_name_completions(word: &str) -> PsResult<Vec<Completion>> {
    let pattern = format!("{}*", completion_word(word));
    let profiles = PsType::from_name("PSColorStyle").call_static("get_Profiles", &[])?;
    if profiles.is_null() {
        return Ok(Vec::new());
    }
    let names = Vec::<String>::from_ps(&profiles.get("Keys")?)?;
    Ok(upper_order(names.into_iter().filter(|name| like(name, &pattern)).collect()).into_iter().map(Completion::value).collect())
}

macro_rules! color_completers {
    ($($name:ident => $cmdlet:tt / $parameter:tt),* $(,)?) => {
        $(
            #[completer(cmdlet = $cmdlet, parameter = $parameter)]
            pub fn $name(ctx: &CompletionContext) -> PsResult<Vec<Completion>> {
                Ok(color_name_completions(&ctx.word))
            }
        )*
    };
}

color_completers! {
    WriteColorEXColor => "Write-ColorEX" / "Color",
    WriteColorEXBackGroundColor => "Write-ColorEX" / "BackGroundColor",
    WriteColorEXUnderlineColor => "Write-ColorEX" / "UnderlineColor",
    WriteColorEXGradient => "Write-ColorEX" / "Gradient",
    WriteColorEXBackGroundGradient => "Write-ColorEX" / "BackGroundGradient",
    FormatColorEXColor => "Format-ColorEX" / "Color",
    FormatColorEXBackGroundColor => "Format-ColorEX" / "BackGroundColor",
    FormatColorEXUnderlineColor => "Format-ColorEX" / "UnderlineColor",
    FormatColorEXGradient => "Format-ColorEX" / "Gradient",
    FormatColorEXBackGroundGradient => "Format-ColorEX" / "BackGroundGradient",
    NewColorStyleForegroundColor => "New-ColorStyle" / "ForegroundColor",
    NewColorStyleBackgroundColor => "New-ColorStyle" / "BackgroundColor",
    NewColorStyleUnderlineColor => "New-ColorStyle" / "UnderlineColor",
    NewColorStyleGradient => "New-ColorStyle" / "Gradient",
    NewColorStyleBackgroundGradient => "New-ColorStyle" / "BackgroundGradient",
    SetColorDefaultForegroundColor => "Set-ColorDefault" / "ForegroundColor",
    SetColorDefaultBackgroundColor => "Set-ColorDefault" / "BackgroundColor",
    RegisterColorNameColor => "Register-ColorName" / "Color",
    ShowColorTableName => "Show-ColorTable" / "Name",
}

#[completer(cmdlet = "Unregister-ColorName", parameter = "Name")]
pub fn UnregisterColorNameName(ctx: &CompletionContext) -> PsResult<Vec<Completion>> {
    Ok(registered_name_completions(&ctx.word))
}

macro_rules! profile_completers {
    ($($name:ident => $cmdlet:tt),* $(,)?) => {
        $(
            #[completer(cmdlet = $cmdlet, parameter = "Name")]
            pub fn $name(ctx: &CompletionContext) -> PsResult<Vec<Completion>> {
                profile_name_completions(&ctx.word)
            }
        )*
    };
}

profile_completers! {
    GetColorProfilesName => "Get-ColorProfiles",
    ExportColorProfileName => "Export-ColorProfile",
    RemoveColorProfileName => "Remove-ColorProfile",
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards() {
        assert!(like("DarkBlue", "*blue*"));
        assert!(like("Blue", "b?ue"));
        assert!(like("Cyan", "[a-c]yan"));
        assert!(!like("Cyan", "[d-z]yan"));
        assert!(like("a*b", "a`*b"));
        assert!(!like("ab", "a`*b"));
        assert!(!like("x", "[x"));
        assert!(like("", "*"));
    }

    #[test]
    fn family_order() {
        let names = vec!["LightRed".to_string(), "Blue".to_string(), "Red".to_string(), "DarkRed".to_string(), "DarkBlue".to_string()];
        assert_eq!(name_order(names), vec!["DarkBlue", "Blue", "DarkRed", "Red", "LightRed"]);
    }

    #[test]
    fn names() {
        assert!(valid_name("Brand-1_x"));
        assert!(!valid_name("1Brand"));
        assert!(!valid_name("none"));
        assert!(!valid_name("ON"));
        assert!(!valid_name("bold"));
        assert!(!valid_name("dim"));
        assert!(!valid_name("a b"));
    }
}
