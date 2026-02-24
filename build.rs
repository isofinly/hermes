use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Regenerate bindings when masscan headers or wrapper change.
    println!("cargo:rerun-if-changed=ffi/masscan_wrapper.h");
    println!("cargo:rerun-if-changed=ffi/masscan_entry.c");
    println!("cargo:rerun-if-changed=masscan/src/masscan.h");
    println!("cargo:rerun-if-changed=masscan/src");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));

    // Build Masscan C sources into a static library and rename C entrypoint
    // from `main` to `masscan_cli_main` so Rust can invoke it directly.
    let mut c_sources: Vec<PathBuf> = fs::read_dir("masscan/src")
        .expect("masscan/src should exist")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "c")
                && path
                    .file_name()
                    .is_none_or(|name| name != "main.c")
        })
        .collect();

    c_sources.sort();

    let mut cc_build = cc::Build::new();
    cc_build
        .include("masscan/src")
        .include("ffi")
        .flag("-w")
        .files(c_sources)
        .file("ffi/masscan_entry.c")
        .compile("masscan_ffi");

    println!("cargo:rustc-link-lib=m");

    let bindings = bindgen::Builder::default()
        .header("ffi/masscan_wrapper.h")
        .clang_arg("-Imasscan/src")
        .allowlist_function("masscan_cli_main")
        .allowlist_function("mainconf_selftest")
        .allowlist_function("main_listscan")
        .allowlist_function("masscan_.*")
        .allowlist_type("Masscan")
        .allowlist_type("TcpCfgPayloads")
        .allowlist_type("Operation")
        .allowlist_type("OutputFormat")
        .rustified_enum("Operation")
        .rustified_enum("OutputFormat")
        .derive_default(true)
        .generate_comments(true)
        // TODO: Link compiled masscan C objects once we move from type/API generation
        // to direct function invocation from Rust.
        .generate()
        .expect("bindgen should generate Masscan bindings");

    bindings
        .write_to_file(out_dir.join("masscan_bindings.rs"))
        .expect("generated bindings should be writable");
}
