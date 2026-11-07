//! Links libjpeg-turbo's TurboJPEG library statically from the vcpkg manifest install (root
//! `vcpkg.json`: `libjpeg-turbo`, triplet `x64-windows-static-md`; ADR 0009 §6.2, option B). No C is
//! compiled: the crate calls the TurboJPEG 3 API directly.

use std::path::PathBuf;

/// Stops the build with a message (a build script must not panic, AGENTS.md).
fn fail(msg: &str) -> ! {
    println!("cargo:warning={msg}");
    eprintln!("pw-libjpeg: {msg}");
    std::process::exit(1)
}

fn main() {
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLED_DIR");
    let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        fail("CARGO_MANIFEST_DIR is not set");
    };
    let installed = std::env::var_os("VCPKG_INSTALLED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../vcpkg_installed"));
    let root = installed.join("x64-windows-static-md");
    let (inc, lib) = (root.join("include"), root.join("lib"));
    if !inc.join("turbojpeg.h").exists() {
        fail(&format!(
            "turbojpeg.h not found in {}. Run `vcpkg install --triplet x64-windows-static-md` in the \
             repository root first (vcpkg.json asks for libjpeg-turbo).",
            inc.display()
        ));
    }
    // the TurboJPEG 3 API (tj3*) needs libjpeg-turbo 3.x
    let status = std::fs::read_to_string(installed.join("vcpkg").join("status"))
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let has_v3 = status.split("\n\n").any(|stanza| {
        stanza.contains("Package: libjpeg-turbo\n")
            && stanza.contains("Version: 3.")
            && stanza.contains("x64-windows-static-md")
            && stanza.contains("install ok installed")
    });
    if !has_v3 {
        fail(&format!(
            "libjpeg-turbo 3.x is not installed in {} (needed for the TurboJPEG 3 API).",
            installed.display()
        ));
    }
    println!(
        "cargo:rerun-if-changed={}",
        installed.join("vcpkg").join("status").display()
    );
    println!("cargo:rustc-link-search=native={}", lib.display());
    let exists = |n: &str| lib.join(format!("{n}.lib")).exists();
    let Some(name) = ["turbojpeg", "turbojpeg-static"]
        .into_iter()
        .find(|n| exists(n))
    else {
        fail(&format!("turbojpeg.lib not found in {}", lib.display()));
    };
    println!("cargo:rustc-link-lib=static={name}");
    // libjpeg-turbo 3.1+ puts tj3LoadImage/tj3SaveImage (PNG through libspng) in turbojpeg.lib;
    // the vcpkg port installs libspng as spng_static.lib, which needs zlib (zs.lib).
    if let Some(spng) = ["spng_static", "spng"].into_iter().find(|n| exists(n)) {
        println!("cargo:rustc-link-lib=static={spng}");
    }
    if exists("zs") {
        println!("cargo:rustc-link-lib=static=zs");
    }
}
