use crate::config::Config;
use ratatui::style::{Color, Modifier, Style};

pub const PAGE_WIDTH: u16 = 61;
pub const LINES_PER_PAGE: usize = 55;

#[derive(Clone, Copy)]
pub struct Fmt {
    pub indent: u16,
    pub width: u16,
}

impl Fmt {
    pub const fn new(indent: u16, width: u16) -> Self {
        Self { indent, width }
    }
}

pub const FMT_ACTION: Fmt = Fmt::new(0, 61);
pub const FMT_SCENE: Fmt = Fmt::new(0, 61);
pub const FMT_CHARACTER: Fmt = Fmt::new(20, 38);
pub const FMT_DIALOGUE: Fmt = Fmt::new(11, 35);
pub const FMT_PAREN: Fmt = Fmt::new(16, 31);
pub const FMT_TRANSITION: Fmt = Fmt::new(0, 61);
pub const FMT_CENTERED: Fmt = Fmt::new(0, 61);
pub const FMT_LYRICS: Fmt = Fmt::new(0, 61);
pub const FMT_SECTION: Fmt = Fmt::new(0, 61);
pub const FMT_SYNOPSIS: Fmt = Fmt::new(0, 61);
pub const FMT_NOTE: Fmt = Fmt::new(0, 61);
pub const FMT_METADATA_KEY: Fmt = Fmt::new(10, 51);
pub const FMT_METADATA_VAL: Fmt = Fmt::new(12, 49);
pub const FMT_METADATA_TITLE: Fmt = Fmt::new(10, 51);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Empty,
    MetadataTitle,
    MetadataKey,
    MetadataValue,
    SceneHeading,
    Action,
    Character,
    DualDialogueCharacter,
    Parenthetical,
    Dialogue,
    Transition,
    Centered,
    Lyrics,
    Section,
    Synopsis,
    Note,
    Boneyard,
    PageBreak,
    Shot,
}

impl LineType {
    pub fn fmt(self) -> Fmt {
        match self {
            Self::SceneHeading | Self::Shot => FMT_SCENE,
            Self::Character | Self::DualDialogueCharacter => FMT_CHARACTER,
            Self::Dialogue => FMT_DIALOGUE,
            Self::Parenthetical => FMT_PAREN,
            Self::Transition => FMT_TRANSITION,
            Self::Centered => FMT_CENTERED,
            Self::Lyrics => FMT_LYRICS,
            Self::Section => FMT_SECTION,
            Self::Synopsis => FMT_SYNOPSIS,
            Self::Note | Self::Boneyard => FMT_NOTE,
            Self::MetadataTitle => FMT_METADATA_TITLE,
            Self::MetadataKey => FMT_METADATA_KEY,
            Self::MetadataValue => FMT_METADATA_VAL,
            _ => FMT_ACTION,
        }
    }
}

pub fn base_style(lt: LineType, config: &Config) -> Style {
    match lt {
        LineType::SceneHeading => {
            let mut style = Style::default().fg(Color::White);
            if config.heading_style.contains("bold") {
                style = style.add_modifier(Modifier::BOLD);
            }
            if config.heading_style.contains("underline") {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            style
        }
        LineType::Shot => {
            let mut style = Style::default().fg(Color::White);
            if config.shot_style.contains("bold") {
                style = style.add_modifier(Modifier::BOLD);
            }
            if config.shot_style.contains("underline") {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            style
        }
        LineType::Character | LineType::DualDialogueCharacter => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        LineType::Parenthetical => Style::default().fg(Color::Gray),
        LineType::Dialogue => Style::default().fg(Color::White),
        LineType::Transition => Style::default(),
        LineType::Centered => Style::default(),
        LineType::Lyrics => Style::default().add_modifier(Modifier::ITALIC),
        LineType::Section | LineType::Synopsis => Style::default().fg(Color::Green),
        LineType::Note | LineType::Boneyard => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::ITALIC),
        LineType::MetadataTitle | LineType::MetadataKey | LineType::MetadataValue => {
            Style::default().fg(Color::White)
        }
        LineType::PageBreak => Style::default().fg(Color::DarkGray),
        LineType::Action | LineType::Empty => Style::default(),
    }
}

pub fn get_marker_color(note_text: &str) -> Option<Color> {
    let lower = note_text.to_lowercase();
    if lower.contains("red") {
        Some(Color::Red)
    } else if lower.contains("blue") {
        Some(Color::Blue)
    } else if lower.contains("green") {
        Some(Color::Green)
    } else if lower.contains("pink") || lower.contains("magenta") {
        Some(Color::Magenta)
    } else if lower.contains("cyan") || lower.contains("teal") {
        Some(Color::Cyan)
    } else if lower.contains("yellow") {
        Some(Color::Yellow)
    } else if lower.contains("orange") || lower.contains("brown") {
        Some(Color::Rgb(255, 165, 0))
    } else if lower.contains("gray") {
        Some(Color::Gray)
    } else if lower.starts_with("marker") {
        Some(Color::Rgb(255, 165, 0))
    } else {
        None
    }
}

#[cfg(test)]
mod types_tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    #[test]
    fn test_fmt_dimensions_action() {
        let fmt = LineType::Action.fmt();
        assert_eq!(fmt.indent, 0);
        assert_eq!(fmt.width, 61);
    }

    #[test]
    fn test_fmt_dimensions_character() {
        let fmt = LineType::Character.fmt();
        assert_eq!(fmt.indent, 20);
        assert_eq!(fmt.width, 38);
    }

    #[test]
    fn test_fmt_dimensions_dialogue() {
        let fmt = LineType::Dialogue.fmt();
        assert_eq!(fmt.indent, 11);
        assert_eq!(fmt.width, 35);
    }

    #[test]
    fn test_fmt_dimensions_parenthetical() {
        let fmt = LineType::Parenthetical.fmt();
        assert_eq!(fmt.indent, 16);
        assert_eq!(fmt.width, 31);
    }

    #[test]
    fn test_fmt_dimensions_metadata() {
        let fmt = LineType::MetadataKey.fmt();
        assert_eq!(fmt.indent, 10);
        assert_eq!(fmt.width, 51);
        let fmt_val = LineType::MetadataValue.fmt();
        assert_eq!(fmt_val.indent, 12);
        assert_eq!(fmt_val.width, 49);
    }

    #[test]
    fn test_base_style_default_heading() {
        let config = Config::default();
        let style = base_style(LineType::SceneHeading, &config);
        assert_eq!(style.fg, Some(Color::White));
        assert!(style.add_modifier.contains(Modifier::BOLD));
        assert!(!style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_base_style_custom_heading() {
        let mut config = Config::default();
        config.heading_style = "underline".to_string();
        let style = base_style(LineType::SceneHeading, &config);
        assert!(!style.add_modifier.contains(Modifier::BOLD));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_base_style_custom_shot() {
        let mut config = Config::default();
        config.shot_style = "bold underline".to_string();
        let style = base_style(LineType::Shot, &config);
        assert!(style.add_modifier.contains(Modifier::BOLD));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_base_style_character() {
        let config = Config::default();
        let style = base_style(LineType::Character, &config);
        assert_eq!(style.fg, Some(Color::White));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_base_style_lyrics() {
        let config = Config::default();
        let style = base_style(LineType::Lyrics, &config);
        assert!(style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_get_marker_color_basic() {
        assert_eq!(get_marker_color("red"), Some(Color::Red));
        assert_eq!(get_marker_color("blue text"), Some(Color::Blue));
        assert_eq!(get_marker_color("green background"), Some(Color::Green));
        assert_eq!(get_marker_color("magenta note"), Some(Color::Magenta));
        assert_eq!(get_marker_color("cyan marker"), Some(Color::Cyan));
        assert_eq!(get_marker_color("yellow"), Some(Color::Yellow));
        assert_eq!(get_marker_color("gray area"), Some(Color::Gray));
    }

    #[test]
    fn test_get_marker_color_aliases() {
        assert_eq!(get_marker_color("pink box"), Some(Color::Magenta));
        assert_eq!(get_marker_color("teal"), Some(Color::Cyan));
        assert_eq!(get_marker_color("orange"), Some(Color::Rgb(255, 165, 0)));
        assert_eq!(get_marker_color("brown"), Some(Color::Rgb(255, 165, 0)));
    }

    #[test]
    fn test_get_marker_color_fallback() {
        assert_eq!(
            get_marker_color("marker custom"),
            Some(Color::Rgb(255, 165, 0))
        );
        assert_eq!(get_marker_color("just a plain note"), None);
    }
}
