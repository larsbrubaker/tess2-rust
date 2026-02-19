// tess2-rust: Pure Rust port of libtess2 (SGI tessellation library)
// Copyright 2025 Lars Brubaker
// License: SGI Free Software License B (MIT-compatible)

pub mod bucketalloc;
pub mod dict;
pub mod geom;
pub mod mesh;
pub mod priorityq;
pub mod sweep;
pub mod tess;

pub use tess::{ElementType, TessOption, TessStatus, Tessellator, TessellatorApi, WindingRule};
