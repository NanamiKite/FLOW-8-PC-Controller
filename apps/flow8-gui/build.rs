use std::{env, fs, io, path::PathBuf, process::Command};

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=assets/flow8.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");
    println!("cargo:rerun-if-env-changed=AR");
    println!("cargo:rerun-if-env-changed=WINDRES_PREPROCESSOR");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }

    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        compile_gnu_icon()?;
    } else {
        winresource::WindowsResource::new()
            .set_icon("assets/flow8.ico")
            .compile()?;
    }
    Ok(())
}

fn compile_gnu_icon() -> io::Result<()> {
    // winresource passes the manifest directory as -I<path> to windres. Its
    // preprocessor splits paths containing spaces, including the shared-folder
    // path used by this project. Keep every compiler input relative to OUT_DIR.
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::copy("assets/flow8.ico", out_dir.join("flow8.ico"))?;
    fs::write(out_dir.join("flow8.rc"), "1 ICON \"flow8.ico\"\n")?;

    let target = env::var("TARGET").expect("Cargo sets TARGET");
    let cross_prefix = if env::var("HOST").expect("Cargo sets HOST") != target {
        match target.as_str() {
            "x86_64-pc-windows-gnu" => "x86_64-w64-mingw32-",
            "i686-pc-windows-gnu" => "i686-w64-mingw32-",
            _ => "",
        }
    } else {
        ""
    };
    let windres = env::var_os("WINDRES").unwrap_or_else(|| format!("{cross_prefix}windres").into());
    let ar = env::var_os("AR").unwrap_or_else(|| format!("{cross_prefix}ar").into());

    let mut resource = Command::new(windres);
    resource
        .current_dir(&out_dir)
        .args(["--input-format=rc", "--output-format=coff"]);
    // Useful for Linux cross-checks with standalone binutils: the icon .rc
    // needs preprocessing, but does not require a target C/C++ compiler.
    if let Some(preprocessor) = env::var_os("WINDRES_PREPROCESSOR") {
        resource.arg("--preprocessor").arg(preprocessor);
    }
    match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => {
            resource.arg("--target=pe-x86-64");
        }
        Ok("x86") => {
            resource.arg("--target=pe-i386");
        }
        _ => {}
    }
    run(resource.args(["flow8.rc", "flow8.o"]), "windres")?;

    run(
        Command::new(ar)
            .current_dir(&out_dir)
            .args(["rcs", "libflow8_icon.a", "flow8.o"]),
        "ar",
    )?;

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static:+whole-archive=flow8_icon");
    Ok(())
}

fn run(command: &mut Command, tool: &str) -> io::Result<()> {
    let status = command.status().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "cannot run {tool} ({command:?}): {error}; install the Windows resource toolchain or set WINDRES/AR to the appropriate tools"
            ),
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{tool} exited with {status}")))
    }
}
