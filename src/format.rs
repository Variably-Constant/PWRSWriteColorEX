//! Format-ColorEX: Write-ColorEX's text, color, style and width parameters, with each line
//! answered as a string instead of written to the host, as PSWriteColorEX's.

use std::collections::HashSet;

use pwrs::prelude::*;

use crate::write::{begin_checks, caller_script_root, color_parameter, gradient_list, render, style_argument, WriteOptions};

/// Answers text with its colors and styles as escape codes, as Write-ColorEX would write it.
///
/// Format-ColorEX takes Write-ColorEX's text, color, style and width parameters and answers each
/// line as a string instead of writing it to the host, for a string built from several pieces, a
/// table column, a file or another command. The strings hold escape codes wherever the host shows
/// them, by the same detection and color variables Write-ColorEX uses. Where it would write
/// console colors instead, or with colors turned off, the strings are the text alone. -Wrap
/// answers one string per line, and a style profile's -LinesBefore and -LinesAfter answer empty
/// strings.
///
/// # Examples
/// $status = Format-ColorEX -Text 'OK' -Color Green -Bold
/// "Result: $status"
/// Format-ColorEX -Text '[red]x[/] failed' -Markup
#[cmdlet(verb = "Format", noun = "ColorEX", alias = ["Format-ColourEX", "FCEX"], output = ["System.String"])]
#[derive(Default)]
#[allow(non_snake_case)]
pub struct FormatColorEX {
    /// The text. Several strings make one line, each with its own colors and styles. Strings
    /// piped in are formatted one line each.
    #[param(position = 0, value_from_pipeline, alias = ["T"])]
    pub text: Option<Vec<String>>,
    /// The text color of each segment, as Write-ColorEX's -Color takes it.
    #[param(position = 1, alias = ["C", "ForegroundColor", "FGC"], clr = "System.Array")]
    pub color: Option<PsObject>,
    /// The background color of each segment, as Write-ColorEX's -BackGroundColor takes it.
    #[param(position = 2, alias = ["B", "BGC"], clr = "System.Array")]
    pub back_ground_color: Option<PsObject>,
    /// Two or more colors to blend across the text, as Write-ColorEX's -Gradient takes them.
    #[param(alias = ["Grad"])]
    pub gradient: Option<Vec<PsObject>>,
    /// Two or more colors to blend across the background, as Write-ColorEX's -BackGroundGradient
    /// takes them.
    #[param(alias = ["BGGrad"])]
    pub back_ground_gradient: Option<Vec<PsObject>>,
    /// How the gradients blend their colors: OKLab (the default) or RGB.
    #[param(validate_set = ["OKLab", "RGB"])]
    pub gradient_space: Option<String>,
    /// Uses ANSI 4-bit colors.
    #[param(alias = ["A4"])]
    pub ANSI4: bool,
    /// Uses ANSI 8-bit colors.
    #[param(alias = ["A8"])]
    pub ANSI8: bool,
    /// Uses 24-bit TrueColor.
    #[param(alias = ["A24", "TrueColor", "TC"])]
    pub ANSI24: bool,
    /// The styles of each segment, as Write-ColorEX's -Style takes them.
    #[param(alias = ["S"])]
    pub style: Option<PsObject>,
    /// A PSColorStyle with colors, styles and layout settings. Parameters given take precedence.
    #[param(clr = "PSColorStyle")]
    pub style_profile: Option<PsObject>,
    /// Applies the default style set with Set-ColorDefault.
    #[param]
    pub default: bool,
    /// Bold text.
    #[param]
    pub bold: bool,
    /// Faint (dimmed) text.
    #[param]
    pub faint: bool,
    /// Italic text.
    #[param]
    pub italic: bool,
    /// Underlined text.
    #[param]
    pub underline: bool,
    /// Blinking text, where the terminal supports it.
    #[param]
    pub blink: bool,
    /// Text struck through.
    #[param(alias = ["Strikethrough"])]
    pub crossed_out: bool,
    /// Text underlined twice, where the terminal supports it.
    #[param]
    pub double_underline: bool,
    /// A line above the text, where the terminal supports it.
    #[param]
    pub overline: bool,
    /// Text and background colors swapped.
    #[param(alias = ["Invert"])]
    pub reverse: bool,
    /// The color of the underline of each segment.
    #[param(clr = "System.Array")]
    pub underline_color: Option<PsObject>,
    /// The kind of underline: Single, Double, Curly, Dotted or Dashed.
    #[param(validate_set = ["Single", "Double", "Curly", "Dotted", "Dashed"])]
    pub underline_style: Option<String>,
    /// Reads markup tags in the text, as Write-ColorEX's -Markup does.
    #[param]
    pub markup: bool,
    /// Cuts each string after each of these separators.
    #[param]
    pub split: Option<Vec<String>>,
    /// Cuts each string before and after each of these separators.
    #[param]
    pub split_around: Option<Vec<String>>,
    /// Cuts each string into as many parts as there are colors.
    #[param]
    pub split_evenly: bool,
    /// Colors and styles the text regular expressions match, as Write-ColorEX's -Highlight does.
    #[param(clr = "System.Collections.IDictionary")]
    pub highlight: Option<PsObject>,
    /// The address each segment links to.
    #[param]
    pub link: Option<Vec<String>>,
    /// The number of tab characters before the text.
    #[param(alias = ["Indent"])]
    pub start_tab: i32,
    /// The number of spaces before the text, after any tabs.
    #[param]
    pub start_spaces: i32,
    /// Pads the text to this display width.
    #[param(alias = ["PadWidth", "Pad"])]
    pub auto_pad: i32,
    /// With -AutoPad, pads on the left (right-aligns the text).
    #[param(alias = ["RightAlign"])]
    pub pad_left: bool,
    /// With -AutoPad, centers the text.
    #[param]
    pub pad_center: bool,
    /// The character -AutoPad pads with.
    #[param(alias = ["PaddingChar", "FillChar"])]
    pub pad_char: Option<char>,
    /// With -AutoPad, cuts text wider than -AutoPad, ending it with an ellipsis.
    #[param]
    pub truncate: bool,
    /// Breaks text wider than the line into lines, one string each.
    #[param]
    pub wrap: bool,
    /// Writes [DEBUG] messages about color processing and detection.
    #[param]
    pub debugging: bool,
    /// Suppresses the warnings about colors and fallbacks.
    #[param]
    pub silent: bool,
}

/// The parameter names of Format-ColorEX that Write-ColorEX takes too, for which ones a call gave.
const PARAMETER_NAMES: &[&str] = &[
    "Color", "BackGroundColor", "Gradient", "BackGroundGradient", "GradientSpace", "ANSI4", "ANSI8", "ANSI24", "Style", "StyleProfile", "Default", "Bold",
    "Faint", "Italic", "Underline", "Blink", "CrossedOut", "DoubleUnderline", "Overline", "Reverse", "UnderlineColor", "UnderlineStyle", "Markup", "Split",
    "SplitAround", "SplitEvenly", "Highlight", "Link", "StartTab", "StartSpaces", "AutoPad", "PadLeft", "PadCenter", "PadChar", "Truncate", "Wrap",
    "Debugging", "Silent",
];

impl Cmdlet for FormatColorEX {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        // Write-ColorEX's checks run for each string, as each string is a call of its own
        let highlight = begin_checks(
            ps,
            [
                (self.color.as_ref(), "Color"),
                (self.back_ground_color.as_ref(), "BackGroundColor"),
                (self.underline_color.as_ref(), "UnderlineColor"),
            ],
            self.split_evenly,
            self.highlight.as_ref(),
            self.silent,
        )?;
        let mut bound = HashSet::new();
        bound.insert("text".to_string());
        for name in PARAMETER_NAMES {
            if ps.parameter_is_bound(name) {
                bound.insert(name.to_ascii_lowercase());
            }
        }
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
            ansi4: self.ANSI4,
            ansi8: self.ANSI8,
            ansi24: self.ANSI24,
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
