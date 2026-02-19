# tess2-rust

A pure Rust port of [libtess2](https://github.com/memononen/libtess2), the SGI tessellation library refactored by Mikko Mononen.

This is an exact mathematical 1-to-1 port of the C library, preserving all algorithmic behavior including edge cases.

## Features

- Polygon tessellation with configurable winding rules
- Multiple output element types (triangles, polygons, boundary contours)
- Support for self-intersecting and complex polygons
- No unsafe code, no external dependencies
- WASM-compatible

## Usage

```rust
use tess2_rust::{Tessellator, WindingRule, ElementType};

let mut tess = Tessellator::new();
tess.add_contour(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
tess.tessellate(WindingRule::Odd, ElementType::Polygons, 3);

let vertices = tess.vertices();
let elements = tess.elements();
```

## Winding Rules

- `Odd` - Fill regions with odd winding number (like even-odd fill)
- `NonZero` - Fill regions with non-zero winding number
- `Positive` - Fill regions with positive winding number
- `Negative` - Fill regions with negative winding number
- `AbsGeqTwo` - Fill regions with winding number >= 2 in absolute value

## Demo

Live WASM demo: https://larsbrubaker.github.io/tess2-rust/

## License

SGI Free Software License B (functionally equivalent to MIT). See [LICENSE](LICENSE).

## C++ Reference

The original C source files are preserved in `cpp_reference/libtess2/` for reference.
