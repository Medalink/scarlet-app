//! Uploading to the Scarlet website (docs/API.md of the scarlet-web repo).
//!
//! Off until the player turns it on. The module builds the parts with the
//! `scarlet-part` crate, so what leaves the PC is exactly what that crate
//! hashes, and talks to the API with a small blocking client. Nothing here
//! touches recording or the local data other than `upload-state.json`.

pub mod api;
pub mod cli;
pub mod config;
pub mod counts;
pub mod keys;
pub mod session;
pub mod state;

use std::fmt;

/// A failure of an upload step, in words a player can read.
#[derive(Debug, Clone)]
pub enum Error {
    /// The server answered with its error shape.
    Api {
        status: u16,
        code: String,
        message: String,
        extra: serde_json::Value,
    },
    /// No answer: DNS, connection, TLS, timeout.
    Net(String),
    /// Anything on this PC: settings, key store, files.
    Local(String),
}

impl Error {
    pub fn local(m: impl Into<String>) -> Error {
        Error::Local(m.into())
    }

    /// True when trying again later may work: no answer, a 5xx or a 429.
    pub fn transient(&self) -> bool {
        match self {
            Error::Net(_) => true,
            Error::Api { status, .. } => *status == 429 || *status >= 500,
            Error::Local(_) => false,
        }
    }

    /// The server's error code, when the server answered.
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Api { code, .. } => Some(code),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Api {
                status,
                code,
                message,
                ..
            } => write!(f, "the server refused ({status} {code}): {message}"),
            Error::Net(m) => write!(f, "could not reach the server: {m}"),
            Error::Local(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
