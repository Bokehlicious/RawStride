//! Compiles the prototype shim and links LibRaw (with OpenMP) statically from the repository's
//! vcpkg manifest install, exactly as `crates/pw-libraw` does. Nothing new is installed.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=csrc/proto.cpp");
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLED_DIR");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let installed = std::env::var_os("VCPKG_INSTALLED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../vcpkg_installed"));
    let root = installed.join("x64-windows-static-md");
    let (inc, lib) = (root.join("include"), root.join("lib"));
    assert!(
        inc.join("libraw").join("libraw.h").exists(),
        "LibRaw headers not found in {} (the repository's vcpkg install)",
        inc.display()
    );
    cc::Build::new()
        .cpp(true)
        .file("csrc/proto.cpp")
        .include(&inc)
        .define("LIBRAW_NODLL", None)
        .compile("pwproto");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TURBOJPEG");
    if std::env::var_os("CARGO_FEATURE_TURBOJPEG").is_some() {
        // The vcpkg port names the static TurboJPEG library differently across versions.
        let name = ["turbojpeg", "turbojpeg-static"]
            .into_iter()
            .find(|n| lib.join(format!("{n}.lib")).exists())
            .unwrap_or_else(|| panic!("turbojpeg.lib not found in {}", lib.display()));
        println!("cargo:rustc-link-lib=static={name}");
        println!("cargo:warning=linking {name}.lib from {}", lib.display());
        // libjpeg-turbo 3.1 puts tj3LoadImage/tj3SaveImage (PNG via libspng) in turbojpeg.lib; the
        // vcpkg port installs libspng as spng_static.lib (it needs zlib = zs.lib, linked below).
        if let Some(spng) = ["spng_static", "spng"].into_iter().find(|n| lib.join(format!("{n}.lib")).exists()) {
            println!("cargo:rustc-link-lib=static={spng}");
        }
    }
    println!("cargo:rustc-link-lib=static=raw_r");
    for dep in ["lcms2", "jasper", "jpeg", "zs"] {
        if lib.join(format!("{dep}.lib")).exists() {
            println!("cargo:rustc-link-lib=static={dep}");
        }
    }
    println!("cargo:rustc-link-lib=ws2_32");
}
