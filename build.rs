use std::env;

fn main() {
    // Get the output directory where Cargo will place the generated files.
    let out_dir = env::var("OUT_DIR").unwrap();

    // Ensure Cargo rebuilds the library if the source file changes.
    println!("cargo:rerun-if-changed=src/c/bitshuffle/bitshuffle.c");
    println!("cargo:rerun-if-changed=src/c/bitshuffle/bitshuffle_core.c");
    println!("cargo:rerun-if-changed=src/c/bitshuffle/iochain.c");
    println!("cargo:rerun-if-changed=src/c/bitshuffle/bitshuffle.h");
    println!("cargo:rerun-if-changed=src/c/bitshuffle/bitshuffle_core.h");
    println!("cargo:rerun-if-changed=src/c/bitshuffle/bitshuffle_internals.h");
    println!("cargo:rerun-if-changed=src/c/bitshuffle/iochain.h");

    // Tell Cargo where to find the compiled library.
    println!("fnative={}", out_dir);

    // Tell Cargo to link the static library.
    println!("cargo:rustc-link-lib=static=bitshuffle");

    cc::Build::new()
        .file("src/c/bitshuffle/bitshuffle.c") // Add other .c files if needed
        .file("src/c/bitshuffle/bitshuffle_core.c")
        .file("src/c/bitshuffle/iochain.c")
        //.file("src/c/lz4/lz4.c")
        .include("src/c/bitshuffle") // Include the header files
        .include("src/c/lz4") // Include the header files
        //.flag_if_supported("-prebind")
        //.flag_if_supported("-dynamiclib")
        .flag_if_supported("-O3")
        .flag_if_supported("-std=c99") // Ensure compatibility with C99 if needed
        .compile("bitshuffle"); // Output static library

        println!("Compiled Bitshuffle");
}
