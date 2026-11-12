//! Shared wire protocol, competitive rank models, profile DTOs, and domain events.
//!
//! Pure Rust + `serde` only — shared cleanly across `server-backend`, `client-core`,
//! `tui-client`, and `wasm-client`.

pub mod events;
pub mod profile;
pub mod ranks;
pub mod ws;

pub use events::DomainEvent;
pub use profile::*;
pub use ranks::*;
pub use ws::{ClientMsg, ServerMsg};
