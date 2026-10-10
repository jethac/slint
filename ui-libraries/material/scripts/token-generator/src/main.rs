// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! `material-token-generator`: regenerate the Slint Material design tokens
//! from the pinned androidx sources.
//!
//! Usage (from `ui-libraries/material/`):
//!   `cargo run -p material-token-generator`          regenerate + write
//!   `cargo run -p material-token-generator -- check` verify committed files
//!   `--source-dir <path>`                            read an existing androidx
//!     checkout's `androidx/compose/material3` directory instead of fetching

mod emit;
mod json;
mod kotlin;
mod material_shapes;
mod model;
mod parity;
mod sources;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Args {
    check: bool,
    source_dir: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut check = false;
    let mut source_dir = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "check" => check = true,
            "update" => {}
            "--source-dir" => {
                let v = it.next().ok_or("--source-dir needs a path")?;
                source_dir = Some(PathBuf::from(v));
            }
            other => {
                return Err(format!(
                    "unknown argument `{other}`\nusage: material-token-generator [update|check] [--source-dir <path>]"
                ));
            }
        }
    }
    Ok(Args { check, source_dir })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let material_root =
        manifest.ancestors().nth(2).ok_or("cannot locate material crate root")?.to_path_buf();
    let repo_root =
        material_root.ancestors().nth(2).ok_or("cannot locate repo root")?.to_path_buf();

    let src = sources::read_tokens_source(&material_root)?;

    let material3_dir = match &args.source_dir {
        Some(dir) => dir.clone(),
        None => {
            // M3_TOKENS_SRC=<.../androidx/compose/material3> overrides too.
            if let Ok(env_dir) = std::env::var("M3_TOKENS_SRC") {
                PathBuf::from(env_dir)
            } else {
                let cache = repo_root.join("target/material-tokens");
                std::fs::create_dir_all(&cache)
                    .map_err(|e| format!("cannot create {}: {e}", cache.display()))?;
                sources::ensure_checkout(&src, &cache)?
            }
        }
    };

    let files = sources::collect_sources(&material3_dir)?;
    let mut parsed = Vec::new();
    let mut ripple_text = String::new();
    let mut color_scheme_text = String::new();
    let mut material_shapes_text = String::new();
    for (name, text) in &files {
        if name == "Ripple.kt" {
            ripple_text = text.clone();
            continue;
        }
        if name == "ColorScheme.kt" {
            color_scheme_text = text.clone();
            continue;
        }
        if name == "MaterialShapes.kt" {
            material_shapes_text = text.clone();
            continue;
        }
        parsed.push(kotlin::parse_file(name, text).map_err(|e| format!("parse {name}: {e}"))?);
    }
    let focus = emit::parse_focus_ring(&ripple_text)?;
    let lib = model::resolve(&parsed).map_err(|e| format!("resolve: {e}"))?;
    let outputs = emit::emit(
        &lib,
        &src.repo,
        &src.commit,
        &src.path,
        &focus,
        &color_scheme_text,
        &material_shapes_text,
    )?;

    // PARITY.md + PARITY_STATUS.json.
    let mut components = parity::scan_components(&material3_dir, &["internal", "tokens"])?;
    components.sort_by(|a, b| a.name.cmp(&b.name).then(a.file.cmp(&b.file)));
    components.dedup_by(|a, b| a.name == b.name);
    let status_path = material_root.join("PARITY_STATUS.json");
    let mut status = parity::load_status(&status_path)?;
    let status_changed = parity::merge_status(&mut status, &components)?;
    let status_text = if status_changed || !status_path.exists() {
        Some(json::write(&parity::status_json(&status)))
    } else {
        None
    };
    let parity_md = parity::render(&lib, &components, &status, &src.repo, &src.commit)?;

    let mut disk_outputs: Vec<(String, String)> =
        outputs.iter().map(|o| (o.rel_path.clone(), o.content.clone())).collect();
    disk_outputs.push(("PARITY.md".to_string(), parity_md));
    if let Some(t) = &status_text {
        disk_outputs.push(("PARITY_STATUS.json".to_string(), t.clone()));
    }

    if args.check {
        let mut drifted = Vec::new();
        for (rel, content) in &disk_outputs {
            let path = material_root.join(rel);
            match std::fs::read_to_string(&path) {
                Ok(existing) if existing.replace("\r\n", "\n") == content.replace("\r\n", "\n") => {
                }
                Ok(_) => drifted.push(format!("{rel} (differs)")),
                Err(_) => drifted.push(format!("{rel} (missing)")),
            }
        }
        // Files that exist but are no longer generated.
        let gen_dir = material_root.join("src/ui/styling/generated");
        if gen_dir.is_dir() {
            for entry in
                std::fs::read_dir(&gen_dir).map_err(|e| format!("read_dir: {e}"))?.flatten()
            {
                let name = entry.file_name().to_string_lossy().to_string();
                let rel = format!("src/ui/styling/generated/{name}");
                if name.ends_with(".slint") && !disk_outputs.iter().any(|(p, _)| p == &rel) {
                    drifted.push(format!("{rel} (stale)"));
                }
            }
        }
        if drifted.is_empty() {
            println!("token files are up to date");
            Ok(())
        } else {
            for d in &drifted {
                eprintln!("out of date: {d}");
            }
            Err(format!(
                "{} generated file(s) differ — run `cargo run -p material-token-generator` in ui-libraries/material/",
                drifted.len()
            ))
        }
    } else {
        for (rel, content) in &disk_outputs {
            let path = material_root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
            }
            std::fs::write(&path, content)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            println!("wrote {rel}");
        }
        Ok(())
    }
}
