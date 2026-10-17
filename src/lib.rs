// SPDX-License-Identifier: MIT OR Apache-2.0
//! MaterialCraft: procedural node-based material authoring in pure Rust.
//! The core library has no external dependencies; it builds on Windows, Linux and macOS.
pub mod assets;
pub mod bake;
pub mod camera;
pub mod checksum;
#[cfg(any(feature = "cms", feature = "craft"))]
pub mod colorspace;
pub mod curve;
pub mod deflate;
pub mod diag;
pub mod export;
pub mod expr;
pub mod exr;
pub mod font;
pub mod func;
pub mod fxmap;
pub mod generators;
// main's wgpu compute engine reinterprets f32/u64 slices as bytes; the one other unsafe exception (AGENTS.md rule 6).
#[allow(unsafe_code)]
pub mod gpu;
#[cfg(feature = "gui")]
pub mod gpu3d;
pub mod graph;
pub mod grunge;
pub mod half;
pub mod history;
pub mod jpeg;
pub mod json;
pub mod material_fill;
pub mod mesh;
pub mod nodes2;
pub mod noise;
pub mod ops;
pub mod paint;
pub mod png;
pub mod projection;
pub mod render3d;
pub mod safeio;
pub mod scatter;
pub mod stamp;
pub mod tex;
pub mod tiff;
pub mod v3;
pub mod vector;
pub mod zip;
