//! Display width of text in terminal cells, as PSWriteColorEX's Measure-DisplayWidth measures it:
//! each code point's width from unicode-width 0.2.2, and the emoji sequence rules over the sets in
//! width_sets.rs.

use pwrs::prelude::*;
use unicode_width::UnicodeWidthChar;

use crate::width_sets::{MODIFIER_BASE, PICTOGRAPHIC, VS15_BASE, VS16_BASE};

/// Whether a code point falls in a set of start and end pairs sorted by start.
fn in_set(set: &[(u32, u32)], cp: u32) -> bool {
    set.binary_search_by(|&(start, end)| {
        if cp < start {
            std::cmp::Ordering::Greater
        } else if cp > end {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    })
    .is_ok()
}

/// The cells a terminal draws a string in. East Asian Ambiguous characters are 1 cell, or 2
/// with `wide_ambiguous`; control characters are 0.
pub fn display_width(text: &str, wide_ambiguous: bool) -> i64 {
    let mut total: i64 = 0;
    // The code point before this one and the cells it added, for the sequence rules.
    let mut previous: Option<u32> = None;
    let mut previous_width: i64 = 0;
    // Whether the character before a U+200D was an emoji, so the next emoji joins it.
    let mut joining = false;

    for c in text.chars() {
        let cp = c as u32;

        // Printable ASCII is one cell and ends any sequence.
        if (0x20..=0x7E).contains(&cp) {
            total += 1;
            previous = Some(cp);
            previous_width = 1;
            joining = false;
            continue;
        }

        if cp == 0xFE0F {
            // Emoji presentation widens a one-cell base that has an emoji form.
            if previous_width == 1 && previous.is_some_and(|p| in_set(VS16_BASE, p)) {
                total += 1;
                previous_width = 2;
            }
            continue;
        }

        if cp == 0xFE0E {
            // Text presentation narrows a two-cell emoji, outside East Asian text.
            if !wide_ambiguous && previous_width == 2 && previous.is_some_and(|p| in_set(VS15_BASE, p)) {
                total -= 1;
                previous_width = 1;
            }
            continue;
        }

        if cp == 0x200D {
            joining = previous_width == 2 && previous.is_some_and(|p| in_set(PICTOGRAPHIC, p));
            continue;
        }

        if (0x1F3FB..=0x1F3FF).contains(&cp)
            && previous_width == 2
            && previous.is_some_and(|p| in_set(MODIFIER_BASE, p))
        {
            // A skin tone belongs to the emoji before it.
            joining = false;
            continue;
        }

        if joining && in_set(PICTOGRAPHIC, cp) {
            // An emoji joined to the one before it is drawn in that emoji's cells.
            joining = false;
            previous = Some(cp);
            previous_width = 2;
            continue;
        }
        joining = false;

        let width = if wide_ambiguous { c.width_cjk() } else { c.width() }.unwrap_or(0) as i64;
        total += width;
        previous = Some(cp);
        previous_width = width;
    }
    total
}

/// The text split into the characters a terminal draws: a code point with the combining marks,
/// variation selectors, skin tone, emoji joined by U+200D, or second flag letter after it, by the
/// rules display_width counts with. A gradient gives each one color, since a color code inside
/// one splits it.
pub fn split_display_characters(text: &str) -> Vec<&str> {
    let mut characters = Vec::new();
    // Where the character being built starts
    let mut start = 0;
    // The code point before this one and the cells it added, for the sequence rules
    let mut previous: Option<u32> = None;
    let mut previous_width: i64 = 0;
    // Whether the character before a U+200D was an emoji, so the next emoji joins it
    let mut joining = false;
    // Whether the character being built is one regional indicator, which the next one pairs with
    let mut flag_open = false;

    for (at, c) in text.char_indices() {
        let cp = c as u32;
        let mut joins = true;
        let mut is_flag = false;
        if (0x20..=0x7E).contains(&cp) {
            joins = false;
            previous = Some(cp);
            previous_width = 1;
            joining = false;
        } else if cp == 0xFE0F {
            if previous_width == 1 && previous.is_some_and(|p| in_set(VS16_BASE, p)) {
                previous_width = 2;
            }
        } else if cp == 0xFE0E {
            if previous_width == 2 && previous.is_some_and(|p| in_set(VS15_BASE, p)) {
                previous_width = 1;
            }
        } else if cp == 0x200D {
            joining = previous_width == 2 && previous.is_some_and(|p| in_set(PICTOGRAPHIC, p));
        } else if (0x1F3FB..=0x1F3FF).contains(&cp)
            && previous_width == 2
            && previous.is_some_and(|p| in_set(MODIFIER_BASE, p))
        {
            joining = false;
        } else if joining && in_set(PICTOGRAPHIC, cp) {
            joining = false;
            previous = Some(cp);
            previous_width = 2;
        } else {
            joining = false;
            // A character of no width belongs to the one before it; a control character is one
            // of its own
            if (0x1F1E6..=0x1F1FF).contains(&cp) {
                is_flag = !flag_open;
                joins = flag_open;
            } else if c.is_control() || c.width().unwrap_or(0) != 0 {
                joins = false;
            }
            previous = Some(cp);
            previous_width = c.width().unwrap_or(0) as i64;
        }
        flag_open = is_flag;

        if !joins && at > start {
            characters.push(&text[start..at]);
            start = at;
        }
    }
    if start < text.len() {
        characters.push(&text[start..]);
    }
    characters
}

/// Measures the display width of a string in terminal cells.
///
/// String.Length counts UTF-16 code units; this counts the cells a terminal draws. The width of
/// each character comes from the table of the Rust crate unicode-width 0.2.2, which
/// PSWriteColorEX's Measure-DisplayWidth is generated from, so both modules measure alike: wide
/// characters such as CJK and most emoji take 2 cells, combining marks, zero-width characters and
/// control characters take 0, East Asian Ambiguous characters such as box drawing take 1, or 2 with
/// -AmbiguousAsWide, and everything else takes 1. Emoji sequences count as the cells a terminal
/// draws for them: U+FE0F widens a character that has an emoji form, U+FE0E narrows a wide emoji
/// outside East Asian text, a skin tone and an emoji joined by U+200D add nothing, and a flag takes
/// 2 cells.
///
/// # Examples
/// Measure-DisplayWidth "Hello"
/// Measure-DisplayWidth "╔═══╗" -AmbiguousAsWide
#[cmdlet(verb = "Measure", noun = "DisplayWidth", alias = ["MDW", "Get-DisplayWidth"], output = ["System.Int32"])]
#[derive(Default)]
pub struct MeasureDisplayWidth {
    /// The text string to measure.
    #[param(mandatory, position = 0, value_from_pipeline, allow_empty_string)]
    pub text: String,
    /// Treats East Asian Ambiguous characters as 2 cells instead of 1, and keeps a wide emoji
    /// followed by U+FE0E at 2 cells, as East Asian terminals draw them.
    #[param]
    pub ambiguous_as_wide: bool,
}

impl Cmdlet for MeasureDisplayWidth {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let width = display_width(&self.text, self.ambiguous_as_wide);
        ps.write(i32::try_from(width).unwrap_or(i32::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::{display_width, split_display_characters};

    fn s(cps: &[u32]) -> String {
        cps.iter().map(|&c| char::from_u32(c).unwrap()).collect()
    }

    #[test]
    fn widths_match_the_powershell_module() {
        let cases: &[(&[u32], i64, i64)] = &[
            (&[0x48, 0x65, 0x6C, 0x6C, 0x6F], 5, 5),
            (&[0x4E16, 0x754C], 4, 4),
            (&[0x2713], 1, 1),
            (&[0x2705], 2, 2),
            (&[0x1F600, 0x1F44D], 4, 4),
            (&[0x25CF], 1, 2),
            (&[0x2554, 0x2550, 0x2550, 0x2550, 0x2557], 5, 10),
            (&[0x65, 0x301], 1, 1),
            (&[0x200B], 0, 0),
            (&[0x61, 0x9, 0x62], 2, 2),
            (&[0x26A0, 0xFE0F], 2, 2),
            (&[0x231A, 0xFE0E], 1, 2),
            (&[0x1F44D, 0x1F3FD], 2, 2),
            (&[0x1F468, 0x200D, 0x1F469, 0x200D, 0x1F467], 2, 2),
            (&[0x1F469, 0x1F3FD, 0x200D, 0x1F4BB], 2, 2),
            (&[0x1F441, 0xFE0F, 0x200D, 0x1F5E8, 0xFE0F], 2, 2),
            (&[0x2764, 0xFE0F, 0x200D, 0x1F525], 2, 2),
            (&[0x1F1FA, 0x1F1F8], 2, 2),
            (&[0x1100, 0x1161], 2, 2),
        ];
        for (cps, width, wide) in cases {
            let text = s(cps);
            assert_eq!(display_width(&text, false), *width, "{cps:X?}");
            assert_eq!(display_width(&text, true), *wide, "{cps:X?} wide");
        }
    }

    #[test]
    fn splits_like_the_powershell_module() {
        let cases: &[(&[u32], &[&[u32]])] = &[
            (&[0x61, 0x62], &[&[0x61], &[0x62]]),
            (&[0x61, 0x1F600, 0x62], &[&[0x61], &[0x1F600], &[0x62]]),
            (&[0x65, 0x301, 0x78], &[&[0x65, 0x301], &[0x78]]),
            (&[0x61, 0x9, 0x62], &[&[0x61], &[0x9], &[0x62]]),
            (&[0x26A0, 0xFE0F, 0x21], &[&[0x26A0, 0xFE0F], &[0x21]]),
            (&[0x1F44D, 0x1F3FD, 0x20, 0x6F, 0x6B], &[&[0x1F44D, 0x1F3FD], &[0x20], &[0x6F], &[0x6B]]),
            (&[0x1F468, 0x200D, 0x1F469, 0x200D, 0x1F467], &[&[0x1F468, 0x200D, 0x1F469, 0x200D, 0x1F467]]),
            (&[0x2764, 0xFE0F, 0x200D, 0x1F525], &[&[0x2764, 0xFE0F, 0x200D, 0x1F525]]),
            (&[0x1F1FA, 0x1F1F8, 0x1F1EC, 0x1F1E7], &[&[0x1F1FA, 0x1F1F8], &[0x1F1EC, 0x1F1E7]]),
            (&[0x4E16, 0x754C], &[&[0x4E16], &[0x754C]]),
        ];
        for (cps, parts) in cases {
            let text = s(cps);
            let expected: Vec<String> = parts.iter().map(|p| s(p)).collect();
            assert_eq!(split_display_characters(&text), expected, "{cps:X?}");
        }
        assert!(split_display_characters("").is_empty());
    }
}
