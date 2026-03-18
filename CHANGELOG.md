# Changelog

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
