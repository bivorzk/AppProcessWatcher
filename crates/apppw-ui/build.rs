use std::{env, fs, path::PathBuf};

fn main() {
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ARCH").as_deref(),
        Ok("x86_64"),
        "the bundled WinDivert driver currently supports only x86_64 Windows"
    );

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo did not provide OUT_DIR"));
    let executable_directory = output
        .ancestors()
        .nth(3)
        .expect("unexpected Cargo output directory");
    let assets = PathBuf::from("vendor/windivert");

    println!("cargo:rerun-if-changed={}", assets.display());
    fs::copy(
        assets.join("WinDivert.dll"),
        executable_directory.join("WinDivert.dll"),
    )
    .expect("could not copy WinDivert.dll beside apppw-ui.exe");
    fs::copy(
        assets.join("WinDivert64.sys"),
        executable_directory.join("WinDivert64.sys"),
    )
    .expect("could not copy WinDivert64.sys beside apppw-ui.exe");
    fs::copy(
        assets.join("LICENSE.txt"),
        executable_directory.join("WinDivert-LICENSE.txt"),
    )
    .expect("could not copy the WinDivert licence beside apppw-ui.exe");
}
