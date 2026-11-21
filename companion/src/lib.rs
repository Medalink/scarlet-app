//! Scarlet companion library: reads the addon's SavedVariables chunks
//! (docs/FORMAT.md), exports them to text and zstd payloads, and renders
//! events. Shared by the `scarlet` CLI and the desktop app.

pub mod addons;
pub mod archive;
pub mod events;
pub mod export;
pub mod loot;
pub mod lua;
pub mod overview;
pub mod tables;

pub const DEFAULT_WOW: &str = r"F:\Games\World of Warcraft";
