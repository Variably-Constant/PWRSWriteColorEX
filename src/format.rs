//! Format-ColorEX: Write-ColorEX's text, color, style and width parameters, with each line
//! returned as a string instead of written to the host, as PSWriteColorEX's.

use pwrs::prelude::*;

use crate::detect::Support;
use crate::session;
use crate::write::{begin_checks, bound_for_profile, caller_script_root, color_parameter, gradient_list, render, style_argument, WriteOptions};

/// Returns text with its colors and styles as escape codes, as Write-ColorEX would write it.
///
/// Format-ColorEX takes Write-ColorEX's text, color, style and width parameters and returns each
/// line as a string instead of writing it to the host, for a string built from several pieces, a
/// table column, a file or another command. The strings hold escape codes wherever the host shows
/// them, by the same detection and color variables Write-ColorEX uses. Where it would write
/// console colors instead, or with colors turned off, the strings are the text alone.
/// -ColorSupport gives the color support to format for in place of the detection and the color
/// variables, so the strings are the same wherever they are made. -Wrap returns one string per
/// line, and a style profile's -LinesBefore and -LinesAfter return empty strings.
///
/// # Examples
/// $status = Format-ColorEX -Text 'OK' -Color Green -Bold
/// "Result: $status"
/// Format-ColorEX -Text '[red]x[/] failed' -Markup
/// Format-ColorEX -Text 'Done' -Color Green -ColorSupport ANSI8
#[cmdlet(verb = "Format", noun = "ColorEX", alias = ["Format-ColourEX", "FCEX"], output = ["System.String"])]
#[derive(Default)]
#[allow(non_snake_case)]
pub struct FormatColorEX {
    /// The text. Several strings make one line, each with its own colors and styles. Strings
    /// piped in are formatted one line each.
    #[param(position = 0, value_from_pipeline, alias = ["T"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub text: Option<Vec<String>>,
    /// The text color of each segment, as Write-ColorEX's -Color takes it.
    #[param(position = 1, alias = ["C", "ForegroundColor", "FGC"], clr = "System.Array", attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub color: Option<PsObject>,
    /// The background color of each segment, as Write-ColorEX's -BackGroundColor takes it.
    #[param(position = 2, alias = ["B", "BGC"], clr = "System.Array", attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub back_ground_color: Option<PsObject>,
    /// Two or more colors to blend across the text, as Write-ColorEX's -Gradient takes them.
    #[param(alias = ["Grad"], allow_null, attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub gradient: Option<Vec<PsObject>>,
    /// Two or more colors to blend across the background, as Write-ColorEX's -BackGroundGradient
    /// takes them.
    #[param(alias = ["BGGrad"], allow_null, attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub back_ground_gradient: Option<Vec<PsObject>>,
    /// How the gradients blend their colors: OKLab (the default), RGB, LinearRGB, OKLCH, HSL, HSV,
    /// Lab or LCh, as Write-ColorEX's -GradientSpace takes them.
    #[param(validate_set = ["OKLab", "RGB", "LinearRGB", "OKLCH", "HSL", "HSV", "Lab", "LCh"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "OKLab")))]
    pub gradient_space: Option<String>,
    /// The way round the hue wheel in OKLCH, HSL, HSV and LCh: Shorter (the default), Longer,
    /// Increasing or Decreasing.
    #[param(validate_set = ["Shorter", "Longer", "Increasing", "Decreasing"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "Shorter")))]
    pub gradient_hue: Option<String>,
    /// A curve that moves the colors along the text, as Write-ColorEX's -GradientEasing takes it.
    #[param(validate_set = [
        "Linear", "InSine", "OutSine", "InOutSine", "InQuad", "OutQuad", "InOutQuad", "InCubic", "OutCubic", "InOutCubic", "InQuart", "OutQuart",
        "InOutQuart", "InQuint", "OutQuint", "InOutQuint", "InExpo", "OutExpo", "InOutExpo", "InCirc", "OutCirc", "InOutCirc", "InBack", "OutBack",
        "InOutBack", "InElastic", "OutElastic", "InOutElastic", "InBounce", "OutBounce", "InOutBounce"
    ], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "Linear")))]
    pub gradient_easing: Option<String>,
    /// What a gradient does past placed first and last colors: None (the default), Repeat or
    /// Mirror.
    #[param(validate_set = ["None", "Repeat", "Mirror"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "None")))]
    pub gradient_repeat: Option<String>,
    /// The way a gradient runs across the text: Forward (the default), Reverse, CenterOut or
    /// EdgesIn.
    #[param(validate_set = ["Forward", "Reverse", "CenterOut", "EdgesIn"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "Forward")))]
    pub gradient_direction: Option<String>,
    /// Cuts a gradient into this many bands of solid color; 0, the default, blends smoothly.
    #[param(validate_range(0, 2147483647), attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = 0)))]
    pub gradient_steps: i32,
    /// Uses ANSI 4-bit colors.
    #[param(alias = ["A4"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub ANSI4: bool,
    /// Uses ANSI 8-bit colors.
    #[param(alias = ["A8"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub ANSI8: bool,
    /// Uses 24-bit TrueColor.
    #[param(alias = ["A24", "TrueColor", "TC"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub ANSI24: bool,
    /// The color support to format for, in place of the detection and the color variables: None,
    /// ANSI4, ANSI8 or TrueColor, as Write-ColorEX's -ColorSupport takes it. ANSI4, ANSI8 and
    /// TrueColor give escape codes for that support, and None gives the text alone.
    #[param(validate_set = ["None", "ANSI4", "ANSI8", "TrueColor"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub color_support: Option<String>,
    /// The styles of each segment, as Write-ColorEX's -Style takes them.
    #[param(alias = ["S"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub style: Option<PsObject>,
    /// A PSColorStyle with colors, styles and layout settings. Parameters given take precedence.
    #[param(clr = "PSColorStyle", attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub style_profile: Option<PsObject>,
    /// Applies the default style set with Set-ColorDefault.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub default: bool,
    /// Bold text.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub bold: bool,
    /// Faint (dimmed) text.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub faint: bool,
    /// Italic text.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub italic: bool,
    /// Underlined text.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub underline: bool,
    /// Blinking text, where the terminal supports it.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub blink: bool,
    /// Text struck through.
    #[param(alias = ["Strikethrough"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub crossed_out: bool,
    /// Text underlined twice, where the terminal supports it.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub double_underline: bool,
    /// A line above the text, where the terminal supports it.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub overline: bool,
    /// Text and background colors swapped.
    #[param(alias = ["Invert"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub reverse: bool,
    /// The color of the underline of each segment.
    #[param(clr = "System.Array", attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub underline_color: Option<PsObject>,
    /// The kind of underline: Single, Double, Curly, Dotted or Dashed.
    #[param(validate_set = ["Single", "Double", "Curly", "Dotted", "Dashed"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub underline_style: Option<String>,
    /// Reads markup tags in the text, as Write-ColorEX's -Markup does.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub markup: bool,
    /// Cuts each string after each of these separators.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub split: Option<Vec<String>>,
    /// Cuts each string before and after each of these separators.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub split_around: Option<Vec<String>>,
    /// Cuts each string into as many parts as there are colors.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub split_evenly: bool,
    /// Colors and styles the text regular expressions match, as Write-ColorEX's -Highlight does.
    #[param(clr = "System.Collections.IDictionary", attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub highlight: Option<PsObject>,
    /// The address each segment links to.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub link: Option<Vec<String>>,
    /// The number of tab characters before the text.
    #[param(alias = ["Indent"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = 0)))]
    pub start_tab: i32,
    /// The number of spaces before the text, after any tabs.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = 0)))]
    pub start_spaces: i32,
    /// Pads the text to this display width.
    #[param(alias = ["PadWidth", "Pad"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = 0)))]
    pub auto_pad: i32,
    /// With -AutoPad, pads on the left (right-aligns the text).
    #[param(alias = ["RightAlign"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub pad_left: bool,
    /// With -AutoPad, centers the text.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub pad_center: bool,
    /// The character -AutoPad pads with.
    #[param(alias = ["PaddingChar", "FillChar"], attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = "")))]
    pub pad_char: Option<char>,
    /// With -AutoPad, cuts text wider than -AutoPad, ending it with an ellipsis.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub truncate: bool,
    /// Breaks text wider than the line into lines, one string each.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub wrap: bool,
    /// Writes [DEBUG] messages about color processing and detection.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub debugging: bool,
    /// Suppresses the warnings about colors and fallbacks.
    #[param(attribute(type = "System.Management.Automation.PSDefaultValueAttribute", named(Value = false)))]
    pub silent: bool,
}

/// The parameter names of Format-ColorEX that Write-ColorEX takes too, for which ones a call gave.
const PARAMETER_NAMES: &[&str] = &[
    "Color", "BackGroundColor", "Gradient", "BackGroundGradient", "GradientSpace", "GradientHue", "GradientEasing", "GradientRepeat", "GradientDirection",
    "GradientSteps", "ANSI4", "ANSI8", "ANSI24", "ColorSupport", "Style", "StyleProfile", "Default", "Bold", "Faint", "Italic", "Underline", "Blink", "CrossedOut",
    "DoubleUnderline", "Overline", "Reverse", "UnderlineColor", "UnderlineStyle", "Markup", "Split", "SplitAround", "SplitEvenly", "Highlight", "Link",
    "StartTab", "StartSpaces", "AutoPad", "PadLeft", "PadCenter", "PadChar", "Truncate", "Wrap", "Debugging", "Silent",
];

impl Cmdlet for FormatColorEX {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let _session = session::enter()?;
        // Write-ColorEX's checks run for each string, as each string is a call of its own
        let highlight = begin_checks(
            ps,
            self.style.as_ref(),
            [
                (self.color.as_ref(), "Color"),
                (self.back_ground_color.as_ref(), "BackGroundColor"),
                (self.underline_color.as_ref(), "UnderlineColor"),
            ],
            self.split_evenly,
            self.highlight.as_ref(),
            self.silent,
        )?;
        let applies_profile = self.default || self.style_profile.as_ref().is_some_and(|p| !p.is_null());
        let mut bound = bound_for_profile(ps, PARAMETER_NAMES, applies_profile)?;
        bound.insert("text".to_string());
        let options = WriteOptions {
            text: self.text.clone().unwrap_or_default(),
            color: color_parameter(self.color.as_ref(), "Color")?,
            background: color_parameter(self.back_ground_color.as_ref(), "BackGroundColor")?,
            gradient: match &self.gradient {
                Some(values) => Some(gradient_list(values)?),
                None => None,
            },
            background_gradient: match &self.back_ground_gradient {
                Some(values) => Some(gradient_list(values)?),
                None => None,
            },
            gradient_space: self.gradient_space.clone().unwrap_or_else(|| "OKLab".to_string()),
            gradient_hue: self.gradient_hue.clone().unwrap_or_else(|| "Shorter".to_string()),
            gradient_easing: self.gradient_easing.clone().unwrap_or_else(|| "Linear".to_string()),
            gradient_repeat: self.gradient_repeat.clone().unwrap_or_else(|| "None".to_string()),
            gradient_direction: self.gradient_direction.clone().unwrap_or_else(|| "Forward".to_string()),
            gradient_steps: self.gradient_steps,
            ansi4: self.ANSI4,
            ansi8: self.ANSI8,
            ansi24: self.ANSI24,
            color_support: self.color_support.as_deref().and_then(Support::from_name),
            style: style_argument(self.style.as_ref())?,
            style_profile: self.style_profile.clone().filter(|p| !p.is_null()),
            default: self.default,
            bold: self.bold,
            faint: self.faint,
            italic: self.italic,
            underline: self.underline,
            blink: self.blink,
            crossed_out: self.crossed_out,
            double_underline: self.double_underline,
            overline: self.overline,
            reverse: self.reverse,
            underline_color: color_parameter(self.underline_color.as_ref(), "UnderlineColor")?,
            underline_style: self.underline_style.clone().unwrap_or_default(),
            markup: self.markup,
            split: self.split.clone(),
            split_around: self.split_around.clone(),
            split_evenly: self.split_evenly,
            link: self.link.clone().unwrap_or_default().into_iter().map(Some).collect(),
            start_tab: self.start_tab,
            start_spaces: self.start_spaces,
            debugging: self.debugging,
            silent: self.silent,
            auto_pad: self.auto_pad,
            pad_left: self.pad_left,
            pad_center: self.pad_center,
            pad_char: self.pad_char.unwrap_or(' '),
            truncate: self.truncate,
            wrap: self.wrap,
            bound,
            caller_script_root: caller_script_root(ps),
            ..WriteOptions::default()
        };
        let mut lines = Vec::new();
        render(ps, options, &highlight, Some(&mut lines))?;
        for line in lines {
            ps.write(line.as_str())?;
        }
        Ok(())
    }
}
