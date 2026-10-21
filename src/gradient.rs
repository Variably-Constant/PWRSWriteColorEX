//! The colors of a gradient: one per character, blended between the colors given, as
//! PSWriteColorEX's New-GradientColorArray computes them, with every option -Gradient takes: the
//! space it blends in and the way round the hue wheel, stops placed with '@', a direction across
//! the text, an easing curve, repeating past the stops, and bands.

use pwrs::prelude::*;

use crate::colors::{self, lookup};
use crate::ease::Easing;
use crate::forms::{read_forms, UnknownNames};
use crate::oklab;
use crate::spaces::{self, Coordinates, HuePath, Space};
use crate::write::{gradient_list, hex_rgb, ColorArg};

/// The named gradients -Gradient and -BackGroundGradient take as a single value, each with its
/// colors in order, as PSWriteColorEX's. Rainbow to Grayscale and Progress are the module's own;
/// Atlas to Summer are gradient-string's presets (<https://github.com/bokub/gradient-string>, MIT,
/// Copyright (c) 2017 Boris K). Progress is the color table's Red, LightRuby, Orange, Yellow and
/// Green.
pub const NAMED_GRADIENTS: [(&str, &[&str]); 21] = [
    ("Rainbow", &["#FF0000", "#FFFF00", "#00FF00", "#00FFFF", "#0000FF", "#FF00FF"]),
    ("Sunset", &["#4A1C6F", "#C2185B", "#FF6F3C", "#FFC857"]),
    ("Ocean", &["#003366", "#0077B6", "#00B4D8", "#90E0EF"]),
    ("Forest", &["#0B3D20", "#1E6B3A", "#4C9A2A", "#A3C94F"]),
    ("Fire", &["#8B0000", "#FF4500", "#FF8C00", "#FFD700"]),
    ("Ice", &["#FFFFFF", "#CAF0F8", "#72C6EF", "#3A86FF"]),
    ("Neon", &["#FF00CC", "#3333FF", "#00FFCC"]),
    ("Pastel", &["#FFB3BA", "#FFDFBA", "#FFFFBA", "#BAFFC9", "#BAE1FF", "#D7BAFF"]),
    ("Grayscale", &["#000000", "#FFFFFF"]),
    ("Atlas", &["#FEAC5E", "#C779D0", "#4BC0C8"]),
    ("Cristal", &["#BDFFF3", "#4AC29A"]),
    ("Teen", &["#77A1D3", "#79CBCA", "#E684AE"]),
    ("Mind", &["#473B7B", "#3584A7", "#30D2BE"]),
    ("Morning", &["#FF5F6D", "#FFC371"]),
    ("Vice", &["#5EE7DF", "#B490CA"]),
    ("Passion", &["#F43B47", "#453A94"]),
    ("Fruit", &["#FF4E50", "#F9D423"]),
    ("Instagram", &["#833AB4", "#FD1D1D", "#FCB045"]),
    ("Retro", &["#3F51B1", "#5A55AE", "#7B5FAC", "#8F6AAE", "#A86AA4", "#CC6B8E", "#F18271", "#F3A469", "#F7C978"]),
    ("Summer", &["#FDBB2D", "#22C1C3"]),
    ("Progress", &["#FF0000", "#FF6699", "#FFA500", "#FFFF00", "#00FF00"]),
];

/// The colors of the named gradient a name names, compared without regard to case.
pub fn named_gradient(name: &str) -> Option<&'static [&'static str]> {
    NAMED_GRADIENTS.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, stops)| *stops)
}

/// A gradient list that holds the one name of a named gradient becomes that gradient's colors;
/// any other list stays as it is.
pub fn resolve_gradient_name(list: &mut Option<Vec<ColorArg>>) {
    if let Some(values) = list {
        resolve_name_in(values);
    }
}

/// resolve_gradient_name for a list that is there.
fn resolve_name_in(values: &mut Vec<ColorArg>) {
    if let [ColorArg::Name(name)] = values.as_slice()
        && let Some(stops) = named_gradient(name)
    {
        *values = stops.iter().map(|stop| ColorArg::Name((*stop).to_string())).collect();
    }
}

/// Which way a gradient runs across the text: from the first character, from the last, from the
/// middle out to both ends, or from both ends in to the middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Forward,
    Reverse,
    CenterOut,
    EdgesIn,
}

impl Direction {
    /// Every direction, in the order -GradientDirection lists them.
    pub const ALL: [Direction; 4] = [Direction::Forward, Direction::Reverse, Direction::CenterOut, Direction::EdgesIn];

    /// The name -GradientDirection takes.
    pub fn name(self) -> &'static str {
        match self {
            Direction::Forward => "Forward",
            Direction::Reverse => "Reverse",
            Direction::CenterOut => "CenterOut",
            Direction::EdgesIn => "EdgesIn",
        }
    }

    /// The direction a name means, compared without regard to case.
    pub fn from_name(name: &str) -> Option<Direction> {
        Direction::ALL.into_iter().find(|direction| direction.name().eq_ignore_ascii_case(name))
    }

    /// The place along the gradient of a place from 0 to 1 across the text.
    fn place(self, across: f64) -> f64 {
        match self {
            Direction::Forward => across,
            Direction::Reverse => 1.0 - across,
            Direction::CenterOut => (2.0 * across - 1.0).abs(),
            Direction::EdgesIn => 1.0 - (2.0 * across - 1.0).abs(),
        }
    }
}

/// What a gradient does past the span its stops cover: holds its end colors, starts over, or
/// goes back and forth, as CSS's gradients and repeating gradients do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Repeat {
    #[default]
    None,
    Repeat,
    Mirror,
}

impl Repeat {
    /// Every value, in the order -GradientRepeat lists them.
    pub const ALL: [Repeat; 3] = [Repeat::None, Repeat::Repeat, Repeat::Mirror];

    /// The name -GradientRepeat takes.
    pub fn name(self) -> &'static str {
        match self {
            Repeat::None => "None",
            Repeat::Repeat => "Repeat",
            Repeat::Mirror => "Mirror",
        }
    }

    /// The value a name means, compared without regard to case.
    pub fn from_name(name: &str) -> Option<Repeat> {
        Repeat::ALL.into_iter().find(|repeat| repeat.name().eq_ignore_ascii_case(name))
    }
}

/// Every option a gradient takes besides its colors; the default is the gradient 1.2.0 drew.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct GradientOptions {
    pub space: Space,
    pub hue: HuePath,
    pub easing: Easing,
    pub repeat: Repeat,
    pub direction: Direction,
    /// The bands the blend is cut into; 0 for a smooth blend.
    pub steps: usize,
}

impl GradientOptions {
    /// The options -GradientSpace, -GradientHue, -GradientEasing, -GradientRepeat and
    /// -GradientDirection name, compared without regard to case, with -GradientSteps' bands. A
    /// name outside its set, or bands below 0, is an error naming the parameter.
    pub fn from_names(space: &str, hue: &str, easing: &str, repeat: &str, direction: &str, steps: i32) -> PsResult<GradientOptions> {
        let refusal = |parameter: &str, value: &str| {
            PsError::new(ErrorCategory::InvalidArgument, "GradientOptionNotInSet", format!("'{value}' is not a value of -{parameter}.")).terminating()
        };
        Ok(GradientOptions {
            space: Space::from_name(space).ok_or_else(|| refusal("GradientSpace", space))?,
            hue: HuePath::from_name(hue).ok_or_else(|| refusal("GradientHue", hue))?,
            easing: Easing::from_name(easing).ok_or_else(|| refusal("GradientEasing", easing))?,
            repeat: Repeat::from_name(repeat).ok_or_else(|| refusal("GradientRepeat", repeat))?,
            direction: Direction::from_name(direction).ok_or_else(|| refusal("GradientDirection", direction))?,
            steps: usize::try_from(steps).map_err(|_| refusal("GradientSteps", &steps.to_string()))?,
        })
    }
}

/// A stop of a gradient: its color, and its place from 0 to 1 when one was given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub rgb: [i64; 3],
    pub place: Option<f64>,
}

/// A gradient color with its place after an '@', as a percent: 'Gold@40' is Gold at 0.4. The
/// place is the digits over 10 to the power of the decimals plus 2, in one division, so
/// PSWriteColorEX computes the same double; above 100 is 1. A text with no place returns None.
pub fn split_place(text: &str) -> Option<(&str, f64)> {
    let (color, place) = text.rsplit_once('@')?;
    let place = place.trim();
    let place = place.strip_suffix('%').unwrap_or(place).trim_end();
    let (whole, fraction) = place.split_once('.').unwrap_or((place, ""));
    let digit_count = whole.len() + fraction.len();
    if digit_count == 0 || digit_count > 15 || !whole.bytes().chain(fraction.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut mantissa = 0u64;
    for digit in whole.bytes().chain(fraction.bytes()) {
        mantissa = mantissa * 10 + u64::from(digit - b'0');
    }
    let mut scale = 100.0;
    for _ in 0..fraction.len() {
        scale *= 10.0;
    }
    Some((color.trim_end(), (mantissa as f64 / scale).min(1.0)))
}

/// Takes the place given with '@' off each of a gradient's colors, and returns the places in
/// order, None for a color with none.
pub fn split_places(values: &mut [ColorArg]) -> Vec<Option<f64>> {
    values
        .iter_mut()
        .map(|value| {
            let ColorArg::Name(text) = value else {
                return None;
            };
            let (color, place) = split_place(text).map(|(color, place)| (color.to_string(), place))?;
            *text = color;
            Some(place)
        })
        .collect()
}

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

/// The gradient 1.2.0 drew, for evenly spaced stops and no other option: with at least as many
/// steps as colors, color w is the step nearest w x (steps - 1) / (colors - 1), a half rounded up,
/// so every color is drawn, and the steps between two colors fall between them at a ratio from 0
/// to 1. With fewer steps than colors, the steps are spread evenly over the whole gradient. With
/// `oklab` the blend is in OKLab, else each channel on its own, rounded half to even.
fn plain_gradient(waypoints: &[[i64; 3]], steps: usize, oklab: bool) -> Vec<[i64; 3]> {
    if steps == 1 {
        return vec![waypoints[0]];
    }

    let labs: Vec<[f64; 3]> = if oklab { waypoints.iter().map(|&rgb| oklab::to_oklab(rgb)).collect() } else { Vec::new() };
    let last = (steps - 1) as f64;
    let segment_count = waypoints.len() - 1;

    // Each color's step, when every color can have one
    let anchors: Option<Vec<usize>> = (segment_count > 1 && steps >= waypoints.len())
        .then(|| (0..waypoints.len()).map(|w| (2 * w * (steps - 1) + segment_count) / (2 * segment_count)).collect());
    let mut anchored_segment = 0usize;

    let mut out = Vec::with_capacity(steps);
    for step in 0..steps {
        let (segment, ratio) = if segment_count == 1 {
            (0, step as f64 / last)
        } else if let Some(anchors) = &anchors {
            while step > anchors[anchored_segment + 1] {
                anchored_segment += 1;
            }
            let (from, to) = (anchors[anchored_segment], anchors[anchored_segment + 1]);
            (anchored_segment, (step - from) as f64 / (to - from) as f64)
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
        out.push(rgb);
    }
    out
}

/// Each stop's place from 0 to 1 as CSS places color stops: the first at 0 and the last at 1
/// unless placed, a place before an earlier stop's moved up to it, and the stops with none spread
/// evenly between the placed ones around them.
fn resolve_places(stops: &[Stop]) -> Vec<f64> {
    let last = stops.len() - 1;
    let mut given: Vec<Option<f64>> = stops.iter().map(|stop| stop.place.map(|place| place.clamp(0.0, 1.0))).collect();
    if given[0].is_none() {
        given[0] = Some(0.0);
    }
    if given[last].is_none() {
        given[last] = Some(1.0);
    }
    let mut highest = 0.0;
    for place in given.iter_mut().flatten() {
        if *place < highest {
            *place = highest;
        } else {
            highest = *place;
        }
    }
    let mut places = vec![0.0; stops.len()];
    let mut k = 0;
    while k <= last {
        if let Some(place) = given[k] {
            places[k] = place;
            k += 1;
            continue;
        }
        let before = k - 1;
        let mut after = k;
        while given[after].is_none() {
            after += 1;
        }
        let (from, to) = (places[before], given[after].unwrap_or(1.0));
        let span = (after - before) as f64;
        for (i, place) in places.iter_mut().enumerate().take(after).skip(k) {
            *place = from + (to - from) * ((i - before) as f64 / span);
        }
        k = after;
    }
    places
}

/// The segment a place falls in among stops' places, and its ratio from the segment's first stop
/// to its last: the first stop's color at or before the first place, the last's at or after the
/// last, and the later stop's where two share a place.
fn locate(places: &[f64], place: f64) -> (usize, f64) {
    let last = places.len() - 1;
    if place <= places[0] {
        return (0, 0.0);
    }
    if place >= places[last] {
        return (last - 1, 1.0);
    }
    let mut segment = 0;
    while segment + 1 < last && places[segment + 1] <= place {
        segment += 1;
    }
    (segment, (place - places[segment]) / (places[segment + 1] - places[segment]))
}

/// A place carried back into the span from the first stop's place to the last's, by `repeat`:
/// unchanged inside it or with no repeat.
fn repeat_place(place: f64, first: f64, last: f64, repeat: Repeat) -> f64 {
    let span = last - first;
    if repeat == Repeat::None || span <= 0.0 || (place >= first && place <= last) {
        return place;
    }
    let turns = (place - first) / span;
    let within = match repeat {
        Repeat::Mirror => {
            let folded = turns - 2.0 * (turns / 2.0).floor();
            if folded > 1.0 { 2.0 - folded } else { folded }
        }
        _ => turns - turns.floor(),
    };
    first + within * span
}

/// A place moved to the place of its band, when the span from the first stop's place to the
/// last's is cut into `bands`: band k of n at k / (n - 1) of the span, so the first band is the
/// first stop and the last band the last; a single band is the first stop.
fn band_place(place: f64, first: f64, last: f64, bands: usize) -> f64 {
    let span = last - first;
    if bands == 0 || span <= 0.0 {
        return place;
    }
    if bands == 1 {
        return first;
    }
    let fraction = ((place - first) / span).clamp(0.0, 1.0);
    let band = ((fraction * bands as f64).floor() as usize).min(bands - 1);
    first + (band as f64 / (bands - 1) as f64) * span
}

/// The place along the gradient a place across the text maps to, through the direction, the
/// easing curve, the repeat and the bands, in that order.
fn shape(across: f64, places: &[f64], options: &GradientOptions) -> f64 {
    let (first, last) = (places[0], places[places.len() - 1]);
    let eased = options.easing.apply(options.direction.place(across));
    band_place(repeat_place(eased, first, last, options.repeat), first, last, options.steps)
}

/// A stop's place moved to the nearest place a character takes: the characters' places are
/// `offset + interval x j` over `denominator`, from `lowest` to `highest`; a half rounds up.
/// Evenly spaced stops, stop `index` of `count`, are placed in integers, exactly.
fn snapped(place: f64, even: Option<(usize, usize)>, denominator: usize, interval: usize, offset: usize, lowest: usize, highest: usize) -> usize {
    let numerator = match even {
        Some((index, count)) => {
            // j = floor((index x denominator / spans - offset) / interval + 1/2) in integers; the
            // sum is never below 0, since an offset of 1 comes with an interval of 2
            let spans = count - 1;
            let sum = 2 * index * denominator + interval * spans - 2 * offset * spans;
            offset + interval * (sum / (2 * interval * spans))
        }
        None => {
            let j = ((place * denominator as f64 - offset as f64) / interval as f64 + 0.5).floor().max(0.0);
            offset + interval * j as usize
        }
    };
    numerator.clamp(lowest, highest)
}

/// The colors of a gradient across `count` characters, as RGB values, with every option. It
/// takes no pipeline, so a linking module may call it from a thread that never touches
/// PowerShell. With evenly spaced stops and no option but a space of OKLab or RGB, the colors
/// are those 1.2.0 drew.
pub fn gradient_rgb(stops: &[Stop], count: usize, options: &GradientOptions) -> Vec<[i64; 3]> {
    if count == 0 || stops.is_empty() {
        return Vec::new();
    }
    let rgbs: Vec<[i64; 3]> = stops.iter().map(|stop| stop.rgb).collect();
    if stops.len() == 1 {
        return vec![rgbs[0]; count];
    }
    let places = resolve_places(stops);
    let spans = stops.len() - 1;
    let even = places.iter().enumerate().all(|(k, &place)| place == k as f64 / spans as f64);
    let plain = even
        && matches!(options.space, Space::OkLab | Space::Rgb)
        && options.easing == Easing::Linear
        && options.repeat == Repeat::None
        && options.direction == Direction::Forward
        && options.steps == 0;
    if plain {
        return plain_gradient(&rgbs, count, options.space == Space::OkLab);
    }

    let coordinates: Vec<Coordinates> = rgbs.iter().map(|&rgb| spaces::to_coordinates(rgb, options.space)).collect();
    let color = |(segment, ratio): (usize, f64)| spaces::blend(rgbs[segment], rgbs[segment + 1], &coordinates[segment], &coordinates[segment + 1], ratio, options.space, options.hue);

    if count == 1 {
        return vec![color(locate(&places, shape(0.0, &places, options)))];
    }

    // Each character's place as a numerator over count - 1, after the direction
    let denominator = count - 1;
    let numerator = |i: usize| match options.direction {
        Direction::Forward => i,
        Direction::Reverse => denominator - i,
        Direction::CenterOut => (2 * i).abs_diff(denominator),
        Direction::EdgesIn => denominator - (2 * i).abs_diff(denominator),
    };

    // The places the characters take: every numerator from 0 to count - 1, or, from the middle
    // out or the ends in, every other one
    let (interval, offset, lowest, highest) = match options.direction {
        Direction::Forward | Direction::Reverse => (1, 0, 0, denominator),
        Direction::CenterOut => (2, denominator % 2, denominator % 2, denominator),
        Direction::EdgesIn => (2, 0, 0, denominator - denominator % 2),
    };
    if (highest - lowest) / interval + 1 < stops.len() {
        return (0..count).map(|i| color(locate(&places, shape(numerator(i) as f64 / denominator as f64, &places, options)))).collect();
    }

    // With a place for every stop, each stop moves to the nearest character's place
    let numerators: Vec<usize> = places
        .iter()
        .enumerate()
        .map(|(k, &place)| snapped(place, even.then_some((k, stops.len())), denominator, interval, offset, lowest, highest))
        .collect();

    let whole_span = numerators[0] == 0 && numerators[spans] == denominator;
    if options.easing == Easing::Linear && options.steps == 0 && (options.repeat == Repeat::None || whole_span) {
        let stop_numerators: Vec<f64> = numerators.iter().map(|&n| n as f64).collect();
        return (0..count).map(|i| color(locate(&stop_numerators, numerator(i) as f64))).collect();
    }
    let snapped_places: Vec<f64> = numerators.iter().map(|&n| n as f64 / denominator as f64).collect();
    (0..count).map(|i| color(locate(&snapped_places, shape(numerator(i) as f64 / denominator as f64, &snapped_places, options)))).collect()
}

/// The color at a place from 0 to 1 along a gradient, with every option, for a single color
/// that moves along it; stops keep their exact places, since there are no characters to move
/// them to. A place outside 0 to 1 is the nearer end, and NaN is 0. It takes no pipeline.
pub fn gradient_rgb_at(stops: &[Stop], place: f64, options: &GradientOptions) -> [i64; 3] {
    if stops.is_empty() {
        return [128, 128, 128];
    }
    if stops.len() == 1 {
        return stops[0].rgb;
    }
    let along = if place.is_nan() { 0.0 } else { place.clamp(0.0, 1.0) };
    let places = resolve_places(stops);
    let (segment, ratio) = locate(&places, shape(along, &places, options));
    let (from, to) = (stops[segment].rgb, stops[segment + 1].rgb);
    spaces::blend(from, to, &spaces::to_coordinates(from, options.space), &spaces::to_coordinates(to, options.space), ratio, options.space, options.hue)
}

/// One color for each of `steps` characters, blended between the colors given at the places
/// given (None for a color with no place), with every option: RGB arrays for TrueColor, else
/// 256-color codes. See gradient_rgb.
pub fn gradient_colors(ps: &Pipeline<'_>, given: &[ColorArg], places: &[Option<f64>], steps: usize, truecolor: bool, options: &GradientOptions) -> PsResult<Vec<ColorArg>> {
    let stops = given
        .iter()
        .enumerate()
        .map(|(k, color)| Ok(Stop { rgb: waypoint(ps, color)?, place: places.get(k).copied().flatten() }))
        .collect::<PsResult<Vec<Stop>>>()?;
    Ok(gradient_rgb(&stops, steps, options)
        .into_iter()
        .map(|rgb| if truecolor { ColorArg::List(rgb.iter().map(|&v| ColorArg::Number(v)).collect()) } else { ColorArg::Number(colors::rgb_to_ansi8(rgb)) })
        .collect())
}

/// Returns the colors of a gradient: one for each of a number of characters, or the color at each
/// of several places along it.
///
/// Get-GradientColor blends a gradient as Write-ColorEX's -Gradient does and returns its colors as
/// RGB arrays, three values from 0 to 255 in each, the form -Color and -Gradient take. -Count
/// returns the colors of that many characters, as Write-ColorEX draws the gradient across a line
/// of that length. -Place returns the color at each place given along the gradient, from 0 at its
/// start to 1 at its end, in the order given, for a single color that moves along it; colors
/// placed with '@' keep their exact places, since there are no characters to move them to. A
/// color name the color table lacks gives a warning and blends as gray.
///
/// # Examples
/// Get-GradientColor -Gradient Red, Blue -Count 5
/// Get-GradientColor -Gradient Progress -Place 0.75
/// Get-GradientColor -Gradient Progress -Place (0..100 | ForEach-Object { $_ / 100 })
/// $colors = Get-GradientColor -Gradient Sunset -Count 3 -GradientSpace OKLCH
#[cmdlet(verb = "Get", noun = "GradientColor", default_parameter_set = "Count", alias = ["Get-GradientColour"], output = ["System.Int32[]"])]
#[derive(Default)]
pub struct GetGradientColor {
    /// Two or more colors, or the name of a named gradient, as Write-ColorEX's -Gradient takes
    /// them. A color may give its place along the gradient after an '@', as a percent: 'Gold@40'.
    #[param(mandatory, position = 0)]
    pub gradient: Vec<PsObject>,
    /// The number of colors to return, as for a line of that many characters.
    #[param(mandatory, set = "Count", validate_range(1, 2147483647))]
    pub count: i32,
    /// The places along the gradient of the colors to return, from 0 at its start to 1 at its end:
    /// one color for each place, in the order given. A place below 0 is 0 and above 1 is 1.
    #[param(mandatory, set = "Place")]
    pub place: Vec<f64>,
    /// How the colors blend: OKLab (the default), RGB, LinearRGB, OKLCH, HSL, HSV, Lab or LCh, as
    /// Write-ColorEX's -GradientSpace takes them.
    #[param(validate_set = ["OKLab", "RGB", "LinearRGB", "OKLCH", "HSL", "HSV", "Lab", "LCh"])]
    pub gradient_space: Option<String>,
    /// The way round the hue wheel in OKLCH, HSL, HSV and LCh: Shorter (the default), Longer,
    /// Increasing or Decreasing.
    #[param(validate_set = ["Shorter", "Longer", "Increasing", "Decreasing"])]
    pub gradient_hue: Option<String>,
    /// A curve that moves the colors along the gradient, as Write-ColorEX's -GradientEasing takes
    /// it.
    #[param(validate_set = [
        "Linear", "InSine", "OutSine", "InOutSine", "InQuad", "OutQuad", "InOutQuad", "InCubic", "OutCubic", "InOutCubic", "InQuart", "OutQuart",
        "InOutQuart", "InQuint", "OutQuint", "InOutQuint", "InExpo", "OutExpo", "InOutExpo", "InCirc", "OutCirc", "InOutCirc", "InBack", "OutBack",
        "InOutBack", "InElastic", "OutElastic", "InOutElastic", "InBounce", "OutBounce", "InOutBounce"
    ])]
    pub gradient_easing: Option<String>,
    /// What the gradient does past placed first and last colors: None (the default), Repeat or
    /// Mirror.
    #[param(validate_set = ["None", "Repeat", "Mirror"])]
    pub gradient_repeat: Option<String>,
    /// The way the gradient runs: Forward (the default), Reverse, CenterOut or EdgesIn.
    #[param(validate_set = ["Forward", "Reverse", "CenterOut", "EdgesIn"])]
    pub gradient_direction: Option<String>,
    /// Cuts the gradient into this many bands of solid color; 0, the default, blends smoothly.
    #[param(validate_range(0, 2147483647))]
    pub gradient_steps: i32,
}

impl Cmdlet for GetGradientColor {
    fn process(&mut self, ps: &Pipeline<'_>) -> PsResult<()> {
        let mut colors = gradient_list(&self.gradient)?;
        resolve_name_in(&mut colors);
        if colors.len() < 2 {
            return Err(PsError::new(
                ErrorCategory::InvalidArgument,
                "GradientTooFewColors",
                format!("Gradient requires at least 2 colors (received {}).", colors.len()),
            )
            .terminating());
        }
        let places = split_places(&mut colors);
        let mut warnings = Vec::new();
        read_forms(&mut colors, &mut UnknownNames::default(), &mut |message| warnings.push(message));
        for message in &warnings {
            ps.warning(message)?;
        }
        let options = GradientOptions::from_names(
            self.gradient_space.as_deref().unwrap_or("OKLab"),
            self.gradient_hue.as_deref().unwrap_or("Shorter"),
            self.gradient_easing.as_deref().unwrap_or("Linear"),
            self.gradient_repeat.as_deref().unwrap_or("None"),
            self.gradient_direction.as_deref().unwrap_or("Forward"),
            self.gradient_steps,
        )?;
        let stops = colors
            .iter()
            .zip(places)
            .map(|(color, place)| Ok(Stop { rgb: waypoint(ps, color)?, place }))
            .collect::<PsResult<Vec<Stop>>>()?;
        // Each color written whole, as one Int32[] of three channels
        let whole = |rgb: [i64; 3]| PsArray(rgb.iter().map(|&channel| channel as i32).collect::<Vec<i32>>());
        if ps.parameter_is_bound("Place") {
            for &at in &self.place {
                ps.write(whole(gradient_rgb_at(&stops, at, &options)))?;
            }
            return Ok(());
        }
        for rgb in gradient_rgb(&stops, self.count.max(0) as usize, &options) {
            ps.write(whole(rgb))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stops(colors: &[[i64; 3]]) -> Vec<Stop> {
        colors.iter().map(|&rgb| Stop { rgb, place: None }).collect()
    }

    #[test]
    fn a_single_gradient_name_becomes_its_colors() {
        let mut list = Some(vec![ColorArg::Name("sunset".to_string())]);
        resolve_gradient_name(&mut list);
        let names: Vec<String> = list.unwrap().iter().map(|c| match c {
            ColorArg::Name(n) => n.clone(),
            other => panic!("not a name: {other:?}"),
        }).collect();
        assert_eq!(names, ["#4A1C6F", "#C2185B", "#FF6F3C", "#FFC857"]);
    }

    #[test]
    fn other_lists_stay_as_they_are() {
        for given in [
            vec![ColorArg::Name("Forest".to_string()), ColorArg::Name("Blue".to_string())],
            vec![ColorArg::Name("Teal".to_string())],
            vec![ColorArg::Number(3)],
            Vec::new(),
        ] {
            let mut list = Some(given.clone());
            resolve_gradient_name(&mut list);
            assert_eq!(list, Some(given));
        }
        let mut none = None;
        resolve_gradient_name(&mut none);
        assert_eq!(none, None);
    }

    #[test]
    fn twenty_one_names_each_of_two_colors_or_more() {
        assert_eq!(NAMED_GRADIENTS.len(), 21);
        assert!(NAMED_GRADIENTS.iter().all(|(_, stops)| stops.len() >= 2 && stops.iter().all(|s| colors::is_hex_text(s))));
        let progress: Vec<[u8; 3]> = ["Red", "LightRuby", "Orange", "Yellow", "Green"].iter().map(|name| lookup(name).unwrap().rgb).collect();
        let named: Vec<[u8; 3]> = named_gradient("progress").unwrap().iter().map(|hex| colors::hex_to_rgb(hex).unwrap()).collect();
        assert_eq!(named, progress, "Progress is the table's Red, LightRuby, Orange, Yellow and Green");
    }

    // A module that links the library calls gradient_colors with this signature, with or without
    // the module feature; it takes a pipeline, so a unit test reaches it by its type, which fails
    // to compile when the function or its signature changes.
    #[test]
    fn gradient_colors_has_the_signature_a_linking_module_calls() {
        type GradientColors = fn(&Pipeline<'_>, &[ColorArg], &[Option<f64>], usize, bool, &GradientOptions) -> PsResult<Vec<ColorArg>>;
        let _: GradientColors = gradient_colors;
    }

    #[test]
    fn places_read_as_percents() {
        assert_eq!(split_place("Gold@40"), Some(("Gold", 0.4)));
        assert_eq!(split_place("#FF8800@12.5%"), Some(("#FF8800", 0.125)));
        assert_eq!(split_place("rgb(1, 2, 3) @ 100"), Some(("rgb(1, 2, 3)", 1.0)));
        assert_eq!(split_place("Gold@250"), Some(("Gold", 1.0)));
        assert_eq!(split_place("Gold@.5"), Some(("Gold", 0.005)));
        for text in ["Gold", "Gold@", "Gold@-5", "Gold@4x", "Gold@1.2.3", "Gold@ ", "Gold@1234567890123456"] {
            assert_eq!(split_place(text), None, "{text}");
        }
        let mut values = vec![ColorArg::Name("Red@10".to_string()), ColorArg::Number(5), ColorArg::Name("Blue".to_string())];
        assert_eq!(split_places(&mut values), [Some(0.1), None, None]);
        assert_eq!(values[0], ColorArg::Name("Red".to_string()));
    }

    #[test]
    fn places_resolve_as_css_resolves_them() {
        let placed = |places: &[Option<f64>]| resolve_places(&places.iter().map(|&place| Stop { rgb: [0, 0, 0], place }).collect::<Vec<_>>());
        assert_eq!(placed(&[None, None, None]), [0.0, 0.5, 1.0]);
        assert_eq!(placed(&[None, Some(0.2), Some(0.1), None]), [0.0, 0.2, 0.2, 1.0], "a place before an earlier one moves up to it");
        let spread = placed(&[Some(0.1), None, None, Some(0.7)]);
        assert!(spread[0] == 0.1 && (spread[1] - 0.3).abs() < 1e-12 && (spread[2] - 0.5).abs() < 1e-12 && spread[3] == 0.7, "{spread:?}");
        assert_eq!(placed(&[None, Some(0.5), Some(0.5), None]), [0.0, 0.5, 0.5, 1.0]);
    }

    #[test]
    fn plain_options_draw_what_1_2_0_drew() {
        let given = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
        for space in [Space::OkLab, Space::Rgb] {
            let options = GradientOptions { space, ..Default::default() };
            for count in [1, 2, 3, 7, 40] {
                assert_eq!(gradient_rgb(&stops(&given), count, &options), plain_gradient(&given, count, space == Space::OkLab));
            }
        }
        let placed_evenly = [Stop { rgb: given[0], place: Some(0.0) }, Stop { rgb: given[1], place: Some(0.5) }, Stop { rgb: given[2], place: Some(1.0) }];
        assert_eq!(gradient_rgb(&placed_evenly, 9, &GradientOptions::default()), plain_gradient(&given, 9, true), "places equal to even spacing change nothing");
    }

    #[test]
    fn every_stop_lands_on_a_character_of_its_own() {
        let given = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]];
        for space in Space::ALL {
            for direction in Direction::ALL {
                let options = GradientOptions { space, direction, ..Default::default() };
                // From the middle out or the ends in, the characters take every other place, so
                // four stops need at least seven of them
                let counts: &[usize] = if matches!(direction, Direction::Forward | Direction::Reverse) { &[4, 5, 9, 10, 23] } else { &[7, 8, 9, 10, 23] };
                for &count in counts {
                    let out = gradient_rgb(&stops(&given), count, &options);
                    for rgb in given {
                        assert!(out.contains(&rgb), "{rgb:?} missing in {space:?} {direction:?} over {count}: {out:?}");
                    }
                }
            }
        }
        let placed = [Stop { rgb: [255, 0, 0], place: None }, Stop { rgb: [255, 215, 0], place: Some(0.4) }, Stop { rgb: [0, 0, 255], place: None }];
        let out = gradient_rgb(&placed, 10, &GradientOptions::default());
        assert_eq!(out[4], [255, 215, 0], "40% of 10 characters is character 3.6, which snaps to 4");
    }

    #[test]
    fn reverse_mirrors_forward() {
        let given = stops(&[[255, 0, 0], [0, 255, 0], [0, 0, 255]]);
        let forward = gradient_rgb(&given, 11, &GradientOptions { space: Space::OkLch, ..Default::default() });
        let mut reverse = gradient_rgb(&given, 11, &GradientOptions { space: Space::OkLch, direction: Direction::Reverse, ..Default::default() });
        reverse.reverse();
        assert_eq!(forward, reverse);
        let center = gradient_rgb(&given, 9, &GradientOptions { direction: Direction::CenterOut, ..Default::default() });
        assert_eq!(center[4], [255, 0, 0], "the middle character is the first stop");
        assert_eq!((center[0], center[8]), ([0, 0, 255], [0, 0, 255]), "both ends are the last stop");
        let edges = gradient_rgb(&given, 9, &GradientOptions { direction: Direction::EdgesIn, ..Default::default() });
        assert_eq!((edges[0], edges[4], edges[8]), ([255, 0, 0], [0, 0, 255], [255, 0, 0]));
    }

    #[test]
    fn bands_keep_both_ends_exact() {
        let given = stops(&[[0, 0, 0], [255, 255, 255]]);
        let out = gradient_rgb(&given, 12, &GradientOptions { space: Space::Rgb, steps: 4, ..Default::default() });
        assert_eq!(out, [[0, 0, 0]; 3].into_iter().chain([[85, 85, 85]; 3]).chain([[170, 170, 170]; 3]).chain([[255, 255, 255]; 3]).collect::<Vec<_>>());
        assert_eq!(gradient_rgb(&given, 5, &GradientOptions { steps: 1, ..Default::default() }), vec![[0, 0, 0]; 5]);
    }

    #[test]
    fn repeat_and_mirror_fill_the_text_with_the_stops_span() {
        let given = [Stop { rgb: [255, 0, 0], place: Some(0.0) }, Stop { rgb: [0, 0, 255], place: Some(0.25) }];
        let repeated = gradient_rgb(&given, 9, &GradientOptions { space: Space::Rgb, repeat: Repeat::Repeat, ..Default::default() });
        assert_eq!((repeated[0], repeated[2], repeated[3], repeated[4]), ([255, 0, 0], [0, 0, 255], [128, 0, 128], [255, 0, 0]));
        let mirrored = gradient_rgb(&given, 9, &GradientOptions { space: Space::Rgb, repeat: Repeat::Mirror, ..Default::default() });
        assert_eq!((mirrored[0], mirrored[2], mirrored[3], mirrored[4]), ([255, 0, 0], [0, 0, 255], [128, 0, 128], [255, 0, 0]));
        let held = gradient_rgb(&given, 9, &GradientOptions { space: Space::Rgb, ..Default::default() });
        assert!(held[3..].iter().all(|&rgb| rgb == [0, 0, 255]), "with no repeat the last stop holds: {held:?}");
    }

    #[test]
    fn easing_moves_the_colors_along_the_text() {
        let given = stops(&[[0, 0, 0], [255, 255, 255]]);
        let linear = gradient_rgb(&given, 5, &GradientOptions { space: Space::Rgb, ..Default::default() });
        let eased = gradient_rgb(&given, 5, &GradientOptions { space: Space::Rgb, easing: Easing::InQuad, ..Default::default() });
        assert_eq!(linear[2], [128, 128, 128]);
        assert_eq!(eased[2], [64, 64, 64], "InQuad puts the middle character a quarter of the way along");
        assert_eq!((eased[0], eased[4]), ([0, 0, 0], [255, 255, 255]));
    }

    #[test]
    fn a_place_along_the_gradient_gives_one_color() {
        let given = stops(&[[0, 0, 0], [255, 255, 255]]);
        let options = GradientOptions { space: Space::Rgb, ..Default::default() };
        assert_eq!(gradient_rgb_at(&given, 0.0, &options), [0, 0, 0]);
        assert_eq!(gradient_rgb_at(&given, 0.5, &options), [128, 128, 128]);
        assert_eq!(gradient_rgb_at(&given, 1.0, &options), [255, 255, 255]);
        assert_eq!(gradient_rgb_at(&given, 7.0, &options), [255, 255, 255], "a place past 1 is 1");
        assert_eq!(gradient_rgb_at(&given, 0.25, &GradientOptions { direction: Direction::Reverse, ..options }), [191, 191, 191]);
    }

    #[test]
    fn option_names_compare_without_case() {
        assert_eq!(Direction::from_name("centerout"), Some(Direction::CenterOut));
        assert_eq!(Repeat::from_name("MIRROR"), Some(Repeat::Mirror));
        assert_eq!(Repeat::from_name("Once"), None);
    }

    #[test]
    fn options_read_every_name_and_refuse_any_other() {
        let read = GradientOptions::from_names("oklch", "LONGER", "inoutsine", "Mirror", "centerout", 4).ok();
        let expected = GradientOptions { space: Space::OkLch, hue: HuePath::Longer, easing: Easing::InOutSine, repeat: Repeat::Mirror, direction: Direction::CenterOut, steps: 4 };
        assert_eq!(read, Some(expected));
        assert_eq!(GradientOptions::from_names("OKLab", "Shorter", "Linear", "None", "Forward", 0).ok(), Some(GradientOptions::default()));
        for (space, hue, easing, repeat, direction, steps) in [
            ("HSB", "Shorter", "Linear", "None", "Forward", 0),
            ("RGB", "Around", "Linear", "None", "Forward", 0),
            ("RGB", "Shorter", "EaseIn", "None", "Forward", 0),
            ("RGB", "Shorter", "Linear", "Twice", "Forward", 0),
            ("RGB", "Shorter", "Linear", "None", "Up", 0),
            ("RGB", "Shorter", "Linear", "None", "Forward", -1),
        ] {
            assert!(GradientOptions::from_names(space, hue, easing, repeat, direction, steps).is_err(), "{space} {hue} {easing} {repeat} {direction} {steps}");
        }
    }

    /// The stops a dump line names: hex codes separated by ';', each with an optional '@'
    /// place as split_place reads it.
    fn parse_stops(spec: &str) -> Vec<Stop> {
        spec.split(';')
            .map(|token| {
                let (hex, place) = match split_place(token) {
                    Some((hex, place)) => (hex, Some(place)),
                    None => (token, None),
                };
                Stop { rgb: colors::hex_to_rgb(hex).unwrap().map(i64::from), place }
            })
            .collect()
    }

    fn rgb_list(colors: &[[i64; 3]]) -> String {
        colors.iter().map(|c| format!("{},{},{}", c[0], c[1], c[2])).collect::<Vec<_>>().join(" ")
    }

    /// Writes gradients over a spread of stops and options, one line each with everything that
    /// made it, for comparing PSWriteColorEX's ColorMath with them after a change to either:
    /// cargo test dump_gradients -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_gradients() {
        let even = ["FF0000;00FF00;0000FF", "000000;FFFFFF", "FF8800;0088FF;FF0088;88FF00", "808080;FF0000", "FF0000;808080;0000FF"];
        let placed = ["FF0000;FFD700@40;0000FF", "FF0000@0;0000FF@25", "FF0000@10;FFD700@30;0000FF@60", "000000;FFFFFF@50;000000@50;FFFFFF", "FF0000@20;00FF00@20;0000FF@80"];
        let line = |stops: &str, count: usize, o: &GradientOptions| {
            println!(
                "G {stops} {count} {} {} {} {} {} {} = {}",
                o.space.name(), o.hue.name(), o.easing.name(), o.repeat.name(), o.direction.name(), o.steps,
                rgb_list(&gradient_rgb(&parse_stops(stops), count, o))
            );
        };
        for stops in even.iter().chain(placed.iter()) {
            for space in Space::ALL {
                let hues: &[HuePath] = if matches!(space, Space::OkLch | Space::Lch | Space::Hsl | Space::Hsv) { &HuePath::ALL } else { &[HuePath::Shorter] };
                for &hue in hues {
                    for direction in Direction::ALL {
                        for count in [1, 2, 3, 7, 10, 16] {
                            line(stops, count, &GradientOptions { space, hue, direction, ..Default::default() });
                        }
                    }
                }
            }
        }
        for stops in ["000000;FFFFFF", "FF0000;00FF00;0000FF", "FF0000@0;0000FF@25"] {
            for easing in Easing::ALL {
                for space in [Space::OkLab, Space::Rgb, Space::OkLch] {
                    for count in [9, 16] {
                        line(stops, count, &GradientOptions { space, easing, ..Default::default() });
                    }
                }
            }
        }
        for stops in placed {
            for repeat in Repeat::ALL {
                for direction in Direction::ALL {
                    for count in [9, 17] {
                        line(stops, count, &GradientOptions { repeat, direction, ..Default::default() });
                        line(stops, count, &GradientOptions { repeat, direction, easing: Easing::InOutCubic, ..Default::default() });
                    }
                }
            }
        }
        for stops in ["000000;FFFFFF", "FF0000;00FF00;0000FF", "FF0000@0;0000FF@25"] {
            for steps in [1, 2, 3, 4, 7] {
                for space in [Space::Rgb, Space::OkLab, Space::Hsl] {
                    for count in [12, 13] {
                        line(stops, count, &GradientOptions { space, steps, ..Default::default() });
                        line(stops, count, &GradientOptions { space, steps, repeat: Repeat::Mirror, direction: Direction::CenterOut, ..Default::default() });
                    }
                }
            }
        }
        for stops in even.iter().chain(placed.iter()) {
            for space in [Space::OkLab, Space::Rgb, Space::OkLch, Space::Lab] {
                for (easing, repeat, direction, steps) in [
                    (Easing::Linear, Repeat::None, Direction::Forward, 0),
                    (Easing::OutBounce, Repeat::Mirror, Direction::Reverse, 0),
                    (Easing::InOutElastic, Repeat::Repeat, Direction::EdgesIn, 5),
                ] {
                    let o = GradientOptions { space, easing, repeat, direction, steps, ..Default::default() };
                    for place in ["0", "10", "33.3", "50", "77.75", "100"] {
                        let at = split_place(&format!("x@{place}")).unwrap().1;
                        println!(
                            "A {stops} {place} {} {} {} {} {} {} = {}",
                            o.space.name(), o.hue.name(), o.easing.name(), o.repeat.name(), o.direction.name(), o.steps,
                            rgb_list(&[gradient_rgb_at(&parse_stops(stops), at, &o)])
                        );
                    }
                }
            }
        }
    }
}
