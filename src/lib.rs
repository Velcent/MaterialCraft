//! MaterialCraft: procedural node-based material authoring in pure Rust.
//! The core library has no external dependencies; it builds on Windows, Linux and macOS.
pub mod bake;
pub mod checksum;
pub mod deflate;
pub mod exr;
pub mod export;
pub mod expr;
pub mod graph;
pub mod half;
#[cfg(feature = "gui")]
pub mod gpu3d;
pub mod jpeg;
pub mod json;
pub mod mesh;
pub mod noise;
pub mod nodes2;
pub mod ops;
pub mod paint;
pub mod png;
pub mod render3d;
pub mod tiff;
pub mod tex;
pub mod v3;
pub mod zip;
