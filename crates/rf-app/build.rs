use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=windows.manifest");
    println!("cargo:rerun-if-env-changed=WINDRES");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("windows.manifest");
    match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => {
            println!("cargo:rustc-link-arg-bin=rf-workbench=/MANIFEST:EMBED");
            println!(
                "cargo:rustc-link-arg-bin=rf-workbench=/MANIFESTINPUT:{}",
                manifest.display()
            );
        }
        Ok("gnu") => {
            let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
            let rc = output.join("rf-workbench.rc");
            let object = output.join("rf-workbench-manifest.o");
            // Resource type 24 is RT_MANIFEST; an executable uses resource ID 1.
            let path = manifest.to_string_lossy().replace('\\', "/");
            fs::write(&rc, format!("1 24 \"{path}\"\n")).expect("write manifest resource");
            let tool = env::var_os("WINDRES").unwrap_or_else(|| "windres".into());
            let status = Command::new(tool)
                .args(["--input-format=rc", "--output-format=coff"])
                .arg("--input")
                .arg(&rc)
                .arg("--output")
                .arg(&object)
                .status()
                .expect("Windows GNU builds require windres (or WINDRES pointing to it)");
            assert!(status.success(), "Windows manifest compilation failed");
            println!("cargo:rustc-link-arg-bin=rf-workbench={}", object.display());
        }
        _ => panic!("Unsupported Windows toolchain for manifest embedding"),
    }
}
