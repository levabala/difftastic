//! Apply colours and styling to strings.

use std::cmp::{max, min};

use line_numbers::LineNumber;
use line_numbers::SingleLineSpan;
use owo_colors::{OwoColorize, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::lines::split_on_newlines;
use crate::parse::syntax::StringKind;
use crate::{
    constants::Side,
    hash::DftHashMap,
    lines::byte_len,
    options::DisplayOptions,
    parse::syntax::{AtomKind, MatchKind, MatchedPos, TokenKind},
    summary::FileFormat,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum BackgroundColor {
    Dark,
    Light,
}

impl BackgroundColor {
    pub(crate) fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

/// Represents a 24-bit RGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RgbColor {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

impl RgbColor {
    pub(crate) fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse a hex color string like "#336699" or "336699" into an RgbColor.
    pub(crate) fn from_hex(hex: &str) -> Result<Self, String> {
        let hex = hex.trim_start_matches('#');

        if hex.len() != 6 {
            return Err(format!("Invalid hex color '{}': expected 6 characters (RRGGBB)", hex));
        }

        let r = u8::from_str_radix(&hex[0..2], 16)
            .map_err(|_| format!("Invalid hex color '{}': invalid red component", hex))?;
        let g = u8::from_str_radix(&hex[2..4], 16)
            .map_err(|_| format!("Invalid hex color '{}': invalid green component", hex))?;
        let b = u8::from_str_radix(&hex[4..6], 16)
            .map_err(|_| format!("Invalid hex color '{}': invalid blue component", hex))?;

        Ok(Self::new(r, g, b))
    }
}

/// Find the largest byte offset in `s` that gives the longest
/// starting substring whose display width does not exceed `width`.
///
/// If `s` contains full-width Unicode characters, or emoji, or tabs,
/// its display width may be less than `width`.
fn byte_offset_for_width(s: &str, width: usize, tab_width: usize) -> usize {
    let mut current_offset = 0;
    let mut current_width = 0;

    for (offset, ch) in s.char_indices() {
        current_offset = offset;

        let char_width = if ch == '\t' {
            tab_width
        } else {
            ch.width().unwrap_or(0)
        };
        current_width += char_width;

        if current_width > width {
            break;
        }
    }

    current_offset
}

fn substring_by_byte(s: &str, start: usize, end: usize) -> &str {
    &s[start..end]
}

fn substring_by_byte_replace_tabs(s: &str, start: usize, end: usize, tab_width: usize) -> String {
    let s = s[start..end].to_string();
    s.replace('\t', &" ".repeat(tab_width))
}

fn width_respecting_tabs(s: &str, tab_width: usize) -> usize {
    let display_width = s.width();

    // .width() on tabs returns 0, whereas we want to model them as
    // `tab_width` spaces.
    debug_assert_eq!("\t".width(), 0);
    let tab_count = s.matches('\t').count();
    let tab_display_width_extra = tab_count * tab_width;

    display_width + tab_display_width_extra
}

/// Split a string into parts whose display length does not
/// exceed `max_width`.
///
/// If any part has a display width less than `max_width`, also
/// specify the number of spaces required to pad the part to reach the
/// desired width.
///
/// ```
/// split_string_by_width("fooba", 3) // vec![("foo", 0), ("ba", 1)]
/// ```
fn split_string_by_width(s: &str, max_width: usize, tab_width: usize) -> Vec<(&str, usize)> {
    let mut parts: Vec<(&str, usize)> = vec![];
    let mut s = s;

    // Optimisation: width_respecting_tabs() walks the whole string,
    // which is slow when we have files with massive lines.
    //
    // A single character (grapheme) in UTF-8 can be 1, 2, 3 or 4
    // bytes. A character's display width can be 0 (control
    // characters), 1 (the typical case), 2 (e.g. fullwidth characters
    // in Chinese, Japanese and Korean) or 4 (the default width for
    // tabs in difftastic).
    //
    // Ignoring control characters, this means an n-byte UTF-8 string
    // has a display width of at least n/4 characters. Check that case
    // first, because it's a cheap conservative calculation.
    while s.len() / 4 > max_width || width_respecting_tabs(s, tab_width) > max_width {
        let offset = byte_offset_for_width(s, max_width, tab_width);

        let part = substring_by_byte(s, 0, offset);
        s = substring_by_byte(s, offset, s.len());

        let part_width = width_respecting_tabs(part, tab_width);
        let padding = if part_width < max_width {
            max_width - part_width
        } else {
            0
        };
        parts.push((part, padding));
    }

    if parts.is_empty() || !s.is_empty() {
        parts.push((s, max_width - width_respecting_tabs(s, tab_width)));
    }

    parts
}

/// Return a copy of `src` with all the tab characters replaced by
/// `tab_width` strings.
pub(crate) fn replace_tabs(src: &str, tab_width: usize) -> String {
    let tab_as_spaces = " ".repeat(tab_width);
    src.replace('\t', &tab_as_spaces)
}

/// Split `line` (from the source code) into multiple lines of
/// `max_len` (i.e. word wrapping), and apply `styles` to each part
/// according to its original position in `line`.
pub(crate) fn split_and_apply(
    line: &str,
    max_len: usize,
    tab_width: usize,
    styles: &[(SingleLineSpan, Style)],
    side: Side,
) -> Vec<String> {
    assert!(
        max_len > 0,
        "Splitting lines into pieces of length 0 will never terminate"
    );
    assert!(
        max_len > tab_width,
        "Parts must be big enough to hold at least one tab (max_len = {} tab_width = {})",
        max_len,
        tab_width
    );

    if styles.is_empty() && !line.trim().is_empty() {
        return split_string_by_width(line, max_len, tab_width)
            .into_iter()
            .map(|(part, pad)| {
                let part = replace_tabs(part, tab_width);

                let mut parts = String::with_capacity(part.len() + pad);
                parts.push_str(&part);

                if matches!(side, Side::Left) {
                    parts.push_str(&" ".repeat(pad));
                }
                parts
            })
            .collect();
    }

    let mut styled_parts = vec![];
    let mut part_start = 0;

    for (line_part, pad) in split_string_by_width(line, max_len, tab_width) {
        let mut res = String::with_capacity(line_part.len() + pad);
        let mut prev_style_end = 0;
        for (span, style) in styles {
            let start_col = span.start_col as usize;
            let end_col = span.end_col as usize;

            // The remaining spans are beyond the end of this line_part.
            if start_col >= part_start + byte_len(line_part) {
                break;
            }

            // If there's an unstyled gap before the next span.
            if start_col > part_start && prev_style_end < start_col {
                // Then append that text without styling.
                let unstyled_start = max(prev_style_end, part_start);
                res.push_str(&substring_by_byte_replace_tabs(
                    line_part,
                    unstyled_start - part_start,
                    start_col - part_start,
                    tab_width,
                ));
            }

            // Apply style to the substring in this span.
            if end_col > part_start {
                let span_s = substring_by_byte_replace_tabs(
                    line_part,
                    max(0, span.start_col as isize - part_start as isize) as usize,
                    min(byte_len(line_part), end_col - part_start),
                    tab_width,
                );
                res.push_str(&span_s.style(*style).to_string());
            }
            prev_style_end = end_col;
        }

        // Ensure that prev_style_end is at least at the start of this
        // line_part.
        if prev_style_end < part_start {
            prev_style_end = part_start;
        }

        // Unstyled text after the last span.
        if prev_style_end < part_start + byte_len(line_part) {
            let span_s = &substring_by_byte_replace_tabs(
                line_part,
                prev_style_end - part_start,
                byte_len(line_part),
                tab_width,
            );
            res.push_str(span_s);
        }

        if matches!(side, Side::Left) {
            res.push_str(&" ".repeat(pad));
        }

        styled_parts.push(res);
        part_start += byte_len(line_part);
    }

    styled_parts
}

/// Return a copy of `line` with styles applied to all the spans
/// specified.
fn apply_line(line: &str, styles: &[(SingleLineSpan, Style)]) -> String {
    let line_bytes = byte_len(line);
    let mut styled_line = String::with_capacity(line.len());
    let mut i = 0;
    for (span, style) in styles {
        let start_col = span.start_col as usize;
        let end_col = span.end_col as usize;

        // The remaining spans are beyond the end of this line. This
        // occurs when we truncate the line to fit on the display.
        if start_col >= line_bytes {
            break;
        }

        // Unstyled text before the next span.
        if i < start_col {
            styled_line.push_str(substring_by_byte(line, i, start_col));
        }

        // Apply style to the substring in this span.
        let span_s = substring_by_byte(line, start_col, min(line_bytes, end_col));
        styled_line.push_str(&span_s.style(*style).to_string());
        i = end_col;
    }

    // Unstyled text after the last span.
    if i < line_bytes {
        let span_s = substring_by_byte(line, i, line_bytes);
        styled_line.push_str(span_s);
    }
    styled_line
}

fn group_by_line(
    ranges: &[(SingleLineSpan, Style)],
) -> DftHashMap<LineNumber, Vec<(SingleLineSpan, Style)>> {
    let mut ranges_by_line: DftHashMap<_, Vec<_>> = DftHashMap::default();
    for range in ranges {
        if let Some(matching_ranges) = ranges_by_line.get_mut(&range.0.line) {
            (*matching_ranges).push(*range);
        } else {
            ranges_by_line.insert(range.0.line, vec![*range]);
        }
    }

    ranges_by_line
}

/// Apply the `Style`s to the spans specified. Return a vec of the
/// styled strings, including trailing newlines.
///
/// Tolerant against lines in `s` being shorter than the spans.
fn style_lines(lines: &[&str], styles: &[(SingleLineSpan, Style)]) -> Vec<String> {
    let mut ranges_by_line = group_by_line(styles);

    let mut styled_lines = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let mut styled_line = String::with_capacity(line.len());
        let ranges = ranges_by_line
            .remove::<LineNumber>(&(i as u32).into())
            .unwrap_or_default();

        styled_line.push_str(&apply_line(line, &ranges));
        styled_line.push('\n');
        styled_lines.push(styled_line);
    }
    styled_lines
}

pub(crate) fn novel_style(
    style: Style,
    side: Side,
    background: BackgroundColor,
    use_background: bool,
    rgb_added: Option<RgbColor>,
    rgb_removed: Option<RgbColor>,
) -> Style {
    if use_background {
        // Check if custom 24-bit RGB colors are provided
        let custom_color = match side {
            Side::Right => rgb_added,
            Side::Left => rgb_removed,
        };

        if let Some(rgb) = custom_color {
            // Use custom 24-bit RGB background color
            style.on_truecolor(rgb.r, rgb.g, rgb.b)
        } else {
            // Use subtle gray background colors instead of red/green
            // This works well in side-by-side view where position indicates add/remove
            // Much less visually aggressive while preserving syntax highlighting
            if background.is_dark() {
                // Use bright_black (gray) for dark terminals
                style.on_bright_black()
            } else {
                // Use black (appears as dark gray) for light terminals
                style.on_black()
            }
        }
    } else {
        // Use foreground colors (original behavior)
        if background.is_dark() {
            match side {
                Side::Left => style.bright_red(),
                Side::Right => style.bright_green(),
            }
        } else {
            match side {
                Side::Left => style.red(),
                Side::Right => style.green(),
            }
        }
    }
}

fn syntax_style(style: Style, atom_kind: AtomKind, background: BackgroundColor) -> Style {
    let style = match atom_kind {
        AtomKind::String(StringKind::StringLiteral) | AtomKind::String(StringKind::Text) => {
            foreground_color(style, background, zenburn("#cc9393"), zenburn("#cc9393"))
        }
        AtomKind::Comment => {
            foreground_color(style, background, zenburn("#7f9f7f"), zenburn("#7f9f7f"))
        }
        AtomKind::Keyword => {
            foreground_color(style, background, zenburn("#f0dfaf"), zenburn("#f0dfaf"))
        }
        AtomKind::Type | AtomKind::Punctuation => {
            foreground_color(style, background, zenburn("#8f8f8f"), zenburn("#8f8f8f"))
        }
        AtomKind::Function => {
            foreground_color(style, background, zenburn("#efef8f"), zenburn("#efef8f"))
        }
        AtomKind::Property => {
            foreground_color(style, background, zenburn("#efdcbc"), zenburn("#efdcbc"))
        }
        AtomKind::Number => {
            foreground_color(style, background, zenburn("#8cd0d3"), zenburn("#8cd0d3"))
        }
        AtomKind::Constant => {
            foreground_color(style, background, zenburn("#dca3a3"), zenburn("#dca3a3"))
        }
        AtomKind::Variable | AtomKind::Normal => {
            foreground_color(style, background, zenburn("#dcdccc"), zenburn("#dcdccc"))
        }
        AtomKind::Operator => {
            foreground_color(style, background, zenburn("#f0efd0"), zenburn("#f0efd0"))
        }
        AtomKind::Tag => {
            foreground_color(style, background, zenburn("#e89393"), zenburn("#e89393"))
        }
        AtomKind::Attribute | AtomKind::Decorator => {
            foreground_color(style, background, zenburn("#ffcfaf"), zenburn("#ffcfaf"))
        }
        AtomKind::Parameter => {
            foreground_color(style, background, zenburn("#ffcfaf"), zenburn("#ffcfaf"))
        }
        AtomKind::Constructor | AtomKind::Namespace => {
            foreground_color(style, background, zenburn("#dfaf8f"), zenburn("#dfaf8f"))
        }
        AtomKind::Label => {
            foreground_color(style, background, zenburn("#dfcfaf"), zenburn("#dfcfaf"))
        }
        AtomKind::TreeSitterError => style.purple(),
    };

    style
}

fn zenburn(hex: &str) -> RgbColor {
    RgbColor::from_hex(hex).expect("Zenburn colors should be valid hex")
}

fn foreground_color(
    style: Style,
    background: BackgroundColor,
    dark_background_color: RgbColor,
    light_background_color: RgbColor,
) -> Style {
    let color = if background.is_dark() {
        dark_background_color
    } else {
        light_background_color
    };

    style.truecolor(color.r, color.g, color.b)
}

fn maybe_syntax_style(
    style: Style,
    highlight: TokenKind,
    background: BackgroundColor,
    syntax_highlight: bool,
) -> Style {
    if !syntax_highlight {
        return style;
    }

    let TokenKind::Atom(atom_kind) = highlight else {
        return style;
    };

    syntax_style(style, atom_kind, background)
}

/// Merge spans where the end of one span matches the start of the
/// next span.
///
/// This reduces the number of ANSI character codes in the
/// output. This is negligible for performance, but makes regression
/// testing easier for difftastic.
///
/// The file compare.expected contains hashes of the output, so it
/// considers `<green>ab</green>` to be distinct from
/// `<green>a</green><green>b</green>`. Merging the spans normalises
/// the output to `<green>ab</green>`.
fn merge_adjacent(items: &[(SingleLineSpan, Style)]) -> Vec<(SingleLineSpan, Style)> {
    let mut merged: Vec<(SingleLineSpan, Style)> = vec![];
    let mut prev_item: Option<(SingleLineSpan, Style)> = None;

    for (span, style) in items.iter().copied() {
        match prev_item.take() {
            Some((mut prev_span, prev_style)) => {
                if prev_style == style
                    && prev_span.line == span.line
                    && prev_span.end_col == span.start_col
                {
                    prev_span.end_col = span.end_col;
                    prev_item = Some((prev_span, style));
                } else {
                    merged.push((prev_span, prev_style));
                    prev_item = Some((span, style));
                }
            }
            None => {
                prev_item = Some((span, style));
            }
        }
    }

    if let Some(last_item) = prev_item {
        merged.push(last_item);
    }

    merged
}

fn apply_whitespace_background_in_changed_regions(
    src: &str,
    side: Side,
    background: BackgroundColor,
    mps: &[MatchedPos],
    styled_spans: &[(SingleLineSpan, Style)],
    rgb_added: Option<RgbColor>,
    rgb_removed: Option<RgbColor>,
) -> Vec<(SingleLineSpan, Style)> {
    let src_lines = split_on_newlines(src).collect::<Vec<_>>();

    let mut mps_by_line: DftHashMap<LineNumber, Vec<&MatchedPos>> = DftHashMap::default();
    for mp in mps {
        mps_by_line
            .entry(mp.pos.line)
            .or_insert_with(Vec::new)
            .push(mp);
    }

    for mps in mps_by_line.values_mut() {
        mps.sort_by_key(|mp| (mp.pos.start_col, mp.pos.end_col));
    }

    let mut changed_regions_by_line: DftHashMap<LineNumber, SingleLineSpan> = DftHashMap::default();

    for mp in mps {
        if mp.kind.is_novel() {
            changed_regions_by_line
                .entry(mp.pos.line)
                .and_modify(|region| {
                    region.start_col = min(region.start_col, mp.pos.start_col);
                    region.end_col = max(region.end_col, mp.pos.end_col);
                })
                .or_insert(mp.pos);
        }
    }

    let mut whitespace_ranges_by_line: DftHashMap<LineNumber, Vec<SingleLineSpan>> =
        DftHashMap::default();

    for (line_num, changed_region) in changed_regions_by_line {
        let Some(line) = src_lines.get(line_num.as_usize()) else {
            continue;
        };

        let region_start = changed_region.start_col as usize;
        let region_end = changed_region.end_col as usize;
        if region_start >= region_end || region_end > byte_len(line) {
            continue;
        }

        let region = substring_by_byte(line, region_start, region_end);
        let mut whitespace_start = None;

        for (offset, ch) in region.char_indices() {
            if ch.is_whitespace() {
                whitespace_start.get_or_insert(offset);

                continue;
            }

            if let Some(start) = whitespace_start.take() {
                push_whitespace_range_between_changes(
                    &mut whitespace_ranges_by_line,
                    &mps_by_line,
                    line,
                    line_num,
                    region_start + start,
                    region_start + offset,
                );
            }
        }

        if let Some(start) = whitespace_start {
            push_whitespace_range_between_changes(
                &mut whitespace_ranges_by_line,
                &mps_by_line,
                line,
                line_num,
                region_start + start,
                region_end,
            );
        }
    }

    let mut styled_spans_by_line: DftHashMap<LineNumber, Vec<SingleLineSpan>> =
        DftHashMap::default();

    for (span, _) in styled_spans {
        styled_spans_by_line
            .entry(span.line)
            .or_insert_with(Vec::new)
            .push(*span);
    }

    for spans in styled_spans_by_line.values_mut() {
        spans.sort_by_key(|span| (span.start_col, span.end_col));
    }

    let whitespace_style =
        novel_style(Style::new(), side, background, true, rgb_added, rgb_removed);

    let mut styled_spans_with_whitespace = Vec::with_capacity(styled_spans.len());

    for (span, style) in styled_spans {
        push_span_with_whitespace_background(
            &mut styled_spans_with_whitespace,
            *span,
            *style,
            whitespace_ranges_by_line.get(&span.line).map(Vec::as_slice),
            side,
            background,
            rgb_added,
            rgb_removed,
        );
    }

    for (line_num, whitespace_ranges) in whitespace_ranges_by_line {
        let styled_spans = styled_spans_by_line.get(&line_num).map(Vec::as_slice);

        for whitespace_range in whitespace_ranges {
            push_unstyled_whitespace_spans(
                &mut styled_spans_with_whitespace,
                line_num,
                whitespace_range.start_col as usize,
                whitespace_range.end_col as usize,
                styled_spans,
                whitespace_style,
            );
        }
    }

    styled_spans_with_whitespace
}

fn push_whitespace_range_between_changes(
    whitespace_ranges_by_line: &mut DftHashMap<LineNumber, Vec<SingleLineSpan>>,
    mps_by_line: &DftHashMap<LineNumber, Vec<&MatchedPos>>,
    line: &str,
    line_num: LineNumber,
    start_col: usize,
    end_col: usize,
) {
    if whitespace_is_inside_unchanged_region(
        line,
        start_col as u32,
        end_col as u32,
        mps_by_line.get(&line_num).map(Vec::as_slice),
    ) {
        return;
    }

    whitespace_ranges_by_line
        .entry(line_num)
        .or_insert_with(Vec::new)
        .push(SingleLineSpan {
            line: line_num,
            start_col: start_col as u32,
            end_col: end_col as u32,
        });
}

fn whitespace_is_inside_unchanged_region(
    line: &str,
    start_col: u32,
    end_col: u32,
    mps: Option<&[&MatchedPos]>,
) -> bool {
    let mps = mps.unwrap_or_default();

    let previous_mp = mps
        .iter()
        .rev()
        .find(|mp| mp.pos.end_col <= start_col && span_has_non_whitespace(line, mp.pos));

    let next_mp = mps
        .iter()
        .find(|mp| mp.pos.start_col >= end_col && span_has_non_whitespace(line, mp.pos));

    matches!(
        (previous_mp, next_mp),
        (Some(previous_mp), Some(next_mp))
            if !previous_mp.kind.is_novel() && !next_mp.kind.is_novel()
    )
}

fn span_has_non_whitespace(line: &str, span: SingleLineSpan) -> bool {
    let line_bytes = byte_len(line);
    let start_col = span.start_col as usize;
    let end_col = span.end_col as usize;

    if start_col >= end_col || end_col > line_bytes {
        return false;
    }

    substring_by_byte(line, start_col, end_col)
        .chars()
        .any(|ch| !ch.is_whitespace())
}

fn push_span_with_whitespace_background(
    styled_spans: &mut Vec<(SingleLineSpan, Style)>,
    span: SingleLineSpan,
    style: Style,
    whitespace_ranges: Option<&[SingleLineSpan]>,
    side: Side,
    background: BackgroundColor,
    rgb_added: Option<RgbColor>,
    rgb_removed: Option<RgbColor>,
) {
    let Some(whitespace_ranges) = whitespace_ranges else {
        styled_spans.push((span, style));

        return;
    };

    let mut next_start = span.start_col;

    for whitespace_range in whitespace_ranges {
        if whitespace_range.end_col <= next_start {
            continue;
        }

        if whitespace_range.start_col >= span.end_col {
            break;
        }

        let whitespace_start = max(next_start, whitespace_range.start_col);
        let whitespace_end = min(span.end_col, whitespace_range.end_col);

        if next_start < whitespace_start {
            styled_spans.push((
                SingleLineSpan {
                    line: span.line,
                    start_col: next_start,
                    end_col: whitespace_start,
                },
                style,
            ));
        }

        styled_spans.push((
            SingleLineSpan {
                line: span.line,
                start_col: whitespace_start,
                end_col: whitespace_end,
            },
            novel_style(style, side, background, true, rgb_added, rgb_removed),
        ));

        next_start = whitespace_end;
        if next_start >= span.end_col {
            return;
        }
    }

    styled_spans.push((
        SingleLineSpan {
            line: span.line,
            start_col: next_start,
            end_col: span.end_col,
        },
        style,
    ));
}

fn push_unstyled_whitespace_spans(
    whitespace_spans: &mut Vec<(SingleLineSpan, Style)>,
    line: LineNumber,
    start_col: usize,
    end_col: usize,
    styled_spans: Option<&[SingleLineSpan]>,
    style: Style,
) {
    let mut unstyled_start = start_col as u32;
    let end_col = end_col as u32;

    for styled_span in styled_spans.unwrap_or_default() {
        if styled_span.end_col <= unstyled_start {
            continue;
        }

        if styled_span.start_col >= end_col {
            break;
        }

        if styled_span.start_col > unstyled_start {
            whitespace_spans.push((
                SingleLineSpan {
                    line,
                    start_col: unstyled_start,
                    end_col: styled_span.start_col,
                },
                style,
            ));
        }

        unstyled_start = max(unstyled_start, styled_span.end_col);
        if unstyled_start >= end_col {

            return;
        }
    }

    whitespace_spans.push((
        SingleLineSpan {
            line,
            start_col: unstyled_start,
            end_col,
        },
        style,
    ));
}

pub(crate) fn color_positions(
    src: &str,
    side: Side,
    background: BackgroundColor,
    syntax_highlight: bool,
    background_diff_colors: bool,
    background_include_whitespace: bool,
    file_format: &FileFormat,
    mps: &[MatchedPos],
    rgb_added: Option<RgbColor>,
    rgb_removed: Option<RgbColor>,
) -> Vec<(SingleLineSpan, Style)> {
    let mut styles = vec![];
    for mp in mps {
        let mut style = Style::new();
        match mp.kind {
            MatchKind::UnchangedToken { highlight, .. } | MatchKind::Ignored { highlight } => {
                style = maybe_syntax_style(style, highlight, background, syntax_highlight);
            }
            MatchKind::Novel { highlight, .. } => {
                if background_diff_colors && syntax_highlight {
                    style = maybe_syntax_style(style, highlight, background, syntax_highlight);
                }

                style = novel_style(
                    style,
                    side,
                    background,
                    background_diff_colors,
                    rgb_added,
                    rgb_removed,
                );

            }
            MatchKind::NovelWord { highlight } => {
                if background_diff_colors && syntax_highlight {
                    style = maybe_syntax_style(style, highlight, background, syntax_highlight);
                }

                style = novel_style(
                    style,
                    side,
                    background,
                    background_diff_colors,
                    rgb_added,
                    rgb_removed,
                )
                .bold();

                // Underline novel words inside comments in code, but
                // don't apply it to every single line in plaintext.
                if matches!(file_format, FileFormat::SupportedLanguage(_)) {
                    style = style.underline();
                }
            }
            MatchKind::UnchangedPartOfNovelItem { highlight, .. } => {
                if background_diff_colors && syntax_highlight {
                    style = maybe_syntax_style(style, highlight, background, syntax_highlight);
                }

                style = novel_style(
                    style,
                    side,
                    background,
                    background_diff_colors,
                    rgb_added,
                    rgb_removed,
                );
            }
        };
        styles.push((mp.pos, style));
    }

    if background_diff_colors && background_include_whitespace {
        styles = apply_whitespace_background_in_changed_regions(
            src,
            side,
            background,
            mps,
            &styles,
            rgb_added,
            rgb_removed,
        );
        styles.sort_by_key(|(span, _)| (span.line, span.start_col, span.end_col));
    }

    merge_adjacent(&styles)
}

pub(crate) fn apply_colors(
    s: &str,
    side: Side,
    syntax_highlight: bool,
    background_diff_colors: bool,
    background_include_whitespace: bool,
    file_format: &FileFormat,
    background: BackgroundColor,
    mps: &[MatchedPos],
    rgb_added: Option<RgbColor>,
    rgb_removed: Option<RgbColor>,
) -> Vec<String> {
    let styles = color_positions(
        s,
        side,
        background,
        syntax_highlight,
        background_diff_colors,
        background_include_whitespace,
        file_format,
        mps,
        rgb_added,
        rgb_removed,
    );
    let lines = split_on_newlines(s).collect::<Vec<_>>();
    style_lines(&lines, &styles)
}

fn apply_header_color(
    s: &str,
    use_color: bool,
    background: BackgroundColor,
    hunk_num: usize,
) -> String {
    if use_color {
        if hunk_num != 1 {
            s.to_owned()
        } else if background.is_dark() {
            s.bright_yellow().to_string()
        } else {
            s.yellow().to_string()
        }
        .bold()
        .to_string()
    } else {
        s.to_owned()
    }
}

/// Style `s` as a warning and write to stderr.
pub(crate) fn print_warning(s: &str, display_options: &DisplayOptions) {
    let prefix = if display_options.use_color {
        if display_options.background_color.is_dark() {
            "warning: ".bright_yellow().to_string()
        } else {
            "warning: ".yellow().to_string()
        }
        .bold()
        .to_string()
    } else {
        "warning: ".to_owned()
    };

    eprint!("{}", prefix);
    eprint!("{}\n\n", s);
}

/// Style `s` as an error and write to stderr.
pub(crate) fn print_error(s: &str, use_color: bool) {
    // TODO: this is inconsistent with print_warning regarding
    // arguments and trailing whitespace.
    let prefix = if use_color {
        "error: ".red().bold().to_string()
    } else {
        "error: ".to_owned()
    };

    eprintln!("{}{}", prefix, s);
}

pub(crate) fn apply_line_number_color(
    s: &str,
    is_novel: bool,
    side: Side,
    display_options: &DisplayOptions,
) -> String {
    if display_options.use_color {
        let mut style = Style::new();

        // The goal here is to choose a style for line numbers that is
        // visually distinct from content.
        if is_novel {
            // For changed lines, show the line number as red/green
            // and bold. This works well for syntactic diffs, where
            // most content is not bold.
            // Note: Line numbers always use foreground colors, not background colors
            style = novel_style(style, side, display_options.background_color, false, None, None).bold();
        } else {
            // For unchanged lines, dim the line numbers so it's
            // clearly separate from the content.
            style = style.dimmed()
        }

        s.style(style).to_string()
    } else {
        s.to_owned()
    }
}

pub(crate) fn header(
    display_path: &str,
    extra_info: Option<&String>,
    hunk_num: usize,
    hunk_total: usize,
    file_format: &FileFormat,
    display_options: &DisplayOptions,
) -> String {
    let divider = if hunk_total == 1 {
        "".to_owned()
    } else {
        format!("{}/{} --- ", hunk_num, hunk_total)
    };

    let display_path_pretty = apply_header_color(
        display_path,
        display_options.use_color,
        display_options.background_color,
        hunk_num,
    );

    let mut trailer = format!(" --- {}{}", divider, file_format);
    if display_options.use_color {
        trailer = trailer.dimmed().to_string();
    }

    match extra_info {
        Some(extra_info) if hunk_num == 1 => {
            let mut extra_info = extra_info.clone();
            if display_options.use_color {
                extra_info = extra_info.dimmed().to_string();
            }

            format!("{}{}\n{}", display_path_pretty, trailer, extra_info)
        }
        _ => {
            format!("{}{}", display_path_pretty, trailer)
        }
    }
}

#[cfg(test)]
mod tests {
    const TAB_WIDTH: usize = 2;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn split_string_simple() {
        assert_eq!(
            split_string_by_width("fooba", 3, TAB_WIDTH),
            vec![("foo", 0), ("ba", 1)]
        );
    }

    #[test]
    fn split_string_unicode() {
        assert_eq!(
            split_string_by_width("ab📦def", 4, TAB_WIDTH),
            vec![("ab📦", 0), ("def", 1)]
        );
    }

    #[test]
    fn test_combining_char() {
        assert_eq!(
            split_string_by_width("aabbcc\u{300}x", 6, TAB_WIDTH),
            vec![("aabbcc\u{300}", 0), ("x", 5)],
        );
    }

    #[test]
    fn split_string_cjk() {
        assert_eq!(
            split_string_by_width("一个汉字两列宽", 8, TAB_WIDTH),
            vec![("一个汉字", 0), ("两列宽", 2)]
        );
    }

    #[test]
    fn split_string_cjk2() {
        assert_eq!(
            split_string_by_width("你好啊", 5, TAB_WIDTH),
            vec![("你好", 1), ("啊", 3)]
        );
    }

    #[test]
    fn test_split_and_apply() {
        let res = split_and_apply(
            "foo",
            3,
            TAB_WIDTH,
            &[(
                SingleLineSpan {
                    line: 0.into(),
                    start_col: 0,
                    end_col: 3,
                },
                Style::new(),
            )],
            Side::Left,
        );
        assert_eq!(res, vec!["foo"])
    }

    #[test]
    fn test_split_and_apply_trailing_text() {
        let res = split_and_apply(
            "foobar",
            6,
            TAB_WIDTH,
            &[(
                SingleLineSpan {
                    line: 0.into(),
                    start_col: 0,
                    end_col: 3,
                },
                Style::new(),
            )],
            Side::Left,
        );
        assert_eq!(res, vec!["foobar"])
    }

    #[test]
    fn test_split_and_apply_gap_between_styles_on_wrap_boundary() {
        let res = split_and_apply(
            "foobar",
            3,
            TAB_WIDTH,
            &[
                (
                    SingleLineSpan {
                        line: 0.into(),
                        start_col: 0,
                        end_col: 2,
                    },
                    Style::new(),
                ),
                (
                    SingleLineSpan {
                        line: 0.into(),
                        start_col: 4,
                        end_col: 6,
                    },
                    Style::new(),
                ),
            ],
            Side::Left,
        );
        assert_eq!(res, vec!["foo", "bar"])
    }

    #[test]
    fn test_split_and_apply_trailing_text_newline() {
        let res = split_and_apply(
            "foobar      ",
            6,
            TAB_WIDTH,
            &[(
                SingleLineSpan {
                    line: 0.into(),
                    start_col: 0,
                    end_col: 3,
                },
                Style::new(),
            )],
            Side::Left,
        );
        assert_eq!(res, vec!["foobar", "      "])
    }

    #[test]
    fn background_include_whitespace_adds_space_gap_between_novel_spans() {
        let positions = color_positions(
            "foo bar",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            true,
            &FileFormat::PlainText,
            &[novel_pos(0, 0, 3), novel_pos(0, 4, 7)],
            None,
            None,
        );
        let spans = positions
            .into_iter()
            .map(|(span, _)| span)
            .collect::<Vec<_>>();

        assert_eq!(
            spans,
            vec![
                span(0, 0, 3),
                span(0, 3, 4),
                span(0, 4, 7),
            ]
        );
    }

    #[test]
    fn background_include_whitespace_styles_rendered_space_gap() {
        let positions = color_positions(
            "foo bar",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            true,
            &FileFormat::PlainText,
            &[novel_pos(0, 0, 3), novel_pos(0, 4, 7)],
            Some(RgbColor::new(1, 2, 3)),
            None,
        );
        let rendered = split_and_apply("foo bar", 80, TAB_WIDTH, &positions, Side::Right);

        assert!(
            rendered[0].contains("\x1b[48;2;1;2;3m \x1b"),
            "expected the space between changed spans to have a background: {:?}",
            rendered[0]
        );
    }

    #[test]
    fn background_include_whitespace_styles_space_inside_punctuation_gap() {
        let positions = color_positions(
            "0, 0, 0",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            true,
            &FileFormat::PlainText,
            &[
                novel_pos(0, 0, 1),
                novel_pos(0, 3, 4),
                novel_pos(0, 6, 7),
            ],
            Some(RgbColor::new(1, 2, 3)),
            None,
        );
        let rendered = split_and_apply("0, 0, 0", 80, TAB_WIDTH, &positions, Side::Right);

        assert!(
            rendered[0].contains(",\x1b[48;2;1;2;3m \x1b"),
            "expected spaces after punctuation between changed spans to have a background: {:?}",
            rendered[0]
        );
    }

    #[test]
    fn background_include_whitespace_styles_if_let_region_gaps() {
        let positions = color_positions(
            "if let Some(start) = whitespace",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            true,
            &FileFormat::PlainText,
            &[
                novel_pos(0, 0, 2),
                unchanged_pos(0, 3, 6),
                novel_pos(0, 7, 11),
                unchanged_pos(0, 11, 12),
                novel_pos(0, 12, 17),
                unchanged_pos(0, 17, 18),
                unchanged_pos(0, 19, 20),
                novel_pos(0, 21, 31),
            ],
            Some(RgbColor::new(1, 2, 3)),
            None,
        );

        let rendered = split_and_apply(
            "if let Some(start) = whitespace",
            80,
            TAB_WIDTH,
            &positions,
            Side::Right,
        );

        assert!(
            rendered[0].contains("\x1b[48;2;1;2;3m \x1b"),
            "expected spaces inside the if-let changed region to have a background: {:?}",
            rendered[0]
        );

        assert!(
            rendered[0].contains("=\x1b[0m\x1b[48;2;1;2;3m \x1b"),
            "expected the space after = inside the changed region to have a background: {:?}",
            rendered[0]
        );
    }

    #[test]
    fn background_include_whitespace_styles_existing_uncolored_space_spans() {
        let positions = color_positions(
            "for spans in values_mut",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            true,
            &FileFormat::PlainText,
            &[
                novel_pos(0, 0, 3),
                unchanged_normal_pos(0, 3, 4),
                novel_pos(0, 4, 9),
                unchanged_normal_pos(0, 9, 10),
                novel_pos(0, 10, 12),
                unchanged_normal_pos(0, 12, 13),
                novel_pos(0, 13, 23),
            ],
            Some(RgbColor::new(1, 2, 3)),
            None,
        );

        let rendered = split_and_apply(
            "for spans in values_mut",
            80,
            TAB_WIDTH,
            &positions,
            Side::Right,
        );

        assert!(
            rendered[0].contains("for\x1b[0m\x1b[38;2;220;220;204;48;2;1;2;3m \x1b"),
            "expected existing uncolored space spans to receive a background: {:?}",
            rendered[0]
        );
    }

    #[test]
    fn background_include_whitespace_ignores_spaces_inside_unchanged_expression() {
        let positions = color_positions(
            "start_col: (region_start + start) as u32,",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            true,
            &FileFormat::PlainText,
            &[
                novel_pos(0, 0, 12),
                unchanged_normal_pos(0, 12, 24),
                unchanged_normal_pos(0, 25, 26),
                unchanged_normal_pos(0, 27, 32),
                novel_pos(0, 32, 40),
            ],
            Some(RgbColor::new(1, 2, 3)),
            None,
        );

        let rendered = split_and_apply(
            "start_col: (region_start + start) as u32,",
            80,
            TAB_WIDTH,
            &positions,
            Side::Right,
        );

        assert!(
            rendered[0].contains(
                "\x1b[38;2;220;220;204mregion_start\x1b[0m \x1b[38;2;220;220;204m+\x1b[0m \x1b[38;2;220;220;204mstart"
            ),
            "expected spaces around unchanged + to stay uncolored: {:?}",
            rendered[0]
        );
    }

    #[test]
    fn background_include_whitespace_adds_tab_gap_between_novel_spans() {
        let positions = color_positions(
            "foo\tbar",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            true,
            &FileFormat::PlainText,
            &[novel_pos(0, 0, 3), novel_pos(0, 4, 7)],
            None,
            None,
        );
        let spans = positions
            .into_iter()
            .map(|(span, _)| span)
            .collect::<Vec<_>>();

        assert_eq!(
            spans,
            vec![
                span(0, 0, 3),
                span(0, 3, 4),
                span(0, 4, 7),
            ]
        );
    }

    #[test]
    fn background_include_whitespace_ignores_non_whitespace_gap_between_novel_spans() {
        let positions = color_positions(
            "foo-bar",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            true,
            &FileFormat::PlainText,
            &[novel_pos(0, 0, 3), novel_pos(0, 4, 7)],
            None,
            None,
        );
        let spans = positions
            .into_iter()
            .map(|(span, _)| span)
            .collect::<Vec<_>>();

        assert_eq!(spans, vec![span(0, 0, 3), span(0, 4, 7)]);
    }

    #[test]
    fn background_include_whitespace_does_not_apply_in_foreground_mode() {
        let positions = color_positions(
            "foo bar",
            Side::Right,
            BackgroundColor::Dark,
            true,
            false,
            true,
            &FileFormat::PlainText,
            &[novel_pos(0, 0, 3), novel_pos(0, 4, 7)],
            None,
            None,
        );
        let spans = positions
            .into_iter()
            .map(|(span, _)| span)
            .collect::<Vec<_>>();

        assert_eq!(spans, vec![span(0, 0, 3), span(0, 4, 7)]);
    }

    #[test]
    fn background_include_whitespace_does_not_apply_when_disabled() {
        let positions = color_positions(
            "foo bar",
            Side::Right,
            BackgroundColor::Dark,
            true,
            true,
            false,
            &FileFormat::PlainText,
            &[novel_pos(0, 0, 3), novel_pos(0, 4, 7)],
            None,
            None,
        );
        let spans = positions
            .into_iter()
            .map(|(span, _)| span)
            .collect::<Vec<_>>();

        assert_eq!(spans, vec![span(0, 0, 3), span(0, 4, 7)]);
    }

    fn novel_pos(line: u32, start_col: u32, end_col: u32) -> MatchedPos {
        MatchedPos {
            kind: MatchKind::Novel {
                highlight: TokenKind::Atom(AtomKind::Keyword),
            },
            pos: span(line, start_col, end_col),
        }
    }

    fn unchanged_normal_pos(line: u32, start_col: u32, end_col: u32) -> MatchedPos {
        MatchedPos {
            kind: MatchKind::UnchangedToken {
                highlight: TokenKind::Atom(AtomKind::Normal),
                self_pos: vec![span(line, start_col, end_col)],
                opposite_pos: vec![span(line, start_col, end_col)],
            },
            pos: span(line, start_col, end_col),
        }
    }

    fn unchanged_pos(line: u32, start_col: u32, end_col: u32) -> MatchedPos {
        MatchedPos {
            kind: MatchKind::UnchangedToken {
                highlight: TokenKind::Atom(AtomKind::Keyword),
                self_pos: vec![span(line, start_col, end_col)],
                opposite_pos: vec![span(line, start_col, end_col)],
            },
            pos: span(line, start_col, end_col),
        }
    }

    fn span(line: u32, start_col: u32, end_col: u32) -> SingleLineSpan {
        SingleLineSpan {
            line: line.into(),
            start_col,
            end_col,
        }
    }
}
