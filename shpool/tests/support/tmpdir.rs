use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use rand::{distributions::Alphanumeric, Rng};

pub struct Dir {
    path: PathBuf,
}

impl Dir {
    pub fn new<P: AsRef<Path>>(prefix: P) -> anyhow::Result<Self> {
        let mut path = PathBuf::new();
        path.push(prefix);
        let rand_dir: String =
            rand::thread_rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();

        path.push(rand_dir);

        fs::create_dir_all(&path).context("ensuring tmp dir")?;

        Ok(Dir { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        if std::env::var("SHPOOL_LEAVE_TEST_LOGS").unwrap_or(String::from("")) == "true" {
            return;
        }

        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
