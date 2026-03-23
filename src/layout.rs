// This file is part of Lottie.
//
// Copyright (c) 2026  René Coignard <contact@renecoignard.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use regex::Regex;
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::LazyLock;
use unicode_width::UnicodeWidthChar;

use crate::config::Config;
use crate::formatting::{LineFormatting, has_markup_bytes, parse_formatting};
use crate::types::{LINES_PER_PAGE, LineType, PAGE_WIDTH, get_marker_color};

static SCENE_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?)\s*#([^#]+)#\s*$").unwrap());

/// A single rendered row on screen, derived from one logical line of text.
///
/// Because Fountain lines can be longer than `PAGE_WIDTH`, one logical line
/// may produce several consecutive `VisualRow`s that all share the same
/// `line_idx`.  The [`char_start`](VisualRow::char_start) /
/// [`char_end`](VisualRow::char_end) range identifies the slice of the original
/// logical line that this row covers.
#[derive(Clone)]
pub struct VisualRow {
    /// Index into the `lines` / `types` arrays of the logical line that produced
    /// this row.
    pub line_idx: usize,

    /// Character offset (inclusive) in the logical line where this visual row begins.
    pub char_start: usize,

    /// Character offset (exclusive) in the logical line where this visual row ends.
    pub char_end: usize,

    /// The text content of this visual row, after sigil stripping and (for
    /// word-wrapped rows) splitting, but *before* case transformation.
    pub raw_text: String,

    /// The semantic type of the originating logical line.
    pub line_type: LineType,

    /// Left indent in columns, accounting for centring/right-alignment where
    /// applicable.
    pub indent: u16,

    /// `true` when the cursor's logical line matches this row's [`line_idx`](VisualRow::line_idx).
    ///
    /// Active rows show raw markup and suppress auto-CONT'D insertion.
    pub is_active: bool,

    /// Scene number to display in the left margin, if scene numbering is enabled
    /// and this is the first visual row of a scene heading.
    pub scene_num: Option<String>,

    /// Page number to display in the right margin, set on the *first* printable
    /// non-empty row of each new page.
    pub page_num: Option<usize>,

    /// An optional foreground colour override derived from an adjacent `[[marker]]`
    /// note.  Supersedes the base style colour when present.
    pub override_color: Option<ratatui::style::Color>,

    /// Inline formatting metadata (bold/italic/underline ranges) for the full
    /// logical line, shared across all visual rows that originate from it.
    pub fmt: Rc<LineFormatting>,

    /// `true` for synthetic empty rows injected by smart heading spacing.
    ///
    /// Phantom rows contribute to the printable line count for page-break
    /// calculations but are invisible in the rendered output.
    pub is_phantom: bool,
}

impl VisualRow {
    /// Converts a *logical* cursor position (a character index in the full
    /// logical line) into a *visual* column offset within this row.
    ///
    /// Hidden markup characters (asterisks, underscores) are excluded from the
    /// visual width when the row is inactive and markup hiding is in effect.
    /// Returns [`indent`](VisualRow::indent) if `logical_x` is before the start of
    /// this row.
    pub fn logical_to_visual_x(&self, logical_x: usize) -> u16 {
        if logical_x <= self.char_start {
            return self.indent;
        }
        let mut vis = self.indent;
        for (i, c) in self.raw_text.chars().enumerate() {
            let global_i = self.char_start + i;
            if global_i >= logical_x {
                break;
            }
            if self.is_active || !self.fmt.hidden_chars.contains(&global_i) {
                vis += c.width().unwrap_or(0) as u16;
            }
        }
        vis
    }

    /// Converts a *visual* column offset within this row back to a *logical*
    /// character index in the full logical line.
    ///
    /// `is_last_in_logical` must be `true` when this is the last visual row for
    /// its logical line, so that the cursor may land *on* the final character
    /// rather than being clamped one position short.
    pub fn visual_to_logical_x(&self, vis_x: u16, is_last_in_logical: bool) -> usize {
        if vis_x <= self.indent {
            return self.char_start;
        }
        let mut current_vis = self.indent;
        let max_logical = if is_last_in_logical {
            self.char_end
        } else {
            self.char_end.saturating_sub(1)
        };

        for (i, c) in self.raw_text.chars().enumerate() {
            let log_x = self.char_start + i;
            if log_x >= max_logical {
                break;
            }
            let w = if self.is_active || !self.fmt.hidden_chars.contains(&log_x) {
                c.width().unwrap_or(0) as u16
            } else {
                0
            };
            if current_vis + w > vis_x {
                return log_x;
            }
            current_vis += w;
        }
        max_logical
    }
}

/// Strips the Fountain sigil characters from the start (and sometimes end) of
/// `raw`, returning the displayable portion of the line.
///
/// Sigils are syntax markers that force a particular line type but are not
/// themselves part of the content (e.g. the leading `~` on a lyric line, or
/// the leading `.` on a forced scene heading).  The returned slice is a
/// sub-slice of `raw`; no allocation is performed.
///
/// # Examples
///
/// ```
/// use lottie_rs::layout::strip_sigils;
/// use lottie_rs::types::LineType;
///
/// assert_eq!(strip_sigils("~Song line", LineType::Lyrics), "Song line");
/// assert_eq!(strip_sigils(">CENTER<",   LineType::Centered), "CENTER");
/// ```
pub fn strip_sigils(raw: &str, lt: LineType) -> &str {
    let trimmed = raw.trim_start();
    match lt {
        LineType::Lyrics if trimmed.starts_with('~') => trimmed[1..].trim_start(),
        LineType::Action | LineType::Shot if trimmed.starts_with("!!") => &trimmed[2..],
        LineType::Action | LineType::Shot if trimmed.starts_with('!') => &trimmed[1..],
        LineType::SceneHeading if trimmed.starts_with('.') && !trimmed.starts_with("..") => {
            &trimmed[1..]
        }
        LineType::Transition if trimmed.starts_with('>') => trimmed[1..].trim_start(),
        LineType::Centered if trimmed.starts_with('>') && trimmed.ends_with('<') => {
            trimmed[1..trimmed.len() - 1].trim()
        }
        LineType::Character | LineType::DualDialogueCharacter if trimmed.starts_with('@') => {
            trimmed[1..].trim_end_matches('^').trim()
        }
        LineType::Character | LineType::DualDialogueCharacter => raw.trim_end_matches('^').trim(),
        LineType::MetadataTitle => {
            if let Some(idx) = raw.find(':') {
                raw[idx + 1..].trim_start()
            } else {
                raw
            }
        }
        _ => raw,
    }
}

/// Returns the number of characters consumed by the sigil prefix of `raw` for
/// the given `lt`.
///
/// This is used to calculate [`VisualRow::char_start`] so that cursor positions
/// remain anchored to the logical line even after sigil stripping.
pub fn sigil_left_chars(raw: &str, lt: LineType) -> usize {
    let stripped = strip_sigils(raw, lt);
    if stripped.as_ptr() >= raw.as_ptr() {
        let byte_offset = stripped.as_ptr() as usize - raw.as_ptr() as usize;
        raw[..byte_offset].chars().count()
    } else {
        0
    }
}

/// Returns `true` if lines of `lt` contribute to the printable line count used
/// for page-break and page-number calculations.
///
/// Non-printable types (metadata, boneyard, notes, page breaks) are excluded
/// from the count so they do not affect pagination.
pub fn is_printable(lt: LineType) -> bool {
    !matches!(
        lt,
        LineType::MetadataTitle
            | LineType::MetadataKey
            | LineType::MetadataValue
            | LineType::Boneyard
            | LineType::Note
            | LineType::PageBreak
    )
}

fn token_metrics(text: &str, is_active: bool, hide_markup: bool) -> (u16, u16, bool) {
    let mut width = 0;
    let mut trailing_spaces = 0;
    let mut is_pure = true;

    for c in text.chars() {
        if !is_active && hide_markup && (c == '*' || c == '_') {
            continue;
        }
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) as u16;
        width += w;

        if c.is_whitespace() {
            trailing_spaces += w;
        } else {
            trailing_spaces = 0;
            is_pure = false;
        }
    }

    let trimmed_width = if is_pure {
        width
    } else {
        width.saturating_sub(trailing_spaces)
    };
    (trimmed_width, width, is_pure)
}

fn calculate_indent(
    lt: LineType,
    text: &str,
    base_indent: u16,
    is_active: bool,
    hide_markup: bool,
) -> u16 {
    match lt {
        LineType::Centered | LineType::Lyrics => {
            let w = token_metrics(text, is_active, hide_markup).1;
            PAGE_WIDTH.saturating_sub(w) / 2
        }
        LineType::Transition => {
            let w = token_metrics(text, is_active, hide_markup).1;
            PAGE_WIDTH.saturating_sub(w)
        }
        _ => base_indent,
    }
}

struct TokenizeText<'a> {
    text: &'a str,
    pos: usize,
    prev_was_sep: bool,
    done: bool,
}

impl<'a> TokenizeText<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            pos: 0,
            prev_was_sep: true,
            done: false,
        }
    }
}

impl<'a> Iterator for TokenizeText<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if self.text.is_empty() {
            self.done = true;
            return Some("");
        }
        if self.pos >= self.text.len() {
            self.done = true;
            return None;
        }

        let start = self.pos;
        let mut current_pos = start;

        for c in self.text[start..].chars() {
            let is_sep = c.is_whitespace() || c == '-';
            current_pos += c.len_utf8();

            if is_sep && !self.prev_was_sep {
                self.prev_was_sep = is_sep;
                self.pos = current_pos;
                return Some(&self.text[start..current_pos]);
            }
            self.prev_was_sep = is_sep;
        }

        self.pos = current_pos;
        Some(&self.text[start..current_pos])
    }
}

/// Converts a sequence of logical Fountain lines into a flat list of
/// [`VisualRow`]s ready for terminal rendering or export.
///
/// The function handles:
/// - Word-wrapping and hard-wrapping to `PAGE_WIDTH`.
/// - Per-type indentation (including dynamic centring and right-alignment).
/// - Smart heading spacing (phantom rows).
/// - Page-break injection for `===` lines.
/// - Scene and page numbering.
/// - CONT'D auto-insertion for consecutive character cues.
/// - Inline note annotation stripping for non-active scene/section lines.
///
/// `active_line` is the index of the logical line that currently holds the
/// cursor; pass `usize::MAX` when rendering for export (no active line).
pub fn build_layout(
    lines: &[String],
    types: &[LineType],
    active_line: usize,
    config: &Config,
) -> Vec<VisualRow> {
    let mut rows: Vec<VisualRow> = Vec::with_capacity(lines.len() + 32);
    let mut last_speaking_character = String::new();
    let mut in_dual_dialogue = false;
    let mut scene_counter = 0;
    let mut printable_row_count = 0;
    let mut page_number = 1;
    let mut page_num_pending = true;
    let mut current_line = String::with_capacity(PAGE_WIDTH as usize * 4);
    let mut active_note_color: Option<ratatui::style::Color> = None;
    let empty_fmt = Rc::new(LineFormatting::default());

    for (i, (line, &lt)) in lines.iter().zip(types.iter()).enumerate() {
        let is_active = i == active_line;
        let mut scene_num: Option<String> = None;
        let mut raw_line = Cow::Borrowed(line.as_str());
        let mut line_override_color = None;
        let format_data = if !has_markup_bytes(&raw_line) {
            empty_fmt.clone()
        } else {
            Rc::new(parse_formatting(&raw_line))
        };

        if lt == LineType::Note {
            if let Some(start) = raw_line.find("[[") {
                let end_offset = raw_line[start..]
                    .find("]]")
                    .unwrap_or(raw_line.len() - start);
                let content = &raw_line[start + 2..start + end_offset];
                active_note_color = get_marker_color(content);
            }
            line_override_color = active_note_color;

            if let Some(end) = raw_line.rfind("]]")
                && !raw_line[end..].contains("[[")
            {
                active_note_color = None;
            }
        } else {
            active_note_color = None;

            if matches!(
                lt,
                LineType::SceneHeading | LineType::Section | LineType::Synopsis
            ) && let Some(start) = raw_line.rfind("[[")
            {
                let end_offset = raw_line[start..]
                    .find("]]")
                    .unwrap_or(raw_line.len() - start);
                let content = &raw_line[start + 2..start + end_offset];
                line_override_color = get_marker_color(content);
            }
        }

        if matches!(lt, LineType::SceneHeading | LineType::Transition) {
            last_speaking_character.clear();
        }

        match lt {
            LineType::DualDialogueCharacter => in_dual_dialogue = true,
            LineType::Character
            | LineType::SceneHeading
            | LineType::Transition
            | LineType::Action
            | LineType::Shot
            | LineType::Section
            | LineType::Synopsis
            | LineType::PageBreak
            | LineType::Centered
            | LineType::Lyrics => {
                in_dual_dialogue = false;
            }
            LineType::Empty => {
                if i > 0
                    && !matches!(
                        types[i - 1],
                        LineType::Character
                            | LineType::DualDialogueCharacter
                            | LineType::Dialogue
                            | LineType::Parenthetical
                    )
                {
                    in_dual_dialogue = false;
                }
            }
            _ => {}
        }

        if lt == LineType::SceneHeading {
            let mut explicit_scene_num = None;

            if raw_line.trim_end().ends_with('#')
                && let Some(caps) = SCENE_NUM_RE.captures(&raw_line)
            {
                let inner = caps[2].trim();
                if !inner.is_empty() {
                    explicit_scene_num = Some(inner.to_string());

                    if !is_active {
                        raw_line = Cow::Owned(caps[1].to_string());
                    }
                }
            }

            if let Some(ref s) = explicit_scene_num {
                let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = digits.parse::<usize>() {
                    scene_counter = n;
                }
            } else {
                scene_counter += 1;
            }

            if config.show_scene_numbers {
                scene_num = explicit_scene_num.or_else(|| Some(scene_counter.to_string()));
            }
        }

        if lt == LineType::SceneHeading && config.heading_spacing > 0 && i > 0 {
            let mut physical_empty_count = 0;
            let mut k = i;
            while k > 0 {
                k -= 1;
                if types[k] == LineType::Empty {
                    physical_empty_count += 1;
                } else {
                    break;
                }
            }

            if physical_empty_count < config.heading_spacing {
                let diff = config.heading_spacing - physical_empty_count;
                for _ in 0..diff {
                    rows.push(VisualRow {
                        line_idx: i.saturating_sub(1),
                        char_start: 0,
                        char_end: 0,
                        raw_text: String::new(),
                        line_type: LineType::Empty,
                        indent: 0,
                        is_active: false,
                        scene_num: None,
                        page_num: None,
                        override_color: None,
                        fmt: Rc::new(LineFormatting::default()),
                        is_phantom: true,
                    });
                    printable_row_count += 1;
                    if printable_row_count % LINES_PER_PAGE == 0 {
                        page_number += 1;
                        page_num_pending = true;
                    }
                }
            }
        }

        if lt == LineType::PageBreak {
            let display_text = if is_active {
                raw_line.to_string()
            } else {
                let fill_char = if config.force_ascii { "-" } else { "─" };
                fill_char.repeat(PAGE_WIDTH as usize)
            };

            rows.push(VisualRow {
                line_idx: i,
                char_start: 0,
                char_end: display_text.chars().count(),
                raw_text: display_text,
                line_type: lt,
                indent: 0,
                is_active,
                scene_num: None,
                page_num: None,
                override_color: None,
                fmt: Rc::clone(&format_data),
                is_phantom: false,
            });
            page_number += 1;
            printable_row_count = 0;
            page_num_pending = true;
            continue;
        }

        let mut fmt_rules = lt.fmt();
        if lt == LineType::Empty && i > 0 {
            match types[i - 1] {
                LineType::Character
                | LineType::DualDialogueCharacter
                | LineType::Parenthetical
                | LineType::Dialogue => {
                    fmt_rules.indent = LineType::Dialogue.fmt().indent;
                }
                _ => {}
            }
        }

        if in_dual_dialogue
            && matches!(
                lt,
                LineType::DualDialogueCharacter
                    | LineType::Dialogue
                    | LineType::Parenthetical
                    | LineType::Empty
            )
        {
            fmt_rules.indent += 10;
            if let Some(wrap_indent) = fmt_rules.wrap_indent.as_mut() {
                *wrap_indent += 10;
            }
        }

        let mut display = if is_active {
            Cow::Borrowed(raw_line.as_ref())
        } else {
            Cow::Borrowed(strip_sigils(&raw_line, lt))
        };

        if !is_active
            && matches!(
                lt,
                LineType::SceneHeading | LineType::Section | LineType::Synopsis
            )
            && display.contains("[[")
        {
            let mut cleaned = String::with_capacity(display.len());
            let mut in_note = false;
            let chars: Vec<char> = display.chars().collect();
            let mut j = 0;
            while j < chars.len() {
                if j + 1 < chars.len() && chars[j] == '[' && chars[j + 1] == '[' {
                    in_note = true;
                    j += 2;
                    continue;
                }
                if j + 1 < chars.len() && chars[j] == ']' && chars[j + 1] == ']' && in_note {
                    in_note = false;
                    j += 2;
                    continue;
                }
                if !in_note {
                    cleaned.push(chars[j]);
                }
                j += 1;
            }
            display = Cow::Owned(cleaned.trim_end().to_string());
        }

        let mut final_display = display.clone();

        if config.auto_contd && (lt == LineType::Character || lt == LineType::DualDialogueCharacter)
        {
            let clean_name = strip_sigils(&raw_line, lt).trim().to_uppercase();
            let compare_name = if let Some(idx) = clean_name.find('(') {
                clean_name[..idx].trim().to_string()
            } else {
                clean_name.clone()
            };

            if compare_name == last_speaking_character
                && !compare_name.is_empty()
                && !is_active
                && !clean_name.contains(&config.contd_extension)
            {
                final_display = Cow::Owned(format!("{} {}", display, config.contd_extension));
            }
            last_speaking_character = compare_name;
        } else if lt == LineType::Character || lt == LineType::DualDialogueCharacter {
            let clean_name = strip_sigils(&raw_line, lt).trim().to_uppercase();
            let compare_name = if let Some(idx) = clean_name.find('(') {
                clean_name[..idx].trim().to_string()
            } else {
                clean_name.clone()
            };
            last_speaking_character = compare_name;
        }

        let sigil_left = if is_active {
            0
        } else {
            sigil_left_chars(&raw_line, lt)
        };
        let total_original_chars = raw_line.chars().count();
        let mut row_disp_start: usize = 0;
        let mut cur_w = 0;
        let tokens = TokenizeText::new(&final_display);
        let mut logical_rows = Vec::new();

        current_line.clear();
        let mut current_line_char_count = 0;

        for token in tokens {
            let mut remaining_token = token;

            while !remaining_token.is_empty() {
                let (token_w_trimmed, token_w_total, token_is_pure_space) =
                    token_metrics(remaining_token, is_active, config.hide_markup);

                if !current_line.is_empty()
                    && !token_is_pure_space
                    && cur_w + token_w_trimmed > fmt_rules.width
                {
                    let raw_start = (sigil_left + row_disp_start).min(total_original_chars);
                    let raw_end = (raw_start + current_line_char_count).min(total_original_chars);
                    let current_indent = calculate_indent(
                        lt,
                        &current_line,
                        fmt_rules.indent,
                        is_active,
                        config.hide_markup,
                    );

                    logical_rows.push(VisualRow {
                        line_idx: i,
                        char_start: raw_start,
                        char_end: raw_end,
                        raw_text: current_line.clone(),
                        line_type: lt,
                        indent: current_indent,
                        is_active,
                        scene_num: scene_num.take(),
                        page_num: None,
                        override_color: line_override_color,
                        fmt: Rc::clone(&format_data),
                        is_phantom: false,
                    });

                    if let Some(wrap_indent) = fmt_rules.wrap_indent {
                        fmt_rules.indent = wrap_indent;
                    }

                    row_disp_start += current_line_char_count;
                    current_line.clear();
                    current_line_char_count = 0;
                    cur_w = 0;
                    continue;
                }

                if cur_w + token_w_trimmed > fmt_rules.width {
                    let space_left = fmt_rules.width.saturating_sub(cur_w);
                    let mut split_byte_idx = 0;
                    let mut acc_w = 0;

                    for (k, (byte_idx, c)) in remaining_token.char_indices().enumerate() {
                        let cw = if !is_active && config.hide_markup && (c == '*' || c == '_') {
                            0
                        } else {
                            unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) as u16
                        };
                        acc_w += cw;

                        if acc_w > space_left {
                            if k == 0 && current_line.is_empty() {
                                split_byte_idx = byte_idx + c.len_utf8();
                            }
                            break;
                        }
                        split_byte_idx = byte_idx + c.len_utf8();
                    }

                    let part1 = &remaining_token[..split_byte_idx];
                    let part2 = &remaining_token[split_byte_idx..];

                    current_line.push_str(part1);
                    current_line_char_count += part1.chars().count();

                    let raw_start = (sigil_left + row_disp_start).min(total_original_chars);
                    let raw_end = (raw_start + current_line_char_count).min(total_original_chars);
                    let current_indent = calculate_indent(
                        lt,
                        &current_line,
                        fmt_rules.indent,
                        is_active,
                        config.hide_markup,
                    );

                    logical_rows.push(VisualRow {
                        line_idx: i,
                        char_start: raw_start,
                        char_end: raw_end,
                        raw_text: current_line.clone(),
                        line_type: lt,
                        indent: current_indent,
                        is_active,
                        scene_num: scene_num.take(),
                        page_num: None,
                        override_color: line_override_color,
                        fmt: Rc::clone(&format_data),
                        is_phantom: false,
                    });

                    if let Some(wrap_indent) = fmt_rules.wrap_indent {
                        fmt_rules.indent = wrap_indent;
                    }

                    row_disp_start += current_line_char_count;
                    current_line.clear();
                    current_line_char_count = 0;
                    cur_w = 0;
                    remaining_token = part2;
                } else {
                    current_line.push_str(remaining_token);
                    current_line_char_count += remaining_token.chars().count();
                    cur_w += token_w_total;
                    break;
                }
            }
        }

        let raw_start = (sigil_left + row_disp_start).min(total_original_chars);
        let raw_end = (raw_start + current_line_char_count).min(total_original_chars);
        let current_indent = calculate_indent(
            lt,
            &current_line,
            fmt_rules.indent,
            is_active,
            config.hide_markup,
        );

        logical_rows.push(VisualRow {
            line_idx: i,
            char_start: raw_start,
            char_end: raw_end,
            raw_text: current_line.clone(),
            line_type: lt,
            indent: current_indent,
            is_active,
            scene_num,
            page_num: None,
            override_color: line_override_color,
            fmt: Rc::clone(&format_data),
            is_phantom: false,
        });

        if !config.break_actions && lt == LineType::Action {
            let current_page_remaining = LINES_PER_PAGE - (printable_row_count % LINES_PER_PAGE);
            if logical_rows.len() > current_page_remaining && logical_rows.len() <= LINES_PER_PAGE {
                printable_row_count += current_page_remaining;
                page_number += 1;
                page_num_pending = true;
            }
        }

        for mut r in logical_rows {
            if is_printable(lt) {
                if page_num_pending && config.show_page_numbers && lt != LineType::Empty {
                    r.page_num = Some(page_number);
                    page_num_pending = false;
                }
                printable_row_count += 1;
                if printable_row_count > 0 && printable_row_count % LINES_PER_PAGE == 0 {
                    page_number += 1;
                    page_num_pending = true;
                }
            }
            rows.push(r);
        }
    }

    rows
}

/// Locates the visual row and column that correspond to a logical cursor
/// position `(cursor_y, cursor_x)`.
///
/// Returns `(visual_row_index, visual_column)`.  When the exact position falls
/// between two visual rows (e.g. at a wrap boundary), the function prefers the
/// row whose range includes `cursor_x`.  Falls back to the last visual row for
/// the logical line if no better match is found.
pub fn find_visual_cursor(layout: &[VisualRow], cursor_y: usize, cursor_x: usize) -> (usize, u16) {
    let mut last_for_line = None;

    for (vi, row) in layout.iter().enumerate() {
        if row.is_phantom {
            continue;
        }
        if row.line_idx != cursor_y {
            continue;
        }
        last_for_line = Some(vi);

        let mut is_last = true;
        for next_row in layout.iter().skip(vi + 1) {
            if !next_row.is_phantom && next_row.line_idx == cursor_y {
                is_last = false;
                break;
            } else if !next_row.is_phantom {
                break;
            }
        }

        if cursor_x >= row.char_start {
            let in_range = if is_last {
                cursor_x <= row.char_end
            } else {
                cursor_x < row.char_end
            };
            if in_range {
                return (vi, row.logical_to_visual_x(cursor_x));
            }
        }
    }

    let fallback_vi = last_for_line.unwrap_or(0);
    let fallback_x = layout
        .get(fallback_vi)
        .map(|r| r.logical_to_visual_x(cursor_x))
        .unwrap_or(0);
    (fallback_vi, fallback_x)
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn test_strip_sigils_scene_heading() {
        assert_eq!(strip_sigils(".HEADING", LineType::SceneHeading), "HEADING");
    }

    #[test]
    fn test_strip_sigils_action() {
        assert_eq!(strip_sigils("!ACTION", LineType::Action), "ACTION");
    }

    #[test]
    fn test_strip_sigils_shot() {
        assert_eq!(strip_sigils("!!SHOT", LineType::Shot), "SHOT");
    }

    #[test]
    fn test_strip_sigils_lyrics() {
        assert_eq!(strip_sigils("~SONG", LineType::Lyrics), "SONG");
    }

    #[test]
    fn test_strip_sigils_transition() {
        assert_eq!(strip_sigils(">FADE", LineType::Transition), "FADE");
    }

    #[test]
    fn test_strip_sigils_centered() {
        assert_eq!(strip_sigils(">CENTER<", LineType::Centered), "CENTER");
    }

    #[test]
    fn test_strip_sigils_character() {
        assert_eq!(strip_sigils("@NAME", LineType::Character), "NAME");
    }

    #[test]
    fn test_strip_sigils_dual_character() {
        assert_eq!(
            strip_sigils("@NAME^", LineType::DualDialogueCharacter),
            "NAME"
        );
        assert_eq!(
            strip_sigils("NAME^", LineType::DualDialogueCharacter),
            "NAME"
        );
    }

    #[test]
    fn test_strip_sigils_metadata() {
        assert_eq!(
            strip_sigils("Title: Value", LineType::MetadataTitle),
            "Value"
        );
        assert_eq!(
            strip_sigils("   Value", LineType::MetadataValue),
            "   Value"
        );
    }

    #[test]
    fn test_sigil_left_chars_calculation() {
        assert_eq!(sigil_left_chars(".HEADING", LineType::SceneHeading), 1);
        assert_eq!(sigil_left_chars("!!SHOT", LineType::Shot), 2);
        assert_eq!(sigil_left_chars(">CENTER<", LineType::Centered), 1);
        assert_eq!(sigil_left_chars("Title: Value", LineType::MetadataTitle), 7);
        assert_eq!(sigil_left_chars("   Value", LineType::MetadataValue), 0);
    }

    #[test]
    fn test_is_printable() {
        assert!(is_printable(LineType::Action));
        assert!(is_printable(LineType::SceneHeading));
        assert!(is_printable(LineType::Character));
        assert!(is_printable(LineType::Dialogue));
        assert!(!is_printable(LineType::Note));
        assert!(!is_printable(LineType::Boneyard));
        assert!(!is_printable(LineType::MetadataTitle));
        assert!(!is_printable(LineType::PageBreak));
    }

    #[test]
    fn test_build_layout_scene_numbering() {
        let config = Config {
            show_scene_numbers: true,
            ..Config::default()
        };

        let lines = vec![
            "INT. SCENE ONE".to_string(),
            "".to_string(),
            "EXT. SCENE TWO".to_string(),
        ];
        let types = vec![
            LineType::SceneHeading,
            LineType::Empty,
            LineType::SceneHeading,
        ];

        let layout = build_layout(&lines, &types, 99, &config);

        assert_eq!(layout[0].scene_num, Some("1".to_string()));
        assert_eq!(layout[2].scene_num, Some("2".to_string()));
    }

    #[test]
    fn test_build_layout_auto_contd() {
        let config = Config {
            auto_contd: true,
            contd_extension: "(ПРОД.)".to_string(),
            ..Config::default()
        };
        let lines = vec![
            "МАТРОС".to_string(),
            "Капитан, у нас айсберг по курсу корабля!".to_string(),
            "".to_string(),
            "КАПИТАН".to_string(),
            "(удивлённо)".to_string(),
            "Айсберг по курсу корабля?".to_string(),
            "".to_string(),
            "КАПИТАН".to_string(),
            "Дороговато!".to_string(),
        ];
        let types = vec![
            LineType::Character,
            LineType::Dialogue,
            LineType::Empty,
            LineType::Character,
            LineType::Parenthetical,
            LineType::Dialogue,
            LineType::Empty,
            LineType::Character,
            LineType::Dialogue,
        ];
        let layout = build_layout(&lines, &types, 99, &config);

        assert_eq!(layout[0].raw_text, "МАТРОС");
        assert_eq!(layout[4].raw_text, "КАПИТАН");
        assert_eq!(layout[8].raw_text, "КАПИТАН (ПРОД.)");
    }

    #[test]
    fn test_build_layout_no_auto_contd_when_active() {
        let config = Config {
            auto_contd: true,
            ..Config::default()
        };
        let lines = vec![
            "CHARLOTTE".to_string(),
            "Text".to_string(),
            "".to_string(),
            "CHARLOTTE".to_string(),
        ];
        let types = vec![
            LineType::Character,
            LineType::Dialogue,
            LineType::Empty,
            LineType::Character,
        ];
        let layout = build_layout(&lines, &types, 3, &config);
        assert_eq!(layout[0].raw_text, "CHARLOTTE");
        assert_eq!(layout[3].raw_text, "CHARLOTTE");
    }

    #[test]
    fn test_build_layout_phantom_lines_for_spacing() {
        let config = Config {
            heading_spacing: 3,
            ..Config::default()
        };
        let lines = vec![
            "INT. ONE".to_string(),
            "Action".to_string(),
            "INT. TWO".to_string(),
        ];
        let types = vec![
            LineType::SceneHeading,
            LineType::Action,
            LineType::SceneHeading,
        ];
        let layout = build_layout(&lines, &types, 99, &config);
        let phantoms = layout.iter().filter(|r| r.is_phantom).count();
        assert_eq!(phantoms, 3);
    }

    #[test]
    fn test_build_layout_page_break_injection() {
        let config = Config::default();
        let lines = vec!["===".to_string()];
        let types = vec![LineType::PageBreak];
        let layout = build_layout(&lines, &types, 99, &config);
        assert_eq!(layout[0].raw_text, "─".repeat(PAGE_WIDTH as usize));
    }

    #[test]
    fn test_layout_page_break_force_ascii() {
        let mut config = Config::default();
        config.force_ascii = true;
        let lines = vec!["===".to_string()];
        let types = vec![LineType::PageBreak];
        let layout = build_layout(&lines, &types, 99, &config);
        assert_eq!(layout[0].raw_text, "-".repeat(PAGE_WIDTH as usize));
    }

    #[test]
    fn test_visual_row_logical_to_visual_x() {
        let row = VisualRow {
            line_idx: 0,
            char_start: 0,
            char_end: 10,
            raw_text: "Test **bold**".to_string(),
            line_type: LineType::Action,
            indent: 5,
            is_active: false,
            scene_num: None,
            page_num: None,
            override_color: None,
            fmt: Rc::new(parse_formatting("Test **bold**")),
            is_phantom: false,
        };
        assert_eq!(row.logical_to_visual_x(0), 5);
        assert_eq!(row.logical_to_visual_x(5), 10);
        assert_eq!(row.logical_to_visual_x(7), 10);
    }

    #[test]
    fn test_visual_row_visual_to_logical_x() {
        let row = VisualRow {
            line_idx: 0,
            char_start: 0,
            char_end: 13,
            raw_text: "Test **bold**".to_string(),
            line_type: LineType::Action,
            indent: 5,
            is_active: false,
            scene_num: None,
            page_num: None,
            override_color: None,
            fmt: Rc::new(parse_formatting("Test **bold**")),
            is_phantom: false,
        };
        assert_eq!(row.visual_to_logical_x(5, true), 0);
        assert_eq!(row.visual_to_logical_x(10, true), 7);
        assert_eq!(row.visual_to_logical_x(100, true), 13);
    }

    #[test]
    fn test_layout_word_wrapping() {
        let config = Config::default();

        let long_action = "This is a very, very, very, very, very long action line that should definitely exceed the standard character limit.".to_string();

        let layout = build_layout(&[long_action], &[LineType::Action], 99, &config);

        assert!(layout.len() >= 2, "Line was not wrapped correctly");

        assert_eq!(layout[0].line_idx, 0);
        assert_eq!(layout[1].line_idx, 0);

        assert_eq!(layout[0].char_start, 0);
        assert!(layout[0].char_end > 0);
        assert_eq!(layout[1].char_start, layout[0].char_end);

        let first_line_width =
            unicode_width::UnicodeWidthStr::width(layout[0].raw_text.trim_end_matches(' '));
        assert!(first_line_width <= crate::types::PAGE_WIDTH as usize);
    }

    #[test]
    fn test_layout_hardcoded_scene_numbers_stripped() {
        let config = Config::default();

        let lines = vec!["INT. KITCHEN - DAY #12A#".to_string()];
        let types = vec![LineType::SceneHeading];

        let layout = build_layout(&lines, &types, 99, &config);

        assert_eq!(layout[0].raw_text, "INT. KITCHEN - DAY");
    }

    #[test]
    fn test_layout_no_break_actions() {
        let mut config = Config::default();
        config.break_actions = false;

        let mut lines = vec!["".to_string(); 54];
        let mut types = vec![LineType::Empty; 54];

        lines.push("A very long action that takes multiple visual lines on the screen because it exceeds the limit.".to_string());
        types.push(LineType::Action);

        let layout = build_layout(&lines, &types, 99, &config);

        let action_rows: Vec<&VisualRow> = layout
            .iter()
            .filter(|r| r.line_type == LineType::Action)
            .collect();

        assert_eq!(action_rows[0].page_num, Some(2));
    }

    #[test]
    fn test_layout_hard_wrap_long_word() {
        let config = Config::default();

        let long_action = "A".repeat(100);

        let layout = build_layout(&[long_action], &[LineType::Action], 99, &config);

        let rows: Vec<_> = layout.into_iter().filter(|r| !r.is_phantom).collect();

        assert_eq!(rows.len(), 2, "Line was not hard-wrapped correctly");

        assert_eq!(rows[0].char_start, 0);
        assert_eq!(rows[0].char_end, 60);
        assert_eq!(rows[1].char_start, 60);
        assert_eq!(rows[1].char_end, 100);

        assert_eq!(rows[0].raw_text, "A".repeat(60));
        assert_eq!(rows[1].raw_text, "A".repeat(40));
    }

    #[test]
    fn test_layout_hard_wrap_with_markup() {
        let config = Config::default();

        let long_action = format!("**{}**", "A".repeat(100));

        let layout = build_layout(&[long_action], &[LineType::Action], 99, &config);

        let rows: Vec<_> = layout.into_iter().filter(|r| !r.is_phantom).collect();
        assert_eq!(rows.len(), 2);

        assert_eq!(rows[0].raw_text, format!("**{}", "A".repeat(60)));
        assert_eq!(rows[1].raw_text, format!("{}**", "A".repeat(40)));
    }

    #[test]
    fn test_layout_page_break_active_vs_inactive() {
        let config = Config::default();
        let lines = vec!["===".to_string()];
        let types = vec![LineType::PageBreak];

        let layout_inactive = build_layout(&lines, &types, 99, &config);
        assert_eq!(layout_inactive[0].raw_text, "─".repeat(PAGE_WIDTH as usize));

        let layout_active = build_layout(&lines, &types, 0, &config);
        assert_eq!(layout_active[0].raw_text, "===");
    }

    #[test]
    fn test_layout_show_scene_numbers_disabled() {
        let mut config = Config::default();
        config.show_scene_numbers = false;

        let lines = vec!["INT. SCENE ONE".to_string()];
        let types = vec![LineType::SceneHeading];

        let layout = build_layout(&lines, &types, 99, &config);
        assert_eq!(
            layout[0].scene_num, None,
            "Scene number should be None when disabled"
        );
    }

    #[test]
    fn test_layout_show_page_numbers_disabled() {
        let mut config = Config::default();
        config.show_page_numbers = false;

        let lines = vec!["Action line".to_string()];
        let types = vec![LineType::Action];

        let layout = build_layout(&lines, &types, 99, &config);

        assert_eq!(
            layout[0].page_num, None,
            "Page number should be None when disabled"
        );
    }

    #[test]
    fn test_layout_auto_contd_disabled() {
        let mut config = Config::default();
        config.auto_contd = false;

        let lines = vec![
            "CHARLOTTE".to_string(),
            "Text".to_string(),
            "".to_string(),
            "CHARLOTTE".to_string(),
        ];
        let types = vec![
            LineType::Character,
            LineType::Dialogue,
            LineType::Empty,
            LineType::Character,
        ];

        let layout = build_layout(&lines, &types, 99, &config);
        assert_eq!(layout[0].raw_text, "CHARLOTTE");
        assert_eq!(
            layout[3].raw_text, "CHARLOTTE",
            "Should NOT append (CONT'D) when disabled"
        );
    }

    #[test]
    fn test_layout_break_actions_enabled() {
        let mut config = Config::default();
        config.break_actions = true;

        let mut lines = vec!["".to_string(); 54];
        let mut types = vec![LineType::Empty; 54];

        lines.push("A very long action that takes multiple visual lines on the screen because it exceeds the limit.".to_string());
        types.push(LineType::Action);

        let layout = build_layout(&lines, &types, 99, &config);

        let action_rows: Vec<&VisualRow> = layout
            .iter()
            .filter(|r| r.line_type == LineType::Action)
            .collect();

        assert_eq!(
            action_rows[0].page_num,
            Some(1),
            "First line of action should remain on page 1 when breaking is allowed"
        );
    }

    #[test]
    fn test_layout_smart_heading_spacing() {
        let config = Config {
            heading_spacing: 2,
            ..Config::default()
        };

        let lines = vec![
            "Action 1".to_string(),
            "INT. SCENE 1".to_string(),
            "Action 2".to_string(),
            "".to_string(),
            "INT. SCENE 2".to_string(),
            "Action 3".to_string(),
            "".to_string(),
            "".to_string(),
            "INT. SCENE 3".to_string(),
            "Action 4".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "INT. SCENE 4".to_string(),
        ];

        let types = vec![
            LineType::Action,
            LineType::SceneHeading,
            LineType::Action,
            LineType::Empty,
            LineType::SceneHeading,
            LineType::Action,
            LineType::Empty,
            LineType::Empty,
            LineType::SceneHeading,
            LineType::Action,
            LineType::Empty,
            LineType::Empty,
            LineType::Empty,
            LineType::SceneHeading,
        ];

        let layout = build_layout(&lines, &types, 99, &config);

        let phantoms: Vec<_> = layout.iter().filter(|r| r.is_phantom).collect();

        assert_eq!(
            phantoms.len(),
            3,
            "Smart spacing failed to calculate correct phantom lines"
        );
    }

    #[test]
    fn test_layout_empty_line_preserves_spaces() {
        let config = Config::default();
        let lines = vec!["   ".to_string()];
        let types = vec![LineType::Empty];

        let layout_active = build_layout(&lines, &types, 0, &config);
        assert_eq!(layout_active[0].raw_text, "   ");
        assert_eq!(layout_active[0].char_end, 3);

        let layout_inactive = build_layout(&lines, &types, 99, &config);
        assert_eq!(layout_inactive[0].raw_text, "   ");
        assert_eq!(layout_inactive[0].char_end, 3);
    }

    #[test]
    fn test_layout_empty_line_exceeding_width_wraps() {
        let config = Config::default();

        let lines = vec![" ".repeat(130)];
        let types = vec![LineType::Empty];

        let layout = build_layout(&lines, &types, 0, &config);

        assert_eq!(layout.len(), 3, "Empty line should be wrapped into 3 rows");

        assert_eq!(layout[0].char_start, 0);
        assert_eq!(layout[0].char_end, 60);
        assert_eq!(layout[0].raw_text, " ".repeat(60));

        assert_eq!(layout[1].char_start, 60);
        assert_eq!(layout[1].char_end, 120);
        assert_eq!(layout[1].raw_text, " ".repeat(60));

        assert_eq!(layout[2].char_start, 120);
        assert_eq!(layout[2].char_end, 130);
        assert_eq!(layout[2].raw_text, " ".repeat(10));
    }

    #[test]
    fn test_layout_empty_line_inherits_indent() {
        let config = Config::default();
        let lines = vec![
            "CHARLOTTE".to_string(),
            "Dialogue line".to_string(),
            "".to_string(),
        ];
        let types = vec![LineType::Character, LineType::Dialogue, LineType::Empty];

        let layout = build_layout(&lines, &types, 99, &config);

        let empty_row = &layout[2];
        assert_eq!(empty_row.line_type, LineType::Empty);
        assert_eq!(empty_row.indent, LineType::Dialogue.fmt().indent);
    }

    #[test]
    fn test_layout_page_number_skips_empty_lines() {
        let config = Config::default();

        let mut lines = vec!["Text".to_string(); LINES_PER_PAGE];
        let mut types = vec![LineType::Action; LINES_PER_PAGE];

        lines.push("   ".to_string());
        types.push(LineType::Empty);

        lines.push("Real Text".to_string());
        types.push(LineType::Action);

        let layout = build_layout(&lines, &types, 999, &config);

        let empty_row = layout
            .iter()
            .find(|r| r.line_type == LineType::Empty)
            .unwrap();

        assert_eq!(empty_row.page_num, None);

        let text_row = layout
            .iter()
            .skip_while(|r| r.line_type != LineType::Empty)
            .nth(1)
            .unwrap();
        assert_eq!(text_row.page_num, Some(2));
    }

    #[test]
    fn test_layout_soft_wrap_preserves_spaces_exactly() {
        let config = Config::default();

        let line = format!("Word{}Next", " ".repeat(58));
        let lines = vec![line];
        let types = vec![LineType::Action];

        let layout = build_layout(&lines, &types, 0, &config);

        assert_eq!(
            layout.len(),
            3,
            "Line should wrap across 3 rows due to hard wrapping"
        );

        assert_eq!(layout[0].char_start, 0);
        assert_eq!(layout[0].char_end, 5);
        assert_eq!(layout[0].raw_text, "Word ");

        assert_eq!(layout[1].char_start, 5);
        assert_eq!(layout[1].char_end, 65);
        assert_eq!(layout[1].raw_text, format!("{}Nex", " ".repeat(57)));

        assert_eq!(layout[2].char_start, 65);
        assert_eq!(layout[2].char_end, 66);
        assert_eq!(layout[2].raw_text, "t".to_string());
    }

    #[test]
    fn test_layout_parenthetical_wrap_indent() {
        let config = Config::default();
        let lines = vec![
            "(this is a very long parenthetical that should wrap with a different indent)"
                .to_string(),
        ];
        let types = vec![LineType::Parenthetical];

        let layout = build_layout(&lines, &types, 99, &config);

        assert!(layout.len() >= 2, "Parenthetical should wrap");

        assert_eq!(layout[0].indent, 16, "First line indent should be 16");
        assert_eq!(layout[1].indent, 17, "Wrapped line indent should be 17");
    }

    #[test]
    fn test_layout_tokenize_preserves_multiple_spaces() {
        let config = Config::default();
        let lines = vec!["A    B".to_string()];
        let types = vec![LineType::Action];

        let layout = build_layout(&lines, &types, 0, &config);

        assert_eq!(layout.len(), 1);
        assert_eq!(
            layout[0].raw_text, "A    B",
            "Multiple spaces should not be collapsed"
        );
    }

    #[test]
    fn test_layout_active_line_with_markup_wraps_correctly() {
        let config = Config::default();
        let text = format!("**{}**", "a".repeat(60));
        let lines = vec![text];
        let types = vec![LineType::Action];

        let layout_active = build_layout(&lines, &types, 0, &config);
        assert_eq!(
            layout_active.len(),
            2,
            "Active line should wrap because visible markup exceeds width"
        );

        let layout_inactive = build_layout(&lines, &types, 99, &config);
        assert_eq!(
            layout_inactive.len(),
            1,
            "Inactive line should not wrap when markup is hidden"
        );
    }

    #[test]
    fn test_layout_phantom_lines_page_break_rollover() {
        let config = Config {
            heading_spacing: 10,
            ..Config::default()
        };
        let mut lines = vec!["Action".to_string(); crate::types::LINES_PER_PAGE - 2];
        let mut types = vec![LineType::Action; crate::types::LINES_PER_PAGE - 2];

        lines.push("INT. ROOM".to_string());
        types.push(LineType::SceneHeading);

        let layout = build_layout(&lines, &types, 999, &config);
        let phantoms: Vec<_> = layout.iter().filter(|r| r.is_phantom).collect();

        assert!(phantoms.len() > 0);
    }

    #[test]
    fn test_strip_sigils_inline_note_in_heading() {
        let config = Config::default();
        let lines = vec![".HEADING [[yellow note]]".to_string()];
        let types = vec![LineType::SceneHeading];
        let layout = build_layout(&lines, &types, 999, &config);

        assert_eq!(layout[0].raw_text, "HEADING");
        assert_eq!(
            layout[0].override_color,
            Some(ratatui::style::Color::Yellow)
        );
    }

    #[test]
    fn test_visual_to_logical_x_max_logical_break() {
        let row = VisualRow {
            line_idx: 0,
            char_start: 0,
            char_end: 2,
            raw_text: "AB".to_string(),
            line_type: LineType::Action,
            indent: 0,
            is_active: true,
            scene_num: None,
            page_num: None,
            override_color: None,
            fmt: Rc::new(LineFormatting::default()),
            is_phantom: false,
        };

        assert_eq!(row.visual_to_logical_x(100, false), 1);
    }

    #[test]
    fn test_layout_explicit_scene_numbers_logic() {
        let config = Config {
            show_scene_numbers: true,
            ..Config::default()
        };

        let lines = vec![
            "INT. ONE".to_string(),
            "INT. TWO #5#".to_string(),
            "INT. THREE".to_string(),
            "INT. FOUR #6A#".to_string(),
            "INT. FIVE".to_string(),
            "INT. SIX#10#".to_string(),
            "INT. SEVEN #B#".to_string(),
            "INT. EIGHT".to_string(),
        ];
        let types = vec![LineType::SceneHeading; 8];

        let layout = build_layout(&lines, &types, 99, &config);
        let scenes: Vec<_> = layout
            .iter()
            .filter(|r| r.line_type == LineType::SceneHeading)
            .collect();

        assert_eq!(scenes[0].scene_num.as_deref(), Some("1"));
        assert_eq!(scenes[1].scene_num.as_deref(), Some("5"));
        assert_eq!(scenes[1].raw_text, "INT. TWO");
        assert_eq!(scenes[2].scene_num.as_deref(), Some("6"));
        assert_eq!(scenes[3].scene_num.as_deref(), Some("6A"));
        assert_eq!(scenes[3].raw_text, "INT. FOUR");
        assert_eq!(scenes[4].scene_num.as_deref(), Some("7"));
        assert_eq!(scenes[5].scene_num.as_deref(), Some("10"));
        assert_eq!(scenes[5].raw_text, "INT. SIX");
        assert_eq!(scenes[6].scene_num.as_deref(), Some("B"));
        assert_eq!(scenes[6].raw_text, "INT. SEVEN");
        assert_eq!(scenes[7].scene_num.as_deref(), Some("11"));
    }

    #[test]
    fn test_layout_explicit_scene_numbers_active_line() {
        let config = Config {
            show_scene_numbers: true,
            ..Config::default()
        };
        let lines = vec!["INT. KITCHEN #5#".to_string()];
        let types = vec![LineType::SceneHeading];
        let layout = build_layout(&lines, &types, 0, &config);

        assert_eq!(layout[0].scene_num.as_deref(), Some("5"));
        assert_eq!(layout[0].raw_text, "INT. KITCHEN #5#");
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    fn any_line_type() -> impl Strategy<Value = LineType> {
        (0..15u8).prop_map(|idx| match idx {
            0 => LineType::Action,
            1 => LineType::SceneHeading,
            2 => LineType::Character,
            3 => LineType::Dialogue,
            4 => LineType::Parenthetical,
            5 => LineType::Transition,
            6 => LineType::Centered,
            7 => LineType::Lyrics,
            8 => LineType::Note,
            9 => LineType::Boneyard,
            10 => LineType::PageBreak,
            11 => LineType::MetadataKey,
            12 => LineType::MetadataValue,
            13 => LineType::Shot,
            _ => LineType::Empty,
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(90000))]

        #[test]
        fn prop_formatting_parser_never_panics(s in "\\PC*") {
            let _fmt = parse_formatting(&s);
        }

        #[test]
        fn prop_sigil_stripping_is_safe_for_utf8(s in "[^\n]*", lt in any_line_type()) {
            let stripped = strip_sigils(&s, lt);
            let left_chars = sigil_left_chars(&s, lt);

            assert!(
                s.ends_with(stripped) || s.contains(stripped),
                "Stripped string must be a substring of the original"
            );

            assert!(
                left_chars <= s.chars().count(),
                "Sigil left chars exceeded total chars!"
            );
        }

        #[test]
        fn prop_layout_conserves_text_all_types(s in "[^\n]*", lt in any_line_type()) {
            let config = Config::default();
            let lines = vec![s.clone()];
            let types = vec![lt];

            let layout = build_layout(&lines, &types, 0, &config);

            if !layout.is_empty() {
                let reconstructed: String = layout
                    .iter()
                    .filter(|r| !r.is_phantom)
                    .map(|r| r.raw_text.as_str())
                    .collect();

                assert_eq!(
                    s, reconstructed,
                    "Text conservation failed for type {:?}! Original vs Reconstructed differ.", lt
                );
            }
        }

        #[test]
        fn prop_layout_width_never_exceeds_limit(s in "[^\n]*", lt in any_line_type()) {
            let config = Config::default();
            let lines = vec![s];
            let types = vec![lt];
            let layout = build_layout(&lines, &types, 0, &config);
            let max_width = lt.fmt().width;

            for row in layout.iter().filter(|r| !r.is_phantom) {
                let w = token_metrics(&row.raw_text, row.is_active, config.hide_markup).0;

                assert!(
                    w <= max_width,
                    "Row exceeded max width for type {:?}! Width: {}, Max: {}, Text: '{}'",
                    lt, w, max_width, row.raw_text
                );
            }
        }

        #[test]
        fn prop_char_boundaries_are_valid(s in "[^\n]*", lt in any_line_type()) {
            let config = Config::default();
            let lines = vec![s.clone()];
            let types = vec![lt];
            let layout = build_layout(&lines, &types, 0, &config);

            let mut expected_start = 0;
            let total_chars = s.chars().count();

            for row in layout.iter().filter(|r| !r.is_phantom) {
                assert_eq!(
                    row.char_start, expected_start,
                    "Gap or overlap detected in char_start"
                );
                assert!(
                    row.char_end >= row.char_start,
                    "char_end cannot be less than char_start"
                );
                assert!(
                    row.char_end <= total_chars,
                    "char_end exceeded total characters"
                );

                let row_char_count = row.raw_text.chars().count();
                assert_eq!(
                    row.char_end - row.char_start, row_char_count,
                    "Mismatch between raw_text length and (char_end - char_start)"
                );

                expected_start = row.char_end;
            }

            if !layout.is_empty() {
                assert_eq!(
                    expected_start, total_chars,
                    "Final char_end did not reach the end of the string"
                );
            }
        }

        #[test]
        fn prop_cursor_roundtrip_never_panics(s in "[^\n]*", cursor_pos in 0usize..2000) {
            let config = Config::default();
            let lines = vec![s.clone()];
            let types = vec![LineType::Action];
            let layout = build_layout(&lines, &types, 0, &config);

            let char_count = s.chars().count();
            let safe_cursor = if char_count == 0 { 0 } else { cursor_pos % (char_count + 1) };

            let (vi, visual_x) = find_visual_cursor(&layout, 0, safe_cursor);
            if vi < layout.len() {
                let row = &layout[vi];
                let is_last = row.char_end == char_count;
                let logical_back = row.visual_to_logical_x(visual_x, is_last);

                assert!(
                    logical_back <= char_count,
                    "visual_to_logical_x returned an out-of-bounds index: {} > {}",
                    logical_back, char_count
                );

                assert!(
                    logical_back >= row.char_start,
                    "Returned logical index is before the visual row start"
                );
            }
        }

        #[test]
        fn prop_to_uppercase_1to1_invariant(s in ".*") {
            use crate::formatting::StringCaseExt;
            let upper = s.to_uppercase_1to1();

            assert_eq!(
                s.chars().count(),
                upper.chars().count(),
                "to_uppercase_1to1 MUST strictly preserve character count"
            );
        }
    }
}
