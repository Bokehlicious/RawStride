//! Compiles the C++ shim and links LibRaw statically from the vcpkg manifest install (root
//! `vcpkg.json`: `libraw[openmp]`, triplet `x64-windows-static-md`; see docs/libraw-build.md).

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=csrc/pwlr.cpp");
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLED_DIR");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let installed = std::env::var_os("VCPKG_INSTALLED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../vcpkg_installed"));
    let root = installed.join("x64-windows-static-md");
    let (inc, lib) = (root.join("include"), root.join("lib"));
    assert!(
        inc.join("libraw").join("libraw.h").exists(),
        "LibRaw headers not found in {}. Run `vcpkg install --triplet x64-windows-static-md` \
         in the repository root first (docs/libraw-build.md).",
        inc.display()
    );
    // ADR 0006: the app uses LibRaw built with OpenMP. Refuse to build against another variant.
    let status = std::fs::read_to_string(installed.join("vcpkg").join("status"))
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let has_openmp = status.split("\n\n").any(|stanza| {
        stanza.contains("Package: libraw")
            && stanza.contains("Feature: openmp")
            && stanza.contains("x64-windows-static-md")
            && stanza.contains("install ok installed")
    });
    assert!(
        has_openmp,
        "libraw[openmp] is not installed in {}. Run `vcpkg install --triplet \
         x64-windows-static-md` in the repository root (vcpkg.json asks for the openmp feature).",
        installed.display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        installed.join("vcpkg").join("status").display()
    );

    // C++ (LibRaw's own data-stream class is subclassed to detect truncated files).
    cc::Build::new()
        .cpp(true)
        .file("csrc/pwlr.cpp")
        .include(&inc)
        .define("LIBRAW_NODLL", None)
        .compile("pwlr");

    println!("cargo:rustc-link-search=native={}", lib.display());
    let exists = |n: &str| lib.join(format!("{n}.lib")).exists();
    assert!(
        exists("raw_r"),
        "raw_r.lib (thread-safe LibRaw) not found in {}",
        lib.display()
    );
    println!("cargo:rustc-link-lib=static=raw_r");
    for dep in ["lcms2", "jasper", "jpeg", "zs"] {
        assert!(
            exists(dep),
            "{dep}.lib (LibRaw dependency) not found in {}",
            lib.display()
        );
        println!("cargo:rustc-link-lib=static={dep}");
    }
    println!("cargo:rustc-link-lib=ws2_32");
}
