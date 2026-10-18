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

    let capture_source = repository_root.join("native/windows-bridge/window-capture");
    let capture_output = PathBuf::from(env::var("OUT_DIR")?).join("window-capture-bridge");
    run(Command::new("cmake")
        .arg("-S")
        .arg(&capture_source)
        .arg("-B")
        .arg(&capture_output)
        .arg("-G")
        .arg("Visual Studio 17 2022")
        .arg("-A")
        .arg(architecture))?;
    run(Command::new("cmake")
        .arg("--build")
        .arg(&capture_output)
        .arg("--config")
        .arg("Release")
        .arg("--target")
        .arg("crosswarp-window-capture")
        .arg("--parallel")
        .arg("4"))?;

    println!(
        "cargo:rustc-link-search=native={}",
        capture_output.join("Release").display()
    );
    println!("cargo:rustc-link-lib=static=crosswarp-window-capture");
    println!("cargo:rustc-link-lib=d3d11");
    println!("cargo:rustc-link-lib=dxgi");
    println!("cargo:rustc-link-lib=windowsapp");
    println!(
        "cargo:rerun-if-changed={}",
        capture_source.join("window_capture.cpp").display()
    );

    let idd_client_source = repository_root.join("native/windows-bridge/idd-client");
    let idd_client_output = PathBuf::from(env::var("OUT_DIR")?).join("idd-client-bridge");
    run(Command::new("cmake")
        .arg("-S")
        .arg(&idd_client_source)
        .arg("-B")
        .arg(&idd_client_output)
        .arg("-G")
        .arg("Visual Studio 17 2022")
        .arg("-A")
        .arg(architecture))?;
    run(Command::new("cmake")
        .arg("--build")
        .arg(&idd_client_output)
        .arg("--config")
        .arg("Release")
        .arg("--target")
        .arg("crosswarp-idd-client")
        .arg("--parallel")
        .arg("4"))?;
    println!(
        "cargo:rustc-link-search=native={}",
        idd_client_output.join("Release").display()
    );
    println!("cargo:rustc-link-lib=static=crosswarp-idd-client");
    println!("cargo:rustc-link-lib=setupapi");
    println!(
        "cargo:rerun-if-changed={}",
        idd_client_source.join("idd_client.cpp").display()
    );
    Ok(())
}

fn run(command: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("native input bridge build failed: {command:?}").into());
    }
    Ok(())
}
