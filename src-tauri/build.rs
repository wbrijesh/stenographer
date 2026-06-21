use std::process::Command;

fn main() {
    #[cfg(target_os = "macos")]
    build_fm_bridge();

    tauri_build::build();
}

/// Compile the Swift FoundationModels bridge (`swift/fm_bridge.swift`) into a
/// static archive in `OUT_DIR` and emit the link directives so it (plus the
/// FoundationModels framework and the Swift runtime) is linked into the binary.
///
/// We use `swiftc` directly (SwiftPM / the `foundation-models` crate fails to
/// build in this environment). The bridge object is archived with `ar` into
/// `libfmbridge.a` and statically linked. The Swift standard library itself is
/// resolved dynamically against the OS-provided runtime in `/usr/lib/swift`
/// (present at runtime on macOS 26+), so we add that to the link search path and
/// set an rpath to it.
#[cfg(target_os = "macos")]
fn build_fm_bridge() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let src = "swift/fm_bridge.swift";

    println!("cargo:rerun-if-changed={src}");

    let obj = format!("{out_dir}/fm_bridge.o");
    let archive = format!("{out_dir}/libfmbridge.a");

    // 1. Compile the Swift bridge to an object file.
    let status = Command::new("swiftc")
        .args([
            "-O",
            "-emit-object",
            "-c",
            src,
            "-o",
            &obj,
        ])
        .status()
        .expect("failed to invoke swiftc — is the Swift toolchain installed?");
    assert!(status.success(), "swiftc failed to compile {src}");

    // 2. Archive it into a static library.
    let _ = std::fs::remove_file(&archive);
    let status = Command::new("ar")
        .args(["rcs", &archive, &obj])
        .status()
        .expect("failed to invoke ar");
    assert!(status.success(), "ar failed to build {archive}");

    // 3. Link directives.
    println!("cargo:rustc-link-search=native={out_dir}");
    println!("cargo:rustc-link-lib=static=fmbridge");
    println!("cargo:rustc-link-lib=framework=FoundationModels");

    // Swift runtime: resolved dynamically against the OS runtime in
    // /usr/lib/swift (present on macOS 26+). Add it to the search path and the
    // runtime search path so the Swift stdlib dylibs load at launch.
    println!("cargo:rustc-link-search=native=/usr/lib/swift");
    println!("cargo:rustc-link-arg=-L/usr/lib/swift");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
