//! snipd's Rust half: the cxx-qt bridge (`Engine`, QML module Snip.Core) and
//! the engine behind it. Linked into the `snipd` executable by CMake.

pub mod bridge;
mod edits;
mod engine;
mod frames;
mod watch;
