//! The color conversion cmdlets: hex codes to RGB values, RGB values to ANSI codes, the color
//! table, and lighter forms of a color.

use pwrs::prelude::*;

use crate::colors;

/// A channel of an RGB array as PowerShell reads a missing one: 0.
fn channel(rgb: &[i32], index: usize) -> i64 {
    rgb.get(index).copied().map(i64::from).unwrap_or(0)
}

fn rgb_of(rgb: &[i32]) -> [i64; 3] {
    [channel(rgb, 0), channel(rgb, 1), channel(rgb, 2)]
}

/// A value the cmdlets write as System.Int32, as PSWriteColorEX's functions do.
fn int32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

/// Converts a hex color code to RGB values.
///
/// Converts #RRGGBB, 0xRRGGBB or RRGGBB to its red, green and blue values, 0-255, written as
/// three integers. An invalid code gives a warning and gray, 128 128 128.
///
/// # Examples
/// Convert-HexToRGB '#FF8000'
#[cmdlet(verb = "Convert", noun = "HexToRGB", alias = ["CHR", "Hex2RGB"], output = ["System.Int32"])]
#[derive(Default)]
pub struct ConvertHexToRGB {
    /// The hex color code to convert.
    #[param(mandatory, position = 0)]
    pub hex: String,
}

impl Cmdlet for ConvertHexToRGB {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        for value in crate::write::hex_rgb(ps, &self.hex)? {
            ps.write(int32(value))?;
        }
        Ok(())
    }
}

/// Converts RGB values to the nearest ANSI 256-color code.
///
/// A color whose channels are within 10 of each other maps to the 24 grays (232-255), or to 16
/// (black) and 231 (white) at the ends. Any other color maps to the 6x6x6 color cube (16-231).
/// Values outside 0-255 are clamped.
///
/// # Examples
/// Convert-RGBToANSI8 @(255, 128, 0)
#[cmdlet(verb = "Convert", noun = "RGBToANSI8", alias = ["CRA8", "RGB2ANSI8"], output = ["System.Int32"])]
#[derive(Default)]
#[allow(non_snake_case)]
pub struct ConvertRGBToANSI8 {
    /// The red, green and blue values.
    #[param(mandatory, position = 0)]
    pub RGB: Vec<i32>,
}

impl Cmdlet for ConvertRGBToANSI8 {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        ps.write(int32(colors::rgb_to_ansi8(rgb_of(&self.RGB))))
    }
}

/// Converts RGB values to the nearest ANSI 16-color foreground code.
///
/// Answers 30-37 for the normal colors and 90-97 for the bright ones, by the strongest channel
/// and whether it reaches 200. Grays map to black, dark gray, gray or white by brightness.
///
/// # Examples
/// Convert-RGBToANSI4 @(255, 0, 0)
#[cmdlet(verb = "Convert", noun = "RGBToANSI4", alias = ["CRA4", "RGB2ANSI4"], output = ["System.Int32"])]
#[derive(Default)]
#[allow(non_snake_case)]
pub struct ConvertRGBToANSI4 {
    /// The red, green and blue values.
    #[param(mandatory, position = 0)]
    pub RGB: Vec<i32>,
}

impl Cmdlet for ConvertRGBToANSI4 {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        ps.write(int32(colors::rgb_to_ansi4(rgb_of(&self.RGB))))
    }
}

/// Gets the color table: every color name with its value in each color mode.
///
/// A hashtable of the 129 color names, compared without regard to case. Each value is an array:
/// the nearest console color, the ANSI4 foreground code, the ANSI4 background code, the ANSI8
/// code, and the RGB values.
///
/// # Examples
/// (Get-ColorTableWithRGB)['Orange']
#[cmdlet(verb = "Get", noun = "ColorTableWithRGB", alias = ["GCT", "Get-ColorTable", "Get-ColourTable"], output = ["System.Collections.Hashtable"])]
#[derive(Default)]
pub struct GetColorTableWithRGB {}

impl Cmdlet for GetColorTableWithRGB {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let table = crate::host::new_table()?;
        for (name, color) in colors::table() {
            let rgb: Vec<PsObject> = color.rgb.iter().map(|&v| i32::from(v).into_ps()).collect::<PsResult<_>>()?;
            let entry = vec![
                color.native.into_ps()?,
                i32::from(color.ansi4_fg).into_ps()?,
                i32::from(color.ansi4_bg).into_ps()?,
                i32::from(color.ansi8).into_ps()?,
                PsArray(rgb).into_ps()?,
            ];
            table.set(&name, PsArray(entry).into_ps()?)?;
        }
        ps.write_object(&table.0)
    }
}

/// Makes RGB values lighter.
///
/// Multiplies each channel by the factor, raises it to at least 255 x (factor - 1) so black
/// becomes dark gray, and caps it at 255. Writes three integers.
///
/// # Examples
/// Get-LighterRGBColor @(139, 0, 0)
/// Get-LighterRGBColor @(100, 100, 100) -Factor 1.8
#[cmdlet(verb = "Get", noun = "LighterRGBColor", alias = ["Lighten-RGBColor"], output = ["System.Int32"])]
#[derive(Default)]
#[allow(non_snake_case)]
pub struct GetLighterRGBColor {
    /// The red, green and blue values.
    #[param(mandatory, position = 0)]
    pub RGB: Vec<i32>,
    /// How much lighter: each channel is multiplied by it. 1.4 when left out.
    #[param(position = 1)]
    pub factor: Option<f64>,
}

impl Cmdlet for GetLighterRGBColor {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        for value in colors::lighter_rgb(rgb_of(&self.RGB), self.factor.unwrap_or(1.4)) {
            ps.write(int32(value))?;
        }
        Ok(())
    }
}

/// Gets the next lighter color name in a color's family.
///
/// A Dark name loses Dark (DarkRed to Red), and any other name gains Light (Red to LightRed),
/// when the color table has the result. A Light name, or a name with no lighter name in the color
/// table, such as White or LightRed, is returned unchanged.
///
/// # Examples
/// Get-LighterColorName 'DarkBlue'
#[cmdlet(verb = "Get", noun = "LighterColorName", alias = ["Lighten-ColorName"], output = ["System.String"])]
#[derive(Default)]
pub struct GetLighterColorName {
    /// The color name to lighten.
    #[param(mandatory, position = 0)]
    pub color_name: String,
}

impl Cmdlet for GetLighterColorName {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        ps.write(colors::lighter_name(&self.color_name))
    }
}

/// Gets a lighter ANSI 256-color code.
///
/// A dark standard color (0-7) becomes its bright form (8-15), a gray (232-255) moves up the gray
/// ramp, and any other color is lightened through its RGB values with Get-LighterRGBColor.
///
/// # Examples
/// Get-LighterANSI8Color 4
/// Get-LighterANSI8Color 240
#[cmdlet(verb = "Get", noun = "LighterANSI8Color", alias = ["LA8", "Lighten-ANSI8", "Lighten-ANSI8Color"], output = ["System.Int32"])]
#[derive(Default)]
#[allow(non_snake_case)]
pub struct GetLighterANSI8Color {
    /// The ANSI 256-color code to lighten, 0-255.
    #[param(mandatory, position = 0, validate_range(0, 255))]
    pub ANSI8Code: i32,
    /// How much lighter. 1.4 when left out.
    #[param(position = 1)]
    pub factor: Option<f64>,
}

impl Cmdlet for GetLighterANSI8Color {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        ps.write(int32(colors::lighter_ansi8(i64::from(self.ANSI8Code), self.factor.unwrap_or(1.4))))
    }
}
