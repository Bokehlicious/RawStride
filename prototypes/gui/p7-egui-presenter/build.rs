//! The LibRaw prototype shim (a copy of the phase 1 shim), linked against LibRaw with OpenMP from the
//! repository's vcpkg install, as crates/pw-libraw does. Nothing new is installed.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=csrc/proto.cpp");
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLED_DIR");
    let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        println!("cargo:warning=CARGO_MANIFEST_DIR is not set");
        std::process::exit(1);
    };
    let installed = std::env::var_os("VCPKG_INSTALLED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../vcpkg_installed"));
    let root = installed.join("x64-windows-static-md");
    let (inc, lib) = (root.join("include"), root.join("lib"));
    if !inc.join("libraw").join("libraw.h").exists() {
        println!("cargo:warning=LibRaw headers not found in {}", inc.display());
        std::process::exit(1);
    }
    cc::Build::new()
        .cpp(true)
        .file("csrc/proto.cpp")
        .include(&inc)
        .define("LIBRAW_NODLL", None)
        .compile("p7shim");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=raw_r");
    for dep in ["lcms2", "jasper", "jpeg", "zs"] {
        if lib.join(format!("{dep}.lib")).exists() {
            println!("cargo:rustc-link-lib=static={dep}");
        }
    }
    println!("cargo:rustc-link-lib=ws2_32");
}
