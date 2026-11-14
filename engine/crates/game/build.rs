//! Stamps the build with its commit (card BLKF): the playtest black box's
//! header names the commit the BINARY was built from, not the checkout's HEAD
//! at run time. `OPENBF_BUILD_COMMIT` is the short hash, `OPENBF_BUILD_DIRTY`
//! is `1` when a tracked file under `engine/crates` or the workspace manifest
//! differed from it. Re-run when HEAD, the index or a crate's file changes;
//! `unknown` outside a git checkout.
//!
//! `OPENBF_BUILD_DIFF` (issue #168, LAN builds must match exactly) is a
//! 64-bit FNV-1a of the uncommitted changes under the same paths, as 16 hex
//! digits: the tracked diff against HEAD plus every untracked source file's
//! name and bytes. Empty for a clean tree. Two dirty builds of the same
//! commit agree only when their changes are the same bytes. The same commit
//! gives the same id on every OS (`build_rules.rs`): OS and editor litter
//! is not a change, CRLF reads as LF, and the diff is asked for in one fixed
//! format whatever the player's git config says.

use std::process::Command;

#[path = "build_rules.rs"]
mod build_rules;

fn git(args: &[&str]) -> Option<String> {
    git_bytes(args).map(|b| String::from_utf8_lossy(&b).trim().to_owned())
}

fn git_bytes(args: &[&str]) -> Option<Vec<u8>> {
    let o = Command::new("git")
        .args(args)
        // `git status` must not refresh the index: that would re-run this.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    o.status.success().then_some(o.stdout)
}

const PATHS: [&str; 4] = ["--", "..", "../../Cargo.toml", "../../Cargo.lock"];

fn main() {
    for p in ["HEAD", "index"] {
        if let Some(f) = git(&["rev-parse", "--path-format=absolute", "--git-path", p]) {
            println!("cargo:rerun-if-changed={f}");
        }
    }
    for p in [
        "build.rs",
        "build_rules.rs",
        "..",
        "../../Cargo.toml",
        "../../Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={p}");
    }
    // The first 12 digits of the full hash: `--short` grows when a clone
    // holds an ambiguous prefix, and every clone must print the same id.
    let commit: String = git(&["rev-parse", "HEAD"])
        .map(|h| h.chars().take(12).collect())
        .unwrap_or_else(|| "unknown".into());
    let status = [&["status", "--porcelain", "-uno"][..], &PATHS[..]].concat();
    let dirty = git(&status).is_some_and(|s| !s.is_empty());
    let untracked_args = [
        &["ls-files", "-z", "--others", "--exclude-standard"][..],
        &PATHS[..],
    ]
    .concat();
    let untracked = git_bytes(&untracked_args).unwrap_or_default();
    let untracked = String::from_utf8_lossy(&untracked);
    let names = untracked.split('\0').filter(|n| !n.is_empty());
    // One diff format on every machine: no external diff or textconv, no
    // colour, no renames, the default algorithm, context and prefixes.
    let diff_args = [
        &[
            "-c",
            "diff.noprefix=false",
            "-c",
            "diff.mnemonicPrefix=false",
            "-c",
            "diff.relative=false",
            "-c",
            "core.quotePath=true",
            "diff",
            "--binary",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--no-renames",
            "--diff-algorithm=myers",
            "--indent-heuristic",
            "-U3",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "HEAD",
        ][..],
        &PATHS[..],
    ]
    .concat();
    let diff = if dirty {
        git_bytes(&diff_args).unwrap_or_default()
    } else {
        Vec::new()
    };
    let diff = build_rules::diff_hash(
        &diff,
        names.map(|n| (n, std::fs::read(n).unwrap_or_default())),
    );
    println!("cargo:rustc-env=OPENBF_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=OPENBF_BUILD_DIRTY={}", u8::from(dirty));
    println!("cargo:rustc-env=OPENBF_BUILD_DIFF={diff}");
}
