//! Compiles the C shim and links LibRaw (static, from the vcpkg manifest install).

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=csrc/shim.c");
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLED_DIR");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = std::env::var_os("VCPKG_INSTALLED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../vcpkg_installed"))
        .join("x64-windows-static-md");
    let (inc, lib) = (root.join("include"), root.join("lib"));
    assert!(
        inc.join("libraw").join("libraw.h").exists(),
        "LibRaw headers not found in {}. Run `vcpkg install --triplet x64-windows-static-md` \
         in the repository root first.",
        inc.display()
    );
    cc::Build::new()
        .file("csrc/shim.c")
        .include(&inc)
        .define("LIBRAW_NODLL", None)
        .compile("pwshim");

    println!("cargo:rustc-link-search=native={}", lib.display());
    let exists = |n: &str| lib.join(format!("{n}.lib")).exists();
    // Thread-safe LibRaw first (needed for parallel decoding), then its dependencies.
    let libraw = ["raw_r", "raw", "libraw_static", "libraw"]
        .into_iter()
        .find(|n| exists(n))
        .unwrap_or_else(|| panic!("no LibRaw .lib found in {}", lib.display()));
    println!("cargo:rustc-link-lib=static={libraw}");
    for dep in [
        "lcms2",
        "jasper",
        "libjasper",
        "jpeg",
        // zlib (LibRaw's deflate DNG support): vcpkg's static zlib is named zs.lib.
        "zs",
        "zlib",
        "zlibstatic",
        "z",
    ] {
        if exists(dep) {
            println!("cargo:rustc-link-lib=static={dep}");
        }
    }
    println!("cargo:rustc-link-lib=ws2_32");
    println!("cargo:warning=LibRaw: {libraw}.lib from {}", lib.display());
}
