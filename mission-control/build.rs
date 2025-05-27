use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    Command::new("sh")
        .arg(format!("{}/../build-can-controller.sh", project_dir))
        .output()
        .expect("failed to build can-controller library");

    println!(
        "cargo:rustc-link-search={}/../can-controller/build",
        project_dir
    );
    println!("cargo:rustc-link-lib=can-controller");
    println!("cargo:rustc-link-lib=wiringPi");

    // The bindgen::Builder is the main entry point to bindgen,
    // and lets you build up options for the resulting bindings.
    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}/..", project_dir))
        // The input header we would like to generate bindings for.
        .header("bindings.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
