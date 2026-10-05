// AVX-512 target features were stabilized in Rust 1.89.
fn main() {
    println!("cargo:rustc-check-cfg=cfg(butteraugli_avx512)");
    println!("cargo:rerun-if-env-changed=RUSTC");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = std::process::Command::new(rustc).arg("--version").output();
    let supported = output.ok().filter(|o| o.status.success()).and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| {
        let mut parts = s.split_whitespace().nth(1)?.split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor: u32 = parts.next()?.parse().ok()?;
        Some(major > 1 || (major == 1 && minor >= 89))
    }).unwrap_or(false);
    if supported { println!("cargo:rustc-cfg=butteraugli_avx512"); }
}
