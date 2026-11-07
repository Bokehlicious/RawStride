//! Tauri, plus the P6 shim (copied from P6b), linked against LibRaw
//! with OpenMP from the repository's vcpkg install, as crates/pw-libraw does. Nothing new is installed.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=csrc/p6.cpp");
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLED_DIR");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let installed = std::env::var_os("VCPKG_INSTALLED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../vcpkg_installed"));
    let root = installed.join("x64-windows-static-md");
    let (inc, lib) = (root.join("include"), root.join("lib"));
    assert!(inc.join("libraw").join("libraw.h").exists(), "LibRaw headers not found in {}", inc.display());
    cc::Build::new()
        .cpp(true)
        .file("csrc/p6.cpp")
        .include(&inc)
        .define("LIBRAW_NODLL", None)
        .compile("p6shim");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=raw_r");
    for dep in ["lcms2", "jasper", "jpeg", "zs"] {
        if lib.join(format!("{dep}.lib")).exists() {
            println!("cargo:rustc-link-lib=static={dep}");
        }
    }
    println!("cargo:rustc-link-lib=ws2_32");
    tauri_build::build()
}
