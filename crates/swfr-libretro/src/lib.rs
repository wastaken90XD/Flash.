//! libretro frontend bindings (`cdylib` and `rlib`) for `swfr` (TSD 2.5, 2.6).
//!
//! `unsafe` is denied across this crate except inside [`ffi`], which encapsulates all raw
//! C pointers, `extern "C"` entrypoints, and callback invocations behind `catch_unwind`.

pub mod convert;
#[allow(unsafe_code)]
pub mod ffi;
pub mod input_map;
pub mod options;
pub mod state;
