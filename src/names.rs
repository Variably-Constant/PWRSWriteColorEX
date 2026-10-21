//! Color names: Register-ColorName and Unregister-ColorName, which add names to the color table
//! and remove them, Show-ColorTable, which writes every name with samples, and the tab completion
//! of color names and profile names, as PSWriteColorEX's.

use pwrs::prelude::*;

use crate::colors::{self, lookup, Entry};
use crate::forms::color_form;
use crate::gradient::{named_gradient, NAMED_GRADIENTS};
use crate::oklab;
use crate::runs::{markup_style, MARKUP_STYLES};
use crate::write::{render, splat_refusal, ColorArg, WriteOptions};

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

/// Registers a name for a color, with the errors Register-ColorName writes; returns whether it
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

/// Writes the color names with a sample of each in every color mode, or a view of the text
/// styles, the 256 colors or the style profiles.
///
/// Show-ColorTable writes one line for each name in the color table, registered names among
/// them: the name, a sample of the color in TrueColor, in 256 colors, in 16 colors and as a
/// console color, then its hex code, its 256-color number and its console color. The names go by
/// family, each family's Dark, normal and Light variants together. A terminal without a mode shows
/// the nearest color it has in that mode's column.
///
/// -Styles writes each text style with a sample in it, the Write-ColorEX parameter that gives it
/// and the markup tags that give it. -Palette writes the 256-color chart, each number on its own
/// color in black or white, whichever has the higher contrast: the terminal's 16 colors, the
/// 6x6x6 color cube as six blocks of 6x6, and the 24 grays; on the 16 colors, which the terminal's
/// scheme sets, the numbers 0-6 and 8 are white and the others black. -Profiles writes each style
/// profile with a sample in its colors and styles, then its settings in words, the default style
/// marked; layout settings such as AutoPad are listed, not applied, and a color Write-ColorEX does
/// not take, such as 5.5, gives Write-ColorEX's error in place of the sample.
///
/// # Examples
/// Show-ColorTable
/// Show-ColorTable *Blue*, *Cyan* -Background
/// Show-ColorTable -Styles
/// Show-ColorTable -Palette
/// Show-ColorTable -Profiles
#[cmdlet(verb = "Show", noun = "ColorTable", default_parameter_set = "Names", alias = ["Show-ColourTable"])]
#[derive(Default)]
pub struct ShowColorTable {
    /// The names to show, or with -Profiles the profiles. Wildcards are allowed. Every name when
    /// left out.
    #[param(position = 0, set = ["Names", "Profiles"])]
    pub name: Option<Vec<String>>,
    /// Shows each sample as a background behind spaces rather than as blocks of text color.
    #[param(set = "Names")]
    pub background: bool,
    /// Writes the text styles instead of the color names.
    #[param(mandatory, set = "Styles")]
    pub styles: bool,
    /// Writes the 256-color chart instead of the color names.
    #[param(mandatory, set = "Palette")]
    pub palette: bool,
    /// Writes the style profiles instead of the color names.
    #[param(mandatory, set = "Profiles")]
    pub profiles: bool,
}

/// Text padded with spaces to a width in UTF-16 units, as .PadRight pads it.
fn pad_right(text: &str, width: usize) -> String {
    format!("{text}{}", " ".repeat(width.saturating_sub(text.encode_utf16().count())))
}

/// One Write-ColorEX call of a view: text alone, or with the options a closure sets.
fn write_line(ps: &Pipeline<'_>, text: String, no_new_line: bool, set: impl FnOnce(&mut WriteOptions)) -> PsResult<()> {
    let mut options = WriteOptions { text: vec![text], no_new_line, ..WriteOptions::default() };
    set(&mut options);
    render(ps, options, &[], None)
}

/// Show-ColorTable -Styles: each style, a sample in it, its parameter and its markup tags, the
/// tags wrapping under the markup column to stay within 80 columns.
fn show_styles(ps: &Pipeline<'_>) -> PsResult<()> {
    type Set = fn(&mut WriteOptions);
    let rows: [(&str, Set, &str); 12] = [
        ("Bold", |o| o.bold = true, "-Bold"),
        ("Faint", |o| o.faint = true, "-Faint"),
        ("Italic", |o| o.italic = true, "-Italic"),
        ("Underline", |o| o.underline = true, "-Underline"),
        ("DoubleUnderline", |o| o.double_underline = true, "-DoubleUnderline"),
        ("Curly", |o| o.underline_style = "Curly".to_string(), "-UnderlineStyle Curly"),
        ("Dotted", |o| o.underline_style = "Dotted".to_string(), "-UnderlineStyle Dotted"),
        ("Dashed", |o| o.underline_style = "Dashed".to_string(), "-UnderlineStyle Dashed"),
        ("Blink", |o| o.blink = true, "-Blink"),
        ("Reverse", |o| o.reverse = true, "-Reverse"),
        ("CrossedOut", |o| o.crossed_out = true, "-CrossedOut"),
        ("Overline", |o| o.overline = true, "-Overline"),
    ];
    const MARKUP_COLUMN: usize = 17 + 13 + 24;
    write_line(ps, format!("{}{}{}Markup", pad_right("Style", 17), pad_right("Sample", 13), pad_right("Parameter", 24)), false, |o| o.bold = true)?;
    for (name, set, parameter) in rows {
        // The style's own name first, then its other tags in ordinal order
        let own = name.to_lowercase();
        let mut others: Vec<&str> = MARKUP_STYLES.iter().filter(|(word, style)| *style == name && *word != own).map(|(word, _)| *word).collect();
        others.sort_unstable();
        let mut tag_lines: Vec<String> = Vec::new();
        let mut tag_line = String::new();
        for tag in std::iter::once(own.as_str()).chain(others) {
            let word = format!("[{tag}]");
            if !tag_line.is_empty() && MARKUP_COLUMN + tag_line.len() + 1 + word.len() > 80 {
                tag_lines.push(std::mem::replace(&mut tag_line, word));
            } else if !tag_line.is_empty() {
                tag_line.push(' ');
                tag_line.push_str(&word);
            } else {
                tag_line = word;
            }
        }
        tag_lines.push(tag_line);
        write_line(ps, pad_right(name, 17), true, |_| {})?;
        write_line(ps, "Sample text".to_string(), true, set)?;
        write_line(ps, format!("  {}{}", pad_right(parameter, 24), tag_lines[0]), false, |_| {})?;
        for line in &tag_lines[1..] {
            write_line(ps, format!("{}{line}", " ".repeat(MARKUP_COLUMN)), false, |_| {})?;
        }
    }
    Ok(())
}

/// The 256-color code of the number Show-ColorTable -Palette writes on a code: 16, black, or 231,
/// white, whichever has the higher contrast on the code's color. On the 16 colors, which the
/// terminal's scheme sets, white on 0-6 and 8 and black on the others.
fn palette_text_color(code: i64) -> i64 {
    if code < 16 {
        return if code <= 6 || code == 8 { 231 } else { 16 };
    }
    if oklab::black_text_reads(oklab::ansi8_rgb(code)) { 16 } else { 231 }
}

/// Show-ColorTable -Palette: the 16 colors in two rows of 8, the cube as six 6x6 blocks in two
/// bands of three, and the 24 grays in two rows of 12, a blank line after each part but the last.
fn show_palette(ps: &Pipeline<'_>) -> PsResult<()> {
    // Each row's cells, None for the gap between two blocks, and whether a blank line follows
    let mut rows: Vec<(Vec<Option<i64>>, bool)> = vec![((0..8).map(Some).collect(), false), ((8..16).map(Some).collect(), true)];
    for band in 0..2i64 {
        for r in 0..6i64 {
            let mut cells = Vec::new();
            for block in 0..3i64 {
                if block > 0 {
                    cells.push(None);
                }
                let start = 16 + (band * 3 + block) * 36 + r * 6;
                cells.extend((start..start + 6).map(Some));
            }
            rows.push((cells, r == 5));
        }
    }
    rows.push(((232..244).map(Some).collect(), false));
    rows.push(((244..256).map(Some).collect(), false));
    for (cells, blank) in rows {
        let mut options = WriteOptions { ansi8: true, silent: true, lines_after: i32::from(blank), ..WriteOptions::default() };
        for cell in cells {
            match cell {
                Some(code) => {
                    options.text.push(format!("{code:>3} "));
                    options.color.push(ColorArg::Number(palette_text_color(code)));
                    options.background.push(ColorArg::Number(code));
                }
                None => {
                    options.text.push("  ".to_string());
                    options.color.push(ColorArg::Null);
                    options.background.push(ColorArg::Null);
                }
            }
        }
        render(ps, options, &[], None)?;
    }
    Ok(())
}

/// Whether a value is a whole number of 32 or 64 bits, as PowerShell's -is [int] and -is [long]
/// test it.
fn is_whole(value: &PsObject) -> PsResult<bool> {
    use pwrs::sys::*;
    Ok(!value.is_null() && matches!(value.type_tag()?, PS_TYPE_I32 | PS_TYPE_I64))
}

/// A color setting as Show-ColorTable -Profiles writes it: a name, hex code or number as it is, an
/// RGB array as @(r, g, b), and a list of colors with commas between them.
fn setting_text(value: &PsObject) -> PsResult<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    if value.type_name()?.ends_with("[]") {
        let items = Vec::<PsObject>::from_ps(value)?;
        if items.len() == 3 && items.iter().map(is_whole).collect::<PsResult<Vec<_>>>()?.into_iter().all(|whole| whole) {
            let numbers = items.iter().map(|item| i64::from_ps(item).map(|n| n.to_string())).collect::<PsResult<Vec<_>>>()?;
            return Ok(format!("@({})", numbers.join(", ")));
        }
        return Ok(items.iter().map(setting_text).collect::<PsResult<Vec<_>>>()?.join(", "));
    }
    if is_whole(value)? {
        return Ok(i64::from_ps(value)?.to_string());
    }
    String::from_ps(value)
}

/// A profile's settings in words, as Show-ColorTable -Profiles writes them: its colors ("White on
/// DarkRed"), then each style and layout setting it has by property name, then "(the default
/// style)" for the default style; "no settings" for a profile with none.
fn style_summary(style: &PsObject) -> PsResult<String> {
    let flag = |name: &str| -> PsResult<bool> { bool::from_ps(&style.get(name)?) };
    let number = |name: &str| -> PsResult<i64> { i64::from_ps(&style.get(name)?) };
    let text = |name: &str| -> PsResult<String> {
        let value = style.get(name)?;
        if value.is_null() { Ok(String::new()) } else { String::from_ps(&value) }
    };
    let mut parts: Vec<String> = Vec::new();
    let foreground = setting_text(&style.get("ForegroundColor")?)?;
    let background = setting_text(&style.get("BackgroundColor")?)?;
    match (foreground.is_empty(), background.is_empty()) {
        (false, false) => parts.push(format!("{foreground} on {background}")),
        (false, true) => parts.push(foreground),
        (true, false) => parts.push(format!("on {background}")),
        (true, true) => {}
    }
    for property in ["Bold", "Faint", "Italic", "Underline", "DoubleUnderline", "Blink", "Reverse", "CrossedOut", "Overline"] {
        if flag(property)? {
            parts.push(property.to_string());
        }
    }
    let underline_style = text("UnderlineStyle")?;
    if !underline_style.is_empty() {
        parts.push(format!("UnderlineStyle {underline_style}"));
    }
    let underline_color = style.get("UnderlineColor")?;
    if !underline_color.is_null() {
        parts.push(format!("UnderlineColor {}", setting_text(&underline_color)?));
    }
    let styles = style.get("Style")?;
    if !styles.is_null() {
        let names = Vec::<String>::from_ps(&styles)?;
        if !names.is_empty() {
            parts.push(format!("Style {}", names.join(", ")));
        }
    }
    for property in ["Gradient", "BackgroundGradient"] {
        let value = style.get(property)?;
        if !value.is_null() && !Vec::<PsObject>::from_ps(&value)?.is_empty() {
            parts.push(format!("{property} {}", setting_text(&value)?));
        }
    }
    for property in ["GradientSpace", "GradientHue", "GradientEasing", "GradientRepeat", "GradientDirection"] {
        let value = text(property)?;
        if !value.is_empty() {
            parts.push(format!("{property} {value}"));
        }
    }
    for property in ["GradientSteps", "StartTab", "StartSpaces", "LinesBefore", "LinesAfter", "AutoPad"] {
        let value = number(property)?;
        if value > 0 {
            parts.push(format!("{property} {value}"));
        }
    }
    for property in ["ShowTime", "NoNewLine", "HorizontalCenter", "PadLeft", "PadCenter", "Truncate", "Wrap"] {
        if flag(property)? {
            parts.push(property.to_string());
        }
    }
    let pad_char = char::from_ps(&style.get("PadChar")?)?;
    if pad_char != ' ' {
        parts.push(format!("PadChar '{pad_char}'"));
    }
    let joined = parts.join(", ");
    let default = PsType::from_name("PSColorStyle").call_static("get_Default", &[])?;
    let is_default = !default.is_null() && bool::from_ps(&PsType::from_name("System.Object").call_static("ReferenceEquals", &[style.clone(), default])?)?;
    if is_default {
        return Ok(format!("{joined} (the default style)").trim_start().to_string());
    }
    if parts.is_empty() {
        return Ok("no settings".to_string());
    }
    Ok(joined)
}

/// Show-ColorTable -Profiles: each profile the patterns match, in the order of their names in
/// upper case, with a sample in its colors and styles and its settings in words.
fn show_profiles(ps: &Pipeline<'_>, patterns: &[String]) -> PsResult<()> {
    let profiles = PsType::from_name("PSColorStyle").call_static("get_Profiles", &[])?;
    let keys = if profiles.is_null() { Vec::new() } else { Vec::<String>::from_ps(&profiles.get("Keys")?)? };
    let names = upper_order(keys.into_iter().filter(|name| like_any(name, patterns)).collect());
    if names.is_empty() {
        return Ok(());
    }
    let width = names.iter().map(|name| name.encode_utf16().count()).max().unwrap_or(0).max(7);
    write_line(ps, format!("{}  {}Settings", pad_right("Profile", width), pad_right("Sample", 13)), false, |o| o.bold = true)?;
    // The sample takes the profile's colors and styles, not its layout
    const LAYOUT: [&str; 13] =
        ["starttab", "startspaces", "linesbefore", "linesafter", "showtime", "nonewline", "horizontalcenter", "autopad", "padleft", "padchar", "padcenter", "truncate", "wrap"];
    for name in &names {
        let style = profiles.call("get_Item", &[name.as_str().into_ps()?])?;
        write_line(ps, format!("{}  ", pad_right(name, width)), true, |_| {})?;
        let params = crate::style::write_color_params(&style)?;
        // A value Write-ColorEX does not take gives its error in place of the sample, written as
        // one that does not stop the view, and the other profiles follow
        if let Some(refusal) = splat_refusal(&params)? {
            ps.write_error(&refusal)?;
        } else {
            let mut sample = WriteOptions { text: vec!["Sample text".to_string()], silent: true, no_new_line: true, ..WriteOptions::default() };
            for key in params.keys()? {
                let lower = key.to_ascii_lowercase();
                if !LAYOUT.contains(&lower.as_str())
                    && let Some(refusal) = sample.set_parameter(&lower, &params.get(&key)?)?
                {
                    ps.write_error(&refusal)?;
                }
            }
            render(ps, sample, &[], None)?;
        }
        write_line(ps, format!("  {}", style_summary(&style)?), false, |_| {})?;
    }
    Ok(())
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
        if self.styles {
            return show_styles(ps);
        }
        if self.palette {
            return show_palette(ps);
        }
        if self.profiles {
            return show_profiles(ps, &patterns);
        }
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

/// Every named gradient that starts with the text typed, in the table's order, each with its
/// colors, then every color name that starts with it and names no gradient.
pub fn gradient_completions(word: &str) -> Vec<Completion> {
    let pattern = format!("{}*", completion_word(word));
    let mut out: Vec<Completion> = NAMED_GRADIENTS
        .iter()
        .filter(|(name, _)| like(name, &pattern))
        .map(|(name, stops)| Completion::value(*name).with_tooltip(format!("{name}  {}", stops.join(" "))))
        .collect();
    out.extend(color_name_completions(word).into_iter().filter(|completion| named_gradient(&completion.text).is_none()));
    out
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
    FormatColorEXColor => "Format-ColorEX" / "Color",
    FormatColorEXBackGroundColor => "Format-ColorEX" / "BackGroundColor",
    FormatColorEXUnderlineColor => "Format-ColorEX" / "UnderlineColor",
    NewColorStyleForegroundColor => "New-ColorStyle" / "ForegroundColor",
    NewColorStyleBackgroundColor => "New-ColorStyle" / "BackgroundColor",
    NewColorStyleUnderlineColor => "New-ColorStyle" / "UnderlineColor",
    SetColorDefaultForegroundColor => "Set-ColorDefault" / "ForegroundColor",
    SetColorDefaultBackgroundColor => "Set-ColorDefault" / "BackgroundColor",
    RegisterColorNameColor => "Register-ColorName" / "Color",
    ShowColorTableName => "Show-ColorTable" / "Name",
    GetColorContrastColor => "Get-ColorContrast" / "Color",
    GetColorContrastBackGroundColor => "Get-ColorContrast" / "BackGroundColor",
}

macro_rules! gradient_completers {
    ($($name:ident => $cmdlet:tt / $parameter:tt),* $(,)?) => {
        $(
            #[completer(cmdlet = $cmdlet, parameter = $parameter)]
            pub fn $name(ctx: &CompletionContext) -> PsResult<Vec<Completion>> {
                Ok(gradient_completions(&ctx.word))
            }
        )*
    };
}

gradient_completers! {
    WriteColorEXGradient => "Write-ColorEX" / "Gradient",
    WriteColorEXBackGroundGradient => "Write-ColorEX" / "BackGroundGradient",
    FormatColorEXGradient => "Format-ColorEX" / "Gradient",
    FormatColorEXBackGroundGradient => "Format-ColorEX" / "BackGroundGradient",
    NewColorStyleGradient => "New-ColorStyle" / "Gradient",
    NewColorStyleBackgroundGradient => "New-ColorStyle" / "BackgroundGradient",
    GetGradientColorGradient => "Get-GradientColor" / "Gradient",
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
