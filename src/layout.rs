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
use std::sync::LazyLock;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::Config;
use crate::formatting::{LineFormatting, parse_formatting};
use crate::types::{LINES_PER_PAGE, LineType, PAGE_WIDTH, get_marker_color};

static SCENE_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(.*?)\s*(#[\w.\-)]+#)\s*$").unwrap());

#[derive(Clone)]
pub struct VisualRow {
    pub line_idx: usize,
    pub char_start: usize,
    pub char_end: usize,
    pub raw_text: String,
    pub line_type: LineType,
    pub indent: u16,
    pub is_active: bool,
    pub scene_num: Option<usize>,
    pub page_num: Option<usize>,
    pub override_color: Option<ratatui::style::Color>,
    pub fmt: LineFormatting,
    pub is_phantom: bool,
}

impl VisualRow {
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

pub fn sigil_left_chars(raw: &str, lt: LineType) -> usize {
    let stripped = strip_sigils(raw, lt);
    if stripped.as_ptr() >= raw.as_ptr() {
        let byte_offset = stripped.as_ptr() as usize - raw.as_ptr() as usize;
        raw[..byte_offset].chars().count()
    } else {
        0
    }
}

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

fn tokenize_text(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut prev_was_sep = true;
    for c in text.chars() {
        let is_sep = c.is_whitespace() || c == '-';
        current.push(c);
        if is_sep && !prev_was_sep {
            tokens.push(current);
            current = String::new();
        }
        prev_was_sep = is_sep;
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if tokens.is_empty() {
        tokens.push(String::new());
    }
    tokens
}

fn calculate_indent(lt: LineType, text: &str, base_indent: u16) -> u16 {
    match lt {
        LineType::Centered | LineType::Lyrics => {
            let plain = text.replace("**", "").replace(['*', '_'], "");
            let w = UnicodeWidthStr::width(plain.as_str()) as u16;
            PAGE_WIDTH.saturating_sub(w) / 2
        }
        LineType::Transition => {
            let plain = text.replace("**", "").replace(['*', '_'], "");
            let w = UnicodeWidthStr::width(plain.as_str()) as u16;
            PAGE_WIDTH.saturating_sub(w)
        }
        _ => base_indent,
    }
}

pub fn build_layout(
    lines: &[String],
    types: &[LineType],
    active_line: usize,
    config: &Config,
) -> Vec<VisualRow> {
    let mut rows: Vec<VisualRow> = Vec::with_capacity(lines.len() + 32);
    let mut last_speaking_character = String::new();
    let mut scene_counter = 0;
    let mut printable_row_count = 0;
    let mut page_number = 1;
    let mut page_num_pending = true;

    let mut active_note_color: Option<ratatui::style::Color> = None;

    for (i, (line, &lt)) in lines.iter().zip(types.iter()).enumerate() {
        let is_active = i == active_line;
        let mut scene_num = None;
        let mut raw_line = line.clone();
        let mut line_override_color = None;

        let format_data = parse_formatting(&raw_line);

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

        if lt == LineType::SceneHeading {
            scene_counter += 1;
            if config.show_scene_numbers {
                scene_num = Some(scene_counter);
            }
            if let Some(caps) = SCENE_NUM_RE.captures(&raw_line)
                && !is_active
            {
                raw_line = caps[1].to_string();
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
                        fmt: Default::default(),
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
                raw_line.clone()
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
                fmt: format_data.clone(),
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
        let mut display = if is_active {
            raw_line.clone()
        } else {
            strip_sigils(&raw_line, lt).to_string()
        };

        if !is_active
            && matches!(
                lt,
                LineType::SceneHeading | LineType::Section | LineType::Synopsis
            )
        {
            let mut cleaned = String::new();
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
            display = cleaned.trim_end().to_string();
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
                final_display = format!("{} {}", display, config.contd_extension);
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
        let mut current_line = String::new();
        let tokens = tokenize_text(&final_display);
        let mut logical_rows = Vec::new();

        for token in tokens {
            let mut remaining_token = token;

            while !remaining_token.is_empty() {
                let token_plain = remaining_token.replace("**", "").replace(['*', '_'], "");
                let is_pure_space = token_plain.chars().all(|c| c.is_whitespace());

                let token_w_trimmed = if is_pure_space {
                    UnicodeWidthStr::width(token_plain.as_str()) as u16
                } else {
                    UnicodeWidthStr::width(token_plain.trim_end_matches(' ')) as u16
                };

                let cur_plain = current_line.replace("**", "").replace(['*', '_'], "");
                let cur_w = UnicodeWidthStr::width(cur_plain.as_str()) as u16;

                if !current_line.is_empty()
                    && !is_pure_space
                    && cur_w + token_w_trimmed > fmt_rules.width
                {
                    let disp_char_len = current_line.chars().count();
                    let raw_start = (sigil_left + row_disp_start).min(total_original_chars);
                    let raw_end = (raw_start + disp_char_len).min(total_original_chars);
                    let current_indent = calculate_indent(lt, &current_line, fmt_rules.indent);

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
                        fmt: format_data.clone(),
                        is_phantom: false,
                    });

                    if let Some(wrap_indent) = fmt_rules.wrap_indent {
                        fmt_rules.indent = wrap_indent;
                    }

                    row_disp_start += disp_char_len;
                    current_line.clear();
                    scene_num = None;

                    continue;
                }

                if cur_w + token_w_trimmed > fmt_rules.width {
                    let space_left = fmt_rules.width.saturating_sub(cur_w);

                    let mut chars_to_take = 0;
                    let t_chars: Vec<char> = remaining_token.chars().collect();
                    for (k, _) in t_chars.iter().enumerate() {
                        let test_str: String = t_chars[..=k].iter().collect();
                        let test_plain = test_str.replace("**", "").replace(['*', '_'], "");
                        let w = UnicodeWidthStr::width(test_plain.as_str()) as u16;
                        if w > space_left {
                            if chars_to_take == 0 {
                                chars_to_take = 1;
                            }
                            break;
                        }
                        chars_to_take = k + 1;
                    }

                    let part1: String = t_chars[..chars_to_take].iter().collect();
                    let part2: String = t_chars[chars_to_take..].iter().collect();

                    current_line.push_str(&part1);

                    let disp_char_len = current_line.chars().count();
                    let raw_start = (sigil_left + row_disp_start).min(total_original_chars);
                    let raw_end = (raw_start + disp_char_len).min(total_original_chars);
                    let current_indent = calculate_indent(lt, &current_line, fmt_rules.indent);

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
                        fmt: format_data.clone(),
                        is_phantom: false,
                    });

                    if let Some(wrap_indent) = fmt_rules.wrap_indent {
                        fmt_rules.indent = wrap_indent;
                    }

                    row_disp_start += disp_char_len;
                    current_line.clear();
                    scene_num = None;

                    remaining_token = part2;
                } else {
                    current_line.push_str(&remaining_token);
                    break;
                }
            }
        }

        let disp_char_len = current_line.chars().count();
        let raw_start = (sigil_left + row_disp_start).min(total_original_chars);
        let raw_end = (raw_start + disp_char_len).min(total_original_chars);
        let current_indent = calculate_indent(lt, &current_line, fmt_rules.indent);

        logical_rows.push(VisualRow {
            line_idx: i,
            char_start: raw_start,
            char_end: raw_end,
            raw_text: current_line,
            line_type: lt,
            indent: current_indent,
            is_active,
            scene_num,
            page_num: None,
            override_color: line_override_color,
            fmt: format_data.clone(),
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

        assert_eq!(layout[0].scene_num, Some(1));
        assert_eq!(layout[2].scene_num, Some(2));
    }

    #[test]
    fn test_build_layout_auto_contd() {
        let config = Config {
            auto_contd: true,
            contd_extension: "(CONT'D)".to_string(),
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
        let layout = build_layout(&lines, &types, 99, &config);
        assert_eq!(layout[0].raw_text, "CHARLOTTE");
        assert_eq!(layout[3].raw_text, "CHARLOTTE (CONT'D)");
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
            fmt: parse_formatting("Test **bold**"),
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
            fmt: parse_formatting("Test **bold**"),
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
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use unicode_width::UnicodeWidthStr;

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
        #![proptest_config(ProptestConfig::with_cases(30000))]

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
                let plain = row.raw_text.replace("**", "").replace(['*', '_'], "");
                let trimmed = plain.trim_end_matches(' ');
                let w = UnicodeWidthStr::width(trimmed) as u16;

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
