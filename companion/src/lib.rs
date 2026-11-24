//! Scarlet companion library: reads the addon's SavedVariables chunks
//! (docs/FORMAT.md), exports them to text and zstd payloads, and renders
//! events. Shared by the `scarlet` CLI and the desktop app.

#[macro_export]
macro_rules! out {
    () => {
        $crate::out!("")
    };
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _ = ::std::writeln!(::std::io::stdout().lock(), $($arg)*);
    }};
}

#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _ = ::std::writeln!(::std::io::stderr().lock(), $($arg)*);
    }};
}

pub mod addons;
pub mod archive;
pub mod counts;
pub mod dungeonjournal;
pub mod events;
pub mod export;
pub mod fishing;
pub mod foreverfishing;
pub mod gamedir;
pub mod import;
pub mod importarchive;
pub mod inventory;
pub mod legacy;
pub mod lifetime;
pub mod loot;
pub mod lua;
pub mod mastercraft;
pub mod naowhforever;
pub mod overview;
pub mod rxp;
pub mod tables;
pub mod talents;
pub mod upload;

pub const DEFAULT_WOW: &str = r"F:\Games\World of Warcraft";
