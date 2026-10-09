//! The colors of a gradient: one per character, blended between the colors given, as
//! PSWriteColorEX's New-GradientColorArray computes them.

use pwrs::prelude::*;

use crate::colors::{self, lookup};
use crate::oklab;
use crate::write::{hex_rgb, ColorArg};

/// The RGB value of one color given to a gradient: an RGB array clamped to 0-255, a hex code, or
/// a color name. Anything else, such as an unknown name, is gray.
fn waypoint(ps: &Pipeline<'_>, color: &ColorArg) -> PsResult<[i64; 3]> {
    if let Some(rgb) = color.rgb() {
        return Ok(rgb.map(|v| v.clamp(0, 255)));
    }
    match color {
        ColorArg::Name(name) if colors::is_hex_text(name) => hex_rgb(ps, name),
        ColorArg::Name(name) => Ok(lookup(name).map(|entry| entry.rgb.map(i64::from)).unwrap_or([128, 128, 128])),
        _ => Ok([128, 128, 128]),
    }
}

/// One color for each of `steps` characters, blended evenly between the colors given: RGB arrays
/// for TrueColor, else 256-color codes. Each step falls between the two colors around it, at a
/// ratio from 0 to 1 between them; the first and last step of each stretch are the colors
/// themselves. With `oklab` the blend is in OKLab, else each channel on its own, rounded half to
/// even.
pub fn gradient_colors(ps: &Pipeline<'_>, given: &[ColorArg], steps: usize, truecolor: bool, oklab: bool) -> PsResult<Vec<ColorArg>> {
    let waypoints = given.iter().map(|color| waypoint(ps, color)).collect::<PsResult<Vec<_>>>()?;
    let as_color = |rgb: [i64; 3]| {
        if truecolor {
            ColorArg::List(rgb.iter().map(|&v| ColorArg::Number(v)).collect())
        } else {
            ColorArg::Number(colors::rgb_to_ansi8(rgb))
        }
    };
    if steps == 1 {
        return Ok(vec![as_color(waypoints[0])]);
    }

    let labs: Vec<[f64; 3]> = if oklab { waypoints.iter().map(|&rgb| oklab::to_oklab(rgb)).collect() } else { Vec::new() };
    let last = (steps - 1) as f64;
    let segment_count = waypoints.len() - 1;
    let mut out = Vec::with_capacity(steps);
    for step in 0..steps {
        let (segment, ratio) = if segment_count == 1 {
            (0, step as f64 / last)
        } else {
            let position = step as f64 / last * segment_count as f64;
            let segment = (position.floor() as usize).min(segment_count - 1);
            (segment, position - segment as f64)
        };
        let start = waypoints[segment];
        let end = waypoints[segment + 1];
        let rgb = if ratio == 0.0 {
            start
        } else if ratio == 1.0 {
            end
        } else if oklab {
            let (from, to) = (labs[segment], labs[segment + 1]);
            oklab::from_oklab([
                from[0] + (to[0] - from[0]) * ratio,
                from[1] + (to[1] - from[1]) * ratio,
                from[2] + (to[2] - from[2]) * ratio,
            ])
        } else {
            let mut rgb = [0i64; 3];
            for (k, slot) in rgb.iter_mut().enumerate() {
                *slot = colors::round_even(start[k] as f64 + (end[k] - start[k]) as f64 * ratio);
            }
            rgb
        };
        out.push(as_color(rgb));
    }
    Ok(out)
}
