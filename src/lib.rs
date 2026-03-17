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

//! # Lottie
//!
//! `lottie` is a simple yet powerful Fountain screenplay editor.
//!
//! This library exposes the core engine of Lottie, allowing other developers to:
//! - Parse Fountain documents ([`parser`], [`types`]).
//! - Generate terminal user interface layouts ([`layout`], [`formatting`]).
//! - Export scripts to different formats ([`export`]).
//! - Embed the editor application ([`app`], [`config`]).
//!
//! # Architecture
//!
//! The pipeline flows as follows:
//!
//! ```text
//! Raw text lines
//!     → parser::Parser::parse()  → Vec<LineType>
//!     → layout::build_layout()   → Vec<VisualRow>
//!     → app::draw() / export::export_document()
//! ```

#![warn(missing_docs)]

/// The interactive TUI editor application, event loop, and rendering logic.
pub mod app;

/// CLI argument parsing and runtime configuration loading.
pub mod config;

/// Plain-text and ANSI export of a laid-out screenplay.
pub mod export;

/// Inline markdown parsing (`**bold**`, `*italic*`, `_underline_`, notes, boneyard)
/// and span rendering for ratatui.
pub mod formatting;

/// Visual layout engine: word-wrapping, indentation, page numbering, scene numbering.
pub mod layout;

/// Fountain markup language parser that classifies each line as a [`types::LineType`].
pub mod parser;

/// Core type definitions: [`types::LineType`], [`types::Fmt`], style helpers, and layout constants.
pub mod types;
