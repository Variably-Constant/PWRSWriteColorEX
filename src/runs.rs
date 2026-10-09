//! The text of a Write-ColorEX call as segments, each a list of runs: text with the colors,
//! styles and link that markup and -Highlight give it. -Split, -SplitAround and -SplitEvenly cut
//! the segments, -Truncate shortens them, and -Wrap breaks them into lines, each a list of items
//! that keep the index of the segment whose colors they take. The steps are PSWriteColorEX's
//! private functions of the same purpose, one for one.

use crate::colors::lookup;
use crate::forms::{color_form, is_hex6, is_white_space};
use crate::width::{display_width, split_display_characters};
use crate::write::ColorArg;

/// What a markup tag or a -Highlight style gives text: styles, a text color, a background color
/// and a link. A color is kept as the tag writes it, after its color form is read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spec {
    pub styles: Vec<&'static str>,
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub link: Option<String>,
}

impl Spec {
    /// A spec that takes what `inner` sets and the rest from `outer`, with the styles of both.
    pub fn merge(outer: Option<&Spec>, inner: &Spec) -> Spec {
        let Some(outer) = outer else { return inner.clone() };
        let mut styles = outer.styles.clone();
        styles.extend(inner.styles.iter().copied());
        Spec {
            styles,
            fg: inner.fg.clone().or_else(|| outer.fg.clone()),
            bg: inner.bg.clone().or_else(|| outer.bg.clone()),
            link: inner.link.clone().or_else(|| outer.link.clone()),
        }
    }
}

/// A run of text and what markup or -Highlight gives it.
#[derive(Clone, Debug, Default)]
pub struct Run {
    pub text: String,
    pub spec: Spec,
    /// The run's colors converted for the color mode in use.
    pub mode_fg: ColorArg,
    pub mode_bg: ColorArg,
    /// The run's display characters, and where the run starts in the gradients, which run across
    /// every character written.
    pub characters: Vec<String>,
    pub gradient_index: usize,
}

impl Run {
    pub fn new(text: impl Into<String>) -> Run {
        Run { text: text.into(), ..Run::default() }
    }

    fn with_spec(text: impl Into<String>, spec: Spec) -> Run {
        Run { text: text.into(), spec, ..Run::default() }
    }

    /// The same run with other text.
    fn piece(&self, text: impl Into<String>) -> Run {
        Run { text: text.into(), ..self.clone() }
    }
}

/// A segment: the runs of one -Text string, or of one part of it.
pub type Segment = Vec<Run>;

/// One piece of a line: the segment whose colors, styles, underline color and link it takes, and
/// its runs.
#[derive(Clone, Debug)]
pub struct Item {
    pub index: usize,
    pub runs: Vec<Run>,
}

/// A segment of one run with no colors, styles or link of its own.
pub fn plain_segment(text: impl Into<String>) -> Segment {
    vec![Run::new(text)]
}

/// The text of a segment, its runs joined.
pub fn segment_text(segment: &[Run]) -> String {
    segment.iter().map(|run| run.text.as_str()).collect()
}

/// The text of segments, their runs joined.
pub fn segments_text(segments: &[Segment]) -> String {
    segments.iter().flat_map(|segment| segment.iter().map(|run| run.text.as_str())).collect()
}

/// The text of a line's items, their runs joined.
pub fn items_text(items: &[Item]) -> String {
    items.iter().flat_map(|item| item.runs.iter().map(|run| run.text.as_str())).collect()
}

/// The style a markup word names, and the style it stands for.
pub fn markup_style(word: &str) -> Option<&'static str> {
    Some(match word.to_lowercase().as_str() {
        "bold" => "Bold",
        "faint" | "dim" => "Faint",
        "italic" => "Italic",
        "underline" => "Underline",
        "blink" => "Blink",
        "crossedout" | "strike" | "strikethrough" => "CrossedOut",
        "doubleunderline" => "DoubleUnderline",
        "overline" => "Overline",
        "reverse" | "invert" => "Reverse",
        "curly" => "Curly",
        "dotted" => "Dotted",
        "dashed" => "Dashed",
        _ => return None,
    })
}

/// Whether a word of a tag or a -Highlight style is a color: a name in the color table, or a hex
/// code of six digits.
fn is_spec_color(form: &str) -> bool {
    is_hex6(form) || lookup(form).is_some()
}

/// The link a word names, as `link=URL` in any letter case; a line end ends the address, unless
/// it is the last character.
fn link_word(word: &str) -> Option<&str> {
    let prefix = word.get(..5)?;
    if !prefix.eq_ignore_ascii_case("link=") {
        return None;
    }
    let rest = &word[5..];
    let rest = rest.strip_suffix('\n').unwrap_or(rest);
    (!rest.is_empty() && !rest.contains('\n')).then_some(rest)
}

/// Reads a markup tag or a -Highlight style: style names, a text color, 'on' and a background
/// color, and link=URL, separated by white space outside parentheses. None when a word is none
/// of these. `warn` gets the warnings reading a color form gives.
pub fn parse_spec(text: &str, warn: &mut dyn FnMut(String)) -> Option<Spec> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for c in text.chars() {
        if c == '(' {
            depth += 1;
        } else if c == ')' && depth > 0 {
            depth -= 1;
        }
        if is_white_space(c) && depth == 0 {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        words.push(current);
    }
    if words.is_empty() {
        return None;
    }

    let mut spec = Spec::default();
    let mut background = false;
    for word in &words {
        if background {
            let form = color_form(word, warn);
            if !is_spec_color(&form) {
                return None;
            }
            spec.bg = Some(form);
            background = false;
            continue;
        }
        if word.eq_ignore_ascii_case("on") {
            if spec.bg.is_some() {
                return None;
            }
            background = true;
            continue;
        }
        if let Some(link) = link_word(word) {
            spec.link = Some(link.to_string());
            continue;
        }
        if let Some(style) = markup_style(word) {
            spec.styles.push(style);
            continue;
        }
        let form = color_form(word, warn);
        if spec.fg.is_some() || !is_spec_color(&form) {
            return None;
        }
        spec.fg = Some(form);
    }
    if background {
        return None;
    }
    Some(spec)
}

/// The runs of a string written with markup: [style]text[/], where [/...] closes the tag opened
/// last, and [[ and ]] write [ and ]. A tag that is not a style, and a closing tag with no tag
/// open, are written as text, with a warning.
pub fn parse_markup(text: &str, warn: &mut dyn FnMut(String)) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut stack: Vec<Spec> = Vec::new();
    let mut buffer = String::new();
    let mut current: Option<Spec> = None;
    let flush = |buffer: &mut String, current: &Option<Spec>, runs: &mut Vec<Run>| {
        if !buffer.is_empty() {
            let run = match current {
                Some(spec) => Run::with_spec(std::mem::take(buffer), spec.clone()),
                None => Run::new(std::mem::take(buffer)),
            };
            runs.push(run);
        }
    };
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < text.len() {
        let c = bytes[i];
        if c == b'[' {
            if bytes.get(i + 1) == Some(&b'[') {
                buffer.push('[');
                i += 2;
                continue;
            }
            let Some(offset) = text[i + 1..].find(']') else {
                buffer.push_str(&text[i..]);
                break;
            };
            let close = i + 1 + offset;
            let tag = &text[i + 1..close];
            if tag.starts_with('/') {
                if stack.is_empty() {
                    warn(format!("Markup tag [{tag}] closes no tag; it is written as text."));
                    buffer.push('[');
                    buffer.push_str(tag);
                    buffer.push(']');
                } else {
                    flush(&mut buffer, &current, &mut runs);
                    stack.pop();
                    current = stack.last().cloned();
                }
            } else {
                match parse_spec(tag, warn) {
                    None => {
                        warn(format!("Markup tag [{tag}] is not a style; it is written as text."));
                        buffer.push('[');
                        buffer.push_str(tag);
                        buffer.push(']');
                    }
                    Some(spec) => {
                        flush(&mut buffer, &current, &mut runs);
                        let merged = Spec::merge(current.as_ref(), &spec);
                        stack.push(merged.clone());
                        current = Some(merged);
                    }
                }
            }
            i = close + 1;
            continue;
        }
        if c == b']' && bytes.get(i + 1) == Some(&b']') {
            buffer.push(']');
            i += 2;
            continue;
        }
        let ch = text[i..].chars().next().unwrap_or_default();
        buffer.push(ch);
        i += ch.len_utf8();
    }
    flush(&mut buffer, &current, &mut runs);
    runs
}

/// A segment's runs cut at the text positions given, byte offsets in ascending order, as one list
/// of runs per part.
fn split_runs(segment: &[Run], cuts: &[usize]) -> Vec<Segment> {
    let mut parts: Vec<Segment> = Vec::new();
    let mut part: Segment = Vec::new();
    let mut offset = 0usize;
    let mut cut_index = 0usize;
    for run in segment {
        let text = run.text.as_str();
        let mut start = 0usize;
        while cut_index < cuts.len() && cuts[cut_index] <= offset + text.len() {
            let at = cuts[cut_index] - offset.min(cuts[cut_index]);
            if at > start {
                part.push(run.piece(&text[start..at]));
            }
            if !part.is_empty() {
                parts.push(std::mem::take(&mut part));
            }
            start = start.max(at);
            cut_index += 1;
        }
        if start < text.len() {
            part.push(run.piece(&text[start..]));
        }
        offset += text.len();
    }
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

/// The places separators occur in a text, as start and end byte offsets, left to right without
/// overlap; where several start at one place, the longest.
fn find_separators(text: &str, separators: &[&str]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut i = 0usize;
    while i < text.len() {
        let rest = &text[i..];
        let longest = separators.iter().filter(|s| rest.starts_with(**s)).map(|s| s.len()).max().unwrap_or(0);
        if longest > 0 {
            found.push((i, i + longest));
            i += longest;
        } else {
            i += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    found
}

/// How -Split, -SplitAround and -SplitEvenly cut the segments.
pub enum SplitBy<'a> {
    /// After each separator, which stays with the part before it.
    After(&'a [String]),
    /// Before and after each separator, which becomes a part of its own.
    Around(&'a [String]),
    /// Into this many parts of display characters as equal as they can be, the first ones a
    /// character longer.
    Evenly(usize),
}

/// The segments cut as `by` says.
pub fn split_segments(segments: Vec<Segment>, by: SplitBy<'_>) -> Vec<Segment> {
    let separators: Vec<&str> = match &by {
        SplitBy::After(list) | SplitBy::Around(list) => list.iter().map(String::as_str).filter(|s| !s.is_empty()).collect(),
        SplitBy::Evenly(_) => Vec::new(),
    };
    let mut result = Vec::with_capacity(segments.len());
    for segment in segments {
        let text = segment_text(&segment);
        let mut cuts: Vec<usize> = Vec::new();
        match by {
            SplitBy::Evenly(parts) => {
                let characters = split_display_characters(&text);
                let count = characters.len();
                let part_count = parts.min(count);
                if part_count > 1 {
                    let size = count / part_count;
                    let longer = count % part_count;
                    let mut position = 0usize;
                    let mut character = 0usize;
                    for p in 0..part_count - 1 {
                        let take = size + usize::from(p < longer);
                        for _ in 0..take {
                            position += characters[character].len();
                            character += 1;
                        }
                        cuts.push(position);
                    }
                }
            }
            SplitBy::After(_) | SplitBy::Around(_) if !separators.is_empty() => {
                let around = matches!(by, SplitBy::Around(_));
                for (start, end) in find_separators(&text, &separators) {
                    if around && start > 0 {
                        cuts.push(start);
                    }
                    if end < text.len() {
                        cuts.push(end);
                    }
                }
            }
            _ => {}
        }
        if cuts.is_empty() {
            result.push(segment);
            continue;
        }
        result.extend(split_runs(&segment, &cuts));
    }
    result
}

/// The segments with -Highlight's styles over the text its patterns match. `matches` answers the
/// matches of entry `entry` in the line's text as start and end byte offsets. Where matches of
/// several patterns overlap, the pattern given first wins; an empty match colors nothing.
pub fn highlight_segments(
    segments: Vec<Segment>,
    styles: &[Spec],
    mut matches: impl FnMut(usize, &str) -> Vec<(usize, usize)>,
) -> Vec<Segment> {
    let line = segments_text(&segments);
    if line.is_empty() {
        return segments;
    }
    let mut owner: Vec<Option<usize>> = vec![None; line.len()];
    for entry in 0..styles.len() {
        for (start, end) in matches(entry, &line) {
            for slot in owner.iter_mut().take(end.min(line.len())).skip(start) {
                if slot.is_none() {
                    *slot = Some(entry);
                }
            }
        }
    }

    let mut result = Vec::with_capacity(segments.len());
    let mut offset = 0usize;
    for segment in segments {
        let mut new_segment: Segment = Vec::new();
        for run in &segment {
            let text = run.text.as_str();
            let mut start = 0usize;
            while start < text.len() {
                let who = owner[offset + start];
                let mut end = start + text[start..].chars().next().map_or(1, char::len_utf8);
                while end < text.len() && owner[offset + end] == who {
                    end += text[end..].chars().next().map_or(1, char::len_utf8);
                }
                let piece = match who {
                    Some(entry) => Run::with_spec(&text[start..end], Spec::merge(Some(&run.spec), &styles[entry])),
                    None => run.piece(&text[start..end]),
                };
                new_segment.push(piece);
                start = end;
            }
            offset += text.len();
        }
        if new_segment.is_empty() {
            new_segment.push(Run::new(""));
        }
        result.push(new_segment);
    }
    result
}

/// The segments cut to a display width with an ellipsis, when they are wider: as many display
/// characters as fit in `width` - 1 cells, then an ellipsis in the colors of the first character
/// that did not fit.
pub fn truncate_segments(segments: Vec<Segment>, width: i64) -> Vec<Segment> {
    if display_width(&segments_text(&segments), false) <= width {
        return segments;
    }
    let room = width - 1;
    let mut used = 0i64;
    let mut done = false;
    let mut result: Vec<Segment> = Vec::new();
    for segment in &segments {
        if done {
            break;
        }
        let mut new_segment: Segment = Vec::new();
        for run in segment {
            if done {
                break;
            }
            let mut kept = String::new();
            for character in split_display_characters(&run.text) {
                let cells = display_width(character, false);
                if used + cells > room {
                    done = true;
                    break;
                }
                used += cells;
                kept.push_str(character);
            }
            if done {
                kept.push('\u{2026}');
            }
            if !kept.is_empty() {
                new_segment.push(run.piece(kept));
            }
        }
        if !new_segment.is_empty() {
            result.push(new_segment);
        }
    }
    if !done {
        return segments;
    }
    if result.is_empty() {
        result.push(plain_segment("\u{2026}"));
    }
    result
}

/// The segments as one line of items, one item per segment.
pub fn single_line(segments: Vec<Segment>) -> Vec<Vec<Item>> {
    vec![segments.into_iter().enumerate().map(|(index, runs)| Item { index, runs }).collect()]
}

/// The segments wrapped into lines no wider than `width` cells. A line breaks at the last space
/// that fits, which is dropped; a word wider than the line breaks at a display character. A line
/// end in the text starts a new line. A run cut across lines keeps its colors, styles and link
/// on each.
pub fn wrap_segments(segments: &[Segment], width: i64) -> Vec<Vec<Item>> {
    struct Character<'a> {
        segment: usize,
        run: usize,
        text: &'a str,
        cells: i64,
    }
    let mut characters: Vec<Character<'_>> = Vec::new();
    for (segment_index, segment) in segments.iter().enumerate() {
        for (run_index, run) in segment.iter().enumerate() {
            for text in split_display_characters(&run.text) {
                characters.push(Character { segment: segment_index, run: run_index, text, cells: display_width(text, false) });
            }
        }
    }

    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut line: Vec<usize> = Vec::new();
    let mut used = 0i64;
    let mut last_space: Option<usize> = None;
    for (k, character) in characters.iter().enumerate() {
        if character.text == "\r" {
            continue;
        }
        if character.text == "\n" {
            lines.push(std::mem::take(&mut line));
            used = 0;
            last_space = None;
            continue;
        }
        if used + character.cells > width && !line.is_empty() {
            if character.text == " " {
                lines.push(std::mem::take(&mut line));
                used = 0;
                last_space = None;
                continue;
            }
            match last_space {
                Some(space) => {
                    let rest = line.split_off(space + 1);
                    line.truncate(space);
                    lines.push(std::mem::replace(&mut line, rest));
                    used = line.iter().map(|&c| characters[c].cells).sum();
                    last_space = None;
                }
                None => {
                    lines.push(std::mem::take(&mut line));
                    used = 0;
                }
            }
        }
        if character.text == " " {
            last_space = Some(line.len());
        }
        line.push(k);
        used += character.cells;
    }
    lines.push(line);

    lines
        .into_iter()
        .map(|characters_of_line| {
            let mut items: Vec<Item> = Vec::new();
            let mut source: Option<(usize, usize)> = None;
            for c in characters_of_line {
                let character = &characters[c];
                if items.last().is_none_or(|item| item.index != character.segment) {
                    items.push(Item { index: character.segment, runs: Vec::new() });
                    source = None;
                }
                let item = items.last_mut().expect("an item was just added");
                if source != Some((character.segment, character.run)) {
                    let run = &segments[character.segment][character.run];
                    item.runs.push(Run { text: String::new(), characters: Vec::new(), ..run.clone() });
                    source = Some((character.segment, character.run));
                }
                let piece = item.runs.last_mut().expect("a run was just added");
                piece.text.push_str(character.text);
                piece.characters.push(character.text.to_string());
            }
            items
        })
        .collect()
}

/// Which side -AutoPad pads wrapped lines on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PadSide {
    Left,
    Center,
    Right,
}

/// Wrapped lines each padded to `width` cells, the padding a segment of its own: before the text
/// with Left, on both sides with Center (the right side one character more when the count is
/// odd), and after it with Right. A padding segment before the text moves the text's segments one
/// on, when any line has padding there. Answers the lines and the count of segments with the
/// padding segments.
pub fn pad_lines(lines: Vec<Vec<Item>>, segment_count: usize, width: i64, pad_char: char, pad_width: i64, side: PadSide) -> (Vec<Vec<Item>>, usize) {
    let counts: Vec<(usize, usize)> = lines
        .iter()
        .map(|items| {
            let line_width = display_width(&items_text(items), false);
            let count = if line_width < width { ((width - line_width) / pad_width.max(1)) as usize } else { 0 };
            match side {
                PadSide::Left => (count, 0),
                PadSide::Center => (count / 2, count - count / 2),
                PadSide::Right => (0, count),
            }
        })
        .collect();
    let any_left = counts.iter().any(|&(left, _)| left > 0);
    let any_right = counts.iter().any(|&(_, right)| right > 0);
    let offset = usize::from(any_left);
    let right_index = segment_count + offset;
    let padding = |count: usize| -> Vec<Run> { plain_segment(std::iter::repeat_n(pad_char, count).collect::<String>()) };
    let padded = lines
        .into_iter()
        .zip(counts)
        .map(|(items, (left, right))| {
            let mut out = Vec::with_capacity(items.len() + 2);
            if left > 0 {
                out.push(Item { index: 0, runs: padding(left) });
            }
            out.extend(items.into_iter().map(|item| Item { index: item.index + offset, runs: item.runs }));
            if right > 0 {
                out.push(Item { index: right_index, runs: padding(right) });
            }
            out
        })
        .collect();
    (padded, segment_count + offset + usize::from(any_right))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(segments: &[Segment]) -> Vec<String> {
        segments.iter().map(|s| segment_text(s)).collect()
    }

    #[test]
    fn markup_runs() {
        let mut warnings = Vec::new();
        let runs = parse_markup("[bold red]Error:[/] file [[x]] [nope]y[/]", &mut |w| warnings.push(w));
        let shown: Vec<(String, Vec<&str>, Option<String>)> = runs.iter().map(|r| (r.text.clone(), r.spec.styles.clone(), r.spec.fg.clone())).collect();
        assert_eq!(
            shown,
            vec![
                ("Error:".to_string(), vec!["Bold"], Some("red".to_string())),
                (" file [x] [nope]y[/]".to_string(), vec![], None),
            ]
        );
        assert_eq!(warnings, vec!["Markup tag [nope] is not a style; it is written as text.", "Markup tag [/] closes no tag; it is written as text."]);
    }

    #[test]
    fn nested_tags_merge() {
        let runs = parse_markup("[red on white]a[bold]b[/]c[/]", &mut |_| {});
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[1].spec.styles, vec!["Bold"]);
        assert_eq!(runs[1].spec.fg.as_deref(), Some("red"));
        assert_eq!(runs[1].spec.bg.as_deref(), Some("white"));
        assert_eq!(runs[2].spec.styles, Vec::<&str>::new());
    }

    #[test]
    fn spec_words() {
        let none = |text: &str| parse_spec(text, &mut |_| {});
        assert!(none("bold on").is_none());
        assert!(none("red blue").is_none());
        assert!(none("red on blue on green").is_none());
        assert_eq!(none("link=https://x.example/a").and_then(|s| s.link).as_deref(), Some("https://x.example/a"));
        assert_eq!(none("rgb(255, 128, 0)").and_then(|s| s.fg).as_deref(), Some("#FF8000"));
        assert_eq!(none("ON #fff").and_then(|s| s.bg).as_deref(), Some("#FFFFFF"));
    }

    #[test]
    fn splitting() {
        let segments = vec![plain_segment("a, b, c")];
        let separators = vec![", ".to_string()];
        assert_eq!(texts(&split_segments(segments.clone(), SplitBy::After(&separators))), vec!["a, ", "b, ", "c"]);
        assert_eq!(texts(&split_segments(segments.clone(), SplitBy::Around(&separators))), vec!["a", ", ", "b", ", ", "c"]);
        assert_eq!(texts(&split_segments(vec![plain_segment("abcdefg")], SplitBy::Evenly(3))), vec!["abc", "de", "fg"]);
    }

    #[test]
    fn truncating() {
        assert_eq!(texts(&truncate_segments(vec![plain_segment("abcdef")], 4)), vec!["abc\u{2026}"]);
        assert_eq!(texts(&truncate_segments(vec![plain_segment("ab"), plain_segment("cdef")], 4)), vec!["ab", "c\u{2026}"]);
        assert_eq!(texts(&truncate_segments(vec![plain_segment("abc")], 4)), vec!["abc"]);
    }

    #[test]
    fn wrapping() {
        let lines = wrap_segments(&[plain_segment("the quick brown fox")], 10);
        let shown: Vec<String> = lines.iter().map(|items| items_text(items)).collect();
        assert_eq!(shown, vec!["the quick", "brown fox"]);
        let lines = wrap_segments(&[plain_segment("abcdefghij"), plain_segment("kl")], 4);
        let shown: Vec<String> = lines.iter().map(|items| items_text(items)).collect();
        assert_eq!(shown, vec!["abcd", "efgh", "ijkl"]);
        assert_eq!(lines[2].iter().map(|i| i.index).collect::<Vec<_>>(), vec![0, 1]);
    }
}
