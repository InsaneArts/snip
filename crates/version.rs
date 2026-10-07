// The version Snip shows (`snip --version`, Settings › About): the release
// tag this was built from (git describe), or the crate's version and the
// commit when no release is tagged yet. Build scripts `include!` this.
fn snip_version() {
    let describe = std::process::Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let pkg = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let version = match describe.strip_prefix('v') {
        Some(tagged) => tagged.to_string(),
        None if describe.is_empty() => pkg,
        None => format!("{pkg}+{describe}"),
    };
    println!("cargo:rustc-env=SNIP_VERSION={version}");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");
}
