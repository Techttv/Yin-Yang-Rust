use std::path::{Path, PathBuf};

/// Every location the program reads or writes, derived from one home directory.
#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub config_file: PathBuf,
    pub state_dir: PathBuf,
    pub systemd_dir: PathBuf,
}

impl Paths {
    pub fn from_env() -> Paths {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .expect("HOME is not set");
        Paths::under(&home)
    }

    pub fn under(home: &Path) -> Paths {
        Paths {
            home: home.to_path_buf(),
            config_file: home.join(".config/yinyang.json"),
            state_dir: home.join(".local/state/yinyang"),
            systemd_dir: home.join(".local/share/systemd/user"),
        }
    }

    /// Config files of the original Python program (`-O` runs use the `_dev` name).
    pub fn legacy_config_files(&self) -> [PathBuf; 2] {
        [
            self.home.join(".config/yin_yang_dev.json"),
            self.home.join(".config/yin_yang.json"),
        ]
    }
}
