use std::{env, path::PathBuf, process::Command};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let out_dir = manifest_dir.join("../target");
    let asm_dir = manifest_dir.join("asm");
    println!("cargo:rustc-link-search={}", out_dir.display());

    let asm_files = ["boot.asm", "interrupts.asm", "msr.asm"];

    for asm_file in asm_files.iter() {
        // Generate the full path to the assembly file
        let asm_path = asm_dir.join(asm_file);

        // Generate the corresponding object file path
        let obj_file = {
            let mut path = out_dir.clone();
            let obj_filename = format!("{}.o", asm_file.trim_end_matches(".asm"));
            path.push(&obj_filename);
            path
        };

        // Compile the assembly file to an object file
        Command::new("nasm")
            .args(["-f", "elf64", "-o", obj_file.to_str().unwrap(), asm_path.to_str().unwrap()])
            .status()
            .expect("Failed to compile assembly file");

        // Create a static library from the object file
        let lib_name = format!("lib{}.a", asm_file.trim_end_matches(".asm"));
        Command::new("ar")
            .args(["crUs", &lib_name, obj_file.file_name().unwrap().to_str().unwrap()])
            .current_dir(&out_dir)
            .status()
            .expect("Failed to create static library");

        // Link the static library
        println!("cargo:rustc-link-lib=static={}", asm_file.trim_end_matches(".asm"));

        println!("cargo:rustc-link-search={}", out_dir.display());
    }
}
