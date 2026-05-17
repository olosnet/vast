use std::env;
use std::path::PathBuf;

fn main() {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Get the manifest directory (where Cargo.toml is)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let base_path = PathBuf::from(manifest_dir);

    // Map Rust target architecture to SVB SDK library directory
    let lib_subdir = match target_arch.as_str() {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "armv8",
        "armv7" => "armv7",
        "arm" => "armv6",
        _ => panic!("Unsupported architecture: {}", target_arch),
    };

    let lib_dir = base_path
        .join("../external/svb/lib")
        .join(lib_subdir)
        .canonicalize()
        .expect("Failed to resolve library directory");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=SVBCameraSDK");

    // Link C++ standard library and other dependencies
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=usb-1.0");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=m");

    let bindings = bindgen::Builder::default()
        .header("../external/svb/include/SVBCameraSDK.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file("./src/drivers/bindings/svb.rs")
        .expect("Couldn't write bindings!");
}
