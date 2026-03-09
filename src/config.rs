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

    /// Export the rendered script to a file
    #[arg(long, value_name = "FILE")]
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

    pub contd_extension: String,
    pub break_actions: bool,
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

            contd_extension: "(CONT'D)".to_string(),
            break_actions: true,
            heading_style: "bold".to_string(),
            heading_spacing: 1,
            shot_style: "bold".to_string(),
        }
    }
}

impl Config {
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
                            "show_scene_numbers" => config.show_scene_numbers = true,
                            "show_page_numbers" => config.show_page_numbers = true,
                            "hide_markup" => config.hide_markup = true,
                            "autocomplete" => config.autocomplete = true,
                            "auto_contd" => config.auto_contd = true,
                            "auto_paragraph_breaks" => config.auto_paragraph_breaks = true,
                            "match_parentheses" => config.match_parentheses = true,
                            "close_elements" => config.close_elements = true,
                            "auto_title_page" => config.auto_title_page = true,
                            "typewriter_mode" => config.typewriter_mode = true,
                            "break_actions" => config.break_actions = true,
                            "contd_extension" => config.contd_extension = val,
                            "heading_style" => config.heading_style = val,
                            "heading_spacing" => {
                                if let Ok(v) = val.parse() {
                                    config.heading_spacing = v
                                }
                            }
                            "shot_style" => config.shot_style = val,
                            _ => {}
                        }
                    } else if cmd == "unset" {
                        match key {
                            "show_scene_numbers" => config.show_scene_numbers = false,
                            "show_page_numbers" => config.show_page_numbers = false,
                            "hide_markup" => config.hide_markup = false,
                            "autocomplete" => config.autocomplete = false,
                            "auto_contd" => config.auto_contd = false,
                            "auto_paragraph_breaks" => config.auto_paragraph_breaks = false,
                            "match_parentheses" => config.match_parentheses = false,
                            "close_elements" => config.close_elements = false,
                            "auto_title_page" => config.auto_title_page = false,
                            "typewriter_mode" => config.typewriter_mode = false,
                            "break_actions" => config.break_actions = false,
                            _ => {}
                        }
                    }
                }
            }
        }

        // Apply CLI overrides over loaded config
        if cli.hide_scene_numbers {
            config.show_scene_numbers = false;
        }
        if cli.hide_page_numbers {
            config.show_page_numbers = false;
        }
        if cli.show_markup {
            config.hide_markup = false;
        }
        if cli.no_autocomplete {
            config.autocomplete = false;
        }
        if cli.no_auto_contd {
            config.auto_contd = false;
        }
        if cli.no_auto_paragraph_breaks {
            config.auto_paragraph_breaks = false;
        }
        if cli.no_match_parentheses {
            config.match_parentheses = false;
        }
        if cli.no_close_elements {
            config.close_elements = false;
        }
        if cli.auto_title_page {
            config.auto_title_page = true;
        }
        if cli.typewriter_mode {
            config.typewriter_mode = true;
        }
        if cli.no_break_actions {
            config.break_actions = false;
        }
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
        assert!(config.break_actions);
        assert_eq!(config.contd_extension, "(CONT'D)");
        assert_eq!(config.heading_style, "bold");
        assert_eq!(config.heading_spacing, 1);
        assert_eq!(config.shot_style, "bold");
    }
}
