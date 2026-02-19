---
name: testing-philosophy
description: "This skill provides guidance on writing and running tests in this project. It should be used when writing new tests, understanding the test infrastructure, or making decisions about what to test. Covers cargo test configuration and Rust testing best practices."
---

# Testing Philosophy

This skill documents the testing approach and infrastructure for the tess2-rust project.

## Test Runner

### cargo test (Rust tests)

```bash
# Run all tests
cargo test

# Run with output visible
cargo test -- --nocapture

# Run a specific test by name
cargo test default_alloc_success

# Run tests matching a pattern
cargo test star -- --nocapture

# Run only integration tests (tests/ directory)
cargo test --test libtess2_port

# Run only unit tests (inside src/)
cargo test --lib

# Run fast with no output on success
cargo test -q
```

## Test Organization

- `tests/libtess2_port.rs` — Port of C++ Google Test suite (17+ tests)
- `tests/file_compliance.rs` — File size compliance checks
- `src/*/mod tests` — Unit tests for internal logic (inside source modules)

**Test naming:**
- Use descriptive `snake_case` function names
- Name should describe what's being verified

## Core Testing Principles

### Speed Matters

Tests should run as fast as possible. Fast tests get run more often, which means faster feedback and fewer bugs reaching release.

- Prefer unit tests over integration tests when possible
- Avoid unnecessary setup
- Don't test the same behavior multiple times

### Test What Matters

Write tests for:
- Regressions (bugs that were fixed — prevent them from returning)
- Complex logic (algorithms, geometric edge cases, numerical precision)
- Boundary conditions (overflow, NaN, degenerate geometry)
- Public API contracts

Avoid:
- Redundant tests that verify the same behavior
- Tests for trivial code
- Tests that just verify standard library behavior

### Test Failures Are Real Bugs (No Cheating)

**Every test failure indicates a real bug in the production code.** Tests protect correctness — there are no workarounds.

When a test fails:

1. Investigate the failure
2. Add instrumentation (`eprintln!`, `dbg!`) to understand what's happening
3. Find and fix the root cause in production code
4. Never weaken or skip tests to make them pass

**Forbidden actions:**
- Weakening assertions or changing expected values
- Using `#[ignore]` as a permanent solution
- Swallowing errors with `unwrap_or_default()`
- Testing copied logic instead of calling real code

See the `fix-test-failures` skill for the detailed debugging process.

## Integration Tests

Integration tests live in `tests/` and test the public API from the outside.

**Characteristics:**
- Fast (pure computation, no I/O or network)
- Each test creates its own `Tessellator` (fully isolated)
- Test through the public API only

**Example structure:**
```rust
use tess2_rust::{ElementType, Tessellator, WindingRule};

#[test]
fn descriptive_test_name() {
    // Arrange
    let mut tess = Tessellator::new();
    tess.add_contour(2, &[0.0, 0.0, 0.0, 1.0, 1.0, 0.0]);

    // Act
    let ok = tess.tessellate(
        WindingRule::Positive,
        ElementType::Polygons,
        3, 2, None,
    );

    // Assert
    assert!(ok, "tessellation should succeed");
    assert_eq!(tess.element_count(), 1);
}
```

**Important:** Tests MUST import and test actual production code. Never duplicate production logic in tests.

## When to Write Tests

**Always write tests for:**
- Bug fixes (regression test to prevent the bug from returning)
- Complex algorithms or geometric edge cases
- Numerical boundary conditions
- New public API methods

**Consider skipping tests for:**
- Trivial accessor functions
- Code that's just wiring (no logic)
- Temporary/experimental code that will be rewritten

## Bug Fix Workflow: Failing Test First

**When fixing a bug, always write a failing test before writing the fix.**

This approach:
1. Proves the bug exists and is reproducible
2. Ensures you understand the actual problem
3. Verifies your fix actually works
4. Prevents the bug from returning (regression protection)

**The process:**
1. Reproduce the bug to understand it
2. Write a test that fails because of the bug
3. Run the test to confirm it fails (red)
4. Fix the bug in production code
5. Run the test to confirm it passes (green)
6. Commit both the test and the fix together
