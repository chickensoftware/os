use std::{env, path::PathBuf, process::Command};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let out_dir = manifest_dir.join("../target");
    println!("cargo:rustc-link-search={}", out_dir.display());

    // boot.s -> boot.o -> libboot.a
    let out_boot_asm = {
        let mut path = out_dir.clone();
        path.push("boot.o");
        path
    };
    Command::new("nasm")
        .args(["-f", "elf64", "-o", out_boot_asm.to_str().unwrap(), "boot.s"])
        .status()
        .unwrap();
    Command::new("ar")
        .args(["crUs", "libboot.a", "boot.o"])
        .current_dir(&out_dir)
        .status()
        .unwrap();
    println!("cargo:rustc-link-lib=static=boot");
    println!("cargo:rustc-link-search={}", out_dir.display());
}
