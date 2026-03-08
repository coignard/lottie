use std::collections::{HashMap, HashSet};

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::types::get_marker_color;

#[derive(Default, Clone)]
pub struct LineFormatting {
    pub bold: HashSet<usize>,
    pub italic: HashSet<usize>,
    pub underlined: HashSet<usize>,
    pub note: HashSet<usize>,
    pub boneyard: HashSet<usize>,
    pub note_color: HashMap<usize, Color>,
    pub hidden_chars: HashSet<usize>,
}

pub fn parse_formatting(text: &str) -> LineFormatting {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut fmt = LineFormatting::default();
    let mut skip = HashSet::new();

    for (i, &c) in chars.iter().enumerate() {
        if c == '\\' && i + 1 < len {
            skip.insert(i);
            fmt.hidden_chars.insert(i);
            skip.insert(i + 1);
        }
    }

    let mut find_pairs =
        |open: &str, close: &str, hide_markers: bool, apply: &mut dyn FnMut(usize, usize)| {
            let open_chars: Vec<char> = open.chars().collect();
            let close_chars: Vec<char> = close.chars().collect();
            let mut i = 0;
            while i < len {
                if skip.contains(&i) {
                    i += 1;
                    continue;
                }
                let mut match_open = true;
                for (k, &c) in open_chars.iter().enumerate() {
                    if i + k >= len || chars[i + k] != c || skip.contains(&(i + k)) {
                        match_open = false;
                        break;
                    }
                }
                if match_open {
                    let mut j = i + open_chars.len();
                    while j < len {
                        if skip.contains(&j) {
                            j += 1;
                            continue;
                        }
                        let mut match_close = true;
                        for (k, &c) in close_chars.iter().enumerate() {
                            if j + k >= len || chars[j + k] != c || skip.contains(&(j + k)) {
                                match_close = false;
                                break;
                            }
                        }
                        if match_close {
                            apply(i, j);
                            for k in 0..open_chars.len() {
                                skip.insert(i + k);
                                if hide_markers {
                                    fmt.hidden_chars.insert(i + k);
                                }
                            }
                            for k in 0..close_chars.len() {
                                skip.insert(j + k);
                                if hide_markers {
                                    fmt.hidden_chars.insert(j + k);
                                }
                            }
                            i = j + close_chars.len() - 1;
                            break;
                        }
                        j += 1;
                    }
                }
                i += 1;
            }
        };

    find_pairs("/*", "*/", false, &mut |start, end| {
        for i in start..(end + 2) {
            fmt.boneyard.insert(i);
        }
    });

    find_pairs("[[", "]]", false, &mut |start, end| {
        let content: String = chars[start + 2..end].iter().collect();
        let color = get_marker_color(&content);
        for i in start..(end + 2) {
            fmt.note.insert(i);
            if let Some(c) = color {
                fmt.note_color.insert(i, c);
            }
        }
    });

    find_pairs("***", "***", true, &mut |start, end| {
        for i in (start + 3)..end {
            fmt.bold.insert(i);
            fmt.italic.insert(i);
        }
    });
    find_pairs("**", "**", true, &mut |start, end| {
        for i in (start + 2)..end {
            fmt.bold.insert(i);
        }
    });
    find_pairs("*", "*", true, &mut |start, end| {
        for i in (start + 1)..end {
            fmt.italic.insert(i);
        }
    });
    find_pairs("_", "_", true, &mut |start, end| {
        for i in (start + 1)..end {
            fmt.underlined.insert(i);
        }
    });

    fmt
}

pub fn render_inline(
    text: &str,
    base: Style,
    reveal_markup: bool,
    skip_markdown: bool,
    fmt: &LineFormatting,
    char_offset: usize,
    meta_key_end: usize,
) -> Vec<Span<'static>> {
    if skip_markdown {
        return vec![Span::styled(text.to_string(), base)];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut current_style = base;

    for (local_i, &c) in chars.iter().enumerate() {
        let global_i = char_offset + local_i;

        if !reveal_markup && fmt.hidden_chars.contains(&global_i) {
            continue;
        }

        let mut s = base;
        let is_key = global_i < meta_key_end;

        if fmt.bold.contains(&global_i) {
            s.add_modifier = s.add_modifier.union(Modifier::BOLD);
        }
        if fmt.italic.contains(&global_i) || fmt.note.contains(&global_i) {
            s.add_modifier = s.add_modifier.union(Modifier::ITALIC);
        }
        if fmt.underlined.contains(&global_i) {
            s.add_modifier = s.add_modifier.union(Modifier::UNDERLINED);
        }

        if fmt.boneyard.contains(&global_i) {
            s.fg = Some(Color::DarkGray);
        } else if fmt.note.contains(&global_i) {
            s.fg = Some(
                fmt.note_color
                    .get(&global_i)
                    .copied()
                    .unwrap_or(base.fg.unwrap_or(Color::Green)),
            );
        } else if is_key {
            s.fg = Some(Color::DarkGray);
        }

        if s != current_style && !buf.is_empty() {
            spans.push(Span::styled(buf.clone(), current_style));
            buf.clear();
        }
        current_style = s;
        buf.push(c);
    }

    if !buf.is_empty() {
        spans.push(Span::styled(buf, current_style));
    }
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    spans
}

#[cfg(test)]
mod formatting_tests {
    use super::*;

    #[test]
    fn test_parse_formatting_bold() {
        let fmt = parse_formatting("This is **bold** text.");
        assert!(!fmt.bold.contains(&7));
        assert!(!fmt.bold.contains(&8));
        assert!(fmt.bold.contains(&10));
        assert!(fmt.bold.contains(&11));
        assert!(fmt.bold.contains(&12));
        assert!(fmt.bold.contains(&13));
        assert!(!fmt.bold.contains(&14));
        assert!(!fmt.bold.contains(&15));
        assert!(fmt.hidden_chars.contains(&8));
        assert!(fmt.hidden_chars.contains(&9));
        assert!(fmt.hidden_chars.contains(&14));
        assert!(fmt.hidden_chars.contains(&15));
    }

    #[test]
    fn test_parse_formatting_italic() {
        let fmt = parse_formatting("An *italic* word.");
        assert!(fmt.italic.contains(&4));
        assert!(fmt.italic.contains(&9));
        assert!(fmt.hidden_chars.contains(&3));
        assert!(fmt.hidden_chars.contains(&10));
    }

    #[test]
    fn test_parse_formatting_underline() {
        let fmt = parse_formatting("An _underlined_ word.");
        assert!(fmt.underlined.contains(&4));
        assert!(fmt.underlined.contains(&13));
        assert!(fmt.hidden_chars.contains(&3));
        assert!(fmt.hidden_chars.contains(&14));
    }

    #[test]
    fn test_parse_formatting_bold_italic() {
        let fmt = parse_formatting("Some ***bold italic*** text.");
        assert!(fmt.bold.contains(&8));
        assert!(fmt.italic.contains(&8));
        assert!(fmt.bold.contains(&18));
        assert!(fmt.italic.contains(&18));
        assert!(fmt.hidden_chars.contains(&5));
        assert!(fmt.hidden_chars.contains(&6));
        assert!(fmt.hidden_chars.contains(&7));
        assert!(fmt.hidden_chars.contains(&19));
        assert!(fmt.hidden_chars.contains(&20));
        assert!(fmt.hidden_chars.contains(&21));
    }

    #[test]
    fn test_parse_formatting_escaped() {
        let fmt = parse_formatting("Not \\*italic\\*.");
        assert!(fmt.italic.is_empty());
        assert!(fmt.hidden_chars.contains(&4));
        assert!(fmt.hidden_chars.contains(&12));
    }

    #[test]
    fn test_parse_formatting_boneyard() {
        let fmt = parse_formatting("/*hidden*/");
        assert!(fmt.boneyard.contains(&0));
        assert!(fmt.boneyard.contains(&1));
        assert!(fmt.boneyard.contains(&2));
        assert!(fmt.boneyard.contains(&8));
        assert!(fmt.boneyard.contains(&9));
        assert!(!fmt.hidden_chars.contains(&0));
    }

    #[test]
    fn test_parse_formatting_notes() {
        let fmt = parse_formatting("[[note text]]");
        assert!(fmt.note.contains(&0));
        assert!(fmt.note.contains(&2));
        assert!(fmt.note.contains(&11));
        assert!(fmt.note.contains(&12));
        assert!(!fmt.hidden_chars.contains(&0));
    }

    #[test]
    fn test_parse_formatting_notes_with_color() {
        let fmt = parse_formatting("[[blue note]]");
        assert!(fmt.note.contains(&5));
        assert_eq!(fmt.note_color.get(&5), Some(&ratatui::style::Color::Blue));
    }

    #[test]
    fn test_render_inline_no_markdown_skip() {
        let fmt = parse_formatting("**bold**");
        let spans = render_inline("**bold**", Style::default(), false, true, &fmt, 0, 0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "**bold**");
    }

    #[test]
    fn test_render_inline_reveal_markup() {
        let fmt = parse_formatting("**bold**");
        let spans = render_inline("**bold**", Style::default(), true, false, &fmt, 0, 0);
        let complete_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(complete_text, "**bold**");
    }

    #[test]
    fn test_render_inline_hide_markup() {
        let fmt = parse_formatting("**bold**");
        let spans = render_inline("**bold**", Style::default(), false, false, &fmt, 0, 0);
        let complete_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(complete_text, "bold");
    }

    #[test]
    fn test_render_inline_metadata_key_color() {
        let fmt = LineFormatting::default();
        let spans = render_inline("Title: Text", Style::default(), false, false, &fmt, 0, 7);
        assert_eq!(spans[0].content, "Title: ");
        assert_eq!(spans[0].style.fg, Some(ratatui::style::Color::DarkGray));
        assert_eq!(spans[1].content, "Text");
        assert_eq!(spans[1].style.fg, None);
    }
}
