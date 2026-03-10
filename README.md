<p>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://github.com/coignard/lottie/blob/main/assets/logo.svg?raw=true">
    <source media="(prefers-color-scheme: light)" srcset="https://github.com/coignard/lottie/blob/main/assets/logo-dark.svg?raw=true">
    <img src="assets/logo.svg" alt="Lottie Logo" height="38">
  </picture>
</p>

[![CI](https://github.com/coignard/lottie/workflows/CI/badge.svg)](https://github.com/coignard/lottie/actions)
[![CodeQL](https://github.com/coignard/lottie/workflows/CodeQL/badge.svg)](https://github.com/coignard/lottie/security/code-scanning)
[![License: GPL-3.0-or-later](http://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
[![Ko-fi](https://img.shields.io/badge/Ko--fi-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/coignard)

A simple yet powerful screenwriting editor for the [Fountain](https://www.fountain.io/) plain-text screenplay format. Fast, lightweight, and built for writers who live in the terminal. Lottie is a Rust port of [Beat](https://www.beat-app.fi/), built with [ratatui](https://github.com/ratatui/ratatui).

[![asciicast](https://asciinema.org/a/1jYgFFAeaettGJZa.svg)](https://asciinema.org/a/1jYgFFAeaettGJZa)

## Install

To download the source code, build the lottie binary, and install it in `$HOME/.cargo/bin` in one go run:

```bash
cargo install --locked --git https://github.com/coignard/lottie
```

Alternatively, you can manually download the source code and build the lottie binary with:

```bash
git clone https://github.com/coignard/lottie
cd lottie
cargo build --release
sudo cp target/release/lottie /usr/local/bin/
```

## Usage

```bash
lottie screenplay.fountain
```

Open a new script:

```bash
lottie
```

## Features

```
     1      INT. TERMINAL - NIGHT                                             1.

            A cursor blinks. Text appears on screen, letter by letter.

                                LOTTIE
                       Hi there! I handle the Fountain
                       formatting as you go.

                                WRITER
                       What about markup? Do I have to
                       look at ugly asterisks all day?

                                LOTTIE
                       Only when your cursor is on the
                       line. Bold, italic, underlined is
                       hidden the rest of the time.

            The Writer makes a mistake. ^Z. Gone.

                                WRITER
                       I hate typing closing brackets.

                                LOTTIE
                            (smiling)
                       Don't. I close them for you. And
                       if you keep talking...

                                LOTTIE (CONT'D)
                       ...I'll add the (CONT'D) myself.

            /* PRO TIP: did you know that in typewriter mode, the active
            line stays fixed to the centre of the screen? */

            ────────────────────────────────────────────────────────────

     2      EXT. OUTSIDE A COMPUTER - DAY                                     2.

                                LOTTIE
                       Look. Scene numbers on the left.
                       Page numbers on the right. Both
                       automatic. Blank lines before
                       headings are configurable.

                                WRITER
                       What about autocompletion?

                                LOTTIE
                       I pick up character names and
                       locations as you write. Tab
                       finishes the word and cycles the
                       line type while you're at it.

            /* PRO TIP: did you know that this screenplay was exported
            directly from Lottie using --export? */

                                LOTTIE (CONT'D)
                       See you space cowboy...

                                                               FADE OUT.
```


## Keyboard shortcuts

|  Key  |        Action        |
|-------|----------------------|
| `^S`  | Save                 |
| `^X`  | Exit                 |
| `^K`  | Cut line             |
| `^U`  | Paste                |
| `^Z`  | Undo                 |
| `^R`  | Redo                 |
| `^W`  | Search               |
| `^C`  | Cursor position      |
| `Tab` | Cycle element types  |

## Configuration

Lottie reads from `~/.config/lottie/lottie.conf`. Example:

```
## Lottie configuration file
## Place this file at ~/.config/lottie/lottie.conf
##
## Use "set <option>" to enable a boolean option or assign a value.
## Use "unset <option>" to disable a boolean option.

## Editor View

# Show scene numbers in the left margin.
set show_scene_numbers

# Show page numbers on the right side of the screen.
set show_page_numbers

# Automatically hide Fountain markup when the cursor
# is not on the current line.
set hide_markup

# Typewriter mode
unset typewriter_mode

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

# Styling applied to scene headings.
# Available values: "bold", "underline", "bold underline"
set heading_style "bold"

# Number of blank lines before a scene heading.
# Set to 2 for double spacing before each new scene.
set heading_spacing 1

# Styling applied to shots (e.g. !! CLOSE UP).
# Available values: "bold", "underline", "bold underline"
set shot_style "bold"

## Display & Terminal

# Disable all terminal colors. Lottie will still render bold,
# italic, and underline modifiers if supported by your terminal.
# Lottie tries to detect color support automatically.
unset no_color

# Disable all text formatting (bold, italic, underline).
unset no_formatting

# Force output of ANSI color escape codes, even if Lottie detects
# that your terminal does not support them.
unset force_ansi

# Force the use of ASCII characters instead of Unicode
# (e.g., for page break lines). Useful for older terminals.
# Lottie will try to detect Unicode support automatically.
unset force_ascii
```

A sample config is also included here in the repository.

## CLI options

```
--hide-scene-numbers         Hide scene numbers
--hide-page-numbers          Hide page numbers
--show-markup                Show formatting markup
--no-autocomplete            Disable autocomplete
--no-auto-contd              Disable automatic (CONT'D)
--no-auto-paragraph-breaks   Disable auto paragraph breaks
--no-match-parentheses       Disable matching parentheses
--no-close-elements          Disable auto-closing elements
--auto-title-page            Generate title page if file is new
--typewriter-mode            Enable typewriter mode
--focus-mode                 Enable focus mode
--no-break-actions           Disable breaking actions across pages
--contd-extension <text>     Set the (CONT'D) extension text
--heading-style <style>      Set heading style, e.g. "bold underline"
--heading-spacing <n>        Set spacing before scene headings
--shot-style <style>         Set shot style, e.g. "bold"
--no-color                   Disable color formatting
--no-formatting              Disable text formatting (bold, italic, underline)
--force-ascii                Use ASCII characters instead of Unicode
--force-ansi                 Force ANSI color output even if unsupported by the terminal
--export [FILE]              Export rendered script to file or stdout
--format <FORMAT>            Export format: plain, ascii, ansi (default: plain)
```

## Test

```bash
cargo test
```

## License

© 2026 René Coignard.

All code is licensed under the GPL, v3 or later. See [LICENSE](./LICENSE) file
for details.
