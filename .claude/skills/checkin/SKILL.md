---
name: checkin
description: "Automates the full commit workflow: analyzes changes, runs tests, writes commit message, stages files, commits, and handles failures by fixing issues until the commit succeeds. Use when the user wants to commit their changes."
---

# Checkin Skill

Automates the full commit workflow: analyzes changes, runs tests, writes commit message, stages files, commits, and handles failures by fixing issues until the commit succeeds.

## Workflow

### Step 1: Analyze Changes

Run these commands in parallel to understand the current state:

```bash
git status
git diff
git diff --staged
```

From this analysis:
- Identify what files changed and why
- Determine which files should be staged (exclude secrets, generated files, etc.)

### Step 2: Run Tests and Checks First

**CRITICAL: Always run all checks before committing.** This catches issues early and prevents failed commits.

```bash
# Check formatting
cargo fmt --check

# Run clippy linting
cargo clippy -- -D warnings

# Run all tests
cargo test -q
```

If **formatting issues** are found:
1. Auto-fix with `cargo fmt`
2. Re-stage modified files

If **clippy warnings** are found:
1. Fix each warning in the source code
2. Do NOT add `#[allow(...)]` unless the warning is a genuine false positive

If **tests fail**:
1. **Do NOT proceed with the commit**
2. Launch the `fix-test-failures` agent to diagnose and fix the failures
3. Re-run tests after fixes
4. Only proceed to staging when all checks pass

### Step 3: Stage and Commit

1. Stage relevant files using `git add`
2. Write a commit message following this format:
   - **Subject line**: Imperative mood, max 50 chars, no period (e.g., "Add boundary contour support" not "Added boundary contour support.")
   - **Body** (if needed): Blank line after subject, wrap at 72 chars, explain *why* not *what*
   - End with the co-author line
3. Commit using PowerShell syntax (this is a Windows/PowerShell environment):

```powershell
$msg = "Subject line`n`nBody text`n`nCo-Authored-By: Claude <noreply@anthropic.com>"; git commit -m $msg
```

### Step 4: Handle Failures

If the commit fails due to pre-commit hooks or other issues:

1. **Identify the failure type** from the output:
   - **rustfmt formatting** — Run `cargo fmt` to auto-fix, then re-stage
   - **clippy warnings** — Fix each warning manually
   - **test failures** — Launch the `fix-test-failures` agent
2. **After fixing**: Attempt the commit again

### Step 5: Iterate Until Success

Repeat Step 4 until the commit succeeds. Each iteration:
- Run the commit
- If it fails, fix the issues
- Try again

Use `--amend` only when:
- The previous commit attempt succeeded but hooks modified files that need including
- The HEAD commit was created in this session
- The commit has NOT been pushed to remote

Otherwise, create a new commit with the fixes.

### Step 6: Confirm Success

After a successful commit:
- Run `git status` to verify the commit succeeded
- Report the commit hash and summary to the user

## Important Notes

- Do NOT push to remote — the user will handle that
- Do NOT commit files that contain secrets (.env, credentials, API keys, etc.)
- Do NOT weaken tests to make them pass — fix the actual bugs
- **Shell is PowerShell on Windows** — Do NOT use bash heredoc syntax (`<<'EOF'`). Use PowerShell string variables with backtick-n (`` `n ``) for newlines

## Quality Commitment

**Fix every single error. No exceptions.**

When errors occur during the commit process:
- Do NOT skip errors or mark them as "known issues"
- Do NOT disable tests to make them pass
- Do NOT add workarounds that hide problems
- Do NOT give up after a few attempts

Take the hard path:
- Investigate root causes, not just symptoms
- Fix the underlying bug, even if it requires significant changes
- Ensure the fix is correct, not just passing
- Keep iterating until the code is genuinely right

Quality matters. Every error is an opportunity to make the code better.
