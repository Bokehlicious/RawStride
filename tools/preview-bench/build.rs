//! Links libjpeg-turbo (TurboJPEG API) from the vcpkg manifest install when the
//! `turbojpeg` feature is enabled. Override the location with `VCPKG_INSTALLED_DIR`.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLED_DIR");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TURBOJPEG");
    if std::env::var_os("CARGO_FEATURE_TURBOJPEG").is_none() {
        return;
    }
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let installed = std::env::var_os("VCPKG_INSTALLED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../vcpkg_installed"));
    let lib_dir = installed.join("x64-windows-static-md").join("lib");
    // The vcpkg port names the static TurboJPEG library differently across versions.
    let name = ["turbojpeg", "turbojpeg-static"]
        .into_iter()
        .find(|n| lib_dir.join(format!("{n}.lib")).exists())
        .unwrap_or_else(|| {
            panic!(
                "turbojpeg.lib not found in {}. Run `vcpkg install --triplet x64-windows-static-md` \
                 in the repository root first.",
                lib_dir.display()
            )
        });
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static={name}");
    println!("cargo:warning=linking {name}.lib from {}", lib_dir.display());
}
