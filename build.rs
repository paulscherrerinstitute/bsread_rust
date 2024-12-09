use std::env;

fn main() {
    // Get the output directory where Cargo will place the generated files.
    let out_dir = env::var("OUT_DIR").unwrap();

    // Ensure Cargo rebuilds the library if the source file changes.
    println!("cargo:rerun-if-changed=bitshuffle.c");

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
    /*
    match (env::consts::OS){
        "linux" => {
            cc::Build::new()
                .file("src/c/bitshuffle.c") // Add other .c files if needed
                .file("src/c/bitshuffle_core.c")
                .file("src/c/iochain.c")
                .file("src/c/lz4.c")
                .include("src/c/bitshuffle") // Include the header files
                .include("src/c/lz4") // Include the header files
                .flag_if_supported("-std=c99") // Ensure compatibility with C99 if needed
                .compile("libbitshuffle.a"); // Output static library
        }
        "macos" => {
            cc::Build::new()
                .file("src/c/bitshuffle.c") // Add other .c files if needed
                .file("src/c/bitshuffle_core.c")
                .file("src/c/iochain.c")
                .file("src/c/lz4.c")
                .include("src/c/bitshuffle") // Include the header files
                .include("src/c/lz4") // Include the header files
                .flag_if_supported("-std=c99") // Ensure compatibility with C99 if needed
                .compile("libbitshuffle.a"); // Output static library

        }
        _ => {}
         */



/*
    if (osName == 'darwin'){
        exec {
          executable 'gcc'
          args  '-prebind',
                '-dynamiclib',
                '-O3',
                '-std=c99',
                '-I', "${home}/../include/${osName}",
                '-I', "${home}/../include",
                '-I', 'src/main/c',
                '-I', 'src/main/c/lz4',
                '-I', 'src/main/c/bitshuffle',
                '-o', "src/main/resources/${osName}/${osArch}/libbitshuffle-lz4-java.dylib",
                'src/main/c/lz4/lz4.c',
                'src/main/c/bitshuffle/iochain.c',
                'src/main/c/bitshuffle/bitshuffle_core.c',
                'src/main/c/bitshuffle/bitshuffle.c',
                'src/main/c/ch_psi_bitshuffle_lz4_JNI.c'
        }
    }
    else if (osName == 'linux'){
        exec {
          executable 'gcc'
          args  '-shared',
                '-O3',
                '-fPIC',
                '-std=c99',
                '-I', "${home}/../include/${osName}",
                '-I', "${home}/../include",
                '-I', 'src/main/c',
                '-I', 'src/main/c/lz4',
                '-I', 'src/main/c/bitshuffle',
                '-o', "src/main/resources/${osName}/${osArch}/libbitshuffle-lz4-java.so",
                'src/main/c/lz4/lz4.c',
                'src/main/c/bitshuffle/iochain.c',
                'src/main/c/bitshuffle/bitshuffle_core.c',
                'src/main/c/bitshuffle/bitshuffle.c',
                'src/main/c/ch_psi_bitshuffle_lz4_JNI.c'
        }
    }

 */

//}
