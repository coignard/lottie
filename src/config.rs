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

use clap::Parser;
use std::fs;
use std::path::PathBuf;

const DEFAULT_CONFIG: &str = r#"## Lottie configuration file
## Place this file at ~/.config/lottie/lottie.conf
##
## Use "set <option>" to enable a boolean option or assign a value.
## Use "unset <option>" to disable a boolean option.

## Editor View

# Show scene numbers in the left margin.
set show_scene_numbers

# Show page numbers on the right side of the screen.
set show_page_numbers

# Automatically hide Fountain markup when the cursor is not
# on the current line.
set hide_markup

# Highlight active action line (or nearest action line above)
# in bright white color.
unset highlight_active_action

# Typewriter mode
unset typewriter_mode

# Strict typewriter mode (forces the active line to stay in the exact
# vertical center of the terminal at all times, even at the beginning
# of the document).
unset strict_typewriter_mode

# Focus mode
unset focus_mode

## Editor Behavior

# Auto-complete scene headings (INT./EXT.) and character names.
set autocomplete

# Automatically append (CONT'D) to a character name when they speak
# consecutively.
set auto_contd

# Automatically insert paragraph breaks (double newlines) after Action,
# Dialogue, and similar elements.
set auto_paragraph_breaks

# Automatically insert a closing parenthesis when typing an opening one.
set match_parentheses

# Automatically close paired elements such as [[]], /**/, and ****.
set close_elements

# Insert a blank Title Page template when creating a new file.
unset auto_title_page

## Formatting

# The string appended to a character name when they speak consecutively.
set contd_extension "(CONT'D)"

# Allow action blocks to be split across pages.
# Use "unset break_actions" to keep action blocks on a single page.
set break_actions

# Open the file with the cursor at the end
unset goto_end

# Styling applied to scene headings. Available values: "bold",
# "underline", "bold underline"
set heading_style "bold"

# Number of blank lines before a scene heading. Set to 2 for double
# spacing before each new scene.
set heading_spacing 1

# Styling applied to shots (e.g. !! CLOSE UP). Available values: "bold",
# "underline", "bold underline"
set shot_style "bold"

## Display & Terminal

# Disable all terminal colors. Lottie will still render bold, italic,
# and underline modifiers if supported by your terminal. Lottie tries
# to detect color support automatically.
unset no_color

# Disable all text formatting (bold, italic, underline).
unset no_formatting

# Force output of ANSI color escape codes, even if Lottie detects
# that your terminal does not support them.
unset force_ansi

# Force the use of ASCII characters instead of Unicode (e.g., for page
# break lines). Useful for older terminals. Lottie will try to detect
# Unicode support automatically.
unset force_ascii
"#;

/// Command-line arguments parsed by [`clap`].
///
/// All flag names mirror the configuration file directives so that CLI options
/// act as overrides on top of whatever the config file specifies.  See
/// [`Config::load`] for the precedence order.
#[derive(Parser, Debug, Default, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The fountain file(s) to open
    #[arg(num_args = 0..)]
    pub files: Vec<PathBuf>,

    /// Path to a custom configuration file
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Hide scene numbers
    #[arg(long)]
    pub hide_scene_numbers: bool,

    /// Hide page numbers
    #[arg(long)]
    pub hide_page_numbers: bool,

    /// Show formatting markup
    #[arg(long)]
    pub show_markup: bool,

    /// Disable autocomplete
    #[arg(long)]
    pub no_autocomplete: bool,

    /// Disable automatic (CONT'D)
    #[arg(long)]
    pub no_auto_contd: bool,

    /// Disable auto paragraph breaks
    #[arg(long)]
    pub no_auto_paragraph_breaks: bool,

    /// Disable matching parentheses
    #[arg(long)]
    pub no_match_parentheses: bool,

    /// Disable auto-closing elements
    #[arg(long)]
    pub no_close_elements: bool,

    /// Generate title page if file is new
    #[arg(long)]
    pub auto_title_page: bool,

    /// Enable typewriter mode
    #[arg(long)]
    pub typewriter_mode: bool,

    /// Enable strict typewriter mode (always center)
    #[arg(long)]
    pub strict_typewriter_mode: bool,

    /// Enable focus mode
    #[arg(long)]
    pub focus_mode: bool,

    /// Highlight active action line in white
    #[arg(long)]
    pub highlight_active_action: bool,

    /// Disable breaking actions across pages
    #[arg(long)]
    pub no_break_actions: bool,

    /// Open the file with the cursor at the end
    #[arg(long)]
    pub goto_end: bool,

    /// Set (CONT'D) extension text
    #[arg(long)]
    pub contd_extension: Option<String>,

    /// Set heading style (e.g., "bold underline")
    #[arg(long)]
    pub heading_style: Option<String>,

    /// Set spacing before scene headings
    #[arg(long)]
    pub heading_spacing: Option<usize>,

    /// Set shot style (e.g., "bold")
    #[arg(long)]
    pub shot_style: Option<String>,

    /// Disable color formatting
    #[arg(long)]
    pub no_color: bool,

    /// Disable text formatting (bold, italic, underline)
    #[arg(long)]
    pub no_formatting: bool,

    /// Use ASCII characters instead of Unicode
    #[arg(long)]
    pub force_ascii: bool,

    /// Force ANSI color output even if unsupported by the terminal
    #[arg(long)]
    pub force_ansi: bool,

    /// Export the rendered script to a file or stdout (use '-' or omit value for stdout)
    #[arg(long, value_name = "FILE", num_args = 0..=1, default_missing_value = "-")]
    pub export: Option<PathBuf>,

    /// Format for the export (plain, ascii, ansi)
    #[arg(long, default_value = "plain", value_name = "FORMAT")]
    pub format: String,
}

/// Runtime configuration for the Lottie editor and export pipeline.
///
/// Values are loaded from the user's config file (`~/.config/lottie/lottie.conf`)
/// and then overridden by any matching CLI flags.  Defaults match the shipped
/// `DEFAULT_CONFIG` template.
#[derive(Clone, Debug)]
pub struct Config {
    /// Display scene numbers in the left margin next to each scene heading.
    pub show_scene_numbers: bool,

    /// Display page numbers in the right margin at the first printable line of
    /// each new page.
    pub show_page_numbers: bool,

    /// Hide Fountain inline markup characters (asterisks, underscores) when the
    /// cursor is not on the same line.
    pub hide_markup: bool,

    /// Enable auto-completion for character names and scene heading locations.
    pub autocomplete: bool,

    /// Automatically append the [`contd_extension`](Config::contd_extension) string
    /// to a character cue when the same character speaks consecutively.
    pub auto_contd: bool,

    /// Automatically insert blank lines after action, dialogue, and similar
    /// elements when the user presses Enter at the end of a line.
    pub auto_paragraph_breaks: bool,

    /// Automatically insert a closing `)` when the user types `(`.
    pub match_parentheses: bool,

    /// Automatically insert closing delimiters for `[[`, `/*`, and `**` pairs.
    pub close_elements: bool,

    /// Insert a blank title-page template when creating a new empty file.
    pub auto_title_page: bool,

    /// Keep the cursor vertically centred in the viewport as the user types.
    pub typewriter_mode: bool,

    /// Like `typewriter_mode` but forces the active line to the exact centre of
    /// the terminal at all times, even at the beginning of the document.
    pub strict_typewriter_mode: bool,

    /// Hide the title bar and shortcut bar to maximise writing space.
    pub focus_mode: bool,

    /// Render the nearest action line above (or on) the cursor in bright white to
    /// indicate the currently active paragraph.
    pub highlight_active_action: bool,

    /// Allow action blocks to be split across page boundaries.
    ///
    /// When `false`, the layout engine attempts to keep each action block on a
    /// single page by pushing it to the next page if it would otherwise be split.
    pub break_actions: bool,

    /// Open files with the cursor positioned at the very end of the document.
    pub goto_end: bool,

    /// Disable all terminal colour output.  Text formatting (bold, italic, underline)
    /// is not affected unless `no_formatting` is also set.
    pub no_color: bool,

    /// Disable all bold, italic, and underline text modifiers.  Colour output is
    /// not affected unless `no_color` is also set.
    pub no_formatting: bool,

    /// Force the use of ASCII characters (e.g. `-` for page-break lines) instead
    /// of Unicode box-drawing characters.
    pub force_ascii: bool,

    /// Force emission of ANSI escape codes even when the terminal is not detected
    /// as supporting colour.  Overrides `no_color`.
    pub force_ansi: bool,

    /// The string appended to a character name for consecutive speech, e.g.
    /// `"(CONT'D)"`.
    pub contd_extension: String,

    /// Visual style applied to scene headings.  Accepted values: `"bold"`,
    /// `"underline"`, `"bold underline"`.
    pub heading_style: String,

    /// Minimum number of blank lines inserted before each scene heading by the
    /// layout engine.
    pub heading_spacing: usize,

    /// Visual style applied to shot lines (`!! text`).  Accepted values match
    /// [`heading_style`](Config::heading_style).
    pub shot_style: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_scene_numbers: true,
            show_page_numbers: true,
            hide_markup: true,

            autocomplete: true,
            auto_contd: true,
            auto_paragraph_breaks: true,
            match_parentheses: true,
            close_elements: true,
            auto_title_page: false,
            typewriter_mode: false,
            strict_typewriter_mode: false,
            focus_mode: false,
            highlight_active_action: false,
            break_actions: true,
            goto_end: false,

            contd_extension: "(CONT'D)".to_string(),
            heading_style: "bold".to_string(),
            heading_spacing: 1,
            shot_style: "bold".to_string(),

            no_color: false,
            no_formatting: false,
            force_ascii: false,
            force_ansi: false,
        }
    }
}

impl Config {
    /// Applies `set` / `unset` directives from a configuration file string to
    /// this `Config` instance.
    ///
    /// Lines that start with `#` are treated as comments and ignored.  Unknown
    /// keys are silently skipped for forward compatibility.  String values must
    /// be quoted with `"..."` in the file; the quotes are stripped during parsing.
    pub fn parse_config_str(&mut self, content: &str) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let cmd = parts[0];
                let key = parts[1];
                let val = if parts.len() > 2 {
                    parts[2..].join(" ").trim_matches('"').to_string()
                } else {
                    String::new()
                };

                if cmd == "set" {
                    match key {
                        "show_scene_numbers" => self.show_scene_numbers = true,
                        "show_page_numbers" => self.show_page_numbers = true,
                        "hide_markup" => self.hide_markup = true,
                        "autocomplete" => self.autocomplete = true,
                        "auto_contd" => self.auto_contd = true,
                        "auto_paragraph_breaks" => self.auto_paragraph_breaks = true,
                        "match_parentheses" => self.match_parentheses = true,
                        "close_elements" => self.close_elements = true,
                        "auto_title_page" => self.auto_title_page = true,
                        "typewriter_mode" => self.typewriter_mode = true,
                        "strict_typewriter_mode" => self.strict_typewriter_mode = true,
                        "focus_mode" => self.focus_mode = true,
                        "highlight_active_action" => self.highlight_active_action = true,
                        "break_actions" => self.break_actions = true,
                        "goto_end" => self.goto_end = true,
                        "contd_extension" => self.contd_extension = val,
                        "heading_style" => self.heading_style = val,
                        "heading_spacing" => {
                            if let Ok(v) = val.parse() {
                                self.heading_spacing = v
                            }
                        }
                        "shot_style" => self.shot_style = val,
                        "no_color" => self.no_color = true,
                        "no_formatting" => self.no_formatting = true,
                        "force_ascii" => self.force_ascii = true,
                        "force_ansi" => self.force_ansi = true,
                        _ => {}
                    }
                } else if cmd == "unset" {
                    match key {
                        "show_scene_numbers" => self.show_scene_numbers = false,
                        "show_page_numbers" => self.show_page_numbers = false,
                        "hide_markup" => self.hide_markup = false,
                        "autocomplete" => self.autocomplete = false,
                        "auto_contd" => self.auto_contd = false,
                        "auto_paragraph_breaks" => self.auto_paragraph_breaks = false,
                        "match_parentheses" => self.match_parentheses = false,
                        "close_elements" => self.close_elements = false,
                        "auto_title_page" => self.auto_title_page = false,
                        "typewriter_mode" => self.typewriter_mode = false,
                        "strict_typewriter_mode" => self.strict_typewriter_mode = false,
                        "focus_mode" => self.focus_mode = false,
                        "highlight_active_action" => self.highlight_active_action = false,
                        "break_actions" => self.break_actions = false,
                        "goto_end" => self.goto_end = false,
                        "no_color" => self.no_color = false,
                        "no_formatting" => self.no_formatting = false,
                        "force_ascii" => self.force_ascii = false,
                        "force_ansi" => self.force_ansi = false,
                        _ => {}
                    }
                }
            }
        }
    }

    /// Constructs a `Config` by loading the user's config file (creating it from
    /// the built-in template if absent) and then applying CLI overrides.
    ///
    /// Precedence (highest to lowest):
    /// 1. CLI flags passed at invocation time.
    /// 2. `~/.config/lottie/lottie.conf` (or the path given by `--config`).
    /// 3. Hard-coded [`Default`] values.
    ///
    /// Terminal capability detection (Unicode support, colour support) is also
    /// performed here; `force_ascii` and `no_color` are set automatically when
    /// the terminal does not advertise the relevant capabilities, unless
    /// `force_ansi` is set.
    pub fn load(cli: &Cli) -> Self {
        let mut config = Self::default();

        let is_custom_path = cli.config.is_some();
        let config_path = cli.config.clone().or_else(|| {
            directories::ProjectDirs::from("org", "coignard", "lottie")
                .map(|proj_dirs| proj_dirs.config_dir().join("lottie.conf"))
        });

        if let Some(path) = config_path {
            if !is_custom_path && !path.exists() {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(&path, DEFAULT_CONFIG);
            }

            match fs::read_to_string(&path) {
                Ok(content) => config.parse_config_str(&content),
                Err(e) if is_custom_path => {
                    eprintln!(
                        "Warning: Failed to load custom config file at '{}': {}",
                        path.display(),
                        e
                    );
                }
                _ => {}
            }
        }

        config.show_scene_numbers &= !cli.hide_scene_numbers;
        config.show_page_numbers &= !cli.hide_page_numbers;
        config.hide_markup &= !cli.show_markup;
        config.autocomplete &= !cli.no_autocomplete;
        config.auto_contd &= !cli.no_auto_contd;
        config.auto_paragraph_breaks &= !cli.no_auto_paragraph_breaks;
        config.match_parentheses &= !cli.no_match_parentheses;
        config.close_elements &= !cli.no_close_elements;
        config.break_actions &= !cli.no_break_actions;

        config.auto_title_page |= cli.auto_title_page;
        config.typewriter_mode |= cli.typewriter_mode;
        config.strict_typewriter_mode |= cli.strict_typewriter_mode;
        config.focus_mode |= cli.focus_mode;
        config.highlight_active_action |= cli.highlight_active_action;
        config.no_color |= cli.no_color;
        config.no_formatting |= cli.no_formatting;
        config.force_ascii |= cli.force_ascii;
        config.force_ansi |= cli.force_ansi;
        config.goto_end |= cli.goto_end;

        if let Some(ref ext) = cli.contd_extension {
            config.contd_extension = ext.clone();
        }
        if let Some(ref style) = cli.heading_style {
            config.heading_style = style.clone();
        }
        if let Some(spacing) = cli.heading_spacing {
            config.heading_spacing = spacing;
        }
        if let Some(ref style) = cli.shot_style {
            config.shot_style = style.clone();
        }

        let supports_unicode = supports_unicode::on(supports_unicode::Stream::Stdout);
        let supports_color = supports_color::on(supports_color::Stream::Stdout).is_some();

        config.force_ascii |= !supports_unicode;

        if config.force_ansi {
            config.no_color = false;
        } else if !supports_color {
            config.no_color = true;
        }

        config
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn test_config_default_values() {
        let config = Config::default();
        assert!(config.show_scene_numbers);
        assert!(config.show_page_numbers);
        assert!(config.hide_markup);
        assert!(config.autocomplete);
        assert!(config.auto_contd);
        assert!(config.auto_paragraph_breaks);
        assert!(config.match_parentheses);
        assert!(config.close_elements);
        assert!(!config.auto_title_page);
        assert!(!config.typewriter_mode);
        assert!(!config.focus_mode);
        assert!(config.break_actions);
        assert!(!config.no_color);
        assert!(!config.no_formatting);
        assert!(!config.force_ascii);
        assert!(!config.force_ansi);
        assert_eq!(config.contd_extension, "(CONT'D)");
        assert_eq!(config.heading_style, "bold");
        assert_eq!(config.heading_spacing, 1);
        assert_eq!(config.shot_style, "bold");
    }

    #[test]
    fn test_config_parsing_appearance_flags() {
        let mut config = Config::default();

        let mock_file_content = "
            set no_color
            set no_formatting
            set force_ascii
            set force_ansi
        ";

        config.parse_config_str(mock_file_content);

        assert!(config.no_color, "no_color should be set by parsing");
        assert!(
            config.no_formatting,
            "no_formatting should be set by parsing"
        );
        assert!(config.force_ascii, "force_ascii should be set by parsing");
        assert!(config.force_ansi, "force_ansi should be set by parsing");
    }

    #[test]
    fn test_cli_overrides_for_appearance() {
        let mut cli = Cli::default();
        cli.force_ascii = true;
        cli.no_color = true;
        cli.no_formatting = true;

        let config = Config::load(&cli);
        assert!(config.no_color);
        assert!(config.no_formatting);
        assert!(config.force_ascii);
        assert!(!config.force_ansi);
    }

    #[test]
    fn test_force_ansi_overrides_no_color() {
        let mut cli = Cli::default();
        cli.no_color = true;
        cli.force_ansi = true;

        let config = Config::load(&cli);
        assert!(
            !config.no_color,
            "force_ansi should override no_color to false"
        );
        assert!(config.force_ansi);
    }
}
