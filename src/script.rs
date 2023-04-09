use std::{
    path::{Path, PathBuf},
    process::Command,
};

use eyre::{ensure, eyre, Result, WrapErr as _};
use path_absolutize::Absolutize as _;

pub struct Script {
    rust_script: PathBuf,
    script: PathBuf,
    package_dir: PathBuf,
    source_in_package: PathBuf,
}

impl Script {
    pub fn new(rust_script: PathBuf, script: PathBuf) -> Result<Self> {
        let script = script
            .absolutize()
            .wrap_err("failed to obtain the absolute path for the script path")?
            .to_path_buf();
        let package_dir = package_dir(&rust_script, &script)
            .wrap_err("failed to obtain the package path")?
            .absolutize()
            .wrap_err("failed to obtain the absolute path for the package path")?
            .to_path_buf();

        let mut filename = script
            .file_stem()
            .ok_or(eyre!("failed to obtain file stem"))?
            .to_owned();
        filename.push(".rs");
        let source_in_package = package_dir.join(filename);

        Ok(Self {
            rust_script,
            script,
            package_dir,
            source_in_package,
        })
    }

    pub fn script(&self) -> &PathBuf {
        &self.script
    }

    pub fn package_dir(&self) -> &PathBuf {
        &self.package_dir
    }

    pub fn source_in_package(&self) -> &PathBuf {
        &self.source_in_package
    }

    pub fn regenerate(&self) -> Result<()> {
        let output = Command::new(&self.rust_script)
            .arg("--package")
            .arg(&self.script)
            .output()
            .wrap_err("failed to spawn rust-script")?;
        ensure!(
            output.status.success(),
            "rust-script process terminated with a nonzero exit code {}",
            output.status
        );
        Ok(())
    }
}

fn package_dir(rust_script: impl AsRef<Path>, script: impl AsRef<Path>) -> Result<PathBuf> {
    let output = Command::new(rust_script.as_ref())
        .arg("--package")
        .arg(script.as_ref())
        .output()
        .wrap_err("failed to spawn rust-script")?;
    ensure!(
        output.status.success(),
        "rust-script process terminated with a nonzero exit code {}",
        output.status
    );
    let output = String::from_utf8(output.stdout).wrap_err("got an invalid path")?;
    Ok(PathBuf::from(output.trim_end()))
}
