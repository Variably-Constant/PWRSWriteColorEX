//! The color table and the conversions between color forms: hex codes, RGB values, the
//! 256-color and 16-color ANSI codes and the console's colors. The table and every formula match
//! PSWriteColorEX, including PowerShell's rounding of halves to even.

use std::collections::HashMap;
use std::sync::Arc;

use subetha_core::swap_cell::SwapCell;

use crate::set_once::SetOnce;

/// One named color in every color mode.
pub struct Color {
    pub name: &'static str,
    /// The console color (System.ConsoleColor) nearest it.
    pub native: &'static str,
    pub ansi4_fg: u8,
    pub ansi4_bg: u8,
    pub ansi8: u8,
    pub rgb: [u8; 3],
}

/// What the table holds for a name, built-in or registered: the color in every color mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    /// The console color (System.ConsoleColor) nearest it.
    pub native: &'static str,
    pub ansi4_fg: u8,
    pub ansi4_bg: u8,
    pub ansi8: u8,
    pub rgb: [u8; 3],
}

impl Color {
    fn entry(&self) -> Entry {
        Entry { native: self.native, ansi4_fg: self.ansi4_fg, ansi4_bg: self.ansi4_bg, ansi8: self.ansi8, rgb: self.rgb }
    }
}

impl Entry {
    /// The entry Register-ColorName makes for an RGB color: the nearest 16-color code and its
    /// console color, and the nearest 256-color code.
    pub fn from_rgb(rgb: [u8; 3]) -> Entry {
        let wide = rgb.map(i64::from);
        let ansi4 = rgb_to_ansi4(wide);
        Entry { native: ansi4_to_native(ansi4), ansi4_fg: ansi4 as u8, ansi4_bg: (ansi4 + 10) as u8, ansi8: rgb_to_ansi8(wide) as u8, rgb }
    }
}

/// The 129 color names of PSWriteColorEX's table, in 44 families.
pub static COLORS: &[Color] = &[
    Color { name: "Amber", native: "Yellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 214, rgb: [255, 191, 0] },
    Color { name: "Aqua", native: "Cyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 6, rgb: [0, 255, 255] },
    Color { name: "Black", native: "Black", ansi4_fg: 30, ansi4_bg: 40, ansi8: 0, rgb: [0, 0, 0] },
    Color { name: "Blue", native: "Blue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 4, rgb: [0, 0, 255] },
    Color { name: "Brick", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 124, rgb: [178, 34, 34] },
    Color { name: "Brown", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 130, rgb: [150, 75, 0] },
    Color { name: "Chartreuse", native: "Green", ansi4_fg: 32, ansi4_bg: 42, ansi8: 118, rgb: [127, 255, 0] },
    Color { name: "Coral", native: "Red", ansi4_fg: 31, ansi4_bg: 41, ansi8: 209, rgb: [255, 127, 80] },
    Color { name: "Crimson", native: "Red", ansi4_fg: 31, ansi4_bg: 41, ansi8: 160, rgb: [220, 20, 60] },
    Color { name: "Cyan", native: "Cyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 6, rgb: [0, 255, 255] },
    Color { name: "DarkAmber", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 214, rgb: [255, 160, 0] },
    Color { name: "DarkAqua", native: "DarkCyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 30, rgb: [0, 128, 128] },
    Color { name: "DarkBlue", native: "DarkBlue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 18, rgb: [0, 0, 139] },
    Color { name: "DarkBrick", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 88, rgb: [139, 26, 26] },
    Color { name: "DarkBrown", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 239, rgb: [101, 67, 33] },
    Color { name: "DarkChartreuse", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 64, rgb: [69, 139, 0] },
    Color { name: "DarkCoral", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 167, rgb: [205, 91, 69] },
    Color { name: "DarkCrimson", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 88, rgb: [139, 0, 0] },
    Color { name: "DarkCyan", native: "DarkCyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 30, rgb: [0, 139, 139] },
    Color { name: "DarkEmerald", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 22, rgb: [0, 100, 0] },
    Color { name: "DarkForest", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 22, rgb: [34, 75, 34] },
    Color { name: "DarkGold", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 136, rgb: [184, 134, 11] },
    Color { name: "DarkGray", native: "DarkGray", ansi4_fg: 90, ansi4_bg: 100, ansi8: 8, rgb: [128, 128, 128] },
    Color { name: "DarkGreen", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 22, rgb: [0, 100, 0] },
    Color { name: "DarkIndigo", native: "DarkBlue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 17, rgb: [25, 25, 112] },
    Color { name: "DarkJade", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 22, rgb: [0, 100, 50] },
    Color { name: "DarkLavender", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 60, rgb: [100, 100, 150] },
    Color { name: "DarkLime", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 41, rgb: [50, 205, 50] },
    Color { name: "DarkMagenta", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 90, rgb: [139, 0, 139] },
    Color { name: "DarkMaroon", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 52, rgb: [69, 0, 0] },
    Color { name: "DarkMint", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 71, rgb: [60, 179, 113] },
    Color { name: "DarkNavy", native: "DarkBlue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 17, rgb: [0, 0, 80] },
    Color { name: "DarkOlive", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 58, rgb: [85, 107, 47] },
    Color { name: "DarkOrange", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 208, rgb: [255, 140, 0] },
    Color { name: "DarkPeach", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 215, rgb: [255, 164, 96] },
    Color { name: "DarkPink", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 162, rgb: [199, 21, 133] },
    Color { name: "DarkPlum", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 54, rgb: [102, 51, 153] },
    Color { name: "DarkPurple", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 54, rgb: [75, 0, 130] },
    Color { name: "DarkRed", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 88, rgb: [139, 0, 0] },
    Color { name: "DarkRose", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 89, rgb: [128, 0, 64] },
    Color { name: "DarkRuby", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 88, rgb: [155, 17, 30] },
    Color { name: "DarkSalmon", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 173, rgb: [233, 150, 122] },
    Color { name: "DarkSapphire", native: "DarkBlue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 17, rgb: [8, 37, 103] },
    Color { name: "DarkSky", native: "DarkBlue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 39, rgb: [0, 191, 255] },
    Color { name: "DarkSlate", native: "DarkGray", ansi4_fg: 90, ansi4_bg: 100, ansi8: 238, rgb: [47, 79, 79] },
    Color { name: "DarkSteel", native: "DarkGray", ansi4_fg: 90, ansi4_bg: 100, ansi8: 238, rgb: [70, 70, 70] },
    Color { name: "DarkTan", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 94, rgb: [139, 90, 43] },
    Color { name: "DarkTeal", native: "DarkCyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 30, rgb: [0, 128, 128] },
    Color { name: "DarkTurquoise", native: "DarkCyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 44, rgb: [0, 206, 209] },
    Color { name: "DarkViolet", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 92, rgb: [148, 0, 211] },
    Color { name: "DarkWine", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 52, rgb: [72, 0, 25] },
    Color { name: "DarkYellow", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 184, rgb: [204, 204, 0] },
    Color { name: "Emerald", native: "Green", ansi4_fg: 32, ansi4_bg: 42, ansi8: 78, rgb: [80, 200, 120] },
    Color { name: "Forest", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 28, rgb: [34, 139, 34] },
    Color { name: "Gold", native: "Yellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 220, rgb: [255, 215, 0] },
    Color { name: "Gray", native: "Gray", ansi4_fg: 37, ansi4_bg: 47, ansi8: 7, rgb: [192, 192, 192] },
    Color { name: "Green", native: "Green", ansi4_fg: 32, ansi4_bg: 42, ansi8: 2, rgb: [0, 255, 0] },
    Color { name: "Indigo", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 54, rgb: [75, 0, 130] },
    Color { name: "Jade", native: "DarkGreen", ansi4_fg: 32, ansi4_bg: 42, ansi8: 35, rgb: [0, 168, 107] },
    Color { name: "Lavender", native: "Magenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 254, rgb: [230, 230, 250] },
    Color { name: "LightAmber", native: "Yellow", ansi4_fg: 93, ansi4_bg: 103, ansi8: 220, rgb: [255, 204, 0] },
    Color { name: "LightAqua", native: "Cyan", ansi4_fg: 96, ansi4_bg: 106, ansi8: 14, rgb: [127, 255, 255] },
    Color { name: "LightBlack", native: "DarkGray", ansi4_fg: 90, ansi4_bg: 100, ansi8: 243, rgb: [118, 118, 118] },
    Color { name: "LightBlue", native: "Blue", ansi4_fg: 94, ansi4_bg: 104, ansi8: 12, rgb: [85, 85, 255] },
    Color { name: "LightBrick", native: "Red", ansi4_fg: 31, ansi4_bg: 41, ansi8: 167, rgb: [205, 92, 92] },
    Color { name: "LightBrown", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 172, rgb: [205, 133, 63] },
    Color { name: "LightChartreuse", native: "Green", ansi4_fg: 92, ansi4_bg: 102, ansi8: 156, rgb: [191, 255, 127] },
    Color { name: "LightCoral", native: "Red", ansi4_fg: 91, ansi4_bg: 101, ansi8: 210, rgb: [240, 128, 128] },
    Color { name: "LightCrimson", native: "Red", ansi4_fg: 91, ansi4_bg: 101, ansi8: 197, rgb: [248, 48, 88] },
    Color { name: "LightCyan", native: "Cyan", ansi4_fg: 96, ansi4_bg: 106, ansi8: 14, rgb: [85, 255, 255] },
    Color { name: "LightEmerald", native: "Green", ansi4_fg: 92, ansi4_bg: 102, ansi8: 121, rgb: [128, 255, 170] },
    Color { name: "LightForest", native: "Green", ansi4_fg: 32, ansi4_bg: 42, ansi8: 41, rgb: [50, 205, 50] },
    Color { name: "LightGold", native: "Yellow", ansi4_fg: 93, ansi4_bg: 103, ansi8: 220, rgb: [255, 223, 0] },
    Color { name: "LightGray", native: "Gray", ansi4_fg: 37, ansi4_bg: 47, ansi8: 255, rgb: [238, 238, 238] },
    Color { name: "LightGreen", native: "Green", ansi4_fg: 92, ansi4_bg: 102, ansi8: 10, rgb: [85, 255, 85] },
    Color { name: "LightIndigo", native: "Blue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 60, rgb: [102, 102, 153] },
    Color { name: "LightJade", native: "Green", ansi4_fg: 32, ansi4_bg: 42, ansi8: 78, rgb: [64, 216, 143] },
    Color { name: "LightLavender", native: "Magenta", ansi4_fg: 95, ansi4_bg: 105, ansi8: 255, rgb: [240, 240, 255] },
    Color { name: "LightLime", native: "Green", ansi4_fg: 92, ansi4_bg: 102, ansi8: 46, rgb: [50, 255, 50] },
    Color { name: "LightMagenta", native: "Magenta", ansi4_fg: 95, ansi4_bg: 105, ansi8: 13, rgb: [255, 85, 255] },
    Color { name: "LightMaroon", native: "Red", ansi4_fg: 31, ansi4_bg: 41, ansi8: 125, rgb: [176, 48, 96] },
    Color { name: "LightMint", native: "Green", ansi4_fg: 92, ansi4_bg: 102, ansi8: 158, rgb: [189, 252, 201] },
    Color { name: "LightNavy", native: "Blue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 20, rgb: [0, 0, 205] },
    Color { name: "LightOlive", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 142, rgb: [170, 170, 0] },
    Color { name: "LightOrange", native: "Yellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 214, rgb: [255, 195, 0] },
    Color { name: "LightPeach", native: "Yellow", ansi4_fg: 93, ansi4_bg: 103, ansi8: 230, rgb: [255, 239, 213] },
    Color { name: "LightPink", native: "Magenta", ansi4_fg: 95, ansi4_bg: 105, ansi8: 217, rgb: [255, 182, 193] },
    Color { name: "LightPlum", native: "Magenta", ansi4_fg: 95, ansi4_bg: 105, ansi8: 219, rgb: [238, 174, 238] },
    Color { name: "LightPurple", native: "Magenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 98, rgb: [147, 112, 219] },
    Color { name: "LightRed", native: "Red", ansi4_fg: 91, ansi4_bg: 101, ansi8: 9, rgb: [255, 85, 85] },
    Color { name: "LightRose", native: "Magenta", ansi4_fg: 95, ansi4_bg: 105, ansi8: 217, rgb: [255, 182, 193] },
    Color { name: "LightRuby", native: "Red", ansi4_fg: 91, ansi4_bg: 101, ansi8: 204, rgb: [255, 102, 153] },
    Color { name: "LightSalmon", native: "Red", ansi4_fg: 91, ansi4_bg: 101, ansi8: 216, rgb: [255, 160, 122] },
    Color { name: "LightSapphire", native: "Blue", ansi4_fg: 94, ansi4_bg: 104, ansi8: 68, rgb: [100, 149, 237] },
    Color { name: "LightSky", native: "Cyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 117, rgb: [135, 206, 250] },
    Color { name: "LightSlate", native: "Gray", ansi4_fg: 37, ansi4_bg: 47, ansi8: 102, rgb: [119, 136, 153] },
    Color { name: "LightSteel", native: "White", ansi4_fg: 97, ansi4_bg: 47, ansi8: 146, rgb: [176, 196, 222] },
    Color { name: "LightTan", native: "Yellow", ansi4_fg: 93, ansi4_bg: 103, ansi8: 223, rgb: [245, 222, 179] },
    Color { name: "LightTeal", native: "Cyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 44, rgb: [64, 224, 208] },
    Color { name: "LightTurquoise", native: "Cyan", ansi4_fg: 96, ansi4_bg: 106, ansi8: 159, rgb: [175, 238, 238] },
    Color { name: "LightViolet", native: "Magenta", ansi4_fg: 95, ansi4_bg: 105, ansi8: 182, rgb: [200, 162, 200] },
    Color { name: "LightWine", native: "Red", ansi4_fg: 31, ansi4_bg: 41, ansi8: 131, rgb: [179, 97, 115] },
    Color { name: "LightYellow", native: "Yellow", ansi4_fg: 93, ansi4_bg: 103, ansi8: 11, rgb: [255, 255, 85] },
    Color { name: "Lime", native: "Green", ansi4_fg: 32, ansi4_bg: 42, ansi8: 46, rgb: [0, 255, 0] },
    Color { name: "Magenta", native: "Magenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 5, rgb: [255, 0, 255] },
    Color { name: "Maroon", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 88, rgb: [128, 0, 0] },
    Color { name: "Mint", native: "Green", ansi4_fg: 32, ansi4_bg: 42, ansi8: 121, rgb: [152, 251, 152] },
    Color { name: "Navy", native: "DarkBlue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 18, rgb: [0, 0, 128] },
    Color { name: "Olive", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 100, rgb: [128, 128, 0] },
    Color { name: "Orange", native: "DarkYellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 214, rgb: [255, 165, 0] },
    Color { name: "Peach", native: "Yellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 223, rgb: [255, 218, 185] },
    Color { name: "Pink", native: "Magenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 217, rgb: [255, 192, 203] },
    Color { name: "Plum", native: "Magenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 182, rgb: [221, 160, 221] },
    Color { name: "Purple", native: "DarkMagenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 90, rgb: [128, 0, 128] },
    Color { name: "Red", native: "Red", ansi4_fg: 31, ansi4_bg: 41, ansi8: 1, rgb: [255, 0, 0] },
    Color { name: "Rose", native: "Magenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 198, rgb: [255, 0, 127] },
    Color { name: "Ruby", native: "Red", ansi4_fg: 31, ansi4_bg: 41, ansi8: 161, rgb: [224, 17, 95] },
    Color { name: "Salmon", native: "Red", ansi4_fg: 31, ansi4_bg: 41, ansi8: 210, rgb: [250, 128, 114] },
    Color { name: "Sapphire", native: "Blue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 25, rgb: [15, 82, 186] },
    Color { name: "Sky", native: "Blue", ansi4_fg: 34, ansi4_bg: 44, ansi8: 117, rgb: [135, 206, 235] },
    Color { name: "Slate", native: "Gray", ansi4_fg: 37, ansi4_bg: 47, ansi8: 244, rgb: [112, 128, 144] },
    Color { name: "Steel", native: "Gray", ansi4_fg: 37, ansi4_bg: 47, ansi8: 243, rgb: [113, 121, 126] },
    Color { name: "Tan", native: "Yellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 180, rgb: [210, 180, 140] },
    Color { name: "Teal", native: "DarkCyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 30, rgb: [0, 150, 150] },
    Color { name: "Turquoise", native: "Cyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 44, rgb: [64, 224, 208] },
    Color { name: "Violet", native: "Magenta", ansi4_fg: 35, ansi4_bg: 45, ansi8: 213, rgb: [238, 130, 238] },
    Color { name: "White", native: "White", ansi4_fg: 97, ansi4_bg: 107, ansi8: 15, rgb: [255, 255, 255] },
    Color { name: "Wine", native: "DarkRed", ansi4_fg: 31, ansi4_bg: 41, ansi8: 52, rgb: [114, 47, 55] },
    Color { name: "Yellow", native: "Yellow", ansi4_fg: 33, ansi4_bg: 43, ansi8: 226, rgb: [255, 255, 0] },

];

fn index() -> &'static HashMap<String, usize> {
    static INDEX: SetOnce<HashMap<String, usize>> = SetOnce::new();
    INDEX.get_or_init(|| {
        COLORS
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.to_ascii_lowercase(), i))
            .collect()
    })
}

/// The names Register-ColorName added, in the order they were first added, each with its
/// spelling from then. They last until the module is imported again. Any thread reads them
/// without waiting; a change swaps in a new table, and a replaced one is freed when its last
/// reader lets go.
static REGISTERED: SetOnce<SwapCell<Vec<(String, Entry)>>> = SetOnce::new();

fn registered_names() -> &'static SwapCell<Vec<(String, Entry)>> {
    REGISTERED.get_or_init(|| SwapCell::new(Vec::new()))
}

/// Adds a name, or replaces the color of a name already registered, which keeps its spelling and
/// place.
pub fn register(name: &str, entry: Entry) {
    registered_names().rcu(|current| {
        let mut next = current.clone();
        match next.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            Some(found) => found.1 = entry,
            None => next.push((name.to_string(), entry)),
        }
        next
    });
}

/// Removes a registered name; false when it was not registered.
pub fn unregister(name: &str) -> bool {
    let replaced = registered_names().rcu(|current| current.iter().filter(|(n, _)| !n.eq_ignore_ascii_case(name)).cloned().collect());
    replaced.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
}

/// The registered names and their colors, in the order they were added.
pub fn registered() -> Vec<(String, Entry)> {
    registered_names().load().to_vec()
}

/// Forgets every registered name, as importing the module again does.
pub fn clear_registered() {
    registered_names().store(Arc::new(Vec::new()));
}

/// Every name in the table with its color: the built-in names, a registered name over a built-in
/// one of the same spelling, and the other registered names after them.
pub fn table() -> Vec<(String, Entry)> {
    let registered = registered();
    let mut out: Vec<(String, Entry)> = COLORS.iter().map(|c| (c.name.to_string(), c.entry())).collect();
    for (name, entry) in registered {
        match out.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
            Some(found) => found.1 = entry,
            None => out.push((name, entry)),
        }
    }
    out
}

/// The table entry for a color name, registered or built-in, compared without regard to case as
/// a PowerShell hashtable compares its keys.
pub fn lookup(name: &str) -> Option<Entry> {
    if let Some((_, entry)) = registered_names().load().iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        return Some(*entry);
    }
    index().get(&name.to_ascii_lowercase()).map(|&i| COLORS[i].entry())
}

/// The console colors and their ANSI4 foreground codes; a background code is 10 more.
pub const CONSOLE_COLORS: [(&str, u8); 16] = [
    ("Black", 30),
    ("DarkBlue", 34),
    ("DarkGreen", 32),
    ("DarkCyan", 36),
    ("DarkRed", 31),
    ("DarkMagenta", 35),
    ("DarkYellow", 33),
    ("Gray", 37),
    ("DarkGray", 90),
    ("Blue", 94),
    ("Green", 92),
    ("Cyan", 96),
    ("Red", 91),
    ("Magenta", 95),
    ("Yellow", 93),
    ("White", 97),
];

/// The console color with this number (System.ConsoleColor), 0-15.
pub fn console_color_name(number: i64) -> Option<&'static str> {
    usize::try_from(number).ok().and_then(|i| CONSOLE_COLORS.get(i)).map(|(name, _)| *name)
}

/// The ANSI4 foreground code of a console color name.
pub fn console_color_sgr(name: &str) -> Option<u8> {
    CONSOLE_COLORS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, code)| *code)
}

/// The console color an ANSI4 foreground or background code stands for, Gray for any other.
pub fn ansi4_to_native(code: i64) -> &'static str {
    let code = if (40..=47).contains(&code) || (100..=107).contains(&code) { code - 10 } else { code };
    match code {
        30 => "Black",
        31 => "DarkRed",
        32 => "DarkGreen",
        33 => "DarkYellow",
        34 => "DarkBlue",
        35 => "DarkMagenta",
        36 => "DarkCyan",
        37 => "Gray",
        90 => "DarkGray",
        91 => "Red",
        92 => "Green",
        93 => "Yellow",
        94 => "Blue",
        95 => "Magenta",
        96 => "Cyan",
        97 => "White",
        _ => "Gray",
    }
}

/// PowerShell's rounding: halves go to the even neighbor, as `[Math]::Round` and an `[int]` cast do.
pub fn round_even(value: f64) -> i64 {
    value.round_ties_even() as i64
}

/// A hex code without its # or 0x, the 0x in either case, as Convert-HexToRGB removes them.
pub fn strip_hex_prefix(hex: &str) -> &str {
    if let Some(rest) = hex.strip_prefix('#') {
        return rest;
    }
    match hex.get(..2) {
        Some(prefix) if prefix.eq_ignore_ascii_case("0x") => &hex[2..],
        _ => hex,
    }
}

/// The RGB value of a hex code (#RRGGBB, 0xRRGGBB or RRGGBB), or None for anything else. Six
/// digits with a line end after them pass, as they do PowerShell's `$` in a pattern.
pub fn hex_to_rgb(hex: &str) -> Option<[u8; 3]> {
    let digits = strip_hex_prefix(hex);
    let digits = digits.strip_suffix('\n').unwrap_or(digits);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// Whether a string is a hex code Write-ColorEX reads as one: # or 0x, in either case, in front.
pub fn is_hex_text(text: &str) -> bool {
    text.starts_with('#') || text.get(..2).is_some_and(|prefix| prefix.eq_ignore_ascii_case("0x"))
}

/// The 256-color code, of the cube and the grays (16-255), that looks nearest to a color, by
/// OKLab distance. Values outside 0-255 are clamped.
pub fn rgb_to_ansi8(rgb: [i64; 3]) -> i64 {
    crate::oklab::nearest_ansi8(rgb)
}

/// The nearest 16-color foreground code, 30-37 or 90-97.
pub fn rgb_to_ansi4(rgb: [i64; 3]) -> i64 {
    let [r, g, b] = rgb;
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let brightness = (r + g + b) as f64 / 3.0;
    let bright = max >= 200;
    let pick = |bright_code: i64, normal_code: i64| if bright { bright_code } else { normal_code };

    if max - min < 30 {
        return if brightness < 64.0 {
            30
        } else if brightness < 128.0 {
            90
        } else if brightness < 192.0 {
            37
        } else {
            97
        };
    }
    if r == max {
        if g > b + 30 {
            pick(93, 33)
        } else if b > g + 30 {
            pick(95, 35)
        } else {
            pick(91, 31)
        }
    } else if g == max {
        if r > b + 30 {
            pick(93, 33)
        } else if b > r + 30 {
            pick(96, 36)
        } else {
            pick(92, 32)
        }
    } else if r > g + 30 {
        pick(95, 35)
    } else if g > r + 30 {
        pick(96, 36)
    } else {
        pick(94, 34)
    }
}

/// The RGB value of a 256-color code, from the 16 standard colors, the cube or the grays.
pub fn ansi8_to_rgb(code: i64) -> [i64; 3] {
    const STANDARD: [[i64; 3]; 16] = [
        [0, 0, 0],
        [128, 0, 0],
        [0, 128, 0],
        [128, 128, 0],
        [0, 0, 128],
        [128, 0, 128],
        [0, 128, 128],
        [192, 192, 192],
        [128, 128, 128],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [0, 0, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    const LEVELS: [i64; 6] = [0, 95, 135, 175, 215, 255];
    let code = code.clamp(0, 255);
    if code < 16 {
        return STANDARD[code as usize];
    }
    if code < 232 {
        let index = code - 16;
        return [LEVELS[(index / 36) as usize], LEVELS[((index / 6) % 6) as usize], LEVELS[(index % 6) as usize]];
    }
    let value = 8 + (code - 232) * 10;
    [value, value, value]
}

/// The 16-color foreground code nearest a 256-color code.
pub fn ansi8_to_ansi4(code: i64) -> i64 {
    if (0..8).contains(&code) {
        return 30 + code;
    }
    if (8..16).contains(&code) {
        return 82 + code;
    }
    rgb_to_ansi4(ansi8_to_rgb(code))
}

/// An RGB value made lighter: each channel times the factor, at least 255 x (factor - 1), at
/// most 255.
pub fn lighter_rgb(rgb: [i64; 3], factor: f64) -> [i64; 3] {
    let floor = round_even(255.0 * (factor - 1.0));
    rgb.map(|v| (round_even(v as f64 * factor).max(floor)).min(255))
}

/// A 256-color code made lighter: a dark standard color becomes its bright form, a gray moves up
/// the gray ramp, and any other color is lightened through its RGB value.
pub fn lighter_ansi8(code: i64, factor: f64) -> i64 {
    if code <= 15 {
        if code <= 7 {
            return code + 8;
        }
        return rgb_to_ansi8(lighter_rgb(ansi8_to_rgb(code), factor));
    }
    if code >= 232 {
        let level = code - 232;
        let increment = round_even((factor - 1.0) * 10.0).max(1);
        return 232 + (level + increment).min(23);
    }
    rgb_to_ansi8(lighter_rgb(ansi8_to_rgb(code), factor))
}

/// The next lighter name in a color's family when the table has it: DarkRed to Red, Red to
/// LightRed. A Light name, or a name with no lighter name in the table, is returned unchanged.
pub fn lighter_name(name: &str) -> String {
    if starts_with_ignore_case(name, "Light") {
        return name.to_string();
    }
    let lighter = if starts_with_ignore_case(name, "Dark") {
        name[4..].to_string()
    } else {
        format!("Light{name}")
    };
    match lookup(&lighter) {
        Some(_) => lighter,
        None => name.to_string(),
    }
}

fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.is_char_boundary(prefix.len())
        && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_129_names() {
        assert_eq!(COLORS.len(), 129);
        assert_eq!(lookup("orange").map(|c| c.ansi8), Some(214));
        assert_eq!(lookup("RED").map(|c| c.native), Some("Red"));
    }

    #[test]
    fn conversions_match_the_powershell_module() {
        assert_eq!(hex_to_rgb("#FF8000"), Some([255, 128, 0]));
        assert_eq!(hex_to_rgb("0xff8000"), Some([255, 128, 0]));
        assert_eq!(hex_to_rgb("GGGGGG"), None);
        assert_eq!(hex_to_rgb("0XFF8000"), Some([255, 128, 0]));
        assert_eq!(hex_to_rgb("FF8000\n"), Some([255, 128, 0]));
        assert_eq!(hex_to_rgb("#0xFF8000"), None);
        assert!(is_hex_text("0Xff8000"));
        assert!(!is_hex_text("Red"));
        assert_eq!(rgb_to_ansi8([255, 128, 0]), 208);
        assert_eq!(rgb_to_ansi8([300, 0, 0]), 196);
        assert_eq!(rgb_to_ansi8([0, 0, 0]), 16);
        assert_eq!(rgb_to_ansi8([255, 255, 255]), 231);
        assert_eq!(rgb_to_ansi4([255, 128, 0]), 93);
        assert_eq!(lighter_rgb([139, 0, 0], 1.4), [195, 102, 102]);
        assert_eq!(lighter_rgb([50, 50, 50], 2.0), [255, 255, 255]);
        assert_eq!(lighter_ansi8(196, 1.4), 203);
        assert_eq!(lighter_ansi8(240, 1.4), 244);
        assert_eq!(lighter_ansi8(4, 1.4), 12);
        assert_eq!(lighter_ansi8(9, 1.4), 203);
        assert_eq!(lighter_ansi8(100, 1.6), 186);
        assert_eq!(lighter_name("DarkRed"), "Red");
        assert_eq!(lighter_name("Red"), "LightRed");
        assert_eq!(lighter_name("White"), "White");
        assert_eq!(lighter_name("LightRed"), "LightRed");
        assert_eq!(ansi8_to_ansi4(9), 91);
        assert_eq!(ansi8_to_ansi4(208), 93);
        assert_eq!(ansi8_to_ansi4(0), 30);
        assert_eq!(ansi4_to_native(41), "DarkRed");
    }

    #[test]
    fn registered_names_are_added_replaced_removed_and_cleared() {
        let teal = Entry { native: "DarkCyan", ansi4_fg: 36, ansi4_bg: 46, ansi8: 30, rgb: [0, 128, 128] };
        let gold = Entry { native: "Yellow", ansi4_fg: 93, ansi4_bg: 103, ansi8: 220, rgb: [255, 215, 0] };
        register("ZzFirstTestName", teal);
        register("ZzSecondTestName", gold);
        assert_eq!(lookup("zzfirsttestname"), Some(teal));
        register("ZZFIRSTTESTNAME", gold);
        let mine: Vec<(String, Entry)> =
            registered().into_iter().filter(|(n, _)| n == "ZzFirstTestName" || n == "ZzSecondTestName").collect();
        assert_eq!(mine, [("ZzFirstTestName".to_string(), gold), ("ZzSecondTestName".to_string(), gold)]);
        assert!(unregister("zzsecondtestname"));
        assert!(!unregister("ZzSecondTestName"));
        assert_eq!(lookup("ZzSecondTestName"), None);
        clear_registered();
        assert_eq!(lookup("ZzFirstTestName"), None);
        assert_eq!(lookup("Orange").map(|c| c.ansi8), Some(214));
    }

    #[test]
    fn lookups_read_the_registered_names_while_they_change() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let magenta = Entry { native: "Magenta", ansi4_fg: 95, ansi4_bg: 105, ansi8: 201, rgb: [255, 0, 255] };
        let done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    while !done.load(Ordering::Acquire) {
                        let found = lookup("ZzChangingTestName");
                        assert!(found.is_none() || found == Some(magenta));
                    }
                });
            }
            for _ in 0..500 {
                register("ZzChangingTestName", magenta);
                assert!(unregister("ZzChangingTestName") || lookup("ZzChangingTestName").is_none());
            }
            done.store(true, Ordering::Release);
        });
    }
}
