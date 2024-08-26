use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let out_dir = manifest_dir.join("../target");
    let asm_dir = manifest_dir.join("asm");
    println!("cargo:rustc-link-search={}", out_dir.display());

    let asm_files = fs::read_dir(&asm_dir)
        .expect("Failed to read asm directory")
        .filter_map(|entry| {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("asm") {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<PathBuf>>();

    for asm_path in asm_files.iter() {
        // Generate the corresponding object file path
        let obj_file = {
            let mut path = out_dir.clone();
            let obj_filename = format!(
                "{}.o",
                asm_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .expect("Failed to get file stem")
            );
            path.push(&obj_filename);
            path
        };

        // Compile the assembly file to an object file
        Command::new("nasm")
            .args([
                "-f", "elf64",
                "-o",
                obj_file.to_str().unwrap(),
                asm_path.to_str().unwrap()
            ])
            .status()
            .expect("Failed to compile assembly file");

        // Create a static library from the object file
        let lib_name = format!(
            "lib{}.a",
            asm_path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("Failed to get file stem")
        );
        Command::new("ar")
            .args(["crUs", &lib_name, obj_file.file_name().unwrap().to_str().unwrap()])
            .current_dir(&out_dir)
            .status()
            .expect("Failed to create static library");

        // Link the static library
        println!(
            "cargo:rustc-link-lib=static={}",
            asm_path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("Failed to get file stem")
        );

        println!("cargo:rustc-link-search={}", out_dir.display());
    }
}
