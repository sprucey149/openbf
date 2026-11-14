//! Stamps the build with its commit (card BLKF): the playtest black box's
//! header names the commit the BINARY was built from, not the checkout's HEAD
//! at run time. `OPENBF_BUILD_COMMIT` is the short hash, `OPENBF_BUILD_DIRTY`
//! is `1` when a tracked file under `engine/crates` or the workspace manifest
//! differed from it. Re-run when HEAD, the index or a crate's file changes;
//! `unknown` outside a git checkout.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let o = Command::new("git")
        .args(args)
        // `git status` must not refresh the index: that would re-run this.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

fn main() {
    for p in ["HEAD", "index"] {
        if let Some(f) = git(&["rev-parse", "--path-format=absolute", "--git-path", p]) {
            println!("cargo:rerun-if-changed={f}");
        }
    }
    for p in ["build.rs", "..", "../../Cargo.toml", "../../Cargo.lock"] {
        println!("cargo:rerun-if-changed={p}");
    }
    let commit = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git(&[
        "status",
        "--porcelain",
        "-uno",
        "--",
        "..",
        "../../Cargo.toml",
        "../../Cargo.lock",
    ])
    .is_some_and(|s| !s.is_empty());
    println!("cargo:rustc-env=OPENBF_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=OPENBF_BUILD_DIRTY={}", u8::from(dirty));
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Windows defaults to a 1 MB main-thread stack; unoptimized dev builds
        // of winit + App::resumed + make_walk exceed 1 MB. Match Linux's 8 MB.
        println!("cargo:rustc-link-arg-bin=game=/STACK:8388608");
    }
}
