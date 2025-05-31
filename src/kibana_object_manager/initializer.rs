use super::{Manifest, objects};
use eyre::{OptionExt, Result};
use owo_colors::OwoColorize;
use std::{fs::File, io::Write, path::PathBuf};

impl ToString for Initializer {
    fn to_string(&self) -> String {
        format!("{}", self.file.display())
    }
}

pub struct Initializer {
    pub file: PathBuf,
    pub manifest: PathBuf,
}

impl Initializer {
    pub fn initialize(&self) -> Result<()> {
        log::debug!(
            "Initializing directory {} with manifest {}",
            self.file.display().bright_black(),
            self.manifest.display().bright_black()
        );
        update_gitignore()?;
        Manifest::from_export(&self.file)?.write(&self.manifest)?;

        let path = self
            .file
            .parent()
            .ok_or_eyre("Failed to get parent directory")?
            .to_path_buf();
        objects::unbundle(&self.file, &path)
    }
}

pub fn update_gitignore() -> Result<()> {
    log::info!("Updating {}", ".gitignore".bright_black());
    let git_ignore = PathBuf::from(".gitignore");
    let mut file = File::options()
        .create(true)
        .append(true)
        .open(&git_ignore)?;
    let lines = vec![
        "# Added by --{kibob}-> (Kibana Object Manager)\n",
        ".env*\n",
        "export.ndjson\n",
        "import.ndjson\n",
        "import/\n",
        "response.json\n",
        "manifest_patch.json\n",
    ];

    let existing_content = std::fs::read_to_string(&git_ignore)?;

    for line in lines {
        if !existing_content.contains(line) {
            write!(file, "{}", line)?;
        }
    }
    Ok(())
}
