use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    let target = env::var("TARGET")?;
    if !target.contains("windows") {
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let repository_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .ok_or("could not locate workspace root")?;
    let source = repository_root.join("native/windows-bridge/input-bridge");
    let output = PathBuf::from(env::var("OUT_DIR")?).join("win32-input-bridge");
    let architecture = match env::var("CARGO_CFG_TARGET_ARCH")?.as_str() {
        "x86_64" => "x64",
        "x86" => "Win32",
        "aarch64" => "ARM64",
        other => {
            return Err(format!("unsupported input bridge target architecture: {other}").into())
        }
    };

    run(Command::new("cmake")
        .arg("-S")
        .arg(&source)
        .arg("-B")
        .arg(&output)
        .arg("-G")
        .arg("Visual Studio 17 2022")
        .arg("-A")
        .arg(architecture))?;
    run(Command::new("cmake")
        .arg("--build")
        .arg(&output)
        .arg("--config")
        .arg("Release")
        .arg("--target")
        .arg("unified-input-bridge")
        .arg("--parallel")
        .arg("4"))?;

    println!(
        "cargo:rustc-link-search=native={}",
        output.join("Release").display()
    );
    println!("cargo:rustc-link-lib=static=unified-input-bridge");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rerun-if-changed={}", source.display());
    Ok(())
}

fn run(command: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("native input bridge build failed: {command:?}").into());
    }
    Ok(())
}
