use ar::{Builder, Header};
use crate::error::CDResult;
use crate::manifest::Config;
use std::fs::File;
use std::fs;
use std::path::PathBuf;

pub struct DebArchive {
    out_abspath: PathBuf,
    ar_builder: Builder<File>,
}

impl DebArchive {
    pub fn new(config: &Config) -> CDResult<Self> {
        let out_filename = format!("{}_{}_{}.deb", config.deb_name, config.deb_version, config.architecture);
        let out_abspath = config.deb_output_path(&out_filename);
        {
            let deb_dir = out_abspath.parent().ok_or("invalid dir")?;
            let _ = fs::create_dir_all(deb_dir);
        }
        let ar_builder = Builder::new(File::create(&out_abspath)?);

        Ok(DebArchive {
            out_abspath,
            ar_builder,
        })
    }

    pub(crate) fn filename_glob(config: &Config) -> String {
        format!("{}_*_{}.deb", config.deb_name, config.architecture)
    }

    pub fn add_data(&mut self, dest_path: String, mtime_timestamp: u64, data: &[u8]) -> CDResult<()> {
        let mut header = Header::new(dest_path.into(), data.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(mtime_timestamp);
        header.set_uid(0);
        header.set_gid(0);
        self.ar_builder.append(&header, data)?;
        Ok(())
    }

    pub fn finish(self) -> CDResult<PathBuf> {
        Ok(self.out_abspath)
    }
}
