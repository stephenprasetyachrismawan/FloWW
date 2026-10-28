//! Domain logic for FloWW.
//!
//! This crate has no Tauri dependency so it can be tested on any machine.
//! The desktop app (`src-tauri`) is a thin layer that exposes these
//! functions as commands.

pub mod claude;
pub mod config;
pub mod error;
pub mod odoo;
pub mod pipeline;
pub mod prd_id;

pub use error::{Error, Result};
