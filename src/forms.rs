//! Colors written in the forms #RGB, 0xRGB, rgb(r, g, b) and hsl(h, s%, l%), read as #RRGGBB,
//! and the warning for a color name the table lacks, as PSWriteColorEX's ConvertTo-ColorForm,
//! ConvertTo-ColorFormList and Test-ColorName do them. The forms are read by hand, character for
//! character as PSWriteColorEX's patterns match them.

use std::collections::HashSet;

use crate::colors::{self, lookup};
use crate::oklab;
use crate::write::ColorArg;

/// Whether a character is white space as .NET's char.IsWhiteSpace and the regular expression
/// class \s read it.
pub fn is_white_space(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// A cursor over the text of a color form.
struct Reader<'a> {
    rest: &'a str,
}

impl<'a> Reader<'a> {
    /// Skips white space; answers how many characters it skipped.
    fn spaces(&mut self) -> usize {
        let trimmed = self.rest.trim_start_matches(is_white_space);
        let skipped = self.rest[..self.rest.len() - trimmed.len()].chars().count();
        self.rest = trimmed;
        skipped
    }

    /// Takes the text given, in any letter case.
    fn word(&mut self, word: &str) -> bool {
        match self.rest.get(..word.len()) {
            Some(head) if head.eq_ignore_ascii_case(word) => {
                self.rest = &self.rest[word.len()..];
                true
            }
            _ => false,
        }
    }

    fn char(&mut self, c: char) -> bool {
        match self.rest.strip_prefix(c) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn digits(&mut self) -> &'a str {
        let end = self.rest.bytes().take_while(u8::is_ascii_digit).count();
        let (digits, rest) = self.rest.split_at(end);
        self.rest = rest;
        digits
    }

    /// An integer, with a minus sign when `signed`: -?\d+.
    fn integer(&mut self, signed: bool) -> Option<&'a str> {
        let start = self.rest;
        if signed {
            self.char('-');
        }
        if self.digits().is_empty() {
            self.rest = start;
            return None;
        }
        Some(&start[..start.len() - self.rest.len()])
    }

    /// A number with an optional fraction: -?\d+(?:\.\d+)?.
    fn number(&mut self, signed: bool) -> Option<&'a str> {
        let start = self.rest;
        self.integer(signed)?;
        if self.rest.starts_with('.') && self.rest[1..].starts_with(|c: char| c.is_ascii_digit()) {
            self.rest = &self.rest[1..];
            self.digits();
        }
        Some(&start[..start.len() - self.rest.len()])
    }

    /// What separates two values: a comma with white space around it, or white space alone.
    fn separator(&mut self) -> bool {
        let skipped = self.spaces();
        if self.char(',') {
            self.spaces();
            return true;
        }
        skipped > 0
    }

    /// The end of the form: white space, then nothing.
    fn end(&mut self) -> bool {
        self.spaces();
        self.rest.is_empty()
    }
}

/// '#RGB' or '0xRGB' with each digit doubled, in capitals.
fn short_hex(value: &str) -> Option<String> {
    let digits = if let Some(rest) = value.strip_prefix('#') {
        rest
    } else if value.get(..2).is_some_and(|prefix| prefix.eq_ignore_ascii_case("0x")) {
        &value[2..]
    } else {
        return None;
    };
    let digits = digits.strip_suffix('\n').unwrap_or(digits);
    if digits.len() != 3 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let doubled: String = digits.chars().flat_map(|c| [c, c]).collect();
    Some(format!("#{}", doubled.to_ascii_uppercase()))
}

/// The three channels of 'rgb(r, g, b)', as written.
fn rgb_channels(value: &str) -> Option<[i64; 3]> {
    let mut reader = Reader { rest: value };
    reader.spaces();
    if !reader.word("rgb(") {
        return None;
    }
    let mut channels = [0i64; 3];
    for (k, slot) in channels.iter_mut().enumerate() {
        reader.spaces();
        let text = reader.integer(true)?;
        *slot = text.parse::<i64>().ok()?.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
        if k < 2 && !reader.separator() {
            return None;
        }
    }
    reader.spaces();
    if !reader.char(')') || !reader.end() {
        return None;
    }
    Some(channels)
}

/// The hue, saturation and lightness of 'hsl(h, s%, l%)', the hue in degrees, with or without
/// 'deg' after it.
fn hsl_values(value: &str) -> Option<[f64; 3]> {
    let mut reader = Reader { rest: value };
    reader.spaces();
    if !reader.word("hsl(") {
        return None;
    }
    reader.spaces();
    let hue = reader.number(true)?.parse::<f64>().ok()?;
    // White space before 'deg' separates the hue only where no 'deg' follows it
    let before = reader.spaces();
    let degrees = reader.word("deg");
    let separated = reader.separator();
    if !separated && (degrees || before == 0) {
        return None;
    }
    let saturation = reader.number(false)?.parse::<f64>().ok()?;
    reader.char('%');
    if !reader.separator() {
        return None;
    }
    let lightness = reader.number(false)?.parse::<f64>().ok()?;
    reader.char('%');
    reader.spaces();
    if !reader.char(')') || !reader.end() {
        return None;
    }
    Some([hue, saturation, lightness])
}

/// A color as Write-ColorEX reads it further on: '#RGB', '0xRGB', 'rgb(r, g, b)' and
/// 'hsl(h, s%, l%)' become '#RRGGBB'; any other text is answered as it is. A channel of rgb()
/// outside 0-255 is clamped, with a warning `warn` gets.
pub fn color_form(value: &str, warn: &mut dyn FnMut(String)) -> String {
    if let Some(hex) = short_hex(value) {
        return hex;
    }
    if let Some(channels) = rgb_channels(value) {
        let clamped = channels.map(|c| c.clamp(0, 255));
        if clamped != channels {
            warn(format!(
                "RGB values out of range (0-255). Original: @({},{},{}). Clamped to: @({},{},{})",
                channels[0], channels[1], channels[2], clamped[0], clamped[1], clamped[2]
            ));
        }
        return format!("#{:02X}{:02X}{:02X}", clamped[0], clamped[1], clamped[2]);
    }
    if let Some([hue, saturation, lightness]) = hsl_values(value) {
        let rgb = oklab::from_hsl(hue, saturation, lightness);
        return format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
    }
    value.to_string()
}

/// Whether text is a hex code of six digits after # or 0x, in either case.
pub fn is_hex6(text: &str) -> bool {
    let digits = if let Some(rest) = text.strip_prefix('#') {
        rest
    } else if text.get(..2).is_some_and(|prefix| prefix.eq_ignore_ascii_case("0x")) {
        &text[2..]
    } else {
        return false;
    };
    let digits = digits.strip_suffix('\n').unwrap_or(digits);
    digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A name in a form compared without regard to case, as .NET's OrdinalIgnoreCase compares it.
fn folded(name: &str) -> String {
    name.chars()
        .map(|c| {
            let mut upper = c.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(single), None) => single,
                _ => c,
            }
        })
        .collect()
}

/// The color names one Write-ColorEX call warned about, so each is named once.
#[derive(Default)]
pub struct UnknownNames(HashSet<String>);

impl UnknownNames {
    /// Warns about a color name the table lacks, once per call. Hex codes, 'None' and empty or
    /// blank text, which mean no color, are not names.
    pub fn check(&mut self, name: &str, warn: &mut dyn FnMut(String)) {
        if name.chars().all(is_white_space) || colors::is_hex_text(name) || name.eq_ignore_ascii_case("None") || lookup(name).is_some() {
            return;
        }
        if self.0.insert(folded(name)) {
            warn(format!("Unknown color '{name}'."));
        }
    }
}

/// The entries of a color parameter with their color forms read, and a warning for each name
/// the table lacks.
pub fn read_forms(values: &mut [ColorArg], unknown: &mut UnknownNames, warn: &mut dyn FnMut(String)) {
    for value in values.iter_mut() {
        if let ColorArg::Name(name) = value
            && lookup(name).is_none()
        {
            let form = color_form(name, warn);
            unknown.check(&form, warn);
            *value = ColorArg::Name(form);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(value: &str) -> (String, Vec<String>) {
        let mut warnings = Vec::new();
        let out = color_form(value, &mut |w| warnings.push(w));
        (out, warnings)
    }

    #[test]
    fn forms() {
        assert_eq!(form("#f80").0, "#FF8800");
        assert_eq!(form("0Xabc").0, "#AABBCC");
        assert_eq!(form("rgb(255, 128, 0)").0, "#FF8000");
        assert_eq!(form("  RGB( 255 128 0 )  ").0, "#FF8000");
        assert_eq!(form("rgb(255,,128,0)").0, "rgb(255,,128,0)");
        assert_eq!(form("rgb(300, -5, 0)"), ("#FF0000".to_string(), vec!["RGB values out of range (0-255). Original: @(300,-5,0). Clamped to: @(255,0,0)".to_string()]));
        assert_eq!(form("hsl(30, 100%, 50%)").0, "#FF8000");
        assert_eq!(form("hsl(30deg 100% 50%)").0, "#FF8000");
        assert_eq!(form("hsl(30 100% 50%)").0, "#FF8000");
        assert_eq!(form("hsl(30 deg 100% 50%)").0, "#FF8000");
        assert_eq!(form("hsl(30deg100% 50%)").0, "hsl(30deg100% 50%)");
        assert_eq!(form("hsl(-120, 50%, 25%)").0, "#202060");
        assert_eq!(form("hsl(30., 100%, 50%)").0, "hsl(30., 100%, 50%)");
        assert_eq!(form("Orange").0, "Orange");
        assert_eq!(form("#FF8000").0, "#FF8000");
    }

    #[test]
    fn unknown_names_warn_once() {
        let mut warnings = Vec::new();
        let mut unknown = UnknownNames::default();
        let mut values = vec![ColorArg::Name("Purpel".into()), ColorArg::Name("purpel".into()), ColorArg::Name("None".into()), ColorArg::Name(" ".into())];
        read_forms(&mut values, &mut unknown, &mut |w| warnings.push(w));
        assert_eq!(warnings, vec!["Unknown color 'Purpel'."]);
    }
}
