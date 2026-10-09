//! Writes the code point sets Measure-DisplayWidth reads for emoji sequences, from the crates
//! unicode-width 0.2.2 and unicode-segmentation 1.13.3. With no argument it prints the Rust
//! source of src/width_sets.rs; with --powershell it prints the hashtable PSWriteColorEX keeps in
//! Private/DisplayWidthTable.ps1, which also carries the width classes, so both modules measure
//! alike.
//!
//! cargo run --release --manifest-path tools/widthgen/Cargo.toml > src/width_sets.rs

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The runs of code points a predicate holds for, as start and end pairs.
fn ranges(pred: impl Fn(char) -> bool) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for cp in 0u32..=0x10FFFF {
        let Some(c) = char::from_u32(cp) else { continue };
        if pred(c) {
            match out.last_mut() {
                Some(last) if last.1 + 1 == cp => last.1 = cp,
                // The surrogates are no characters, so they do not break a run.
                Some(last) if last.1 == 0xD7FF && cp == 0xE000 => last.1 = cp,
                _ => out.push((cp, cp)),
            }
        }
    }
    out
}

struct Sets {
    control: Vec<(u32, u32)>,
    zero: Vec<(u32, u32)>,
    wide: Vec<(u32, u32)>,
    three: Vec<(u32, u32)>,
    ambiguous: Vec<(u32, u32)>,
    vs16: Vec<(u32, u32)>,
    vs15: Vec<(u32, u32)>,
    modbase: Vec<(u32, u32)>,
    picto: Vec<(u32, u32)>,
}

fn sets() -> Sets {
    let w = |c: char| c.width();
    let s = |t: &str| UnicodeWidthStr::width(t);
    Sets {
        control: ranges(|c| w(c).is_none()),
        zero: ranges(|c| w(c) == Some(0)),
        wide: ranges(|c| w(c) == Some(2)),
        three: ranges(|c| w(c) == Some(3)),
        ambiguous: ranges(|c| w(c) == Some(1) && c.width_cjk() == Some(2)),
        // A one-cell base that U+FE0F makes two cells.
        vs16: ranges(|c| w(c) == Some(1) && s(&format!("{c}\u{FE0F}")) == 2),
        // A two-cell base that U+FE0E makes one cell.
        vs15: ranges(|c| w(c) == Some(2) && s(&format!("{c}\u{FE0E}")) == 1),
        // A base after which a skin tone adds nothing.
        modbase: ranges(|c| w(c) == Some(2) && s(&format!("{c}\u{1F3FB}")) == 2),
        // Extended_Pictographic as grapheme clustering reads it: a character with width that
        // U+200D after an emoji joins into one cluster, and that is no spacing mark.
        picto: ranges(|c| {
            w(c).is_some_and(|x| x > 0)
                && format!("\u{1F468}\u{200D}{c}").graphemes(true).count() == 1
                && format!("a{c}").graphemes(true).count() == 2
        }),
    }
}

fn rust_set(out: &mut String, name: &str, doc: &str, r: &[(u32, u32)]) {
    out.push_str(&format!("/// {doc}\npub static {name}: &[(u32, u32)] = &[\n"));
    for chunk in r.chunks(4) {
        let row: Vec<String> = chunk.iter().map(|(a, b)| format!("(0x{a:05X}, 0x{b:05X})")).collect();
        out.push_str(&format!("    {},\n", row.join(", ")));
    }
    out.push_str("];\n\n");
}

fn powershell_set(out: &mut String, name: &str, r: &[(u32, u32)]) {
    out.push_str(&format!("    {name} = [int[]]@("));
    for (i, (a, b)) in r.iter().enumerate() {
        if i % 6 == 0 {
            out.push_str("\n        ");
        }
        out.push_str(&format!("0x{a:05X}, 0x{b:05X}"));
        if i + 1 < r.len() {
            out.push_str(", ");
        }
    }
    out.push_str("\n    )\n");
}

fn main() {
    let s = sets();
    let mut out = String::new();
    if std::env::args().any(|a| a == "--powershell") {
        out.push_str("$script:DisplayWidthTable = @{\n");
        for (name, r) in [
            ("Control", &s.control),
            ("Zero", &s.zero),
            ("Wide", &s.wide),
            ("Three", &s.three),
            ("Ambiguous", &s.ambiguous),
            ("Vs16Base", &s.vs16),
            ("Vs15Base", &s.vs15),
            ("ModifierBase", &s.modbase),
            ("Pictographic", &s.picto),
        ] {
            powershell_set(&mut out, name, r);
        }
        out.push_str("}\n");
    } else {
        out.push_str(
            "//! The code point sets Measure-DisplayWidth reads for emoji sequences, written by\n\
             //! tools/widthgen from unicode-width 0.2.2 and unicode-segmentation 1.13.3. Each set is\n\
             //! start and end pairs sorted by start. Do not edit; run the tool again.\n\n",
        );
        rust_set(&mut out, "VS16_BASE", "One-cell characters that U+FE0F after them makes two cells.", &s.vs16);
        rust_set(&mut out, "VS15_BASE", "Two-cell characters that U+FE0E after them makes one cell.", &s.vs15);
        rust_set(&mut out, "MODIFIER_BASE", "Characters after which a skin tone adds nothing.", &s.modbase);
        rust_set(&mut out, "PICTOGRAPHIC", "Emoji that U+200D joins to a two-cell emoji before them, adding nothing.", &s.picto);
    }
    print!("{out}");
}
