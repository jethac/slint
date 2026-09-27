// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Locating and fetching the pinned upstream sources.
//!
//! `TOKENS_SOURCE` at the material crate root records the upstream repo, the
//! pinned commit and the directory that contains the token files. Bumping the
//! pin is a deliberate edit of that file, reviewed like any other change.

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct SourceInfo {
    pub repo: String,
    pub commit: String,
    /// Directory inside the repo that contains `tokens/` and the component
    /// sources, e.g. `compose/material3/.../material3`.
    pub path: String,
}

pub fn read_tokens_source(material_root: &Path) -> Result<SourceInfo, String> {
    let file = material_root.join("TOKENS_SOURCE");
    let text = std::fs::read_to_string(&file)
        .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let mut info = SourceInfo { repo: String::new(), commit: String::new(), path: String::new() };
    for (lineno, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("{}:{}: expected `key = value`", file.display(), lineno + 1))?;
        match key.trim() {
            "repo" => info.repo = value.trim().to_string(),
            "commit" => info.commit = value.trim().to_string(),
            "path" => info.path = value.trim().to_string(),
            other => {
                return Err(format!("{}:{}: unknown key `{other}`", file.display(), lineno + 1));
            }
        }
    }
    for (key, val) in [("repo", &info.repo), ("commit", &info.commit), ("path", &info.path)] {
        if val.is_empty() {
            return Err(format!("{}: missing `{key}`", file.display()));
        }
    }
    Ok(info)
}

/// Run `git` with args, returning trimmed stdout on success.
fn git(dir: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    let out = cmd
        .args(args)
        .output()
        .map_err(|e| format!("failed to run `git {}`: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Ensure the pinned commit is checked out under `cache_dir` and return the
/// directory that corresponds to `SourceInfo.path`.
///
/// Uses a blobless clone plus a path-scoped checkout so only the needed files
/// are downloaded.
pub fn ensure_checkout(src: &SourceInfo, cache_dir: &Path) -> Result<PathBuf, String> {
    let dir = cache_dir.join(&src.commit);
    let stamp = dir.join(".commit");
    let target = dir.join(&src.path);
    if stamp.is_file()
        && std::fs::read_to_string(&stamp).map(|s| s.trim() == src.commit).unwrap_or(false)
        && target.join("tokens").is_dir()
    {
        return Ok(target);
    }

    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("cannot clear {}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    // 1. blobless, no checkout: cheap metadata-only clone of the default head.
    git(
        Some(cache_dir),
        &[
            "clone",
            "--no-checkout",
            "--filter=blob:none",
            "--depth",
            "1",
            &src.repo,
            dir.file_name().unwrap().to_str().unwrap(),
        ],
    )?;
    // 2. fetch the pinned commit itself.
    git(Some(&dir), &["fetch", "--depth", "1", "origin", &src.commit])?;
    // 3. materialize only the subtree we need; blobs come from the promisor.
    git(Some(&dir), &["checkout", "FETCH_HEAD", "--", &src.path])?;

    std::fs::write(&stamp, format!("{}\n", src.commit))
        .map_err(|e| format!("cannot write {}: {e}", stamp.display()))?;
    Ok(target)
}

/// The `(relative_name, contents)` pairs the generator consumes: every
/// `tokens/*.kt` file plus `Ripple.kt` (focus indicator defaults) and
/// `ColorScheme.kt` (the `expressiveLightColorScheme` overrides).
pub fn collect_sources(material3_dir: &Path) -> Result<Vec<(String, String)>, String> {
    let tokens_dir = material3_dir.join("tokens");
    let mut files = Vec::new();
    let entries = std::fs::read_dir(&tokens_dir)
        .map_err(|e| format!("cannot read {}: {e}", tokens_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read_dir: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".kt") {
            let text = std::fs::read_to_string(entry.path())
                .map_err(|e| format!("cannot read {name}: {e}"))?;
            files.push((format!("tokens/{name}"), text));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    if files.is_empty() {
        return Err(format!("{}: no token files found", tokens_dir.display()));
    }
    for extra in ["Ripple.kt", "ColorScheme.kt"] {
        let path = material3_dir.join(extra);
        files.push((
            extra.to_string(),
            std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        ));
    }
    Ok(files)
}
