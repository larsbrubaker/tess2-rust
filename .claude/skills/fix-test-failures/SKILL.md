---
name: fix-test-failures
description: "This skill should be used after running tests when failures occur. It ensures test failures are properly diagnosed through instrumentation and logging until the root cause is found and fixed. The skill treats all test failures as real bugs that must be resolved, never skipped."
---

# Fix Test Failures

This skill provides a systematic approach to diagnosing and fixing test failures. The core philosophy is that **test failures are real bugs** — they must be understood and fixed, never ignored or worked around.

## NO CHEATING — Critical Rules

**These tests protect correctness. There are no workarounds.**

Every test exists because it validates behavior that users depend on. Bypassing tests means shipping broken software.

**Forbidden actions (no exceptions):**
- Weakening assertions to make tests pass
- Changing expected values to match broken behavior
- Using `unwrap_or_default()` or `.ok()` to swallow errors
- Adding conditional logic to skip checks in test environments
- Commenting out assertions or test functions
- Using `#[ignore]` to hide failures
- Relaxing tolerances to mask numerical regressions
- Mocking away the actual behavior being tested

**The only acceptable outcome is fixing the actual bug in the production code.**

## When to Use This Skill

Use this skill when:
- Tests fail and the cause isn't immediately obvious
- A test is flaky or intermittently failing
- You need to understand why a test is failing before fixing it
- You've made changes and tests are now failing

## Core Principles

1. **Test failures are real bugs** — Never skip, disable, or delete failing tests without understanding and fixing the underlying issue
2. **No cheating** — Never weaken tests, change expected values, or work around failures
3. **Instrument to understand** — Add `eprintln!`/`dbg!` to expose internal state and execution flow
4. **Fix the root cause** — Don't patch symptoms; find and fix the actual bug
5. **Clean up after** — Remove instrumentation once the fix is verified

## Test Failure Resolution Process

### Step 1: Run Tests and Capture Failures

Run the failing test(s) to see the current error:

```bash
# Run all tests
cargo test

# Run a specific test
cargo test test_name -- --nocapture

# Run with backtrace on panic
RUST_BACKTRACE=1 cargo test test_name -- --nocapture

# Run a specific test file
cargo test --test libtess2_port -- --nocapture
```

Record the exact error message and stack trace. This is your starting point.

### Step 2: Analyze the Failure

Before adding instrumentation, understand what the test is checking:

1. Read the test code carefully
2. Identify what assertion is failing
3. Note what values were expected vs. received
4. Form a hypothesis about what might be wrong

### Step 3: Add Strategic Instrumentation

Add `eprintln!()` or `dbg!()` to expose the state at key points:

**For state-related failures:**
```rust
eprintln!("State before operation: {:?}", state);
// ... operation ...
eprintln!("State after operation: {:?}", state);
```

**For numerical issues:**
```rust
eprintln!("a={}, b={}, a+b={}, overflow={}", a, b, a.wrapping_add(b), a.checked_add(b).is_none());
```

**For function execution flow:**
```rust
eprintln!("Entering function with args: {:?}, {:?}", arg1, arg2);
// ... function body ...
eprintln!("Returning result: {:?}", result);
```

**Quick one-off debugging:**
```rust
let result = dbg!(some_computation());
```

### Step 4: Run Instrumented Tests

Run the test again with `--nocapture`:

```bash
cargo test test_name -- --nocapture
```

Analyze the output to understand:
- What values are actually present
- Where the execution diverges from expectations
- What state is incorrect and when it became incorrect

### Step 5: Identify Root Cause

Based on instrumentation output, determine:
- Is the test wrong (rare — only if test assumptions were incorrect)?
- Is the code under test wrong (common)?
- Is there a numerical precision or overflow issue?
- Is there a state pollution issue from shared mutable state?

### Step 6: Fix the Bug

Fix the actual bug in the production code, not by modifying the test to accept wrong behavior.

Common fixes:
- **Logic errors**: Fix the algorithm or condition
- **Numeric issues**: Add overflow checks, use wider types for intermediates
- **Index errors**: Fix bounds checking, handle empty collections
- **Lifetime issues**: Fix ownership or borrowing patterns

### Step 7: Verify and Clean Up

1. Run the test again to confirm it passes
2. Run the full test suite to ensure no regressions: `cargo test`
3. **Remove all instrumentation** (`eprintln!`, `dbg!`) — they were for debugging only
4. Commit the fix

## What NOT to Do (NO CHEATING)

These are all forms of cheating that bypass the purpose of testing:

- **Don't skip failing tests** — Every test failure is meaningful
- **Don't delete tests** to make the suite pass
- **Don't use `#[ignore]`** as a permanent solution
- **Don't weaken assertions** — If a test expects 8 triangles, don't change it to expect 7
- **Don't change expected values** to match broken output
- **Don't use `unwrap_or_default()`** to swallow errors in production code
- **Don't relax tolerances** to hide numerical regressions
- **Don't leave instrumentation** in committed code

**If you find yourself wanting to do any of these, STOP. The test is telling you something is broken. Fix the broken thing.**

## Iterative Debugging

If the first round of instrumentation doesn't reveal the issue:

1. Add more instrumentation at earlier points in execution
2. Log intermediate values, not just final state
3. Check for side effects from other code
4. Use `RUST_BACKTRACE=1` for better stack traces
5. Check if the issue is platform-specific (f32 precision, endianness)

Keep iterating until the root cause is clear. The goal is understanding, then fixing.
