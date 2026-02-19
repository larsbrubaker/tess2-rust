---
name: file-size-refactoring
description: "This skill provides guidance for fixing file size violations detected by file_compliance tests. Use when a Rust source file exceeds its line limit (800 lines default, or explicit limit for legacy files). The skill explains strategies to reduce file size while maintaining code quality."
---

# File Size Refactoring

This skill provides strategies for reducing file size when the file compliance test reports a violation.

## When This Applies

The test counts **non-empty lines** (excluding blank lines and whitespace-only lines). A file fails when it exceeds:
- **800 lines** (default limit for all files)
- **Explicit limit** for legacy files listed in `EXPLICIT_FILE_LIMITS` in `tests/file_compliance.rs`

## Quick Fixes (for small overages of 1-10 lines)

When a file is only slightly over the limit, prefer minimal changes:

1. **Remove unnecessary blank lines** — Look for double blank lines or blank lines inside functions that don't aid readability
2. **Consolidate short statements** — Combine related single-line statements where it doesn't hurt readability
3. **Remove dead code** — Look for commented-out code, unused imports, or unreachable branches
4. **Simplify conditionals** — Early returns can sometimes eliminate nesting

## Refactoring Strategies (for larger overages)

When a file significantly exceeds the limit, extract cohesive functionality:

### 1. Extract by Responsibility

Identify distinct responsibilities and move them to separate modules:
- Helper functions that form a cohesive group → new submodule
- Geometric utilities → `geom.rs` (already exists)
- Data structure operations → separate module
- Algorithm phases → separate modules

### 2. Extract by Feature

Group related functions that implement a specific feature:
- All functions related to "sweep line" → `sweep.rs`
- All functions related to "mesh operations" → `mesh.rs`
- All functions related to "output generation" → separate module

### 3. Extract Impl Blocks

When a struct has a very large `impl` block, split into multiple files:

```rust
// tess.rs - core struct and primary methods
pub struct Tessellator { ... }

impl Tessellator {
    pub fn new() -> Self { ... }
    pub fn tessellate(...) -> bool { ... }
}

// tess_output.rs - output-related methods
impl Tessellator {
    pub fn vertices(&self) -> &[f32] { ... }
    pub fn elements(&self) -> &[i32] { ... }
    pub fn element_count(&self) -> usize { ... }
}
```

### 4. Extract Constants and Types

Large constant arrays, lookup tables, or type definitions can often be moved to dedicated modules.

## What NOT to Do

- **Don't increase `EXPLICIT_FILE_LIMITS`** — That dict is only for freezing existing large files at their current size. Limits should only ever decrease.
- **Don't sacrifice readability** — If consolidating code makes it harder to understand, don't do it
- **Don't create artificial splits** — Extracted modules should represent cohesive functionality, not arbitrary chunks
- **Don't just delete comments** — Comments don't count toward the line limit anyway (only non-empty lines are counted)

## After Refactoring

1. Run `cargo test --test file_compliance` to verify the fix
2. Run `cargo test` to ensure no regressions
3. If a legacy file has been reduced, update `EXPLICIT_FILE_LIMITS` in `tests/file_compliance.rs` to the new (lower) count
4. Remove the entry entirely when a legacy file reaches 800 lines or less
