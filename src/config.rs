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

#[derive(Parser, Debug, Default, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The fountain file to open
    pub file: Option<PathBuf>,

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

    /// Enable focus mode
    #[arg(long)]
    pub focus_mode: bool,

    /// Disable breaking actions across pages
    #[arg(long)]
    pub no_break_actions: bool,

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

#[derive(Clone, Debug)]
pub struct Config {
    pub show_scene_numbers: bool,
    pub show_page_numbers: bool,
    pub hide_markup: bool,

    pub autocomplete: bool,
    pub auto_contd: bool,
    pub auto_paragraph_breaks: bool,
    pub match_parentheses: bool,
    pub close_elements: bool,
    pub auto_title_page: bool,
    pub typewriter_mode: bool,
    pub focus_mode: bool,
    pub break_actions: bool,

    pub no_color: bool,
    pub no_formatting: bool,
    pub force_ascii: bool,
    pub force_ansi: bool,

    pub contd_extension: String,
    pub heading_style: String,
    pub heading_spacing: usize,
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
            focus_mode: false,
            break_actions: true,

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
                        "focus_mode" => self.focus_mode = true,
                        "break_actions" => self.break_actions = true,
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
                        "focus_mode" => self.focus_mode = false,
                        "break_actions" => self.break_actions = false,
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

    pub fn load(cli: &Cli) -> Self {
        let mut config = Self::default();

        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config")
            });
        let config_path = config_dir.join("lottie").join("lottie.conf");

        if let Ok(content) = fs::read_to_string(&config_path) {
            config.parse_config_str(&content);
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
        config.focus_mode |= cli.focus_mode;
        config.no_color |= cli.no_color;
        config.no_formatting |= cli.no_formatting;
        config.force_ascii |= cli.force_ascii;
        config.force_ansi |= cli.force_ansi;

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
