//! Style profiles: the PSColorStyle class's methods as the cmdlets reach them, and the cmdlets
//! that make styles, set the default style and list the profiles.

use pwrs::prelude::*;

use crate::host::this_cmdlet;

/// The class the style cmdlets make and read, from src/csharp/PSColorStyle.cs.
fn style_type() -> PsType {
    PsType::from_name("PSColorStyle")
}

/// The Write-ColorEX parameters a style sets, read from its properties.
pub fn write_color_params(style: &PsObject) -> PsResult<PsHashtable> {
    Ok(PsHashtable(style.call("ToWriteColorParams", &[])?))
}

/// The style -Default applies, [PSColorStyle]::Default, when one is set.
pub fn default_style() -> PsResult<Option<PsObject>> {
    let style = style_type().call_static("get_Default", &[])?;
    Ok((!style.is_null()).then_some(style))
}

/// A new style with a name and its colors, as [PSColorStyle]::new(name, foreground, background).
pub fn new_style(name: &str, foreground: PsObject, background: PsObject) -> PsResult<PsObject> {
    style_type().new(&[name.into_ps()?, foreground, background])
}

/// A built-in profile from [PSColorStyle]::Profiles, or the style it starts as when it was
/// removed, as the Write-Color* helpers read it on each call.
pub fn helper_profile(name: &str) -> PsResult<PsObject> {
    let style = style_type().call_static("GetProfile", &[name.into_ps()?])?;
    if !style.is_null() {
        return Ok(style);
    }
    let null = PsObject::null;
    let (foreground, background) = match name {
        "Error" => ("Red", null()),
        "Warning" => ("Yellow", null()),
        "Info" => ("Cyan", null()),
        "Success" => ("Green", null()),
        "Critical" => ("White", "DarkRed".into_ps()?),
        _ => ("DarkGray", null()),
    };
    let style = new_style(name, foreground.into_ps()?, background)?;
    let flags: &[&str] = match name {
        "Error" => &["Bold"],
        "Critical" => &["Bold", "Blink"],
        "Debug" => &["Italic"],
        _ => &[],
    };
    for flag in flags {
        style.set(flag, &true.into_ps()?)?;
    }
    Ok(style)
}

/// Sets the default style -Default applies.
///
/// With -Style, the style given becomes [PSColorStyle]::Default. With the other parameters, or with
/// none, a new style named Default is made from them, becomes the default and replaces the Default
/// profile; with none it is plain Gray text.
///
/// # Examples
/// Set-ColorDefault -ForegroundColor Cyan -Bold
/// Set-ColorDefault -Style (New-ColorStyle -Name 'Mine' -ForegroundColor Green)
/// Set-ColorDefault
#[cmdlet(verb = "Set", noun = "ColorDefault", default_parameter_set = "Properties", alias = ["SCD", "Set-ColourDefault", "Set-DefaultColor", "Set-DefaultColour"])]
#[derive(Default)]
pub struct SetColorDefault {
    /// A PSColorStyle to make the default.
    #[param(set = "Object", clr = "PSColorStyle")]
    pub style: Option<PsObject>,
    /// The text color of the new default style: a color name, a hex code, an RGB array or an ANSI
    /// color number. Gray when left out.
    #[param(set = "Properties")]
    pub foreground_color: Option<PsObject>,
    /// The background color of the new default style.
    #[param(set = "Properties")]
    pub background_color: Option<PsObject>,
    /// Makes the default style bold.
    #[param(set = "Properties")]
    pub bold: bool,
    /// Makes the default style italic.
    #[param(set = "Properties")]
    pub italic: bool,
    /// Underlines the default style.
    #[param(set = "Properties")]
    pub underline: bool,
    /// Writes the time before the text with the default style.
    #[param(set = "Properties")]
    pub show_time: bool,
    /// The number of tabs before the text with the default style.
    #[param(set = "Properties")]
    pub start_tab: i32,
    /// The number of spaces before the text with the default style.
    #[param(set = "Properties")]
    pub start_spaces: i32,
}

impl Cmdlet for SetColorDefault {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let set = String::from_ps(&this_cmdlet(ps).get("ParameterSetName")?)?;
        if set == "Object" {
            return match &self.style {
                Some(style) if !style.is_null() => style.call("SetAsDefault", &[]).map(|_| ()),
                _ => Err(PsError::new(ErrorCategory::InvalidOperation, "InvokeMethodOnNull", "You cannot call a method on a null-valued expression.").terminating()),
            };
        }
        let foreground = if ps.parameter_is_bound("ForegroundColor") {
            self.foreground_color.clone().unwrap_or_default()
        } else {
            "Gray".into_ps()?
        };
        let background = self.background_color.clone().unwrap_or_default();
        let style = new_style("Default", foreground, background)?;
        style.set("Bold", &self.bold.into_ps()?)?;
        style.set("Italic", &self.italic.into_ps()?)?;
        style.set("Underline", &self.underline.into_ps()?)?;
        style.set("ShowTime", &self.show_time.into_ps()?)?;
        style.set("StartTab", &self.start_tab.into_ps()?)?;
        style.set("StartSpaces", &self.start_spaces.into_ps()?)?;
        style.call("SetAsDefault", &[])?;
        style.call("AddToProfiles", &[])?;
        Ok(())
    }
}

/// Gets the style profiles.
///
/// Without -Name, writes every style in [PSColorStyle]::Profiles. With -Name, writes the profile
/// of that name, or $null when there is none. The built-in profiles are Default, Error, Warning,
/// Info, Success, Critical and Debug.
///
/// # Examples
/// Get-ColorProfiles
/// Get-ColorProfiles -Name Error
#[cmdlet(verb = "Get", noun = "ColorProfiles", alias = ["GCP", "Get-ColourProfiles", "Get-Profiles", "gcprofiles"], output = ["PSColorStyle"])]
#[derive(Default)]
pub struct GetColorProfiles {
    /// The name of the profile to get, compared without regard to case.
    #[param(position = 0)]
    pub name: Option<String>,
}

impl Cmdlet for GetColorProfiles {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        match self.name.as_deref().filter(|name| !name.is_empty()) {
            Some(name) => ps.write_object(&style_type().call_static("GetProfile", &[name.into_ps()?])?),
            None => {
                let profiles = style_type().call_static("get_Profiles", &[])?;
                if profiles.is_null() {
                    return ps.write_object(&PsObject::null());
                }
                ps.write_enumerated(&profiles.get("Values")?)
            }
        }
    }
}

/// Creates a PSColorStyle: colors, styles and layout to reuse with Write-ColorEX -StyleProfile.
///
/// The style holds the Write-ColorEX settings given. -AddToProfiles adds it to
/// [PSColorStyle]::Profiles under its name, and -SetAsDefault makes it the style -Default applies.
///
/// # Examples
/// $header = New-ColorStyle -Name 'Header' -ForegroundColor Cyan -Bold -Underline
/// New-ColorStyle -Name 'Alert' -ForegroundColor '#FF6B35' -Bold -AddToProfiles
/// New-ColorStyle -Name 'Sunset' -Gradient @('Orange', 'Magenta') -SetAsDefault
#[cmdlet(verb = "New", noun = "ColorStyle", alias = ["NCS", "New-ColourStyle", "New-Style", "ncstyle"], output = ["PSColorStyle"])]
#[derive(Default)]
pub struct NewColorStyle {
    /// The name of the style, its key in [PSColorStyle]::Profiles.
    #[param(mandatory, position = 0)]
    pub name: String,
    /// The text color: a color name, a hex code, an RGB array or an ANSI color number. Gray when
    /// left out.
    #[param(position = 1)]
    pub foreground_color: Option<PsObject>,
    /// The background color, in the same forms as -ForegroundColor.
    #[param(position = 2)]
    pub background_color: Option<PsObject>,
    /// Two or more colors to blend across the text, in place of -ForegroundColor.
    #[param(position = 3)]
    pub gradient: Option<Vec<PsObject>>,
    /// Bold text.
    #[param]
    pub bold: bool,
    /// Italic text.
    #[param]
    pub italic: bool,
    /// Underlined text.
    #[param]
    pub underline: bool,
    /// Blinking text, where the terminal supports it.
    #[param]
    pub blink: bool,
    /// Faint (dimmed) text.
    #[param]
    pub faint: bool,
    /// Text struck through.
    #[param]
    pub crossed_out: bool,
    /// Text underlined twice, where the terminal supports it.
    #[param]
    pub double_underline: bool,
    /// A line above the text, where the terminal supports it.
    #[param]
    pub overline: bool,
    /// The time before the text.
    #[param]
    pub show_time: bool,
    /// The line left open, so the next output continues it.
    #[param]
    pub no_new_line: bool,
    /// The text centered in the console window.
    #[param]
    pub horizontal_center: bool,
    /// The number of tabs before the text.
    #[param(position = 4)]
    pub start_tab: i32,
    /// The number of spaces before the text.
    #[param(position = 5)]
    pub start_spaces: i32,
    /// The number of blank lines before the text.
    #[param(position = 6)]
    pub lines_before: i32,
    /// The number of blank lines after the text.
    #[param(position = 7)]
    pub lines_after: i32,
    /// The display width to pad the text to.
    #[param(position = 8)]
    pub auto_pad: i32,
    /// Pads on the left, right-aligning the text.
    #[param]
    pub pad_left: bool,
    /// The character to pad with.
    #[param(position = 9)]
    pub pad_char: Option<char>,
    /// Two or more colors to blend across the background. It replaces -BackgroundColor.
    #[param(position = 10)]
    pub background_gradient: Option<Vec<PsObject>>,
    /// How the gradients blend their colors: OKLab or RGB. Without it, Write-ColorEX's default,
    /// OKLab.
    #[param(position = 11, validate_set = ["OKLab", "RGB"])]
    pub gradient_space: Option<String>,
    /// Swaps the text and background colors.
    #[param]
    pub reverse: bool,
    /// The color of the underline.
    #[param(position = 12)]
    pub underline_color: Option<PsObject>,
    /// The kind of underline: Single, Double, Curly, Dotted or Dashed.
    #[param(position = 13, validate_set = ["Single", "Double", "Curly", "Dotted", "Dashed"])]
    pub underline_style: Option<String>,
    /// With -AutoPad, centers the text.
    #[param]
    pub pad_center: bool,
    /// With -AutoPad, cuts text wider than -AutoPad, ending it with an ellipsis.
    #[param]
    pub truncate: bool,
    /// Breaks text wider than the line into lines.
    #[param]
    pub wrap: bool,
    /// Adds the style to [PSColorStyle]::Profiles under its name.
    #[param]
    pub add_to_profiles: bool,
    /// Makes the style the one -Default applies.
    #[param]
    pub set_as_default: bool,
}

impl Cmdlet for NewColorStyle {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let foreground = if ps.parameter_is_bound("ForegroundColor") {
            self.foreground_color.clone().unwrap_or_default()
        } else {
            "Gray".into_ps()?
        };
        let background = self.background_color.clone().unwrap_or_default();
        let style = new_style(&self.name, foreground, background)?;
        let gradient = match &self.gradient {
            Some(values) => PsArray(values.clone()).into_ps()?,
            None => PsObject::null(),
        };
        style.set("Gradient", &gradient)?;
        let background_gradient = match &self.background_gradient {
            Some(values) => PsArray(values.clone()).into_ps()?,
            None => PsObject::null(),
        };
        style.set("BackgroundGradient", &background_gradient)?;
        style.set("GradientSpace", &self.gradient_space.clone().unwrap_or_default().into_ps()?)?;
        style.set("UnderlineColor", &self.underline_color.clone().unwrap_or_default())?;
        style.set("UnderlineStyle", &self.underline_style.clone().unwrap_or_default().into_ps()?)?;
        for (property, on) in [
            ("Bold", self.bold),
            ("Italic", self.italic),
            ("Underline", self.underline),
            ("Blink", self.blink),
            ("Faint", self.faint),
            ("CrossedOut", self.crossed_out),
            ("DoubleUnderline", self.double_underline),
            ("Overline", self.overline),
            ("ShowTime", self.show_time),
            ("NoNewLine", self.no_new_line),
            ("HorizontalCenter", self.horizontal_center),
            ("PadLeft", self.pad_left),
            ("Reverse", self.reverse),
            ("PadCenter", self.pad_center),
            ("Truncate", self.truncate),
            ("Wrap", self.wrap),
        ] {
            style.set(property, &on.into_ps()?)?;
        }
        for (property, value) in [
            ("StartTab", self.start_tab),
            ("StartSpaces", self.start_spaces),
            ("LinesBefore", self.lines_before),
            ("LinesAfter", self.lines_after),
            ("AutoPad", self.auto_pad),
        ] {
            style.set(property, &value.into_ps()?)?;
        }
        style.set("PadChar", &self.pad_char.unwrap_or(' ').into_ps()?)?;

        if self.add_to_profiles {
            style.call("AddToProfiles", &[])?;
        }
        if self.set_as_default {
            style.call("SetAsDefault", &[])?;
        }
        ps.write_object(&style)
    }
}
