//! The spaces a gradient blends colors in, each to and from sRGB, and the blend of two colors in
//! one: RGB, each channel on its own; linear light; OKLab; CIELAB with D50 white, as CSS lab()
//! defines it; and around the hue wheel in OKLCH, CIE LCh, HSL and HSV, by CSS Color 4's hue
//! paths. The arithmetic is +, -, *, / and square roots, with arith's sine, cosine and arctangent,
//! in the order PSWriteColorEX's ColorMath writes it.

use crate::arith;
use crate::colors::round_even;
use crate::oklab;

/// A space a gradient blends in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Space {
    #[default]
    OkLab,
    Rgb,
    LinearRgb,
    OkLch,
    Hsl,
    Hsv,
    Lab,
    Lch,
}

impl Space {
    /// Every space, in the order -GradientSpace lists them.
    pub const ALL: [Space; 8] = [Space::OkLab, Space::Rgb, Space::LinearRgb, Space::OkLch, Space::Hsl, Space::Hsv, Space::Lab, Space::Lch];

    /// The name -GradientSpace takes.
    pub fn name(self) -> &'static str {
        match self {
            Space::OkLab => "OKLab",
            Space::Rgb => "RGB",
            Space::LinearRgb => "LinearRGB",
            Space::OkLch => "OKLCH",
            Space::Hsl => "HSL",
            Space::Hsv => "HSV",
            Space::Lab => "Lab",
            Space::Lch => "LCh",
        }
    }

    /// The space a name means, compared without regard to case.
    pub fn from_name(name: &str) -> Option<Space> {
        Space::ALL.into_iter().find(|space| space.name().eq_ignore_ascii_case(name))
    }
}

/// The way around the hue wheel from one hue to another, CSS Color 4's hue interpolation methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HuePath {
    #[default]
    Shorter,
    Longer,
    Increasing,
    Decreasing,
}

impl HuePath {
    /// Every path, in the order -GradientHue lists them.
    pub const ALL: [HuePath; 4] = [HuePath::Shorter, HuePath::Longer, HuePath::Increasing, HuePath::Decreasing];

    /// The name -GradientHue takes.
    pub fn name(self) -> &'static str {
        match self {
            HuePath::Shorter => "Shorter",
            HuePath::Longer => "Longer",
            HuePath::Increasing => "Increasing",
            HuePath::Decreasing => "Decreasing",
        }
    }

    /// The path a name means, compared without regard to case.
    pub fn from_name(name: &str) -> Option<HuePath> {
        HuePath::ALL.into_iter().find(|path| path.name().eq_ignore_ascii_case(name))
    }
}

/// A color in a space. The round spaces keep their hue last, in degrees from 0 to 360: OKLCH and
/// LCh hold lightness, chroma and hue; HSL saturation, lightness and hue; HSV saturation, value
/// and hue, the saturations and lightness and value from 0 to 1. A color with no chroma has no
/// hue, and a blend takes the other color's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coordinates {
    pub values: [f64; 3],
    pub hueless: bool,
}

/// Degrees in a radian and radians in a degree.
const DEGREES_PER_RADIAN: f64 = f64::from_bits(0x404CA5DC1A63C1F8);
const RADIANS_PER_DEGREE: f64 = f64::from_bits(0x3F91DF46A2529D39);

/// Below these chromas a color counts as gray, with no hue: 2^-20 in OKLab and 2^-10 in CIELAB,
/// above the rounding left in an sRGB gray and below the chroma of any color one step from one.
const OKLCH_HUELESS: f64 = 1.0 / 1048576.0;
const LCH_HUELESS: f64 = 1.0 / 1024.0;

/// CSS Color 4's matrices: linear sRGB to CIE XYZ with D65 white and back, and the Bradford
/// adaptation from D65 to D50 and back.
const TO_XYZ: [[f64; 3]; 3] = [
    [f64::from_bits(0x3FDA649C610130B1), f64::from_bits(0x3FD6E2A96CCDCB17), f64::from_bits(0x3FC719FE95DEFF92)],
    [f64::from_bits(0x3FCB37C144093A36), f64::from_bits(0x3FE6E2A96CCDCB17), f64::from_bits(0x3FB27B32117F32DB)],
    [f64::from_bits(0x3F93CB7548C0E484), f64::from_bits(0x3FBE838C9112641F), f64::from_bits(0x3FEE6AC26776AE5E)],
];
const FROM_XYZ: [[f64; 3]; 3] = [
    [f64::from_bits(0x4009ED81A61E643A), -f64::from_bits(0x3FF8991F1A515886), -f64::from_bits(0x3FDFE93D1B3918E5)],
    [-f64::from_bits(0x3FEF040B3AF5D706), f64::from_bits(0x3FFE03F67FB55A13), f64::from_bits(0x3FA546B459182D75)],
    [f64::from_bits(0x3FAC7B8BB9F1E66D), -f64::from_bits(0x3FCA1BEABFC8688F), f64::from_bits(0x3FF0E95AF667A0D1)],
];
const D65_TO_D50: [[f64; 3]; 3] = [
    [f64::from_bits(0x3FF0C45207B7C1D9), f64::from_bits(0x3F977F626B1E2A0E), -f64::from_bits(0x3FA9B2CCFC72D22E)],
    [f64::from_bits(0x3F9E56C096E0CD1E), f64::from_bits(0x3FEFB1A389F7B189), -f64::from_bits(0x3F917BCB426228F0)],
    [-f64::from_bits(0x3F82EE03EA54107A), f64::from_bits(0x3F8ED54198AFD11C), f64::from_bits(0x3FE80F5AA72DFEC3)],
];
const D50_TO_D65: [[f64; 3]; 3] = [
    [f64::from_bits(0x3FEE933CFF2FA1D7), -f64::from_bits(0x3F97A71F125F4EBC), f64::from_bits(0x3FB031C1FCB7565E)],
    [-f64::from_bits(0x3F9D0CF2F724D576), f64::from_bits(0x3FF028F0EF3DEA4D), f64::from_bits(0x3F958BE3371EB644)],
    [f64::from_bits(0x3F8938171900D5F0), -f64::from_bits(0x3F94FFF50C39F78C), f64::from_bits(0x3FF5492DC80EC342)],
];

/// D50 white's X and Z, Y being 1: 0.3457 / 0.3585 and (1 - 0.3457 - 0.3585) / 0.3585, as CSS
/// writes them.
const WHITE_X: f64 = f64::from_bits(0x3FEEDB829B3E0DDB);
const WHITE_Z: f64 = f64::from_bits(0x3FEA6741C471F7DC);

/// CIELAB's 216/24389 and 24389/27.
const EPSILON: f64 = f64::from_bits(0x3F822354D28F7CD6);
const KAPPA: f64 = f64::from_bits(0x408C3A5ED097B426);

/// A matrix times a column, each row's products added left to right.
fn multiply(matrix: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        matrix[0][0] * v[0] + matrix[0][1] * v[1] + matrix[0][2] * v[2],
        matrix[1][0] * v[0] + matrix[1][1] * v[1] + matrix[1][2] * v[2],
        matrix[2][0] * v[0] + matrix[2][1] * v[1] + matrix[2][2] * v[2],
    ]
}

/// The CIELAB coordinates L, a and b of an sRGB color, with D50 white.
pub fn to_lab(rgb: [i64; 3]) -> [f64; 3] {
    let d50 = multiply(&D65_TO_D50, multiply(&TO_XYZ, rgb.map(oklab::srgb_to_linear)));
    let f = [d50[0] / WHITE_X, d50[1], d50[2] / WHITE_Z].map(|v| if v > EPSILON { oklab::cube_root(v) } else { (KAPPA * v + 16.0) / 116.0 });
    [116.0 * f[1] - 16.0, 500.0 * (f[0] - f[1]), 200.0 * (f[1] - f[2])]
}

/// The sRGB channel values 0-255 of CIELAB coordinates with D50 white, out-of-range channels
/// clamped.
pub fn from_lab(lab: [f64; 3]) -> [i64; 3] {
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = lab[1] / 500.0 + fy;
    let fz = fy - lab[2] / 200.0;
    let x = if fx * fx * fx > EPSILON { fx * fx * fx } else { (116.0 * fx - 16.0) / KAPPA };
    let y = if lab[0] > KAPPA * EPSILON { fy * fy * fy } else { lab[0] / KAPPA };
    let z = if fz * fz * fz > EPSILON { fz * fz * fz } else { (116.0 * fz - 16.0) / KAPPA };
    multiply(&FROM_XYZ, multiply(&D50_TO_D65, [x * WHITE_X, y, z * WHITE_Z])).map(oklab::from_linear)
}

/// Lightness, chroma and hue from a lightness and two opponent axes.
fn polar(lab: [f64; 3], hueless_below: f64) -> Coordinates {
    let chroma = (lab[1] * lab[1] + lab[2] * lab[2]).sqrt();
    let mut hue = arith::atan2(lab[2], lab[1]) * DEGREES_PER_RADIAN;
    if hue < 0.0 {
        hue += 360.0;
    }
    Coordinates { values: [lab[0], chroma, hue], hueless: chroma < hueless_below }
}

/// A lightness and two opponent axes from lightness, chroma and hue.
fn from_polar(lightness: f64, chroma: f64, hue: f64) -> [f64; 3] {
    let radians = hue * RADIANS_PER_DEGREE;
    [lightness, chroma * arith::cos(radians), chroma * arith::sin(radians)]
}

/// The hue in degrees of channels from 0 to 1 whose largest is `max`, `range` above the smallest.
fn sextant_hue(r: f64, g: f64, b: f64, max: f64, range: f64) -> f64 {
    let sextant = if max == r {
        (g - b) / range + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / range + 2.0
    } else {
        (r - g) / range + 4.0
    };
    sextant * 60.0
}

fn to_hsl(rgb: [i64; 3]) -> Coordinates {
    let [r, g, b] = rgb.map(|c| c as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let range = max - min;
    if range == 0.0 {
        return Coordinates { values: [0.0, lightness, 0.0], hueless: true };
    }
    let saturation = range / (1.0 - (2.0 * lightness - 1.0).abs());
    Coordinates { values: [saturation, lightness, sextant_hue(r, g, b, max, range)], hueless: false }
}

fn to_hsv(rgb: [i64; 3]) -> Coordinates {
    let [r, g, b] = rgb.map(|c| c as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let range = max - min;
    if range == 0.0 {
        return Coordinates { values: [0.0, max, 0.0], hueless: true };
    }
    Coordinates { values: [range / max, max, sextant_hue(r, g, b, max, range)], hueless: false }
}

/// The sRGB channel values of a hue in degrees from 0 to 360 and a saturation and value from 0
/// to 1: C = V * S, X = C * (1 - |(H / 60) mod 2 - 1|), m = V - C, each channel rounded half to
/// even after multiplying by 255.
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [i64; 3] {
    let chroma = v * s;
    let sector = h / 60.0;
    let x = chroma * (1.0 - ((sector - 2.0 * (sector / 2.0).floor()) - 1.0).abs());
    let offset = v - chroma;
    let parts = match sector.floor() as i64 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    parts.map(|part| round_even((part + offset) * 255.0))
}

/// A color's coordinates in a space.
pub fn to_coordinates(rgb: [i64; 3], space: Space) -> Coordinates {
    let flat = |values: [f64; 3]| Coordinates { values, hueless: false };
    match space {
        Space::Rgb => flat(rgb.map(|c| c as f64)),
        Space::LinearRgb => flat(rgb.map(oklab::srgb_to_linear)),
        Space::OkLab => flat(oklab::to_oklab(rgb)),
        Space::Lab => flat(to_lab(rgb)),
        Space::OkLch => polar(oklab::to_oklab(rgb), OKLCH_HUELESS),
        Space::Lch => polar(to_lab(rgb), LCH_HUELESS),
        Space::Hsl => to_hsl(rgb),
        Space::Hsv => to_hsv(rgb),
    }
}

/// The two hues a blend runs between, after CSS Color 4's fix-up for the path: a hueless color
/// takes the other's hue, and one hue moves a turn so the blend goes the chosen way round.
fn hue_ends(a: &Coordinates, b: &Coordinates, path: HuePath) -> (f64, f64) {
    let (mut h1, mut h2) = match (a.hueless, b.hueless) {
        (true, true) => (0.0, 0.0),
        (true, false) => (b.values[2], b.values[2]),
        (false, true) => (a.values[2], a.values[2]),
        (false, false) => (a.values[2], b.values[2]),
    };
    let difference = h2 - h1;
    match path {
        HuePath::Shorter => {
            if difference > 180.0 {
                h1 += 360.0;
            } else if difference < -180.0 {
                h2 += 360.0;
            }
        }
        HuePath::Longer => {
            if difference > 0.0 && difference < 180.0 {
                h1 += 360.0;
            } else if difference > -180.0 && difference <= 0.0 {
                h2 += 360.0;
            }
        }
        HuePath::Increasing => {
            if difference < 0.0 {
                h2 += 360.0;
            }
        }
        HuePath::Decreasing => {
            if difference > 0.0 {
                h1 += 360.0;
            }
        }
    }
    (h1, h2)
}

/// The color at `ratio` along the blend from one color to another, given both colors and their
/// coordinates in `space`: exactly the first at 0 and the second at 1.
pub fn blend(from: [i64; 3], to: [i64; 3], a: &Coordinates, b: &Coordinates, ratio: f64, space: Space, path: HuePath) -> [i64; 3] {
    if ratio == 0.0 {
        return from;
    }
    if ratio == 1.0 {
        return to;
    }
    let mix = |k: usize| a.values[k] + (b.values[k] - a.values[k]) * ratio;
    match space {
        Space::Rgb => [round_even(mix(0)), round_even(mix(1)), round_even(mix(2))],
        Space::LinearRgb => [oklab::from_linear(mix(0)), oklab::from_linear(mix(1)), oklab::from_linear(mix(2))],
        Space::OkLab => oklab::from_oklab([mix(0), mix(1), mix(2)]),
        Space::Lab => from_lab([mix(0), mix(1), mix(2)]),
        Space::OkLch | Space::Lch | Space::Hsl | Space::Hsv => {
            let (h1, h2) = hue_ends(a, b, path);
            let turned = h1 + (h2 - h1) * ratio;
            let hue = turned - 360.0 * (turned / 360.0).floor();
            match space {
                Space::OkLch => oklab::from_oklab(from_polar(mix(0), mix(1), hue)),
                Space::Lch => from_lab(from_polar(mix(0), mix(1), hue)),
                Space::Hsl => oklab::hsl_to_rgb(hue, mix(0), mix(1)),
                _ => hsv_to_rgb(hue, mix(0), mix(1)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, within: f64) -> bool {
        (a - b).abs() <= within
    }

    #[test]
    fn lab_matches_css_for_srgb_red_and_comes_back() {
        let lab = to_lab([255, 0, 0]);
        assert!(near(lab[0], 54.2905, 0.001) && near(lab[1], 80.8049, 0.001) && near(lab[2], 69.8910, 0.001), "{lab:?}");
        for rgb in [[255, 0, 0], [0, 128, 255], [17, 200, 99], [255, 255, 255], [0, 0, 0], [128, 128, 128], [250, 3, 180]] {
            assert_eq!(from_lab(to_lab(rgb)), rgb, "{rgb:?}");
        }
    }

    /// The references are Color.js's, the CSS Color 4 editors' library: sRGB red is oklch(0.627955
    /// 0.257683 29.2339) and lch(54.2905 106.8372 40.8577).
    #[test]
    fn round_spaces_read_red_as_css_does() {
        let oklch = to_coordinates([255, 0, 0], Space::OkLch);
        assert!(near(oklch.values[0], 0.627955, 0.000001) && near(oklch.values[1], 0.257683, 0.000001) && near(oklch.values[2], 29.2339, 0.0001), "{oklch:?}");
        assert_eq!(to_coordinates([255, 0, 0], Space::Hsl).values, [1.0, 0.5, 0.0]);
        assert_eq!(to_coordinates([255, 0, 0], Space::Hsv).values, [1.0, 1.0, 0.0]);
        let lch = to_coordinates([255, 0, 0], Space::Lch);
        assert!(near(lch.values[0], 54.2905, 0.001) && near(lch.values[1], 106.8372, 0.001) && near(lch.values[2], 40.8577, 0.001), "{lch:?}");
    }

    #[test]
    fn grays_have_no_hue_and_a_step_from_gray_has_one() {
        for level in 0..=255 {
            for space in [Space::OkLch, Space::Lch, Space::Hsl, Space::Hsv] {
                assert!(to_coordinates([level, level, level], space).hueless, "{level} in {space:?}");
            }
        }
        for rgb in [[128, 128, 129], [0, 0, 1], [254, 255, 255], [128, 129, 128]] {
            for space in [Space::OkLch, Space::Lch, Space::Hsl, Space::Hsv] {
                assert!(!to_coordinates(rgb, space).hueless, "{rgb:?} in {space:?}");
            }
        }
    }

    #[test]
    fn hsv_and_hsl_come_back() {
        for rgb in [[255, 0, 0], [0, 128, 255], [17, 200, 99], [250, 3, 180], [12, 34, 56]] {
            let hsv = to_coordinates(rgb, Space::Hsv).values;
            assert_eq!(hsv_to_rgb(hsv[2], hsv[0], hsv[1]), rgb, "{rgb:?}");
            let hsl = to_coordinates(rgb, Space::Hsl).values;
            assert_eq!(oklab::hsl_to_rgb(hsl[2], hsl[0], hsl[1]), rgb, "{rgb:?}");
        }
    }

    #[test]
    fn hue_paths_go_the_way_css_says() {
        let red = to_coordinates([255, 0, 0], Space::Hsl);
        let blue = to_coordinates([0, 0, 255], Space::Hsl);
        let mid = |path| blend([255, 0, 0], [0, 0, 255], &red, &blue, 0.5, Space::Hsl, path);
        assert_eq!(mid(HuePath::Shorter), [255, 0, 255], "red to blue the short way passes magenta");
        assert_eq!(mid(HuePath::Longer), [0, 255, 0], "the long way passes green");
        assert_eq!(mid(HuePath::Increasing), [0, 255, 0], "0 to 240 increasing passes 120");
        assert_eq!(mid(HuePath::Decreasing), [255, 0, 255], "0 to 240 decreasing passes 300");
        let gray = to_coordinates([128, 128, 128], Space::Hsl);
        let towards_red = blend([128, 128, 128], [255, 0, 0], &gray, &red, 0.5, Space::Hsl, HuePath::Shorter);
        assert!(towards_red[0] > towards_red[1] && towards_red[1] == towards_red[2], "a gray takes red's hue: {towards_red:?}");
    }

    #[test]
    fn the_ends_of_a_blend_are_the_colors_themselves() {
        for space in Space::ALL {
            let (from, to) = ([12, 200, 99], [250, 3, 180]);
            let (a, b) = (to_coordinates(from, space), to_coordinates(to, space));
            assert_eq!(blend(from, to, &a, &b, 0.0, space, HuePath::Shorter), from);
            assert_eq!(blend(from, to, &a, &b, 1.0, space, HuePath::Shorter), to);
        }
    }

    #[test]
    fn names_compare_without_case() {
        assert_eq!(Space::from_name("oklch"), Some(Space::OkLch));
        assert_eq!(Space::from_name("LINEARRGB"), Some(Space::LinearRgb));
        assert_eq!(Space::from_name("HSB"), None);
        assert_eq!(HuePath::from_name("longer"), Some(HuePath::Longer));
    }
}
