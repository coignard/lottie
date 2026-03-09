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

use crate::types::LineType;

static META_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z][A-Za-z\s]*):\s*").unwrap());

pub struct Parser;

impl Parser {
    pub fn parse(lines: &[String]) -> Vec<LineType> {
        let mut types = vec![LineType::Empty; lines.len()];

        let mut in_header = false;
        for line in lines {
            let trim = line.trim();
            if trim.is_empty() {
                continue;
            }
            if META_KEY_RE.is_match(trim) {
                in_header = true;
            }
            break;
        }

        let mut in_boneyard = false;
        let mut in_note = false;

        for i in 0..lines.len() {
            let raw = &lines[i];

            if in_header {
                if raw.is_empty() {
                    in_header = false;
                    types[i] = LineType::Empty;
                    continue;
                }

                let trim = raw.trim();
                if trim.is_empty() {
                    if raw.starts_with("  ") || raw.starts_with('\t') {
                        types[i] = LineType::MetadataValue;
                        continue;
                    } else {
                        in_header = false;
                        types[i] = LineType::Empty;
                        continue;
                    }
                }

                if META_KEY_RE.is_match(trim) {
                    if trim.to_lowercase().starts_with("title:") {
                        types[i] = LineType::MetadataTitle;
                    } else {
                        types[i] = LineType::MetadataKey;
                    }
                } else {
                    types[i] = LineType::MetadataValue;
                }
                continue;
            }

            let mut effective = String::new();
            let chars: Vec<char> = raw.chars().collect();
            let mut j = 0;

            let mut boneyard_chars = 0;
            let mut note_chars = 0;
            let mut visible_chars = 0;

            while j < chars.len() {
                if in_boneyard {
                    if j + 1 < chars.len() && chars[j] == '*' && chars[j + 1] == '/' {
                        in_boneyard = false;
                        boneyard_chars += 2;
                        j += 2;
                    } else {
                        boneyard_chars += 1;
                        j += 1;
                    }
                    continue;
                }
                if in_note {
                    if j + 1 < chars.len() && chars[j] == ']' && chars[j + 1] == ']' {
                        in_note = false;
                        note_chars += 2;
                        j += 2;
                    } else {
                        note_chars += 1;
                        j += 1;
                    }
                    continue;
                }

                if j + 1 < chars.len() && chars[j] == '/' && chars[j + 1] == '*' {
                    in_boneyard = true;
                    boneyard_chars += 2;
                    j += 2;
                    continue;
                }
                if j + 1 < chars.len() && chars[j] == '[' && chars[j + 1] == '[' {
                    in_note = true;
                    note_chars += 2;
                    j += 2;
                    continue;
                }

                if !chars[j].is_whitespace() {
                    visible_chars += 1;
                }
                effective.push(chars[j]);
                j += 1;
            }

            let trim = effective.trim();

            if visible_chars == 0 {
                if boneyard_chars > 0 || in_boneyard {
                    types[i] = LineType::Boneyard;
                } else if note_chars > 0 || in_note {
                    types[i] = LineType::Note;
                } else {
                    types[i] = LineType::Empty;
                }
                continue;
            }

            if trim.starts_with("===") {
                types[i] = LineType::PageBreak;
                continue;
            }

            let prev = if i > 0 { types[i - 1] } else { LineType::Empty };
            let prev_empty = prev == LineType::Empty;

            if let Some(t) = Self::forced_type(trim, prev_empty) {
                types[i] = t;
                continue;
            }

            if prev_empty && Self::is_scene_heading(trim) {
                types[i] = LineType::SceneHeading;
                continue;
            }

            let is_transition = prev_empty
                && ((trim.ends_with("TO:") || trim.ends_with("IN:"))
                    && Self::is_uppercase_content(trim)
                    || trim == "FADE TO BLACK."
                    || trim == "FADE OUT."
                    || trim == "CUT TO BLACK.");

            if is_transition {
                types[i] = LineType::Transition;
                continue;
            }

            if trim.starts_with('(') {
                match prev {
                    LineType::Character
                    | LineType::DualDialogueCharacter
                    | LineType::Parenthetical
                    | LineType::Dialogue => {
                        types[i] = LineType::Parenthetical;
                        continue;
                    }
                    _ => {}
                }
            }

            match prev {
                LineType::Character
                | LineType::DualDialogueCharacter
                | LineType::Dialogue
                | LineType::Parenthetical => {
                    types[i] = LineType::Dialogue;
                    continue;
                }
                _ => {}
            }

            if prev_empty && Self::is_character_cue(trim) {
                if trim.ends_with('^') {
                    types[i] = LineType::DualDialogueCharacter;
                } else {
                    types[i] = LineType::Character;
                }
                continue;
            }

            types[i] = LineType::Action;
        }

        types
    }

    fn forced_type(trim: &str, prev_empty: bool) -> Option<LineType> {
        let first = trim.chars().next()?;
        let last = trim.chars().last()?;
        match first {
            '!' | '！' => Some(if trim.starts_with("!!") || trim.starts_with("！！") {
                LineType::Shot
            } else {
                LineType::Action
            }),
            '@' | '＠' => Some(if last == '^' {
                LineType::DualDialogueCharacter
            } else {
                LineType::Character
            }),
            '~' => Some(LineType::Lyrics),
            '>' => Some(if last == '<' {
                LineType::Centered
            } else {
                LineType::Transition
            }),
            '=' => Some(LineType::Synopsis),
            '#' => Some(LineType::Section),
            '.' if prev_empty && trim.len() > 1 && !trim.starts_with("..") => {
                Some(LineType::SceneHeading)
            }
            _ => None,
        }
    }

    fn is_scene_heading(s: &str) -> bool {
        let u = s.to_uppercase();
        let prefixes = ["INT", "EXT", "EST", "I/E", "E/I", "I./E", "E./I"];
        for p in prefixes {
            if let Some(rest) = u.strip_prefix(p) {
                if rest.is_empty() {
                    return true;
                }
                if let Some(next_char) = rest.chars().next()
                    && (next_char == '.' || next_char == ' ' || next_char == '/')
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_uppercase_content(s: &str) -> bool {
        let mut has = false;
        for c in s.chars() {
            if c.is_lowercase() {
                return false;
            }
            if c.is_uppercase() {
                has = true;
            }
        }
        has
    }

    fn is_character_cue(s: &str) -> bool {
        let stripped = s.trim_end_matches('^').trim();
        if stripped.is_empty() || stripped.len() > 50 || stripped.ends_with('.') {
            return false;
        }
        let no_markdown = stripped.replace("**", "").replace(['*', '_'], "");
        let check_part = if let Some(idx) = no_markdown.find('(') {
            no_markdown[..idx].trim()
        } else {
            no_markdown.as_str()
        };
        let alpha_count = check_part.chars().filter(|c| c.is_alphabetic()).count();
        alpha_count >= 2 && Self::is_uppercase_content(check_part)
    }
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn test_is_uppercase_content() {
        assert!(Parser::is_uppercase_content("RENÉ"));
        assert!(Parser::is_uppercase_content("EXT. PARK - DAY"));
        assert!(Parser::is_uppercase_content("CHARACTER (O.S.)"));
        assert!(!Parser::is_uppercase_content("René"));
        assert!(!Parser::is_uppercase_content("ext. park"));
        assert!(!Parser::is_uppercase_content("12345"));
        assert!(!Parser::is_uppercase_content("!@#$"));
    }

    #[test]
    fn test_parse_metadata_block_strict() {
        let lines = vec![
            "Title: Date in Kutaisi".to_string(),
            "Author: René Coignard".to_string(),
            "  Co-Author: Charlotte C.".to_string(),
            "".to_string(),
            "INT. RIONI RIVERBANK - EVENING".to_string(),
        ];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::MetadataTitle);
        assert_eq!(types[1], LineType::MetadataKey);
        assert_eq!(types[2], LineType::MetadataValue);
        assert_eq!(types[3], LineType::Empty);
        assert_eq!(types[4], LineType::SceneHeading);
    }

    #[test]
    fn test_parse_scene_headings() {
        let lines = vec![
            "INT. ROOM - DAY".to_string(),
            "EXT. STREET - NIGHT".to_string(),
            "EST. BUILDING".to_string(),
            "I/E. CAR".to_string(),
            "E/I. TRAIN".to_string(),
            "I./E. HOUSE".to_string(),
            "E./I. BOAT".to_string(),
        ];
        for (i, _) in lines.iter().enumerate() {
            let test_block = vec!["".to_string(), lines[i].clone()];
            let types = Parser::parse(&test_block);
            assert_eq!(types[1], LineType::SceneHeading);
        }
    }

    #[test]
    fn test_parse_forced_scene_heading() {
        let lines = vec!["".to_string(), ".HOUSE".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[1], LineType::SceneHeading);
    }

    #[test]
    fn test_parse_not_scene_heading() {
        let lines = vec!["".to_string(), "..not heading".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[1], LineType::Action);
    }

    #[test]
    fn test_parse_character_and_dialogue() {
        let lines = vec![
            "".to_string(),
            "RENÉ".to_string(),
            "Hallo.".to_string(),
            "".to_string(),
            "CHARLOTTE".to_string(),
            "(en souriant)".to_string(),
            "Coucou.".to_string(),
        ];
        let types = Parser::parse(&lines);
        assert_eq!(types[1], LineType::Character);
        assert_eq!(types[2], LineType::Dialogue);
        assert_eq!(types[3], LineType::Empty);
        assert_eq!(types[4], LineType::Character);
        assert_eq!(types[5], LineType::Parenthetical);
        assert_eq!(types[6], LineType::Dialogue);
    }

    #[test]
    fn test_parse_dual_dialogue_character() {
        let lines = vec![
            "".to_string(),
            "CHARLOTTE ^".to_string(),
            "Coucou.".to_string(),
        ];
        let types = Parser::parse(&lines);
        assert_eq!(types[1], LineType::DualDialogueCharacter);
        assert_eq!(types[2], LineType::Dialogue);
    }

    #[test]
    fn test_parse_transitions() {
        let lines = vec![
            "Action".to_string(),
            "".to_string(),
            "CUT TO:".to_string(),
            "".to_string(),
            "FADE OUT.".to_string(),
            "".to_string(),
            "FADE IN:".to_string(),
            "".to_string(),
            "CUT TO BLACK.".to_string(),
        ];
        let types = Parser::parse(&lines);
        assert_eq!(types[2], LineType::Transition);
        assert_eq!(types[4], LineType::Transition);
        assert_eq!(types[6], LineType::Transition);
        assert_eq!(types[8], LineType::Transition);
    }

    #[test]
    fn test_parse_forced_transition() {
        let lines = vec!["".to_string(), ">SMASH CUT".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[1], LineType::Transition);
    }

    #[test]
    fn test_parse_centered_text() {
        let lines = vec![">THE END<".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::Centered);
    }

    #[test]
    fn test_parse_forced_action_and_shot() {
        let lines = vec!["!ACT".to_string(), "!!SHOT".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::Action);
        assert_eq!(types[1], LineType::Shot);
    }

    #[test]
    fn test_parse_lyrics_synopsis_section() {
        let lines = vec![
            "~Song line".to_string(),
            "=Synopsis block".to_string(),
            "#Section block".to_string(),
        ];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::Lyrics);
        assert_eq!(types[1], LineType::Synopsis);
        assert_eq!(types[2], LineType::Section);
    }

    #[test]
    fn test_parse_boneyard_multiline() {
        let lines = vec![
            "/*".to_string(),
            "Hidden text".to_string(),
            "More hidden".to_string(),
            "*/".to_string(),
            "Visible action".to_string(),
        ];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::Boneyard);
        assert_eq!(types[1], LineType::Boneyard);
        assert_eq!(types[2], LineType::Boneyard);
        assert_eq!(types[3], LineType::Boneyard);
        assert_eq!(types[4], LineType::Action);
    }

    #[test]
    fn test_parse_boneyard_inline() {
        let lines = vec!["/* Inline hidden */".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::Boneyard);
    }

    #[test]
    fn test_parse_note_multiline() {
        let lines = vec!["[[".to_string(), "Note text".to_string(), "]]".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::Note);
        assert_eq!(types[1], LineType::Note);
        assert_eq!(types[2], LineType::Note);
    }

    #[test]
    fn test_parse_note_inline() {
        let lines = vec!["[[Inline note]]".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::Note);
    }

    #[test]
    fn test_parse_page_break() {
        let lines = vec!["===".to_string(), "=====".to_string()];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::PageBreak);
        assert_eq!(types[1], LineType::PageBreak);
    }

    #[test]
    fn test_parse_action_default() {
        let lines = vec![
            "".to_string(),
            "A man walks into a foobar.".to_string(),
            "He orders foo, bar, and baz.".to_string(),
        ];
        let types = Parser::parse(&lines);
        assert_eq!(types[0], LineType::Empty);
        assert_eq!(types[1], LineType::Action);
        assert_eq!(types[2], LineType::Action);
    }
}
