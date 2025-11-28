use std::{
    env,
    error::Error,
    fs::{self, File},
    io::{self, Seek as _, SeekFrom, Write as _},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

const ENV_VAR: &str = "SIZEBENCH";

const TARGET_TRIPLE: &str = include_str!(concat!(env!("OUT_DIR"), "/target.txt"));

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn cargo_build(release: bool) {
    let mut command = Command::new(env::var_os("CARGO").expect("`$CARGO` is not set"));
    command.args(["build", "--bin", "alerta", "--target", TARGET_TRIPLE]);
    if release {
        command.arg("--release");
    }
    let status = command.status().unwrap();
    assert!(status.success());
    thread::sleep(Duration::from_millis(1000));
}

fn upx(release: bool, infile: &str, outfile: &str) {
    let infile = path(release).with_file_name(infile);
    let outfile = path(release).with_file_name(outfile);
    fs::remove_file(&outfile).ok();
    let mut upx = Command::new("upx");
    if release {
        upx.arg("--brute");
    }
    let status = upx
        .arg("-o")
        .arg(outfile)
        .arg(infile)
        .status()
        .map_err(|e| format!("failed to run `upx`: {e}"))
        .unwrap();
    assert!(status.success());
}

fn path(release: bool) -> PathBuf {
    let profile = if release { "release" } else { "debug" };
    PathBuf::from(format!("../../target/{TARGET_TRIPLE}/{profile}/alerta"))
}

fn size(path: impl AsRef<Path>) -> Result<u64> {
    let path = path.as_ref();
    let mut f =
        File::open(path).map_err(|e| format!("failed to open '{}': {e}", path.display()))?;
    f.seek(SeekFrom::End(0))?;
    Ok(f.stream_position()?)
}

fn main() -> Result<()> {
    let mut summary = None;
    if let Some(step) = env::var_os("GITHUB_STEP_SUMMARY")
        && env::var_os(ENV_VAR).is_some()
    {
        summary = Some(File::options().append(true).create(true).open(step)?);
    }

    let mut append = |s: &str| -> io::Result<()> {
        if let Some(f) = &mut summary {
            writeln!(f, "{s}")?;
        }

        println!("{s}");
        Ok(())
    };

    let release = env::var_os(ENV_VAR).is_some();
    let profile = if release { "release" } else { "debug" };
    append("### `alerta` Binary Size")?;
    append("")?;
    append(&format!("Build profile: **`{profile}`**"))?;

    let path_release = path(release);
    let path_upx = path_release.with_file_name("alerta-upx");

    cargo_build(release);
    let size_release = size(&path_release)? / 1024;

    append("| File | Size |")?;
    append("|------|------|")?;
    append(&format!(
        "| `{}` | {size_release} KiB |",
        path_release.display()
    ))?;

    upx(release, "alerta", "alerta-upx");
    let size_upx = size(&path_upx)? / 1024;

    append(&format!("| `{}` | {size_upx} KiB |", path_upx.display()))?;

    Ok(())
}
