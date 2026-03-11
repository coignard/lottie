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

use std::{collections::HashSet, fs, io, path::PathBuf};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};

use crate::{
    config::{Cli, Config},
    formatting::{RenderConfig, render_inline},
    layout::{VisualRow, build_layout, find_visual_cursor, strip_sigils},
    parser::Parser,
    types::{LineType, PAGE_WIDTH, base_style},
};

#[derive(Clone)]
pub struct HistoryState {
    pub lines: Vec<String>,
    pub cursor_y: usize,
    pub cursor_x: usize,
}

#[derive(PartialEq)]
pub enum LastEdit {
    None,
    Insert,
    Delete,
    Cut,
    Other,
}

#[derive(PartialEq, Debug)]
pub enum AppMode {
    Normal,
    Search,
    PromptSave,
    PromptFilename,
}

pub struct App {
    pub config: Config,
    pub lines: Vec<String>,
    pub types: Vec<LineType>,
    pub layout: Vec<VisualRow>,
    pub file: Option<PathBuf>,
    pub dirty: bool,
    pub cursor_y: usize,
    pub cursor_x: usize,
    pub target_visual_x: u16,
    pub visible_height: usize,
    pub scroll: usize,

    pub characters: HashSet<String>,
    pub locations: HashSet<String>,
    pub suggestion: Option<String>,

    pub undo_stack: Vec<HistoryState>,
    pub redo_stack: Vec<HistoryState>,
    pub last_edit: LastEdit,

    pub mode: AppMode,
    pub exit_after_save: bool,
    pub filename_input: String,

    pub status_msg: Option<String>,
    pub cut_buffer: Option<String>,
    pub search_query: String,
    pub last_search: String,

    pub show_search_highlight: bool,
    pub compiled_search_regex: Option<regex::Regex>,
}

impl App {
    pub fn new(path: Option<PathBuf>, cli: Cli) -> Self {
        let mut is_new_or_empty = false;

        let lines = match &path {
            Some(p) if p.exists() => {
                let text = fs::read_to_string(p)
                    .unwrap_or_default()
                    .replace('\t', "    ");
                if text.trim().is_empty() {
                    is_new_or_empty = true;
                    vec![String::new()]
                } else {
                    let ls: Vec<String> = text.lines().map(str::to_string).collect();
                    if ls.is_empty() {
                        vec![String::new()]
                    } else {
                        ls
                    }
                }
            }
            _ => {
                is_new_or_empty = true;
                vec![String::new()]
            }
        };

        let mut app = Self {
            config: Config::load(&cli),
            lines,
            types: vec![],
            layout: vec![],
            file: path,
            dirty: false,
            cursor_y: 0,
            cursor_x: 0,
            target_visual_x: 0,
            visible_height: 0,
            scroll: 0,
            characters: HashSet::new(),
            locations: HashSet::new(),
            suggestion: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit: LastEdit::None,

            mode: AppMode::Normal,
            exit_after_save: false,
            filename_input: String::new(),

            status_msg: None,
            cut_buffer: None,
            search_query: String::new(),
            last_search: String::new(),
            show_search_highlight: false,
            compiled_search_regex: None,
        };

        if is_new_or_empty && app.config.auto_title_page {
            app.lines = vec![
                "Title: Untitled".to_string(),
                "Credit: Written by".to_string(),
                "Author: Author Name".to_string(),
                "Draft date: ".to_string(),
                "Contact: ".to_string(),
                "".to_string(),
                "".to_string(),
            ];
            app.cursor_y = app.lines.len() - 1;
            app.cursor_x = 0;
            app.dirty = true;
        }

        app.parse_document();
        app.update_autocomplete();
        app.update_layout();
        app.target_visual_x = app.current_visual_x();
        app
    }

    pub fn set_status(&mut self, msg: &str) {
        self.status_msg = Some(msg.to_string());
    }

    pub fn clear_status(&mut self) {
        self.status_msg = None;
    }

    pub fn update_search_regex(&mut self) {
        let active_query = if self.search_query.is_empty() {
            &self.last_search
        } else {
            &self.search_query
        };

        if active_query.is_empty() {
            self.compiled_search_regex = None;
        } else {
            self.compiled_search_regex = regex::RegexBuilder::new(&regex::escape(active_query))
                .case_insensitive(true)
                .build()
                .ok();
        }
    }

    pub fn report_cursor_position(&mut self) {
        if self.lines.is_empty() {
            self.set_status("line 1/1 (100%), col 1/1 (100%), char 1/1 (100%)");
            return;
        }

        let total_lines = self.lines.len().max(1);
        let cur_line = self.cursor_y + 1;
        let line_pct = (cur_line as f64 / total_lines as f64 * 100.0) as usize;

        let current_line_text = self
            .lines
            .get(self.cursor_y)
            .map(|s| s.as_str())
            .unwrap_or("");
        let total_cols = current_line_text.chars().count() + 1;
        let cur_col = self.cursor_x + 1;
        let col_pct = (cur_col as f64 / total_cols as f64 * 100.0) as usize;

        let total_chars: usize = self
            .lines
            .iter()
            .map(|l| l.chars().count() + 1)
            .sum::<usize>()
            .max(1);

        let cur_char = self.lines[..self.cursor_y]
            .iter()
            .map(|l| l.chars().count() + 1)
            .sum::<usize>()
            + self.cursor_x
            + 1;

        let char_pct = (cur_char as f64 / total_chars as f64 * 100.0) as usize;

        let msg = format!(
            "line {}/{} ({}%), col {}/{} ({}%), char {}/{} ({}%)",
            cur_line,
            total_lines,
            line_pct,
            cur_col,
            total_cols,
            col_pct,
            cur_char,
            total_chars,
            char_pct
        );
        self.set_status(&msg);
    }

    pub fn cut_line(&mut self) {
        if self.last_edit != LastEdit::Cut {
            self.save_state(true);
        }

        if self.cursor_y < self.lines.len() {
            let cut_line = self.lines.remove(self.cursor_y);

            if self.last_edit == LastEdit::Cut {
                if let Some(buf) = &mut self.cut_buffer {
                    buf.push('\n');
                    buf.push_str(&cut_line);
                }
            } else {
                self.cut_buffer = Some(cut_line);
            }
            self.last_edit = LastEdit::Cut;

            if self.lines.is_empty() {
                self.lines.push(String::new());
            }
            if self.cursor_y >= self.lines.len() {
                self.cursor_y = self.lines.len().saturating_sub(1);
                self.cursor_x = self.line_len(self.cursor_y);
            } else {
                self.cursor_x = 0;
            }
            self.dirty = true;
        }
    }

    pub fn paste_line(&mut self) {
        if let Some(cut_buf) = self.cut_buffer.clone() {
            self.save_state(true);
            let lines_to_paste: Vec<&str> = cut_buf.split('\n').collect();
            for (i, l) in lines_to_paste.iter().enumerate() {
                self.lines
                    .insert(self.cursor_y + i, l.replace('\t', "    "));
            }
            self.cursor_y += lines_to_paste.len();
            self.cursor_x = 0;
            self.dirty = true;
            self.last_edit = LastEdit::Other;
        }
    }

    pub fn execute_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_query = self.last_search.clone();
        }
        if self.search_query.is_empty() {
            self.mode = AppMode::Normal;
            self.set_status("Cancelled");
            self.show_search_highlight = false;
            self.compiled_search_regex = None;
            return;
        }
        self.last_search = self.search_query.clone();
        self.update_search_regex();

        let re = self.compiled_search_regex.as_ref().unwrap();

        let mut wrapped = false;
        let mut found = false;
        let start_y = self.cursor_y;
        let start_char_x = self.cursor_x;

        for i in 0..=self.lines.len() {
            let y = (start_y + i) % self.lines.len();
            let line = &self.lines[y];

            for mat in re.find_iter(line) {
                let char_idx = line[..mat.start()].chars().count();

                if i == 0 && char_idx <= start_char_x {
                    continue;
                }

                if i == self.lines.len() && char_idx > start_char_x {
                    continue;
                }

                self.cursor_y = y;
                self.cursor_x = char_idx;
                found = true;

                if y < start_y || (y == start_y && i > 0) {
                    wrapped = true;
                }
                break;
            }
            if found {
                break;
            }
        }

        self.mode = AppMode::Normal;

        if !found {
            self.set_status(&format!("\"{}\" not found", self.search_query));
            self.show_search_highlight = false;
        } else {
            self.show_search_highlight = true;
            if wrapped {
                self.set_status("Search Wrapped");
            } else {
                self.clear_status();
            }
        }

        self.search_query.clear();
    }

    pub fn save_state(&mut self, force: bool) {
        let state = HistoryState {
            lines: self.lines.clone(),
            cursor_y: self.cursor_y,
            cursor_x: self.cursor_x,
        };
        if force
            || self
                .undo_stack
                .last()
                .is_none_or(|last| last.lines != state.lines)
        {
            self.undo_stack.push(state);
            if self.undo_stack.len() > 500 {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
    }

    pub fn undo(&mut self) -> bool {
        if let Some(state) = self.undo_stack.pop() {
            self.redo_stack.push(HistoryState {
                lines: self.lines.clone(),
                cursor_y: self.cursor_y,
                cursor_x: self.cursor_x,
            });
            self.lines = state.lines;
            self.cursor_y = state.cursor_y;
            self.cursor_x = state.cursor_x;
            self.dirty = true;
            self.last_edit = LastEdit::None;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(state) = self.redo_stack.pop() {
            self.undo_stack.push(HistoryState {
                lines: self.lines.clone(),
                cursor_y: self.cursor_y,
                cursor_x: self.cursor_x,
            });
            self.lines = state.lines;
            self.cursor_y = state.cursor_y;
            self.cursor_x = state.cursor_x;
            self.dirty = true;
            self.last_edit = LastEdit::None;
            true
        } else {
            false
        }
    }

    pub fn parse_document(&mut self) {
        self.types = Parser::parse(&self.lines);
        self.characters.clear();
        self.locations.clear();

        for (i, t) in self.types.iter().enumerate() {
            if *t == LineType::Character || *t == LineType::DualDialogueCharacter {
                let full_name = self.lines[i]
                    .trim_start_matches('@')
                    .trim_end_matches('^')
                    .trim();
                let name = if let Some(idx) = full_name.find('(') {
                    full_name[..idx].trim()
                } else {
                    full_name
                };
                if !name.is_empty() {
                    self.characters.insert(name.to_uppercase());
                }
            } else if *t == LineType::SceneHeading {
                let scene = self.lines[i].trim().to_uppercase();
                if let Some(idx) = scene.find(' ') {
                    let loc = scene[idx + 1..].trim().to_string();
                    if !loc.is_empty() {
                        self.locations.insert(loc);
                    }
                }
            }
        }
    }

    pub fn update_layout(&mut self) {
        self.layout = build_layout(&self.lines, &self.types, self.cursor_y, &self.config);
    }

    pub fn current_visual_x(&self) -> u16 {
        let (_, vis_x) = find_visual_cursor(&self.layout, self.cursor_y, self.cursor_x);
        vis_x
    }

    pub fn update_autocomplete(&mut self) {
        self.suggestion = None;
        if !self.config.autocomplete {
            return;
        }

        if self.cursor_y >= self.lines.len() {
            return;
        }

        let line = &self.lines[self.cursor_y];
        let char_count = line.chars().count();

        if self.cursor_x != char_count || char_count == 0 {
            return;
        }

        let is_char_type = matches!(
            self.types.get(self.cursor_y),
            Some(LineType::Character) | Some(LineType::DualDialogueCharacter)
        );
        let upper_line = line.to_uppercase();

        if is_char_type || upper_line.starts_with('@') {
            let input = upper_line.trim_start_matches('@').trim_start();
            if !input.is_empty() {
                let mut best_match: Option<&String> = None;
                for c in &self.characters {
                    if c.starts_with(input)
                        && c != input
                        && (best_match.is_none() || c.len() < best_match.unwrap().len())
                    {
                        best_match = Some(c);
                    }
                }
                if let Some(c) = best_match {
                    self.suggestion = Some(c[input.len()..].to_string());
                    return;
                }
            }
        }

        let trim_line = upper_line.trim_start();
        let scene_prefixes = [
            "INT. ", "EXT. ", "EST. ", "I/E. ", "E/I. ", "I./E. ", "E./I. ",
        ];
        for prefix in scene_prefixes {
            if let Some(input) = trim_line.strip_prefix(prefix)
                && !input.is_empty()
            {
                let mut best_match: Option<&String> = None;
                for loc in &self.locations {
                    if loc.starts_with(input)
                        && loc != input
                        && (best_match.is_none() || loc.len() < best_match.unwrap().len())
                    {
                        best_match = Some(loc);
                    }
                }
                if let Some(loc) = best_match {
                    self.suggestion = Some(loc[input.len()..].to_string());
                    return;
                }
            }
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(ref p) = self.file {
            let mut content = self.lines.join("\n");
            if !content.ends_with('\n') {
                content.push('\n');
            }
            fs::write(p, content)?;
            self.dirty = false;
            self.set_status(&format!("Wrote {} lines", self.lines.len()));
        }
        Ok(())
    }

    pub fn line_len(&self, y: usize) -> usize {
        self.lines.get(y).map(|l| l.chars().count()).unwrap_or(0)
    }

    pub fn move_up(&mut self) {
        self.last_edit = LastEdit::Other;
        let (vis_row, _) = find_visual_cursor(&self.layout, self.cursor_y, self.cursor_x);
        if vis_row > 0 {
            let mut target_vi = vis_row - 1;
            while target_vi > 0 && self.layout[target_vi].is_phantom {
                target_vi -= 1;
            }
            let target_row = &self.layout[target_vi];
            let is_last = target_row.char_end == self.line_len(target_row.line_idx);
            self.cursor_y = target_row.line_idx;
            self.cursor_x = target_row.visual_to_logical_x(self.target_visual_x, is_last);
        } else {
            self.cursor_y = 0;
            self.cursor_x = 0;
        }
    }

    pub fn move_down(&mut self) {
        self.last_edit = LastEdit::Other;
        let (vis_row, _) = find_visual_cursor(&self.layout, self.cursor_y, self.cursor_x);
        if vis_row + 1 < self.layout.len() {
            let mut target_vi = vis_row + 1;
            while target_vi + 1 < self.layout.len() && self.layout[target_vi].is_phantom {
                target_vi += 1;
            }
            let target_row = &self.layout[target_vi];
            let is_last = target_row.char_end == self.line_len(target_row.line_idx);
            self.cursor_y = target_row.line_idx;
            self.cursor_x = target_row.visual_to_logical_x(self.target_visual_x, is_last);
        } else {
            self.cursor_y = self.lines.len() - 1;
            self.cursor_x = self.line_len(self.cursor_y);
        }
    }

    pub fn move_left(&mut self) {
        self.last_edit = LastEdit::Other;
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = self.line_len(self.cursor_y);
        }
    }

    pub fn move_right(&mut self) {
        self.last_edit = LastEdit::Other;
        let max = self.line_len(self.cursor_y);
        if self.cursor_x < max {
            self.cursor_x += 1;
        } else if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.cursor_x = 0;
        }
    }

    pub fn move_word_left(&mut self) {
        self.last_edit = LastEdit::Other;
        if self.cursor_x == 0 {
            self.move_left();
            return;
        }
        let chars: Vec<char> = self.lines[self.cursor_y].chars().collect();
        while self.cursor_x > 0 && chars[self.cursor_x - 1].is_whitespace() {
            self.cursor_x -= 1;
        }
        while self.cursor_x > 0 && !chars[self.cursor_x - 1].is_whitespace() {
            self.cursor_x -= 1;
        }
    }

    pub fn move_word_right(&mut self) {
        self.last_edit = LastEdit::Other;
        let chars: Vec<char> = self.lines[self.cursor_y].chars().collect();
        let max = chars.len();
        if self.cursor_x == max {
            self.move_right();
            return;
        }
        while self.cursor_x < max && chars[self.cursor_x].is_whitespace() {
            self.cursor_x += 1;
        }
        while self.cursor_x < max && !chars[self.cursor_x].is_whitespace() {
            self.cursor_x += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.last_edit = LastEdit::Other;
        self.cursor_x = 0;
    }

    pub fn move_end(&mut self) {
        self.last_edit = LastEdit::Other;
        self.cursor_x = self.line_len(self.cursor_y);
    }

    pub fn move_page_up(&mut self) {
        let height = self.visible_height.max(1);
        for _ in 0..height {
            self.move_up();
        }
    }

    pub fn move_page_down(&mut self) {
        let height = self.visible_height.max(1);
        for _ in 0..height {
            self.move_down();
        }
    }

    pub fn byte_of(&self, y: usize, cx: usize) -> usize {
        self.lines[y]
            .char_indices()
            .nth(cx)
            .map(|(b, _)| b)
            .unwrap_or(self.lines[y].len())
    }

    pub fn insert_char(&mut self, c: char) {
        if self.last_edit != LastEdit::Insert || c.is_whitespace() || ".,;?!()[]*".contains(c) {
            self.save_state(true);
        }
        self.last_edit = LastEdit::Insert;

        let b = self.byte_of(self.cursor_y, self.cursor_x);
        self.lines[self.cursor_y].insert(b, c);
        let new_b = b + c.len_utf8();
        self.cursor_x += 1;

        if c == '(' && self.config.match_parentheses {
            self.lines[self.cursor_y].insert(new_b, ')');
        } else if c == '[' && self.config.close_elements {
            if self.lines[self.cursor_y][..new_b].ends_with("[[") {
                self.lines[self.cursor_y].insert_str(new_b, "]]");
            }
        } else if c == '*' && self.config.close_elements {
            if self.lines[self.cursor_y][..new_b].ends_with("/*") {
                self.lines[self.cursor_y].insert_str(new_b, "*/");
            } else if self.lines[self.cursor_y][..new_b].ends_with("**") {
                self.lines[self.cursor_y].insert_str(new_b, "**");
            }
        }

        self.dirty = true;
    }

    pub fn insert_newline(&mut self, is_shift: bool) {
        if is_shift {
            let b = self.byte_of(self.cursor_y, self.cursor_x);
            let tail = self.lines[self.cursor_y].split_off(b);
            self.lines.insert(self.cursor_y + 1, tail);
            self.cursor_y += 1;
            self.cursor_x = 0;
            self.dirty = true;
            return;
        }

        self.save_state(true);
        self.last_edit = LastEdit::Other;

        let t = self
            .types
            .get(self.cursor_y)
            .copied()
            .unwrap_or(LineType::Empty);

        let is_smart_element = matches!(
            t,
            LineType::Parenthetical | LineType::Character | LineType::DualDialogueCharacter
        );

        if is_smart_element {
            let b = self.byte_of(self.cursor_y, self.cursor_x);
            let line = &self.lines[self.cursor_y];
            let remainder = &line[b..];
            let trim_rem = remainder.trim();

            if trim_rem.is_empty() || trim_rem == ")" {
                self.lines.insert(self.cursor_y + 1, String::new());
                self.cursor_y += 1;
                self.cursor_x = 0;
                self.dirty = true;
                return;
            }
        }

        let b = self.byte_of(self.cursor_y, self.cursor_x);
        let tail = self.lines[self.cursor_y].split_off(b);
        let head_is_empty = self.lines[self.cursor_y].is_empty();

        let breaks_paragraph = matches!(
            t,
            LineType::Action
                | LineType::SceneHeading
                | LineType::Transition
                | LineType::Section
                | LineType::Synopsis
                | LineType::Shot
                | LineType::Boneyard
                | LineType::Dialogue
                | LineType::Centered
                | LineType::PageBreak
        );

        if self.config.auto_paragraph_breaks && breaks_paragraph && !head_is_empty {
            if tail.trim().is_empty() {
                self.lines.insert(self.cursor_y + 1, String::new());
                self.lines.insert(self.cursor_y + 2, String::new());
                self.cursor_y += 2;
            } else {
                self.lines.insert(self.cursor_y + 1, String::new());
                self.lines.insert(self.cursor_y + 2, String::new());
                self.lines.insert(self.cursor_y + 3, String::new());
                self.lines
                    .insert(self.cursor_y + 4, tail.trim_start().to_string());
                self.cursor_y += 2;
            }
        } else {
            self.lines.insert(self.cursor_y + 1, tail);
            self.cursor_y += 1;
        }

        self.cursor_x = 0;
        self.dirty = true;
    }

    pub fn handle_tab(&mut self) {
        if let Some(sug) = self.suggestion.take() {
            self.save_state(true);
            self.last_edit = LastEdit::Other;
            let b = self.byte_of(self.cursor_y, self.cursor_x);
            self.lines[self.cursor_y].insert_str(b, &sug);
            self.cursor_x += sug.chars().count();
            self.dirty = true;
            return;
        }

        self.save_state(true);
        self.last_edit = LastEdit::Other;

        let lt = self.types[self.cursor_y];
        let line = self.lines[self.cursor_y].clone();
        let trim = line.trim();
        let prev_t = if self.cursor_y > 0 {
            self.types[self.cursor_y - 1]
        } else {
            LineType::Empty
        };

        if trim.is_empty() {
            if matches!(
                prev_t,
                LineType::Character
                    | LineType::DualDialogueCharacter
                    | LineType::Dialogue
                    | LineType::Parenthetical
            ) {
                self.lines[self.cursor_y] = "()".to_string();
                self.cursor_x = 1;
            } else {
                self.lines[self.cursor_y] = "@".to_string();
                self.cursor_x = 1;
            }
        } else if trim == "()" {
            self.lines[self.cursor_y] = "@".to_string();
            self.cursor_x = 1;
        } else if trim == "@" {
            self.lines[self.cursor_y] = ".".to_string();
            self.cursor_x = 1;
        } else if trim == "." {
            self.lines[self.cursor_y] = ">".to_string();
            self.cursor_x = 1;
        } else if trim == ">" {
            self.lines[self.cursor_y] = String::new();
            self.cursor_x = 0;
        } else if lt == LineType::Action {
            if line.starts_with('!')
                || line.starts_with('~')
                || line.starts_with('=')
                || line.starts_with('#')
            {
                let stripped = line.trim_start_matches(['!', '~', '=', '#']);
                self.lines[self.cursor_y] = stripped.to_string();
                self.cursor_x = self.cursor_x.saturating_sub(line.len() - stripped.len());
            } else if !line.starts_with('@') {
                self.lines[self.cursor_y].insert(0, '@');
                self.cursor_x += 1;
            }
        } else if matches!(
            lt,
            LineType::Shot | LineType::Lyrics | LineType::Synopsis | LineType::Section
        ) {
            let stripped = line.trim_start_matches(['!', '~', '=', '#']);
            self.lines[self.cursor_y] = stripped.to_string();
            self.cursor_x = self.cursor_x.saturating_sub(line.len() - stripped.len());
        } else if lt == LineType::Character || lt == LineType::DualDialogueCharacter {
            if line.starts_with('@') {
                self.lines[self.cursor_y] = line.replacen('@', ".", 1);
            } else {
                self.lines[self.cursor_y].insert(0, '.');
                self.cursor_x += 1;
            }
        } else if lt == LineType::Dialogue {
            self.lines[self.cursor_y] = format!("({})", trim);
            self.cursor_x = self.lines[self.cursor_y].chars().count() - 1;
        } else if lt == LineType::Parenthetical {
            if trim.starts_with('(') && trim.ends_with(')') {
                self.lines[self.cursor_y] = trim[1..trim.len() - 1].to_string();
                self.cursor_x = self.lines[self.cursor_y].chars().count();
            } else if line.starts_with('(') {
                let mut s = line.replacen('(', "", 1);
                if let Some(idx) = s.rfind(')') {
                    s.remove(idx);
                }
                self.lines[self.cursor_y] = s;
                self.cursor_x = self.cursor_x.saturating_sub(1);
            }
        } else if lt == LineType::SceneHeading {
            if line.starts_with('.') {
                self.lines[self.cursor_y] = line.replacen('.', ">", 1);
            } else {
                self.lines[self.cursor_y].insert(0, '>');
                self.cursor_x += 1;
            }
        } else if lt == LineType::Transition
            && line.starts_with('>')
            && let Some(stripped) = line.strip_prefix('>')
        {
            self.lines[self.cursor_y] = stripped.to_string();
            self.cursor_x = self.cursor_x.saturating_sub(1);
        } else if line.starts_with('!')
            || line.starts_with('~')
            || line.starts_with('=')
            || line.starts_with('#')
        {
            let stripped = line.trim_start_matches(['!', '~', '=', '#']);
            self.lines[self.cursor_y] = stripped.to_string();
            self.cursor_x = self.cursor_x.saturating_sub(line.len() - stripped.len());
        }
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.last_edit != LastEdit::Delete {
            self.save_state(true);
        }
        self.last_edit = LastEdit::Delete;

        if self.cursor_x > 0 {
            let line = &self.lines[self.cursor_y];
            let cx = self.cursor_x;

            if cx >= 1 && cx < line.chars().count() {
                let bytes = line.char_indices().map(|(b, _)| b).collect::<Vec<_>>();
                let char_idx = cx;
                if let (Some(&b1), Some(&b2)) = (
                    bytes.get(char_idx - 1),
                    bytes.get(char_idx + 1).or(Some(&line.len())),
                ) {
                    let pair = &line[b1..b2];
                    if pair == "()" {
                        self.lines[self.cursor_y].replace_range(b1..b2, "");
                        self.cursor_x -= 1;
                        self.dirty = true;
                        return;
                    }
                }
            }
            if cx >= 2 && cx + 1 < line.chars().count() {
                let chars: String = line.chars().skip(cx - 2).take(4).collect();
                if chars == "[[]]" || chars == "/**/" || chars == "****" {
                    let b_start = self.byte_of(self.cursor_y, cx - 2);
                    let b_end = self.byte_of(self.cursor_y, cx + 2);
                    self.lines[self.cursor_y].replace_range(b_start..b_end, "");
                    self.cursor_x -= 2;
                    self.dirty = true;
                    return;
                }
            }

            let b = self.byte_of(self.cursor_y, self.cursor_x - 1);
            self.lines[self.cursor_y].remove(b);
            self.cursor_x -= 1;
            self.dirty = true;
        } else if self.cursor_y > 0 {
            let tail = self.lines.remove(self.cursor_y);
            self.cursor_y -= 1;
            self.cursor_x = self.line_len(self.cursor_y);
            self.lines[self.cursor_y].push_str(&tail);
            self.dirty = true;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.last_edit != LastEdit::Delete {
            self.save_state(true);
        }
        self.last_edit = LastEdit::Delete;

        let line = &self.lines[self.cursor_y];
        let cx = self.cursor_x;

        if cx > 0 && cx + 1 < line.chars().count() {
            let chars: String = line.chars().skip(cx - 1).take(2).collect();
            if chars == "()" {
                let b_start = self.byte_of(self.cursor_y, cx - 1);
                let b_end = self.byte_of(self.cursor_y, cx + 1);
                self.lines[self.cursor_y].replace_range(b_start..b_end, "");
                self.cursor_x -= 1;
                self.dirty = true;
                return;
            }
        }
        if cx + 3 < line.chars().count() {
            let chars: String = line.chars().skip(cx).take(4).collect();
            if chars == "[[]]" || chars == "/**/" || chars == "****" {
                let b_start = self.byte_of(self.cursor_y, cx);
                let b_end = self.byte_of(self.cursor_y, cx + 4);
                self.lines[self.cursor_y].replace_range(b_start..b_end, "");
                self.dirty = true;
                return;
            }
        }

        let max = self.line_len(self.cursor_y);
        if self.cursor_x < max {
            let b = self.byte_of(self.cursor_y, self.cursor_x);
            self.lines[self.cursor_y].remove(b);
            self.dirty = true;
        } else if self.cursor_y + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_y + 1);
            self.lines[self.cursor_y].push_str(&next);
            self.dirty = true;
        }
    }

    pub fn delete_word_back(&mut self) {
        if self.cursor_x == 0 {
            self.backspace();
            return;
        }
        self.save_state(true);
        self.last_edit = LastEdit::Other;

        let mut chars: Vec<char> = self.lines[self.cursor_y].chars().collect();
        while self.cursor_x > 0 && chars[self.cursor_x - 1].is_whitespace() {
            self.cursor_x -= 1;
            chars.remove(self.cursor_x);
        }
        while self.cursor_x > 0 && !chars[self.cursor_x - 1].is_whitespace() {
            self.cursor_x -= 1;
            chars.remove(self.cursor_x);
        }
        self.lines[self.cursor_y] = chars.into_iter().collect();
        self.dirty = true;
    }

    pub fn delete_word_forward(&mut self) {
        let mut chars: Vec<char> = self.lines[self.cursor_y].chars().collect();
        if self.cursor_x == chars.len() {
            self.delete_forward();
            return;
        }
        self.save_state(true);
        self.last_edit = LastEdit::Other;

        while self.cursor_x < chars.len() && chars[self.cursor_x].is_whitespace() {
            chars.remove(self.cursor_x);
        }
        while self.cursor_x < chars.len() && !chars[self.cursor_x].is_whitespace() {
            chars.remove(self.cursor_x);
        }
        self.lines[self.cursor_y] = chars.into_iter().collect();
        self.dirty = true;
    }

    pub fn handle_event(
        &mut self,
        ev: Event,
        update_target_x: &mut bool,
        text_changed: &mut bool,
        cursor_moved: &mut bool,
    ) -> io::Result<bool> {
        if let Event::Mouse(mouse_event) = ev {
            self.clear_status();
            if self.show_search_highlight {
                self.show_search_highlight = false;
            }

            match mouse_event.kind {
                MouseEventKind::ScrollUp => {
                    self.move_up();
                    *cursor_moved = true;
                }
                MouseEventKind::ScrollDown => {
                    self.move_down();
                    *cursor_moved = true;
                }
                _ => {}
            }
            return Ok(false);
        }

        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }

            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT);
            let shift = key.modifiers.contains(KeyModifiers::SHIFT);

            match self.mode {
                AppMode::Search => {
                    match key.code {
                        KeyCode::Esc => {
                            self.mode = AppMode::Normal;
                            self.set_status("Cancelled");
                            self.show_search_highlight = false;
                            self.search_query.clear();
                        }
                        KeyCode::Char('c') | KeyCode::Char('g') if ctrl => {
                            self.mode = AppMode::Normal;
                            self.set_status("Cancelled");
                            self.show_search_highlight = false;
                            self.search_query.clear();
                        }
                        KeyCode::Enter => {
                            self.execute_search();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Backspace => {
                            self.search_query.pop();
                            self.update_search_regex();
                        }
                        KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                            self.search_query.push(c);
                            self.update_search_regex();
                        }
                        _ => {}
                    }
                    return Ok(false);
                }
                AppMode::PromptSave => {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') if !ctrl => {
                            self.filename_input = self
                                .file
                                .as_ref()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            self.mode = AppMode::PromptFilename;
                            self.exit_after_save = true;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') if !ctrl => {
                            return Ok(true);
                        }
                        KeyCode::Esc => {
                            self.mode = AppMode::Normal;
                            self.set_status("Cancelled");
                        }
                        KeyCode::Char('c') | KeyCode::Char('g') if ctrl => {
                            self.mode = AppMode::Normal;
                            self.set_status("Cancelled");
                        }
                        _ => {}
                    }
                    return Ok(false);
                }
                AppMode::PromptFilename => {
                    match key.code {
                        KeyCode::Esc => {
                            self.mode = AppMode::Normal;
                            self.set_status("Cancelled");
                        }
                        KeyCode::Char('c') | KeyCode::Char('g') if ctrl => {
                            self.mode = AppMode::Normal;
                            self.set_status("Cancelled");
                        }
                        KeyCode::Enter => {
                            if !self.filename_input.trim().is_empty() {
                                self.file = Some(PathBuf::from(self.filename_input.trim()));
                                match self.save() {
                                    Ok(_) => {
                                        if self.exit_after_save {
                                            return Ok(true);
                                        }
                                        self.mode = AppMode::Normal;
                                    }
                                    Err(e) => {
                                        self.set_status(&format!("Error saving: {}", e));
                                        self.mode = AppMode::Normal;
                                    }
                                }
                            } else {
                                self.set_status("Cancelled");
                                self.mode = AppMode::Normal;
                            }
                        }
                        KeyCode::Backspace => {
                            self.filename_input.pop();
                        }
                        KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                            self.filename_input.push(c);
                        }
                        _ => {}
                    }
                    return Ok(false);
                }
                AppMode::Normal => {
                    self.clear_status();

                    if self.show_search_highlight {
                        match key.code {
                            KeyCode::Char('w') if ctrl => {}
                            KeyCode::Char('c') if ctrl => {}
                            _ => {
                                self.show_search_highlight = false;
                                *text_changed = true;
                            }
                        }
                    }

                    match key.code {
                        KeyCode::Esc => {}
                        KeyCode::Char('x') if ctrl => {
                            if self.dirty {
                                self.mode = AppMode::PromptSave;
                            } else {
                                return Ok(true);
                            }
                        }
                        KeyCode::Char('s') if ctrl => {
                            if self.file.is_some() {
                                self.save()?;
                            } else {
                                self.filename_input.clear();
                                self.mode = AppMode::PromptFilename;
                                self.exit_after_save = false;
                            }
                        }
                        KeyCode::Char('z') if ctrl => {
                            if self.undo() {
                                self.set_status("Undo applied");
                                *update_target_x = true;
                                *text_changed = true;
                                *cursor_moved = true;
                            } else {
                                self.set_status("Nothing to undo");
                            }
                        }
                        KeyCode::Char('r') if ctrl => {
                            if self.redo() {
                                self.set_status("Redo applied");
                                *update_target_x = true;
                                *text_changed = true;
                                *cursor_moved = true;
                            } else {
                                self.set_status("Nothing to redo");
                            }
                        }
                        KeyCode::Char('k') if ctrl => {
                            self.cut_line();
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Char('u') if ctrl => {
                            self.paste_line();
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Char('w') if ctrl => {
                            self.mode = AppMode::Search;
                            self.search_query.clear();
                            self.show_search_highlight = true;
                            self.update_search_regex();
                        }
                        KeyCode::Char('c') if ctrl => {
                            self.report_cursor_position();
                        }
                        KeyCode::Up => {
                            self.move_up();
                            *cursor_moved = true;
                        }
                        KeyCode::Down => {
                            self.move_down();
                            *cursor_moved = true;
                        }
                        KeyCode::PageUp => {
                            self.move_page_up();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::PageDown => {
                            self.move_page_down();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Left if ctrl => {
                            self.move_word_left();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Right if ctrl => {
                            self.move_word_right();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Left => {
                            self.move_left();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Right => {
                            self.move_right();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Home => {
                            self.move_home();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::End => {
                            self.move_end();
                            *update_target_x = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Enter => {
                            self.suggestion = None;
                            self.insert_newline(shift);
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Backspace if ctrl => {
                            self.delete_word_back();
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Backspace => {
                            self.backspace();
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Delete if ctrl => {
                            self.delete_word_forward();
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Delete => {
                            self.delete_forward();
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Tab => {
                            self.handle_tab();
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        KeyCode::Char(c) if !ctrl => {
                            self.insert_char(c);
                            *update_target_x = true;
                            *text_changed = true;
                            *cursor_moved = true;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(false)
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    f.render_widget(ratatui::widgets::Clear, area);

    let is_prompt = app.mode != AppMode::Normal;
    let has_status = app.status_msg.is_some();

    let show_top = !app.config.focus_mode;
    let show_bottom = !app.config.focus_mode || is_prompt || has_status;

    let title_height = if show_top { 1 } else { 0 };
    let status_height = if show_bottom { 1 } else { 0 };
    let shortcut_height = if show_bottom { 2 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(title_height),
            Constraint::Min(0),
            Constraint::Length(status_height),
            Constraint::Length(shortcut_height),
        ])
        .split(area);

    let (title_area, text_area, status_area, shortcut_area) =
        (chunks[0], chunks[1], chunks[2], chunks[3]);

    let height = text_area.height as usize;
    app.visible_height = height;
    let page_w = PAGE_WIDTH.min(text_area.width);
    let global_pad = text_area.width.saturating_sub(page_w) / 2;

    let (vis_row, vis_x) = find_visual_cursor(&app.layout, app.cursor_y, app.cursor_x);

    let mut pad_top = 0;

    if app.config.strict_typewriter_mode {
        let absolute_center = area.height / 2;
        let center_offset = absolute_center.saturating_sub(text_area.y) as usize;
        if vis_row < center_offset {
            pad_top = center_offset - vis_row;
        }
        app.scroll = vis_row.saturating_sub(center_offset);
    } else if app.config.typewriter_mode {
        let absolute_center = area.height / 2;
        let center_offset = absolute_center.saturating_sub(text_area.y) as usize;
        app.scroll = vis_row.saturating_sub(center_offset);
    } else {
        if vis_row < app.scroll {
            app.scroll = vis_row;
        }
        if vis_row >= app.scroll + height {
            app.scroll = vis_row + 1 - height;
        }
    }

    let mut dark_gray_style = Style::default();
    if !app.config.no_color {
        dark_gray_style.fg = Some(Color::DarkGray);
    }

    let mut sug_style = Style::default();
    if !app.config.no_formatting {
        sug_style = sug_style.add_modifier(Modifier::DIM);
    }
    if !app.config.no_color {
        sug_style.fg = Some(Color::DarkGray);
    }

    let mut page_num_style = Style::default();
    if !app.config.no_formatting {
        page_num_style = page_num_style.add_modifier(Modifier::BOLD);
    }
    if !app.config.no_color {
        page_num_style.fg = Some(Color::DarkGray);
    }

    let panel_style = Style::default().add_modifier(Modifier::REVERSED);

    let mut visible: Vec<Line> = Vec::new();
    for _ in 0..pad_top {
        visible.push(Line::raw(""));
    }

    let mut active_action_idx = None;
    if app.config.highlight_active_action {
        let mut check_y = app.cursor_y;
        while check_y < app.types.len() {
            match app.types[check_y] {
                LineType::Action => {
                    active_action_idx = Some(check_y);
                    break;
                }
                LineType::Empty => {
                    if check_y == 0 {
                        break;
                    }
                    check_y -= 1;
                }
                _ => break,
            }
        }
    }

    visible.extend(
        app.layout
            .iter()
            .skip(app.scroll)
            .take(height.saturating_sub(pad_top))
            .map(|row| {
                let mut spans = Vec::new();
                let gap_size = 6u16;

                if let Some(snum) = row.scene_num {
                    let s_str = format!("{}", snum);
                    let s_len = s_str.len() as u16;

                    if global_pad >= s_len + gap_size {
                        let pad = global_pad - s_len - gap_size;
                        spans.push(Span::raw(" ".repeat(pad as usize)));
                        spans.push(Span::styled(s_str, dark_gray_style));
                        spans.push(Span::raw(" ".repeat(gap_size as usize)));
                    } else {
                        spans.push(Span::styled(s_str, dark_gray_style));
                        spans.push(Span::raw(" "));
                    }
                } else {
                    spans.push(Span::raw(" ".repeat(global_pad as usize)));
                }

                spans.push(Span::raw(" ".repeat(row.indent as usize)));

                let mut bst = base_style(row.line_type, &app.config);
                if app.config.highlight_active_action
                    && Some(row.line_idx) == active_action_idx
                    && !app.config.no_color
                {
                    bst.fg = Some(Color::White);
                } else if let Some(c) = row.override_color
                    && !app.config.no_color
                {
                    bst.fg = Some(c);
                }

                let mut display = if row.is_active {
                    row.raw_text.clone()
                } else {
                    strip_sigils(&row.raw_text, row.line_type).to_string()
                };

                let reveal_markup = !app.config.hide_markup
                    || row.is_active
                    || row.raw_text.contains("/*")
                    || row.raw_text.contains("*/");
                let skip_md = row.line_type == LineType::Boneyard;

                if row.line_type == LineType::SceneHeading || row.line_type == LineType::Transition
                {
                    display = display.to_uppercase();
                } else if row.line_type == LineType::Character
                    || row.line_type == LineType::DualDialogueCharacter
                {
                    if let Some(idx) = display.find('(') {
                        let name = display[..idx].to_uppercase();
                        let ext = &display[idx..];
                        display = format!("{}{}", name, ext);
                    } else {
                        display = display.to_uppercase();
                    }
                }

                let empty_logical_line = String::new();
                let full_logical_line = app.lines.get(row.line_idx).unwrap_or(&empty_logical_line);

                let is_last_visual_row = row.char_end == full_logical_line.chars().count();
                let mut meta_key_end = 0;

                if (row.line_type == LineType::MetadataKey
                    || (row.line_type == LineType::MetadataTitle && row.is_active))
                    && let Some(idx) = full_logical_line.find(':')
                {
                    meta_key_end = full_logical_line[..=idx].chars().count() + 1;
                }

                let mut row_highlights = HashSet::new();
                if app.show_search_highlight
                    && let Some(re) = &app.compiled_search_regex
                {
                    for mat in re.find_iter(full_logical_line) {
                        let start_byte = mat.start();
                        let end_byte = mat.end();

                        let char_start = full_logical_line[..start_byte].chars().count();
                        let char_len = full_logical_line[start_byte..end_byte].chars().count();

                        for idx in char_start..(char_start + char_len) {
                            row_highlights.insert(idx);
                        }
                    }
                }

                spans.extend(render_inline(
                    &display,
                    bst,
                    &row.fmt,
                    RenderConfig {
                        reveal_markup,
                        skip_markdown: skip_md,
                        exclude_comments: false,
                        char_offset: row.char_start,
                        meta_key_end,
                        no_color: app.config.no_color,
                        no_formatting: app.config.no_formatting,
                    },
                    &row_highlights,
                ));

                if row.is_active
                    && row.line_idx == app.cursor_y
                    && is_last_visual_row
                    && let Some(sug) = &app.suggestion
                {
                    spans.push(Span::styled(sug.clone(), sug_style));
                }

                if let Some(pnum) = row.page_num {
                    let current_line_width: usize = spans
                        .iter()
                        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                        .sum();

                    let target_pos = global_pad as usize + page_w as usize + gap_size as usize;
                    if target_pos > current_line_width {
                        spans.push(Span::raw(" ".repeat(target_pos - current_line_width)));
                        spans.push(Span::styled(format!("{}.", pnum), page_num_style));
                    }
                }

                Line::from(spans)
            }),
    );

    f.render_widget(Paragraph::new(visible), text_area);

    if title_area.height > 0 {
        let app_version = env!("CARGO_PKG_VERSION");
        let left_text = format!("  lottie {}", app_version);
        let right_text = if app.dirty { "Modified  " } else { "  " };
        let center_text = app
            .file
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "New Script".to_string());

        let width = title_area.width as usize;
        let left_len = left_text.chars().count();
        let right_len = right_text.chars().count();
        let center_len = center_text.chars().count();

        let center_start = (width.saturating_sub(center_len)) / 2;
        let pad1 = center_start.saturating_sub(left_len);
        let pad2 = width.saturating_sub(left_len + pad1 + center_len + right_len);

        let title_line = format!(
            "{}{}{}{}{}",
            left_text,
            " ".repeat(pad1),
            center_text,
            " ".repeat(pad2),
            right_text
        );
        f.render_widget(Paragraph::new(title_line).style(panel_style), title_area);
    }

    if status_area.height > 0 {
        match app.mode {
            AppMode::Search => {
                let prompt_base = if app.last_search.is_empty() {
                    "Search: ".to_string()
                } else {
                    format!("Search [{}]: ", app.last_search)
                };
                let prompt_str = format!("{}{}", prompt_base, app.search_query);
                let status_padded =
                    format!("{:<width$}", prompt_str, width = status_area.width as usize);
                f.render_widget(
                    Paragraph::new(status_padded).style(panel_style),
                    status_area,
                );
            }
            AppMode::PromptSave => {
                let prompt_str = "Save modified script?";
                let status_padded =
                    format!("{:<width$}", prompt_str, width = status_area.width as usize);
                f.render_widget(
                    Paragraph::new(status_padded).style(panel_style),
                    status_area,
                );
            }
            AppMode::PromptFilename => {
                let prompt_base = format!("File Name to Write: {}", app.filename_input);
                let status_padded = format!(
                    "{:<width$}",
                    prompt_base,
                    width = status_area.width as usize
                );
                f.render_widget(
                    Paragraph::new(status_padded).style(panel_style),
                    status_area,
                );
            }
            AppMode::Normal => {
                if let Some(msg) = &app.status_msg {
                    let bracketed = format!("[ {} ]", msg);
                    let msg_len = bracketed.chars().count();
                    let pad_left = (status_area.width as usize).saturating_sub(msg_len) / 2;

                    let spans = vec![
                        Span::raw(" ".repeat(pad_left)),
                        Span::styled(bracketed, panel_style),
                    ];
                    f.render_widget(Paragraph::new(Line::from(spans)), status_area);
                } else {
                    f.render_widget(Paragraph::new(""), status_area);
                }
            }
        }
    }

    if shortcut_area.height > 0 {
        let (sc1, sc2) = match app.mode {
            AppMode::PromptSave => (vec![(" Y", "Yes")], vec![(" N", "No"), ("^C", "Cancel")]),
            _ => (
                vec![
                    ("^S", "Save"),
                    ("^K", "Cut"),
                    ("^Z", "Undo"),
                    ("^W", "Where Is"),
                ],
                vec![
                    ("^X", "Exit"),
                    ("^U", "Paste"),
                    ("^R", "Redo"),
                    ("^C", "Cur Pos"),
                ],
            ),
        };

        let col_width = (shortcut_area.width / 4) as usize;

        let render_shortcut_row = |shortcuts: &[(&str, &str)]| -> Line<'static> {
            let mut spans = Vec::new();
            for (key, desc) in shortcuts.iter() {
                spans.push(Span::styled(key.to_string(), panel_style));
                let text = format!(
                    " {:<width$}",
                    desc,
                    width = col_width.saturating_sub(key.chars().count() + 1)
                );
                spans.push(Span::raw(text));
            }
            Line::from(spans)
        };

        let shortcuts_lines = vec![render_shortcut_row(&sc1), render_shortcut_row(&sc2)];
        f.render_widget(Paragraph::new(shortcuts_lines), shortcut_area);
    }

    match app.mode {
        AppMode::Search if status_area.height > 0 => {
            let prompt_base = if app.last_search.is_empty() {
                "Search: ".to_string()
            } else {
                format!("Search [{}]: ", app.last_search)
            };
            let query_w = UnicodeWidthStr::width(prompt_base.as_str())
                + UnicodeWidthStr::width(app.search_query.as_str());
            let cur_screen_x = status_area.x + query_w as u16;
            f.set_cursor_position((cur_screen_x, status_area.y));
        }
        AppMode::PromptFilename if status_area.height > 0 => {
            let prompt_base = "File Name to Write: ";
            let query_w = UnicodeWidthStr::width(prompt_base)
                + UnicodeWidthStr::width(app.filename_input.as_str());
            let cur_screen_x = status_area.x + query_w as u16;
            f.set_cursor_position((cur_screen_x, status_area.y));
        }
        AppMode::PromptSave if status_area.height > 0 => {
            let query_w = UnicodeWidthStr::width("Save modified buffer?");
            let cur_screen_x = (status_area.x + query_w as u16 + 1)
                .min(status_area.x + status_area.width.saturating_sub(1));
            f.set_cursor_position((cur_screen_x, status_area.y));
        }
        AppMode::Normal => {
            let cur_screen_y =
                text_area.y + pad_top as u16 + (vis_row.saturating_sub(app.scroll)) as u16;
            let cur_screen_x = text_area.x + global_pad + vis_x;
            if cur_screen_y < text_area.y + text_area.height {
                f.set_cursor_position((cur_screen_x, cur_screen_y));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;

    fn create_empty_app() -> App {
        App::new(None, crate::config::Cli::default())
    }

    #[test]
    fn test_app_initialization() {
        let app = create_empty_app();
        assert_eq!(app.lines.len(), 1);
        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 0);
        assert!(!app.dirty);
        assert!(app.mode == AppMode::Normal);
    }

    #[test]
    fn test_app_move_down() {
        let mut app = create_empty_app();
        app.lines = vec!["Line 1".to_string(), "Line 2".to_string()];
        app.parse_document();
        app.update_layout();
        app.move_down();
        assert_eq!(app.cursor_y, 1);
    }

    #[test]
    fn test_app_move_up() {
        let mut app = create_empty_app();
        app.lines = vec!["Line 1".to_string(), "Line 2".to_string()];
        app.cursor_y = 1;
        app.parse_document();
        app.update_layout();
        app.move_up();
        assert_eq!(app.cursor_y, 0);
    }

    #[test]
    fn test_app_move_right() {
        let mut app = create_empty_app();
        app.lines = vec!["123".to_string(), "456".to_string()];
        app.move_right();
        assert_eq!(app.cursor_x, 1);
        app.move_right();
        app.move_right();
        assert_eq!(app.cursor_x, 3);
        app.move_right();
        assert_eq!(app.cursor_y, 1);
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_move_left() {
        let mut app = create_empty_app();
        app.lines = vec!["123".to_string(), "456".to_string()];
        app.cursor_y = 1;
        app.cursor_x = 0;
        app.move_left();
        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 3);
        app.move_left();
        assert_eq!(app.cursor_x, 2);
    }

    #[test]
    fn test_app_move_word_right() {
        let mut app = create_empty_app();
        app.lines = vec!["Word one two".to_string()];
        app.move_word_right();
        assert_eq!(app.cursor_x, 4);
        app.move_word_right();
        assert_eq!(app.cursor_x, 8);
    }

    #[test]
    fn test_app_move_word_left() {
        let mut app = create_empty_app();
        app.lines = vec!["Word one two".to_string()];
        app.cursor_x = 9;
        app.move_word_left();
        assert_eq!(app.cursor_x, 5);
        app.move_word_left();
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_move_home_and_end() {
        let mut app = create_empty_app();
        app.lines = vec!["End of line".to_string()];
        app.move_end();
        assert_eq!(app.cursor_x, 11);
        app.move_home();
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_insert_char() {
        let mut app = create_empty_app();
        app.insert_char('A');
        assert_eq!(app.lines[0], "A");
        assert_eq!(app.cursor_x, 1);
        assert!(app.dirty);
    }

    #[test]
    fn test_app_insert_matching_parentheses() {
        let mut app = create_empty_app();
        app.config.match_parentheses = true;
        app.insert_char('(');
        assert_eq!(app.lines[0], "()");
        assert_eq!(app.cursor_x, 1);
    }

    #[test]
    fn test_app_insert_matching_brackets() {
        let mut app = create_empty_app();
        app.config.close_elements = true;
        app.insert_char('[');
        app.insert_char('[');
        assert_eq!(app.lines[0], "[[]]");
        assert_eq!(app.cursor_x, 2);
    }

    #[test]
    fn test_app_insert_matching_boneyard() {
        let mut app = create_empty_app();
        app.config.close_elements = true;
        app.insert_char('/');
        app.insert_char('*');
        assert_eq!(app.lines[0], "/**/");
        assert_eq!(app.cursor_x, 2);
    }

    #[test]
    fn test_app_backspace() {
        let mut app = create_empty_app();
        app.lines = vec!["A".to_string()];
        app.cursor_x = 1;
        app.backspace();
        assert_eq!(app.lines[0], "");
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_backspace_matching_brackets() {
        let mut app = create_empty_app();
        app.lines = vec!["[[]]".to_string()];
        app.cursor_x = 2;
        app.backspace();
        assert_eq!(app.lines[0], "");
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_backspace_merge_lines() {
        let mut app = create_empty_app();
        app.lines = vec!["A".to_string(), "B".to_string()];
        app.cursor_y = 1;
        app.cursor_x = 0;
        app.backspace();
        assert_eq!(app.lines.len(), 1);
        assert_eq!(app.lines[0], "AB");
        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 1);
    }

    #[test]
    fn test_app_delete_forward() {
        let mut app = create_empty_app();
        app.lines = vec!["AB".to_string()];
        app.cursor_x = 0;
        app.delete_forward();
        assert_eq!(app.lines[0], "B");
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_delete_forward_merge_lines() {
        let mut app = create_empty_app();
        app.lines = vec!["A".to_string(), "B".to_string()];
        app.cursor_x = 1;
        app.delete_forward();
        assert_eq!(app.lines.len(), 1);
        assert_eq!(app.lines[0], "AB");
    }

    #[test]
    fn test_app_delete_word_back() {
        let mut app = create_empty_app();
        app.lines = vec!["One Two".to_string()];
        app.cursor_x = 7;
        app.delete_word_back();
        assert_eq!(app.lines[0], "One ");
        assert_eq!(app.cursor_x, 4);
    }

    #[test]
    fn test_app_delete_word_forward() {
        let mut app = create_empty_app();
        app.lines = vec!["One Two".to_string()];
        app.cursor_x = 0;
        app.delete_word_forward();
        assert_eq!(app.lines[0], " Two");
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_insert_newline() {
        let mut app = create_empty_app();
        app.lines = vec!["AB".to_string()];
        app.cursor_x = 1;
        app.insert_newline(false);
        assert_eq!(app.lines.len(), 2);
        assert_eq!(app.lines[0], "A");
        assert_eq!(app.lines[1], "B");
        assert_eq!(app.cursor_y, 1);
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_insert_newline_auto_paragraph_breaks() {
        let mut app = create_empty_app();
        app.config.auto_paragraph_breaks = true;
        app.lines = vec!["Action line.".to_string()];
        app.types = vec![LineType::Action];
        app.cursor_x = 12;
        app.insert_newline(false);
        assert_eq!(app.lines.len(), 3);
        assert_eq!(app.lines[0], "Action line.");
        assert_eq!(app.lines[1], "");
        assert_eq!(app.lines[2], "");
        assert_eq!(app.cursor_y, 2);
    }

    #[test]
    fn test_app_insert_newline_smart_element_escape() {
        let mut app = create_empty_app();
        app.lines = vec!["CHARLOTTE".to_string()];
        app.types = vec![LineType::Character];
        app.cursor_x = 9;
        app.insert_newline(false);
        assert_eq!(app.lines.len(), 2);
        assert_eq!(app.lines[0], "CHARLOTTE");
        assert_eq!(app.lines[1], "");
        assert_eq!(app.cursor_y, 1);
    }

    #[test]
    fn test_app_undo_redo_stack() {
        let mut app = create_empty_app();
        app.lines = vec!["Initial".to_string()];
        app.save_state(true);
        app.lines = vec!["Changed".to_string()];
        app.undo();
        assert_eq!(app.lines[0], "Initial");
        app.redo();
        assert_eq!(app.lines[0], "Changed");
    }

    #[test]
    fn test_app_cut_and_paste() {
        let mut app = create_empty_app();
        app.lines = vec!["Line 1".to_string(), "Line 2".to_string()];
        app.cut_line();
        assert_eq!(app.lines.len(), 1);
        assert_eq!(app.lines[0], "Line 2");
        app.paste_line();
        assert_eq!(app.lines.len(), 2);
        assert_eq!(app.lines[0], "Line 1");
        assert_eq!(app.lines[1], "Line 2");
    }

    #[test]
    fn test_app_cut_append_buffer() {
        let mut app = create_empty_app();
        app.lines = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        app.cut_line();
        app.cut_line();
        assert_eq!(app.cut_buffer, Some("A\nB".to_string()));
    }

    #[test]
    fn test_app_search_forward() {
        let mut app = create_empty_app();
        app.lines = vec!["Alpha".to_string(), "Beta".to_string(), "Gamma".to_string()];
        app.search_query = "eta".to_string();
        app.execute_search();
        assert_eq!(app.cursor_y, 1);
        assert_eq!(app.cursor_x, 1);
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_app_search_wrap_around() {
        let mut app = create_empty_app();
        app.lines = vec!["Alpha".to_string(), "Beta".to_string(), "Gamma".to_string()];
        app.cursor_y = 2;
        app.search_query = "lph".to_string();
        app.execute_search();
        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 1);
    }

    #[test]
    fn test_app_search_regex_not_found() {
        let mut app = create_empty_app();
        app.lines = vec!["Just text".to_string()];
        app.search_query = "unicorn".to_string();

        app.execute_search();

        assert_eq!(app.cursor_y, 0, "Cursor should not move");
        assert_eq!(app.status_msg.as_deref(), Some("\"unicorn\" not found"));
        assert!(
            !app.show_search_highlight,
            "Highlight should be disabled if not found"
        );
    }

    #[test]
    fn test_app_tab_state_machine_empty_to_char() {
        let mut app = create_empty_app();
        app.lines = vec!["".to_string()];
        app.types = vec![LineType::Empty];
        app.handle_tab();
        assert_eq!(app.lines[0], "@");
        assert_eq!(app.cursor_x, 1);
    }

    #[test]
    fn test_app_tab_state_machine_char_to_scene() {
        let mut app = create_empty_app();
        app.lines = vec!["@".to_string()];
        app.types = vec![LineType::Character];
        app.cursor_x = 1;
        app.handle_tab();
        assert_eq!(app.lines[0], ".");
        assert_eq!(app.cursor_x, 1);
    }

    #[test]
    fn test_app_tab_state_machine_scene_to_transition() {
        let mut app = create_empty_app();
        app.lines = vec![".".to_string()];
        app.types = vec![LineType::SceneHeading];
        app.cursor_x = 1;
        app.handle_tab();
        assert_eq!(app.lines[0], ">");
        assert_eq!(app.cursor_x, 1);
    }

    #[test]
    fn test_app_tab_state_machine_transition_to_empty() {
        let mut app = create_empty_app();
        app.lines = vec![">".to_string()];
        app.types = vec![LineType::Transition];
        app.cursor_x = 1;
        app.handle_tab();
        assert_eq!(app.lines[0], "");
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_app_tab_state_machine_after_dialogue_is_paren() {
        let mut app = create_empty_app();
        app.lines = vec!["CHARLOTTE".to_string(), "".to_string()];
        app.types = vec![LineType::Character, LineType::Empty];
        app.cursor_y = 1;
        app.handle_tab();
        assert_eq!(app.lines[1], "()");
        assert_eq!(app.cursor_x, 1);
    }

    #[test]
    fn test_app_tab_dialogue_wrap() {
        let mut app = create_empty_app();
        app.lines = vec!["CHARLOTTE".to_string(), "speaking".to_string()];
        app.types = vec![LineType::Character, LineType::Dialogue];
        app.cursor_y = 1;
        app.handle_tab();
        assert_eq!(app.lines[1], "(speaking)");
    }

    #[test]
    fn test_app_tab_strip_forced_markers() {
        let mut app = create_empty_app();
        app.lines = vec!["!Force".to_string()];
        app.types = vec![LineType::Action];
        app.cursor_x = 6;
        app.handle_tab();
        assert_eq!(app.lines[0], "Force");
        assert_eq!(app.cursor_x, 5);
    }

    #[test]
    fn test_app_autocomplete_character() {
        let mut app = create_empty_app();
        app.lines = vec!["@CHA".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 4;
        app.characters.insert("CHARLOTTE C.".to_string());
        app.update_autocomplete();
        assert_eq!(app.suggestion, Some("RLOTTE C.".to_string()));
    }

    #[test]
    fn test_app_autocomplete_scene_heading() {
        let mut app = create_empty_app();
        app.lines = vec![
            "INT. BIG ROOM - DAY".to_string(),
            "".to_string(),
            "INT. BI".to_string(),
        ];
        app.cursor_y = 2;
        app.cursor_x = 7;
        app.parse_document();
        app.update_autocomplete();
        assert_eq!(app.suggestion, Some("G ROOM - DAY".to_string()));
    }

    #[test]
    fn test_app_utf8_cursor_navigation_and_deletion() {
        let mut app = create_empty_app();

        app.lines = vec!["Привет, мир!".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 7;

        app.backspace();

        assert_eq!(app.lines[0], "Привет мир!");
        assert_eq!(app.cursor_x, 6);

        app.backspace();
        assert_eq!(app.lines[0], "Приве мир!");
        assert_eq!(app.cursor_x, 5);
    }

    #[test]
    fn test_app_word_navigation_utf8() {
        let mut app = create_empty_app();
        app.lines = vec!["Сценарий номер один".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 0;

        app.move_word_right();
        assert_eq!(app.cursor_x, 8);

        app.move_word_right();
        assert_eq!(app.cursor_x, 14);

        app.move_word_left();
        assert_eq!(app.cursor_x, 9);
    }

    #[test]
    fn test_app_auto_title_page_enabled() {
        let mut cli = crate::config::Cli::default();
        cli.auto_title_page = true;

        let app = App::new(None, cli);
        assert!(
            app.lines.len() > 1,
            "Title page should generate multiple lines"
        );
        assert_eq!(
            app.lines[0], "Title: Untitled",
            "First line must be Title metadata"
        );
        assert!(
            app.dirty,
            "App should be marked dirty after generating title page"
        );
    }

    #[test]
    fn test_app_auto_title_page_disabled() {
        let cli = crate::config::Cli::default();

        let app = App::new(None, cli);
        assert_eq!(app.lines.len(), 1, "Should only have one line");
        assert_eq!(app.lines[0], "", "Line should be empty");
        assert!(!app.dirty, "App should NOT be dirty");
    }

    #[test]
    fn test_app_autocomplete_disabled() {
        let mut app = create_empty_app();
        app.config.autocomplete = false;

        app.lines = vec!["@CHA".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 4;
        app.characters.insert("CHARLOTTE C.".to_string());

        app.update_autocomplete();
        assert_eq!(
            app.suggestion, None,
            "Suggestion should be None when disabled"
        );
    }

    #[test]
    fn test_app_match_parentheses_disabled() {
        let mut app = create_empty_app();
        app.config.match_parentheses = false;

        app.insert_char('(');
        assert_eq!(
            app.lines[0], "(",
            "Should only insert '(' without closing ')'"
        );
        assert_eq!(app.cursor_x, 1);
    }

    #[test]
    fn test_app_close_elements_disabled() {
        let mut app = create_empty_app();
        app.config.close_elements = false;

        app.insert_char('[');
        app.insert_char('[');
        assert_eq!(app.lines[0], "[[", "Should NOT insert ']]' automatically");
        assert_eq!(app.cursor_x, 2);

        app.lines = vec!["".to_string()];
        app.cursor_x = 0;
        app.insert_char('/');
        app.insert_char('*');
        assert_eq!(app.lines[0], "/*", "Should NOT insert '*/' automatically");
    }

    #[test]
    fn test_app_auto_paragraph_breaks_disabled() {
        let mut app = create_empty_app();
        app.config.auto_paragraph_breaks = false;

        app.lines = vec!["Action line.".to_string()];
        app.types = vec![LineType::Action];
        app.cursor_x = 12;

        app.insert_newline(false);

        assert_eq!(app.lines.len(), 2, "Should only insert 1 newline");
        assert_eq!(app.lines[1], "");
        assert_eq!(app.cursor_y, 1);
    }

    #[test]
    fn test_draw_focus_mode_hides_panels() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = create_empty_app();
        app.config.focus_mode = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| super::draw(f, &mut app)).unwrap();

        let mut content = String::new();
        let buffer = terminal.backend().buffer();
        for y in 0..24u16 {
            for x in 0..80u16 {
                content.push_str(buffer[(x, y)].symbol());
            }
        }

        assert!(
            !content.contains("lottie"),
            "Top panel should be hidden in focus mode"
        );
        assert!(
            !content.contains("^X"),
            "Bottom panel should be hidden in focus mode"
        );
    }

    #[test]
    fn test_draw_focus_mode_shows_prompt() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = create_empty_app();
        app.config.focus_mode = true;
        app.mode = AppMode::PromptSave;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| super::draw(f, &mut app)).unwrap();

        let mut content = String::new();
        let buffer = terminal.backend().buffer();
        for y in 0..24u16 {
            for x in 0..80u16 {
                content.push_str(buffer[(x, y)].symbol());
            }
        }

        assert!(
            content.contains("Save modified script?"),
            "Prompt should appear even in focus mode"
        );
        assert!(
            content.contains("Yes"),
            "Shortcuts should reappear for the prompt"
        );
    }

    #[test]
    fn test_draw_focus_mode_shows_status_msg() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = create_empty_app();
        app.config.focus_mode = true;
        app.set_status("GNU Terry Pratchett");

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| super::draw(f, &mut app)).unwrap();

        let mut content = String::new();
        let buffer = terminal.backend().buffer();
        for y in 0..24u16 {
            for x in 0..80u16 {
                content.push_str(buffer[(x, y)].symbol());
            }
        }

        assert!(
            content.contains("GNU Terry Pratchett"),
            "Status message should appear even in focus mode"
        );
        assert!(
            content.contains("^X"),
            "Shortcuts should reappear when status is shown"
        );
    }

    #[test]
    fn test_draw_no_formatting_page_numbers() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = create_empty_app();
        app.config.no_formatting = true;

        app.lines = vec!["Action line".to_string()];
        app.types = vec![LineType::Action];
        app.update_layout();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| super::draw(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let mut found = false;
        for y in 0..24 {
            for x in 0..80 {
                let cell = &buffer[(x, y)];
                if cell.symbol() == "1" {
                    assert!(
                        !cell.modifier.contains(Modifier::BOLD),
                        "Page number should not be bold when no_formatting is true"
                    );
                    found = true;
                }
            }
        }
        assert!(found, "Page number not found");
    }

    #[test]
    fn test_draw_panel_style_resets_color() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = create_empty_app();
        app.set_status("Test status");

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| super::draw(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let status_cell = &buffer[(0, 22)];
        assert_eq!(
            status_cell.fg,
            Color::Reset,
            "Panel should explicitly reset foreground color"
        );
        assert_eq!(
            status_cell.bg,
            Color::Reset,
            "Panel should explicitly reset background color"
        );
        assert!(status_cell.modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn test_ux_boundary_beginning_of_file() {
        let mut app = create_empty_app();
        app.lines = vec!["First".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 0;

        app.move_up();
        app.move_left();
        app.move_word_left();
        app.backspace();

        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 0);
        assert_eq!(app.lines[0], "First");
    }

    #[test]
    fn test_ux_boundary_end_of_file() {
        let mut app = create_empty_app();
        app.lines = vec!["Last".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 4;

        app.move_down();
        app.move_right();
        app.move_word_right();
        app.delete_forward();

        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 4);
        assert_eq!(app.lines[0], "Last");
    }

    #[test]
    fn test_ux_line_joining_backspace() {
        let mut app = create_empty_app();
        app.lines = vec!["Hello ".to_string(), "World".to_string()];
        app.cursor_y = 1;
        app.cursor_x = 0;

        app.backspace();

        assert_eq!(app.lines.len(), 1);
        assert_eq!(app.lines[0], "Hello World");
        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 6);
    }

    #[test]
    fn test_ux_line_joining_delete() {
        let mut app = create_empty_app();
        app.lines = vec!["Hello ".to_string(), "World".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 6;

        app.delete_forward();

        assert_eq!(app.lines.len(), 1);
        assert_eq!(app.lines[0], "Hello World");
        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 6);
    }

    #[test]
    fn test_ux_line_splitting_enter() {
        let mut app = create_empty_app();
        app.lines = vec!["HelloWorld".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 5;

        app.insert_newline(false);

        assert_eq!(app.lines.len(), 2);
        assert_eq!(app.lines[0], "Hello");
        assert_eq!(app.lines[1], "World");
        assert_eq!(app.cursor_y, 1);
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_ux_utf8_multibyte_safety() {
        let mut app = create_empty_app();

        app.lines = vec!["пути творчества".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 15;

        app.delete_word_back();
        app.backspace();

        app.insert_char('н');
        app.insert_char(' ');
        app.insert_char('🦀');

        assert_eq!(app.lines[0], "путин 🦀");
        app.cursor_x = 7;

        app.backspace();
        assert_eq!(app.lines[0], "путин ", "backspace should delete emoji");
        assert_eq!(
            app.cursor_x, 6,
            "cursor should move back once after deleting emoji"
        );

        app.backspace();
        assert_eq!(
            app.lines[0], "путин",
            "backspace should delete trailing space"
        );
        assert_eq!(app.cursor_x, 5, "cursor should be at end of word");

        app.insert_char(' ');
        app.insert_char('х');
        app.insert_char('у');
        app.insert_char('й');
        app.insert_char('л');
        app.insert_char('о');
        assert_eq!(
            app.lines[0], "путин хуйло",
            "insert_char should append correctly"
        );
        assert_eq!(app.cursor_x, 11, "cursor should be at end after inserts");

        app.cursor_x = 0;
        for _ in 0..6 {
            app.delete_forward();
        }
        assert_eq!(
            app.lines[0], "хуйло",
            "delete_forward should remove first word char by char"
        );
        assert_eq!(app.cursor_x, 0, "cursor should stay at position 0");

        app.cursor_x = 5;
        app.backspace();
        app.backspace();
        assert_eq!(
            app.lines[0], "хуй",
            "delete_word_back should remove last two chars"
        );
        assert_eq!(app.cursor_x, 3, "cursor should be at end of remaining word");
    }

    #[test]
    fn test_ux_visual_up_down_inside_soft_wrapped_line() {
        let mut app = create_empty_app();
        let long_line = "A".repeat(100);
        app.lines = vec!["Short line".to_string(), long_line];
        app.types = vec![LineType::Action, LineType::Action];

        app.update_layout();

        app.cursor_y = 1;
        app.cursor_x = 80;
        app.target_visual_x = 20;

        app.move_up();

        assert_eq!(
            app.cursor_y, 1,
            "Cursor should stay on the same logical line"
        );
        assert_eq!(
            app.cursor_x, 20,
            "Cursor should move to the upper visual row of the soft-wrapped line"
        );

        app.move_down();
        assert_eq!(app.cursor_y, 1);
        assert_eq!(
            app.cursor_x, 80,
            "Cursor should return to the lower visual row"
        );
    }

    #[test]
    fn test_ux_smart_pairing_deletion() {
        let mut app = create_empty_app();
        app.lines = vec!["()".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 1;

        app.backspace();
        assert_eq!(app.lines[0], "");
        assert_eq!(app.cursor_x, 0);
    }

    #[test]
    fn test_ux_undo_restores_cursor_position_perfectly() {
        let mut app = create_empty_app();
        app.lines = vec!["Some text".to_string()];
        app.cursor_y = 0;
        app.cursor_x = 5;

        app.save_state(true);

        app.insert_char('A');
        assert_eq!(app.cursor_x, 6);

        app.undo();

        assert_eq!(app.lines[0], "Some text");
        assert_eq!(app.cursor_x, 5);
    }

    #[test]
    fn test_ux_ghost_cursor_memory_target_x() {
        let mut app = create_empty_app();
        app.lines = vec!["a".repeat(20), "b".repeat(3), "c".repeat(20)];

        app.parse_document();

        app.cursor_y = 0;
        app.cursor_x = 15;
        app.update_layout();
        app.target_visual_x = app.current_visual_x();

        app.move_down();
        assert_eq!(app.cursor_y, 1);
        assert_eq!(app.cursor_x, 3);

        app.move_down();
        assert_eq!(app.cursor_y, 2);

        assert_eq!(
            app.cursor_x, 15,
            "Cursor forgot its target_visual_x memory!"
        );
    }

    #[test]
    fn test_ux_tab_state_machine_middle_of_line() {
        let mut app = create_empty_app();
        app.lines = vec!["Some text here".to_string()];
        app.types = vec![LineType::Action];
        app.cursor_y = 0;
        app.cursor_x = 5;

        app.handle_tab();

        assert_eq!(app.lines[0], "@Some text here");
        assert_eq!(
            app.cursor_x, 6,
            "Cursor must shift right when a sigil is prepended!"
        );
    }

    #[test]
    fn test_draw_force_ascii_and_no_color_strips_ui_elements() {
        use ratatui::{Terminal, backend::TestBackend};

        let mut app = create_empty_app();
        app.config.force_ascii = true;
        app.config.no_color = true;

        app.lines = vec!["===".to_string(), "INT. TEST SCENE".to_string()];
        app.types = vec![LineType::PageBreak, LineType::SceneHeading];

        app.cursor_y = 1;

        app.update_layout();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| draw(f, &mut app)).unwrap();

        let mut content = String::new();
        let buffer = terminal.backend().buffer();
        for y in 0..24u16 {
            for x in 0..80u16 {
                content.push_str(buffer[(x, y)].symbol());
            }
            content.push('\n');
        }

        assert!(
            content.contains("------------------------------------------------------------"),
            "Page break should use ASCII '-' instead of Unicode '─'"
        );
        assert!(
            !content.contains("────────────────────────────────────────────────────────────"),
            "Page break should NOT contain Unicode '─' in force_ascii mode"
        );

        assert!(
            content.contains("INT. TEST SCENE"),
            "Standard text should be rendered"
        );
    }

    #[test]
    fn test_search_regex_basic_and_highlight_flag() {
        let mut app = create_empty_app();
        app.lines = vec!["Hello world".to_string(), "Line two".to_string()];
        app.search_query = "world".to_string();
        app.cursor_y = 0;
        app.cursor_x = 0;

        app.execute_search();

        assert_eq!(app.cursor_y, 0);
        assert_eq!(app.cursor_x, 6);
        assert!(
            app.show_search_highlight,
            "Highlight flag should be active after finding"
        );
    }

    #[test]
    fn test_search_regex_wrap_around() {
        let mut app = create_empty_app();
        app.lines = vec!["First target".to_string(), "Second line".to_string()];
        app.search_query = "target".to_string();
        app.cursor_y = 1;
        app.cursor_x = 0;

        app.execute_search();

        assert_eq!(app.cursor_y, 0, "Should wrap around to line 0");
        assert_eq!(app.cursor_x, 6, "Index of 't' in 'target'");
        assert_eq!(
            app.status_msg.as_deref(),
            Some("Search Wrapped"),
            "Should display wrapped status message"
        );
    }

    #[test]
    fn test_search_regex_utf8_multibyte_safety() {
        let mut app = create_empty_app();

        app.lines = vec!["путин 🦀 краб".to_string()];
        app.search_query = "краб".to_string();
        app.cursor_y = 0;
        app.cursor_x = 0;

        app.execute_search();

        assert_eq!(
            app.cursor_x, 8,
            "Search must correctly convert byte offsets to char offsets"
        );
    }

    #[test]
    fn test_search_highlight_cleared_on_escape() {
        use crossterm::event::{
            Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers,
        };

        let mut app = create_empty_app();
        app.lines = vec!["Target word".to_string()];
        app.search_query = "word".to_string();
        app.execute_search();

        assert!(app.show_search_highlight);

        let esc_event = Event::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        });

        let mut update_x = false;
        let mut text_ch = false;
        let mut cur_moved = false;

        let _ = app
            .handle_event(esc_event, &mut update_x, &mut text_ch, &mut cur_moved)
            .unwrap();

        assert!(
            !app.show_search_highlight,
            "Highlight flag should be reset when pressing Escape"
        );
        assert!(
            text_ch,
            "Text changed flag should trigger redraw to clear highlights"
        );
    }

    #[test]
    fn test_move_page_down_and_up() {
        let mut app = create_empty_app();
        app.lines = (0..50).map(|i| format!("Line {}", i)).collect();
        app.parse_document();
        app.update_layout();
        app.visible_height = 10;

        app.move_page_down();
        assert_eq!(app.cursor_y, 10);

        app.move_page_up();
        assert_eq!(app.cursor_y, 0);
    }

    #[test]
    fn test_report_cursor_position_empty() {
        let mut app = create_empty_app();
        app.report_cursor_position();

        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 1/1 (100%), col 1/1 (100%), char 1/1 (100%)"),
            "Empty document should report 100% for all metrics"
        );
    }

    #[test]
    fn test_report_cursor_position_basic_math() {
        let mut app = create_empty_app();
        app.lines = vec!["Hello".to_string()];
        app.types = vec![LineType::Action];
        app.update_layout();

        app.cursor_y = 0;
        app.cursor_x = 2;

        app.report_cursor_position();

        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 1/1 (100%), col 3/6 (50%), char 3/6 (50%)")
        );
    }

    #[test]
    fn test_report_cursor_position_soft_wrap() {
        let mut app = create_empty_app();
        let long_line = "A".repeat(100);
        app.lines = vec![long_line];
        app.types = vec![LineType::Action];
        app.update_layout();

        app.cursor_y = 0;
        app.cursor_x = 70;

        app.report_cursor_position();

        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 1/1 (100%), col 71/101 (70%), char 71/101 (70%)"),
            "Soft-wrapped lines count as one logical line"
        );
    }

    #[test]
    fn test_report_cursor_position_multi_line() {
        let mut app = create_empty_app();
        app.lines = vec!["One".to_string(), "Two".to_string(), "Three".to_string()];
        app.types = vec![LineType::Action, LineType::Action, LineType::Action];
        app.update_layout();

        app.cursor_y = 1;
        app.cursor_x = 1;

        app.report_cursor_position();

        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 2/3 (66%), col 2/4 (50%), char 6/14 (42%)")
        );
    }

    #[test]
    fn test_report_cursor_position_utf8_multibyte() {
        let mut app = create_empty_app();

        app.lines = vec!["Дратути 👋".to_string()];
        app.types = vec![LineType::Action];
        app.update_layout();

        app.cursor_y = 0;
        app.cursor_x = 8;

        app.report_cursor_position();

        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 1/1 (100%), col 9/10 (90%), char 9/10 (90%)"),
            "Cursor metrics should count UTF-8 chars, not raw bytes"
        );
    }

    #[test]
    fn test_report_cursor_position_end_of_file() {
        let mut app = create_empty_app();
        app.lines = vec!["123".to_string(), "45".to_string()];
        app.types = vec![LineType::Action, LineType::Action];
        app.update_layout();

        app.cursor_y = 1;
        app.cursor_x = 2;

        app.report_cursor_position();

        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 2/2 (100%), col 3/3 (100%), char 7/7 (100%)"),
            "Should safely handle cursor being positioned at the absolute end of the line"
        );
    }

    #[test]
    fn test_e2e_tutorial_integration() {
        let tutorial_text = r#"Title: Lottie Tutorial
Credit: Written by
Author: René Coignard
Draft date: Version 0.2.6
Contact:
contact@renecoignard.com

INT. FLAT IN WOLFEN-NORD - DAY

RENÉ sits at his desk, typing.

RENÉ
(turning round)
Oh, hello there. It seems you've found my terminal Rust port of Beat. Sit back and I'll show you how everything works.

I sometimes write screenplays on my Gentoo laptop, and doing it in plain nano isn't terribly comfortable (I work entirely in the terminal there). So I decided to put this port of Beat together. I used Beat's source code as a reference when writing Lottie, so things work more or less the same way.

As you may have already noticed, the navigation is rather reminiscent of nano, because I did look at its source code and took inspiration, for the sake of authenticity. I'm rather fond of it, and I hope you will be too. Not quite as nerdy as vim, but honestly, I'm an average nano enjoyer and I'm not ashamed of it.

Anyway, let's get into it.

EXT. NORDPARK - DAY

As I mentioned, things work much the same as in Beat. If you start a line with **int.** or **ext.**, Lottie will automatically turn it into a scene heading. You can also use tab: on an empty line, it will first turn it into a character cue, then a scene heading, and then a transition. If you simply start typing IN CAPS ON AN EMPTY LINE, LIKE SO, the text will automatically become a character cue.

You can also use notes:

/* Two sailors are walking along the deck, when one turns to the other and says: */

SAILOR
I'm not a sailor, actually.

Lottie automatically inserts two blank lines after certain elements, just as Beat does, though this can be adjusted in the configuration file. There's a sample config in the repository; do make use of it. Bonus: try enabling typewriter mode and see what happens.

To create a transition, simply write in capitals and end with a colon, like so:

CUT TO:

That alone is quite enough to write a proper screenplay. But there's more! For instance, we also have these:

/*

A multi-line comment.

For very, very, very long notes.

*/

[[Comments can look like this as well. They don't differ much from other comment types, but for compatibility with Beat, all the same comment types are supported.]]

# This is a new section

= And this is a synopsis.

INT. EDEKA - ABEND

Unlike Beat, there's no full render or PDF export here, but you can always save your screenplay and open it in Beat to do that. In Beat, synopses wouldn't appear in the rendered script, nor would comments. Which is why they share the same colour here, incidentally.

As you may have noticed, there's support for **bold text**, *italics*, and even _underlined text_. When your cursor isn't on a line containing these markers, they'll be hidden from view. Move onto the line, and you'll see all the asterisks and underscores that produce the formatting.

Centred text is supported as well, and works like this:

>Centred text<

You can also force transitions:

>AN ABRUPT TRANSITION TO THE NEXT SCENE:

EXT. WOLFEN(BITTERFELD) RAILWAY STATION - MORNING

Lyrics are supported too, using a tilde at the start of the line:

~Meine Damen, meine Herrn, danke
~Dass Sie mit uns reisen
~Zu abgefahrenen Preisen
~Auf abgefahrenen Gleisen
~Für Ihre Leidensfähigkeit, danken wir spontan
~Sänk ju for träweling wis Deutsche Bahn

That's Wise Guys. Onwards.

EXT. LEIPZIG HBF - MORNING

Well, do have a go on it, write something from scratch, or edit this screenplay. You might even turn up a bug or two; if so, please do let me know :-) Everything seemed to behave itself while I was putting this tutorial together, and I hope it all runs just as smoothly for you. I hope you enjoy working in Lottie.

[[marker Speaking of which, I named the application after a certain Charlotte I once knew, who wrote quite wonderful screenplays.]]
[[marker blue The colour of these comment markers can be changed, as you can see.]]

You can find more information about the Fountain markup language at https://www.fountain.io/

And Beat itself, of course: https://www.beat-app.fi/

> FADE OUT"#;

        let mut app = App::new(None, crate::config::Cli::default());
        app.lines = tutorial_text.lines().map(|s| s.to_string()).collect();
        app.cursor_y = 0;
        app.cursor_x = 0;

        app.parse_document();
        app.update_layout();

        let get_exact_idx =
            |search_str: &str| -> usize { app.lines.iter().position(|l| l == search_str).unwrap() };
        let get_idx = |search_str: &str| -> usize {
            app.lines
                .iter()
                .position(|l| l.starts_with(search_str))
                .unwrap()
        };

        let meta_title_idx = get_idx("Title:");
        let meta_val_idx = get_idx("contact@renecoignard");
        let scene1_idx = get_idx("INT. FLAT");

        let char1_idx = get_exact_idx("RENÉ");

        let paren_idx = get_idx("(turning round)");
        let dial_idx = get_idx("Oh, hello there");
        let boneyard1_idx = get_idx("/* Two sailors");
        let trans1_idx = get_exact_idx("CUT TO:");
        let boneyard_multiline_idx = get_exact_idx("/*");
        let section_idx = get_idx("# This is");
        let syn_idx = get_idx("= And this");
        let inline_note_idx = get_idx("[[Comments");
        let markup_idx = get_idx("As you may have noticed, there's support for");
        let center_idx = get_exact_idx(">Centred text<");
        let force_trans_idx = get_idx(">AN ABRUPT");
        let lyric1_idx = get_idx("~Meine Damen");
        let lyric6_idx = get_idx("~Sänk ju");
        let note_marker_idx = get_idx("[[marker blue");
        let fade_out_idx = get_exact_idx("> FADE OUT");

        assert_eq!(app.types[meta_title_idx], LineType::MetadataTitle);
        assert_eq!(app.types[meta_val_idx], LineType::MetadataValue);
        assert_eq!(app.types[scene1_idx], LineType::SceneHeading);
        assert_eq!(app.types[char1_idx], LineType::Character);
        assert_eq!(app.types[paren_idx], LineType::Parenthetical);
        assert_eq!(app.types[dial_idx], LineType::Dialogue);
        assert_eq!(app.types[boneyard1_idx], LineType::Boneyard);
        assert_eq!(app.types[trans1_idx], LineType::Transition);
        assert_eq!(app.types[boneyard_multiline_idx], LineType::Boneyard);
        assert_eq!(app.types[section_idx], LineType::Section);
        assert_eq!(app.types[syn_idx], LineType::Synopsis);
        assert_eq!(app.types[inline_note_idx], LineType::Note);
        assert_eq!(app.types[center_idx], LineType::Centered);
        assert_eq!(app.types[force_trans_idx], LineType::Transition);
        assert_eq!(app.types[lyric1_idx], LineType::Lyrics);
        assert_eq!(app.types[lyric6_idx], LineType::Lyrics);
        assert_eq!(app.types[note_marker_idx], LineType::Note);
        assert_eq!(app.types[fade_out_idx], LineType::Transition);

        let layout_markup = app
            .layout
            .iter()
            .find(|r| r.line_idx == markup_idx)
            .unwrap();
        assert!(layout_markup.fmt.bold.len() > 0);
        assert!(layout_markup.fmt.italic.len() > 0);
        assert!(layout_markup.fmt.underlined.len() > 0);

        let layout_note = app
            .layout
            .iter()
            .find(|r| r.line_idx == note_marker_idx)
            .unwrap();
        assert!(layout_note.override_color.is_some());
        assert_eq!(
            layout_note.override_color.unwrap(),
            ratatui::style::Color::Blue
        );

        let layout_scene = app
            .layout
            .iter()
            .find(|r| r.line_idx == scene1_idx)
            .unwrap();
        assert_eq!(layout_scene.scene_num, Some(1));

        let layout_trans = app
            .layout
            .iter()
            .find(|r| r.line_idx == trans1_idx)
            .unwrap();
        let expected_indent = crate::types::PAGE_WIDTH.saturating_sub(7);
        assert_eq!(layout_trans.indent, expected_indent);
        assert_eq!(layout_trans.raw_text, "CUT TO:");

        assert!(app.characters.contains("RENÉ"));
        assert!(app.characters.contains("SAILOR"));
        assert!(app.locations.contains("FLAT IN WOLFEN-NORD - DAY"));

        let total_vis_lines = app.layout.len();
        assert!(total_vis_lines > 0, "Layout must not be empty");

        let test_coordinates: Vec<(usize, usize, String, usize)> = app
            .layout
            .iter()
            .filter_map(|r| {
                if r.is_phantom {
                    None
                } else {
                    Some((r.line_idx, r.char_start, r.raw_text.clone(), r.char_end))
                }
            })
            .collect();

        for (line_idx, char_start, raw_text, char_end) in test_coordinates {
            app.cursor_y = line_idx;
            app.cursor_x = char_start;
            app.report_cursor_position();

            let status = app
                .status_msg
                .as_ref()
                .expect("Status message should be set");

            let line_part = status.split(',').next().unwrap();
            let fraction_part = line_part.split(' ').nth(1).unwrap();

            let cur_line_str = fraction_part.split('/').next().unwrap();
            let reported_line: usize = cur_line_str.parse().unwrap();

            let total_lines_str = fraction_part.split('/').nth(1).unwrap();
            let reported_total: usize = total_lines_str.parse().unwrap();

            assert_eq!(
                reported_line,
                line_idx + 1,
                "Mismatch at logical line {} (text: '{}'). Expected logical line {}, but got {}",
                line_idx,
                raw_text,
                line_idx + 1,
                reported_line
            );

            assert_eq!(
                reported_total,
                app.lines.len(),
                "Total logical lines mismatch at logical line {}",
                line_idx
            );

            app.cursor_x = char_end;
            app.report_cursor_position();
            assert!(
                app.status_msg.is_some(),
                "report_cursor_position panicked or failed at the end of logical line {}",
                line_idx
            );
        }

        let coords: Vec<(usize, usize, usize)> = app
            .layout
            .iter()
            .filter(|r| !r.is_phantom)
            .flat_map(|row| {
                (row.char_start..=row.char_end).map(move |cx| (row.line_idx, cx, row.char_start))
            })
            .collect();

        let mut prev_char = 0usize;
        let mut prev_line = 0usize;

        for (line_idx, cx, _) in coords {
            app.cursor_y = line_idx;
            app.cursor_x = cx;
            app.report_cursor_position();

            let status = app.status_msg.as_ref().unwrap();
            let parts: Vec<&str> = status.split(", ").collect();

            let cur_line: usize = parts[0]
                .split('/')
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .parse()
                .unwrap();
            let cur_char: usize = parts[2]
                .split('/')
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .parse()
                .unwrap();

            assert!(
                cur_line >= prev_line,
                "line went backwards at y={} x={}: {} -> {}",
                line_idx,
                cx,
                prev_line,
                cur_line
            );
            assert!(
                cur_char >= prev_char,
                "char went backwards at y={} x={}: {} -> {}",
                line_idx,
                cx,
                prev_char,
                cur_char
            );

            prev_char = cur_char;
            prev_line = cur_line;
        }

        app.cursor_y = app
            .lines
            .iter()
            .position(|l| l.starts_with("INT. FLAT"))
            .unwrap();
        app.cursor_x = 0;
        app.update_layout();
        app.report_cursor_position();
        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 8/93 (8%), col 1/31 (3%), char 126/4074 (3%)")
        );

        app.cursor_y = app
            .lines
            .iter()
            .position(|l| l.starts_with(">AN ABRUPT"))
            .unwrap();
        app.cursor_x = 0;
        app.update_layout();
        app.report_cursor_position();
        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 67/93 (72%), col 1/41 (2%), char 2969/4074 (72%)")
        );

        app.cursor_y = app.lines.iter().position(|l| l == "> FADE OUT").unwrap();
        app.cursor_x = app.lines[app.cursor_y].chars().count();
        app.update_layout();
        app.report_cursor_position();
        assert_eq!(
            app.status_msg.as_deref(),
            Some("line 93/93 (100%), col 11/11 (100%), char 4074/4074 (100%)")
        );
    }

    #[test]
    fn test_draw_typewriter_mode_normal() {
        use ratatui::{
            Terminal,
            backend::{Backend, TestBackend},
        };
        let mut app = create_empty_app();
        app.config.typewriter_mode = true;
        app.lines = vec!["Line 1".to_string()];
        app.types = vec![LineType::Action];
        app.cursor_y = 0;
        app.update_layout();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| super::draw(f, &mut app)).unwrap();

        assert_eq!(app.scroll, 0);
        assert_eq!(terminal.backend_mut().get_cursor_position().unwrap().y, 1);
    }

    #[test]
    fn test_draw_typewriter_mode_strict() {
        use ratatui::{
            Terminal,
            backend::{Backend, TestBackend},
        };
        let mut app = create_empty_app();
        app.config.strict_typewriter_mode = true;
        app.lines = vec!["Line 1".to_string()];
        app.types = vec![LineType::Action];
        app.cursor_y = 0;
        app.update_layout();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| super::draw(f, &mut app)).unwrap();

        assert_eq!(terminal.backend_mut().get_cursor_position().unwrap().y, 12);
    }

    #[test]
    fn test_draw_active_action_highlight() {
        use ratatui::style::Color;
        use ratatui::{Terminal, backend::TestBackend};
        let mut app = create_empty_app();

        app.config.highlight_active_action = true;
        app.config.no_color = false;

        app.lines = vec!["An action line".to_string(), "".to_string(), "".to_string()];
        app.types = vec![LineType::Action, LineType::Empty, LineType::Empty];
        app.cursor_y = 2;
        app.update_layout();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| super::draw(f, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let mut found_action_text = false;

        for y in 0..24 {
            for x in 0..80 {
                let cell = &buffer[(x, y)];
                if cell.symbol() == "A" {
                    found_action_text = true;
                    assert_eq!(
                        cell.fg,
                        Color::White,
                        "Active action line above empty lines should be forced to white"
                    );
                }
            }
        }
        assert!(found_action_text, "Action text should be rendered");
    }
}
