# Changelog

## 0.2.15

### Added
- Dual dialogue layout: `DualDialogueCharacter`, `Dialogue`, `Parenthetical`, and `Empty` lines inside a dual dialogue block now receive a +10 column indent offset, correctly positioning two characters' lines side by side on the page

### Changed
- Version bumped to 0.2.15

### Removed
- Removed `assets/demo.gif`

## 0.2.14

### Added
- Explicit scene numbers: scenes can now be tagged with `#5#`, `#6A#`, `#B#` etc.; the counter syncs to numeric tags and continues from there
- Tab autocomplete now correctly forces `@` prefix for character names ending with a dot (e.g. `R.C.`) and `.` prefix for locations not already prefixed

### Fixed
- Scene number display now uses Unicode-aware width calculation instead of byte length
- Autocomplete suggestion acceptance no longer drops the sigil for names that require `@` or `.` to parse correctly

### Changed
- `scene_num` field type changed from `Option<usize>` to `Option<String>` to support alphanumeric scene numbers
- Scene number regex replaced with `^(.*?)\s*#([^#]+)#\s*$` — anchored, simpler, no character class issues
- `rust-version` bumped to `1.94`
- Version bump to 0.2.14

## 0.2.13

### Added
- Tab autocomplete for character names without `@` prefix: pressing Tab on an Action line now shows ghost text if a match exists, falling back to `@` if not
- Accepting a suggestion via Tab now uppercases the typed prefix automatically

### Fixed
- Exit cleanup no longer clears the screen on non-Linux terminals; screen clear on exit is now limited to `TERM=linux`

### Changed
- `parse_formatting` returns early for lines without markup characters (fast path via byte scan)
- `build_layout` skips `parse_formatting` entirely for lines without markup, reusing a shared empty `LineFormatting`
- `is_pure_space` and `get_visual_width` merged into single-pass `token_metrics` returning trimmed width, total width, and purity flag
- `current_line` buffer hoisted out of the per-line loop to avoid repeated allocations
- `display` and `final_display` use `Cow<str>` to avoid cloning when no transformation is needed
- Note-stripping for scene headings/sections/synopses now guarded by `display.contains("[[")` check
- Scene number regex now pre-checked with `ends_with('#')` before invoking the regex engine
- Version bump to 0.2.13

## 0.2.12

### Added
- Criterion benchmarks for parser and layout engine

### Changed
- `name = "lottie"` added to clap command for consistent `--version` output
- Homepage and repository URLs updated in `Cargo.toml`
- `TokenizeText` zero-allocation iterator replaces `tokenize_text` (vec-based)
- `Rc<LineFormatting>` shared across visual rows of the same logical line instead of cloning
- `is_scene_heading` avoids `to_uppercase()` allocation using `eq_ignore_ascii_case`
- `find_pairs` in formatting parser takes `&[char]` instead of allocating `Vec<char>`
- `build_layout`: 21.6ms → 12.6ms (+41.6%), `Parser::parse`: 2.55ms → 2.39ms (+6.1%) on 10,000 lines
- Version bump to 0.2.12, closes #6

## 0.2.11

### Added
- Sponsors section in README

### Fixed
- Shot lines now render in uppercase in editor and export
- Enhanced TTY support

### Changed
- Undo history limit increased to 640
- Logo files corrected for light/dark theme
- Repository URL updated to lottie.rs

## 0.2.10

### Added

- API documentation for all public items
- Doc-tests for `StringCaseExt::to_uppercase_1to1`, `strip_sigils`, `Parser::is_transition_format`, and `Parser::is_uppercase_content`
- `rust-version = "1.90"` in `Cargo.toml`

### Fixed

- Autocomplete now works for forced scene headings (`.HEADING`) and prefixes without a dot (`INT `, `INT/EXT.` etc.)
- Visual width calculation now correctly respects `hide_markup` and active line state, fixing wrapping of lines with inline markup
- Page navigation (`PageUp`/`PageDown`) now uses correct single-jump logic instead of repeated single-row moves

### Changed

- `main.rs` now imports from the `lottie` crate instead of local modules
- Updated project description and README links

## 0.2.9

### Added

- Workflow to mirror repository to Codeberg
- Workflow for automated releases
- SECURITY.md

### Fixed

- Prevent wide unicode characters from exceeding line width limit
- Fix CI permissions in mirror workflow

### Changed

- Bump `clap` from 4.5.60 to 4.6.0
- Bump `actions/checkout` from v3 to v6

## 0.2.8

### Added

- Multi-buffer support

## 0.2.7

### Added

- `goto_end` flag support

### Changed

- README clarifications for Fountain editor usage

## 0.2.6

### Fixed

- Add `LineType::Centered` and `LineType::PageBreak` to `breaks_paragraph`

## 0.2.5

### Added

- Strict typewriter mode
- Active action highlighting in UI

### Fixed

- Fix `test_draw_active_action_highlight` for CI/CD

## 0.2.4

### Added

- Ensure trailing newline on save
- Page navigation

### Fixed

- Fix rendering artifacts
- Fix cursor position reporting

## 0.2.3

### Added

- Search highlighting with regex and wrap-around detection
- Config flags: `--no-color`, `--no-formatting`, `--force-ascii`, `--force-ansi`

## 0.2.2

### Fixed

- Preserve spaces on empty lines
- Add UX tests

## 0.2.1

### Added

- Focus mode
- Export to stdout
- Refactored `render_inline`

## 0.2.0

### Added

- Export support

## 0.1.3

### Changed

- Release housekeeping

## 0.1.2

### Fixed

- Correct typewriter centering
- Use compile-time version in draw

## 0.1.1

### Added

- Initial commit
- License notices added to source code

### Fixed

- Hard-wrap long words exceeding line width
