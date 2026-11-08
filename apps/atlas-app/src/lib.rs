//! Shared native and browser historical map application.

pub mod application;
pub mod catalog;
pub mod error;
pub mod host;
pub mod navigation;
pub mod scene;
pub mod selection;

#[cfg(not(target_arch = "wasm32"))]
pub mod publication;
#[cfg(target_arch = "wasm32")]
pub mod web;
