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

use std::collections::{HashMap, HashSet};

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::types::get_marker_color;

/// Extension trait that adds a character-count-preserving uppercase conversion
/// to `str`.
///
/// The standard [`str::to_uppercase`] can expand certain characters (e.g. the
/// German *Eszett* `ß` → `SS`), which breaks character-index arithmetic that
/// the layout engine relies on.  [`StringCaseExt::to_uppercase_1to1`]
/// guarantees a strict 1-to-1 character mapping by leaving any character that
/// would expand unchanged.
pub trait StringCaseExt {
    /// Converts the string to uppercase whilst strictly preserving its character
    /// count.
    ///
    /// Characters whose `to_uppercase` expansion produces more than one codepoint
    /// (e.g. `ß`, typographic ligatures, certain Greek letters) are left as-is.
    /// All other alphabetic characters are uppercased normally.
    ///
    /// # Examples
    ///
    /// ```
    /// use lottie_rs::formatting::StringCaseExt;
    ///
    /// assert_eq!("straße".to_uppercase_1to1(), "STRAßE");
    /// assert_eq!("hello".to_uppercase_1to1(),  "HELLO");
    /// ```
    fn to_uppercase_1to1(&self) -> String;
}

impl StringCaseExt for str {
    fn to_uppercase_1to1(&self) -> String {
        self.chars()
            .map(|c| {
                let mut upper = c.to_uppercase();
                let first = upper.next().unwrap();
                if upper.next().is_some() { c } else { first }
            })
            .collect()
    }
}

/// Returns `true` if `text` contains any byte that could introduce inline
/// Fountain/Markdown markup (`*`, `_`, `\`, `[`, `/`).
///
/// Used as a cheap pre-filter before running the full formatting parser.
#[inline]
pub fn has_markup_bytes(text: &str) -> bool {
    text.as_bytes()
        .iter()
        .any(|&b| matches!(b, b'*' | b'_' | b'\\' | b'[' | b'/'))
}

/// Per-line inline formatting metadata produced by [`parse_formatting`].
///
/// Each field is a set of *global* character indices (relative to the start of
/// the logical line, not the visual row) that carry a particular style.  The
/// layout engine stores one `LineFormatting` per [`crate::layout::VisualRow`]
/// so that [`render_inline`] can reconstruct the correct spans even
/// after word-wrapping.
#[derive(Default, Clone)]
pub struct LineFormatting {
    /// Indices of characters that should be rendered in **bold** (`**text**` or `***text***`).
    pub bold: HashSet<usize>,

    /// Indices of characters that should be rendered in *italic* (`*text*` or `***text***`).
    pub italic: HashSet<usize>,

    /// Indices of characters that should be rendered with an underline (`_text_`).
    pub underlined: HashSet<usize>,

    /// Indices of characters that belong to an inline note (`[[text]]`).
    pub note: HashSet<usize>,

    /// Indices of characters that belong to a boneyard comment (`/* text */`).
    pub boneyard: HashSet<usize>,

    /// Per-character colour overrides parsed from the note body (e.g. `[[yellow note]]`).
    ///
    /// Only populated for indices that are also in [`note`](LineFormatting::note).
    pub note_color: HashMap<usize, Color>,

    /// Indices of markup characters (asterisks, underscores, escape backslashes)
    /// that are hidden from view when the cursor is not on the line.
    pub hidden_chars: HashSet<usize>,
}

/// Parses all inline Fountain/Markdown formatting from `text` and returns the
/// resulting [`LineFormatting`] metadata.
///
/// Recognised markup:
/// - `**text**` → bold
/// - `*text*` → italic
/// - `***text***` → bold + italic
/// - `_text_` → underline
/// - `[[text]]` → note (optionally coloured)
/// - `/* text */` → boneyard
/// - `\*` → escaped character (suppresses formatting)
///
/// The function operates entirely on character indices, so it is safe for
/// arbitrary Unicode input including multi-byte sequences.
pub fn parse_formatting(text: &str) -> LineFormatting {
    if !has_markup_bytes(text) {
        return LineFormatting::default();
    }

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

    // stolen from tinder
    let mut find_pairs =
        |open: &[char], close: &[char], hide_markers: bool, apply: &mut dyn FnMut(usize, usize)| {
            let mut i = 0;
            while i < len {
                if skip.contains(&i) {
                    i += 1;
                    continue;
                }
                let mut match_open = true;
                for (k, &c) in open.iter().enumerate() {
                    if i + k >= len || chars[i + k] != c || skip.contains(&(i + k)) {
                        match_open = false;
                        break;
                    }
                }
                if match_open {
                    let mut j = i + open.len();
                    while j < len {
                        if skip.contains(&j) {
                            j += 1;
                            continue;
                        }
                        let mut match_close = true;
                        for (k, &c) in close.iter().enumerate() {
                            if j + k >= len || chars[j + k] != c || skip.contains(&(j + k)) {
                                match_close = false;
                                break;
                            }
                        }
                        if match_close {
                            apply(i, j);
                            for k in 0..open.len() {
                                skip.insert(i + k);
                                if hide_markers {
                                    fmt.hidden_chars.insert(i + k);
                                }
                            }
                            for k in 0..close.len() {
                                skip.insert(j + k);
                                if hide_markers {
                                    fmt.hidden_chars.insert(j + k);
                                }
                            }
                            i = j + close.len() - 1;
                            break;
                        }
                        j += 1;
                    }
                }
                i += 1;
            }
        };

    find_pairs(&['/', '*'], &['*', '/'], false, &mut |start, end| {
        for i in start..(end + 2) {
            fmt.boneyard.insert(i);
        }
    });

    find_pairs(&['[', '['], &[']', ']'], false, &mut |start, end| {
        let content: String = chars[start + 2..end].iter().collect();
        let color = get_marker_color(&content);
        for i in start..(end + 2) {
            fmt.note.insert(i);
            if let Some(c) = color {
                fmt.note_color.insert(i, c);
            }
        }
    });

    find_pairs(
        &['*', '*', '*'],
        &['*', '*', '*'],
        true,
        &mut |start, end| {
            for i in (start + 3)..end {
                fmt.bold.insert(i);
                fmt.italic.insert(i);
            }
        },
    );
    find_pairs(&['*', '*'], &['*', '*'], true, &mut |start, end| {
        for i in (start + 2)..end {
            fmt.bold.insert(i);
        }
    });
    find_pairs(&['*'], &['*'], true, &mut |start, end| {
        for i in (start + 1)..end {
            fmt.italic.insert(i);
        }
    });
    find_pairs(&['_'], &['_'], true, &mut |start, end| {
        for i in (start + 1)..end {
            fmt.underlined.insert(i);
        }
    });

    fmt
}

/// Configuration for a single call to [`render_inline`].
///
/// This is a plain value type passed by copy; fields that default to `false`
/// are safe to leave at `Default::default()` unless the caller needs to
/// override them.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderConfig {
    /// When `true`, markup characters (asterisks, underscores) are included in the
    /// rendered output even for inactive lines
    pub reveal_markup: bool,

    /// When `true`, all inline markdown processing is skipped and the text is
    /// returned as a single unstyled span.  Used for boneyard lines.
    pub skip_markdown: bool,

    /// When `true`, characters tagged as [`boneyard`](LineFormatting::boneyard) or
    /// [`note`](LineFormatting::note) are omitted from the output entirely.
    /// Used by the export path to strip editorial annotations from printed output.
    pub exclude_comments: bool,

    /// The global character index of the first character in the `text` slice.
    ///
    /// Because `text` may be a word-wrapped *sub-string* of a longer logical line,
    /// this offset is added to every local index before looking up formatting sets.
    pub char_offset: usize,

    /// The global character index *after* the colon in a metadata key (e.g.
    /// `Author: ` → 8).  Characters before this index are dimmed to visually
    /// distinguish keys from values.  `0` disables the feature.
    pub meta_key_end: usize,

    /// When `true`, all foreground/background colour styling is suppressed.
    pub no_color: bool,

    /// When `true`, all bold/italic/underline modifier styling is suppressed.
    pub no_formatting: bool,
}

/// Renders `text` into a sequence of styled ratatui [`Span`]s, applying inline
/// formatting, search highlights, and colour overrides.
///
/// Adjacent characters that share the same computed [`Style`] are coalesced
/// into a single span to minimise allocations.  The function always returns at
/// least one span (potentially empty) so callers can unconditionally index
/// position `0`.
///
/// # Parameters
///
/// - `text` -- the display string for this visual row (possibly a sub-string
///   after sigil stripping and case transformation).
/// - `base` -- the base style for the line type, as produced by
///   [`crate::types::base_style`].
/// - `fmt` -- per-line formatting metadata from [`parse_formatting`].
/// - `cfg` -- rendering options; see [`RenderConfig`].
/// - `highlights` - global character indices that should be visually highlighted
///   (e.g. search matches).
pub fn render_inline(
    text: &str,
    base: Style,
    fmt: &LineFormatting,
    cfg: RenderConfig,
    highlights: &HashSet<usize>,
) -> Vec<Span<'static>> {
    if cfg.skip_markdown && !cfg.exclude_comments {
        return vec![Span::styled(text.to_string(), base)];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut current_style = base;

    for (local_i, &c) in chars.iter().enumerate() {
        let global_i = cfg.char_offset + local_i;

        if cfg.exclude_comments
            && (fmt.boneyard.contains(&global_i) || fmt.note.contains(&global_i))
        {
            continue;
        }

        if !cfg.reveal_markup && fmt.hidden_chars.contains(&global_i) {
            continue;
        }

        let mut s = base;

        if !cfg.no_formatting {
            if fmt.bold.contains(&global_i) {
                s.add_modifier = s.add_modifier.union(Modifier::BOLD);
            }
            if fmt.italic.contains(&global_i) || fmt.note.contains(&global_i) {
                s.add_modifier = s.add_modifier.union(Modifier::ITALIC);
            }
            if fmt.underlined.contains(&global_i) {
                s.add_modifier = s.add_modifier.union(Modifier::UNDERLINED);
            }
        }

        if !cfg.no_color {
            let is_key = global_i < cfg.meta_key_end;

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
        }

        if highlights.contains(&global_i) {
            if cfg.no_color {
                s.fg = None;
                s.bg = None;
                s.add_modifier = s.add_modifier.union(Modifier::REVERSED);
            } else {
                s.bg = Some(Color::Yellow);
                s.fg = Some(Color::Black);

                s.sub_modifier = s.sub_modifier.union(Modifier::BOLD).union(Modifier::DIM);
            }
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

    fn assert_upper_1to1(input: &str, expected: &str) {
        let result = input.to_uppercase_1to1();
        assert_eq!(
            result, expected,
            "Uppercase value mismatch for input '{}'",
            input
        );
        assert_eq!(
            input.chars().count(),
            result.chars().count(),
            "FATAL: Length invariant violated for input '{}'. Expected {} chars, got {}.",
            input,
            input.chars().count(),
            result.chars().count()
        );
    }

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
        let fmt = parse_formatting("[[yellow note]]");
        assert!(fmt.note.contains(&5));
        assert_eq!(fmt.note_color.get(&5), Some(&ratatui::style::Color::Yellow));
    }

    #[test]
    fn test_render_inline_no_markdown_skip() {
        let fmt = parse_formatting("**bold**");
        let cfg = RenderConfig {
            skip_markdown: true,
            ..Default::default()
        };
        let hl = HashSet::new();
        let spans = render_inline("**bold**", Style::default(), &fmt, cfg, &hl);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "**bold**");
    }

    #[test]
    fn test_render_inline_reveal_markup() {
        let fmt = parse_formatting("**bold**");
        let cfg = RenderConfig {
            reveal_markup: true,
            ..Default::default()
        };
        let hl = HashSet::new();
        let spans = render_inline("**bold**", Style::default(), &fmt, cfg, &hl);
        let complete_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(complete_text, "**bold**");
    }

    #[test]
    fn test_render_inline_hide_markup() {
        let fmt = parse_formatting("**bold**");
        let hl = HashSet::new();
        let spans = render_inline(
            "**bold**",
            Style::default(),
            &fmt,
            RenderConfig::default(),
            &hl,
        );
        let complete_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(complete_text, "bold");
    }

    #[test]
    fn test_render_inline_metadata_key_color() {
        let fmt = LineFormatting::default();
        let cfg = RenderConfig {
            meta_key_end: 7,
            ..Default::default()
        };
        let hl = HashSet::new();
        let spans = render_inline("Title: Text", Style::default(), &fmt, cfg, &hl);
        assert_eq!(spans[0].content, "Title: ");
        assert_eq!(spans[0].style.fg, Some(ratatui::style::Color::DarkGray));
        assert_eq!(spans[1].content, "Text");
        assert_eq!(spans[1].style.fg, None);
    }

    #[test]
    fn test_render_inline_no_color_only() {
        let fmt = parse_formatting("**bold text** with [[yellow note]]");
        let cfg = RenderConfig {
            reveal_markup: true,
            no_color: true,
            no_formatting: false,
            ..Default::default()
        };
        let hl = HashSet::new();

        let spans = render_inline(
            "**bold text** with [[yellow note]]",
            Style::default(),
            &fmt,
            cfg,
            &hl,
        );

        let bold_span = spans.iter().find(|s| s.content.contains("bold")).unwrap();
        assert_eq!(
            bold_span.style.fg, None,
            "Bold span color should be stripped"
        );
        assert!(
            bold_span
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD),
            "Bold modifier should remain"
        );

        let note_span = spans.iter().find(|s| s.content.contains("yellow")).unwrap();
        assert_eq!(note_span.style.fg, None, "Note color should be stripped");
        assert!(
            note_span
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::ITALIC),
            "Note italic modifier should remain"
        );
    }

    #[test]
    fn test_render_inline_no_formatting_only() {
        let fmt = parse_formatting("**bold text** with [[yellow note]]");
        let cfg = RenderConfig {
            reveal_markup: true,
            no_color: false,
            no_formatting: true,
            ..Default::default()
        };
        let hl = HashSet::new();

        let spans = render_inline(
            "**bold text** with [[yellow note]]",
            Style::default(),
            &fmt,
            cfg,
            &hl,
        );

        let bold_span = spans.iter().find(|s| s.content.contains("bold")).unwrap();
        assert_eq!(
            bold_span.style.add_modifier,
            ratatui::style::Modifier::empty(),
            "Bold modifier should be stripped"
        );

        let note_span = spans.iter().find(|s| s.content.contains("yellow")).unwrap();
        assert_eq!(
            note_span.style.add_modifier,
            ratatui::style::Modifier::empty(),
            "Note italic modifier should be stripped"
        );
        assert_eq!(
            note_span.style.fg,
            Some(ratatui::style::Color::Yellow),
            "Note color should remain"
        );
    }

    #[test]
    fn test_render_inline_no_color_and_no_formatting() {
        let fmt = parse_formatting("**bold text** with [[yellow note]]");
        let cfg = RenderConfig {
            reveal_markup: true,
            no_color: true,
            no_formatting: true,
            ..Default::default()
        };
        let hl = HashSet::new();

        let spans = render_inline(
            "**bold text** with [[yellow note]]",
            Style::default(),
            &fmt,
            cfg,
            &hl,
        );

        for span in spans {
            assert_eq!(
                span.style,
                Style::default(),
                "Everything should be stripped down to default style"
            );
        }
    }

    #[test]
    fn test_render_inline_search_highlight_color() {
        let fmt = LineFormatting::default();
        let cfg = RenderConfig::default();

        let mut hl = HashSet::new();
        hl.extend(0..4);

        let base_style = Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::White);

        let spans = render_inline("test string", base_style, &fmt, cfg, &hl);

        let highlight_span = &spans[0];
        assert_eq!(highlight_span.content, "test");
        assert_eq!(highlight_span.style.bg, Some(Color::Yellow));
        assert_eq!(highlight_span.style.fg, Some(Color::Black));

        assert!(highlight_span.style.sub_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_render_inline_search_highlight_no_color() {
        let fmt = LineFormatting::default();
        let cfg = RenderConfig {
            no_color: true,
            ..Default::default()
        };

        let mut hl = HashSet::new();
        hl.extend(0..4);

        let spans = render_inline("test string", Style::default(), &fmt, cfg, &hl);

        let highlight_span = &spans[0];
        assert_eq!(highlight_span.style.bg, None);
        assert_eq!(highlight_span.style.fg, None);

        assert!(
            highlight_span
                .style
                .add_modifier
                .contains(Modifier::REVERSED)
        );
    }

    #[test]
    fn test_to_uppercase_1to1_ascii_and_latin() {
        assert_upper_1to1("hello", "HELLO");
        assert_upper_1to1("Hello World!", "HELLO WORLD!");
    }

    #[test]
    fn test_to_uppercase_1to1_cyrillic() {
        assert_upper_1to1("привет", "ПРИВЕТ");
        assert_upper_1to1("ёжик", "ЁЖИК");
    }

    #[test]
    fn test_to_uppercase_1to1_german_eszett() {
        assert_upper_1to1("straße", "STRAßE");
        assert_upper_1to1("groß", "GROß");
        assert_upper_1to1("weiß", "WEIß");
    }

    #[test]
    fn test_to_uppercase_1to1_typographic_ligatures() {
        assert_upper_1to1("ﬁnancial", "ﬁNANCIAL");
        assert_upper_1to1("ﬂight", "ﬂIGHT");
        assert_upper_1to1("baﬄe", "BAﬄE");
    }

    #[test]
    fn test_to_uppercase_1to1_emojis_and_zwj() {
        assert_upper_1to1("🦀 rust", "🦀 RUST");
        assert_upper_1to1("🧑‍🧑‍🧒‍🧒 family", "🧑‍🧑‍🧒‍🧒 FAMILY");
        assert_upper_1to1("🏳️‍🌈 pride", "🏳️‍🌈 PRIDE");
    }

    #[test]
    fn test_to_uppercase_1to1_greek_expanding() {
        assert_upper_1to1("αβγ", "ΑΒΓ");
        assert_upper_1to1("φαΐ", "ΦΑΐ");
    }

    #[test]
    fn test_to_uppercase_1to1_combining_diacritics() {
        assert_upper_1to1("áb́ć", "ÁB́Ć");
        assert_upper_1to1("приве́т", "ПРИВЕ́Т");
    }

    #[test]
    fn test_to_uppercase_1to1_dutch_ligature() {
        let input = "ĳsvogel";
        let result = input.to_uppercase_1to1();

        assert_eq!(
            input.chars().count(),
            result.chars().count(),
            "Length invariant failed for Dutch ligature"
        );

        assert!(result.ends_with("SVOGEL"));
    }
}
