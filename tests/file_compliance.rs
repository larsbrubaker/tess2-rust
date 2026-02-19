// Copyright 2025 Lars Brubaker
// File size compliance tests — ensures source files stay within line limits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn explicit_file_limits() -> HashMap<&'static str, usize> {
    // Frozen at current size for existing large files.
    // Limits should only ever decrease as files are refactored.
    // Remove an entry when the file reaches 800 lines or less.
    let mut limits = HashMap::new();
    limits.insert("src\\tess.rs", 2065);
    limits.insert("src\\mesh.rs", 1044);
    limits
}

const DEFAULT_LINE_LIMIT: usize = 800;

const EXCLUDE_DIRS: &[&str] = &[
    "target",
    ".git",
    "node_modules",
    "cpp_reference",
    "demo",
    ".cursor",
    ".claude",
    "pkg",
    "dist",
];

const INCLUDE_EXTENSIONS: &[&str] = &[".rs"];

fn get_all_project_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files);
    files
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if EXCLUDE_DIRS.iter().any(|ex| dir_name.contains(ex)) {
                continue;
            }
            collect_files(root, &path, files);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let dotted = format!(".{}", ext);
            if INCLUDE_EXTENSIONS.contains(&dotted.as_str()) {
                files.push(path);
            }
        }
    }
}

fn count_non_empty_lines(path: &Path) -> usize {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn get_file_limit(path: &Path, limits: &HashMap<&str, usize>) -> usize {
    let path_str = path.to_string_lossy();
    for (pattern, &limit) in limits {
        // Normalize separators for cross-platform matching
        let normalized = path_str.replace('/', "\\");
        if normalized.ends_with(pattern) {
            return limit;
        }
    }
    DEFAULT_LINE_LIMIT
}

#[test]
fn file_size_compliance() {
    let root = Path::new(".");
    let files = get_all_project_files(root);
    let limits = explicit_file_limits();
    let mut violations = Vec::new();

    for file in &files {
        let line_count = count_non_empty_lines(file);
        let limit = get_file_limit(file, &limits);

        if line_count > limit {
            violations.push(format!(
                "CRITICAL: {} has {} non-empty lines (limit: {}). \
                 MUST refactor into multiple files.",
                file.display(),
                line_count,
                limit,
            ));
        }
    }

    if !violations.is_empty() {
        let msg = violations
            .iter()
            .map(|v| format!("  {}", v))
            .collect::<Vec<_>>()
            .join("\n");
        panic!("File size violations found:\n{}", msg);
    }
}

#[test]
fn compliance_summary() {
    let root = Path::new(".");
    let files = get_all_project_files(root);
    let limits = explicit_file_limits();

    eprintln!("\nFile Size Compliance Summary:");
    eprintln!("  Total .rs files analyzed: {}", files.len());

    let mut violations = Vec::new();
    for file in &files {
        let line_count = count_non_empty_lines(file);
        let limit = get_file_limit(file, &limits);
        if line_count > limit {
            violations.push(format!(
                "{}: {} lines (limit: {})",
                file.display(),
                line_count,
                limit,
            ));
        }
    }

    if violations.is_empty() {
        eprintln!("  All files comply with size limits!");
    } else {
        eprintln!("  VIOLATIONS: {}", violations.len());
        for v in &violations {
            eprintln!("    {}", v);
        }
    }
}
