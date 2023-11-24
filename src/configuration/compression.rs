use std::{
    fs,
    fs::File,
    io::{Read, Write},
    os::unix::fs::chown,
    path::Path,
};

use bzip2::read::BzDecoder;
use log::{error, info};
use regex::Regex;

use crate::configuration::{Configuration, Directory};

use super::database::Database;

#[derive(Clone, Debug, PartialEq)]
pub enum Compression {
    None,
    Tar,
    TarBZ2,
}

impl Compression {
    pub fn decompress_file<S: AsRef<str>>(
        &self,
        file: S,
        output_dirs: &Vec<Directory>,
        dbs: &Vec<Database>,
    ) -> Result<(), String> {
        match self {
            Self::None => Ok(()),
            Self::Tar => self.decompress_tar_file(file, output_dirs, dbs),
            Self::TarBZ2 => self.decompress_tar_bz2_file(file, output_dirs, dbs),
        }
    }

    fn decompress_tar_bz2_file<S: AsRef<str>>(
        &self,
        file: S,
        output_dirs: &Vec<Directory>,
        dbs: &Vec<Database>,
    ) -> Result<(), String> {
        let file = file.as_ref();
        info!("extracting bz2-file: {}", file);
        let mut bz2 = match File::open(file) {
            Ok(file) => BzDecoder::new(file),
            Err(err) => return Err(format!("{}", err)),
        };
        lazy_static! {
            static ref REGEX_BZ2_EXT: Regex = Regex::new(r"\.bz2$").unwrap();
        }
        let tar_filename = REGEX_BZ2_EXT.replace(file, "").to_string();
        let mut tar_file = match File::create(&tar_filename) {
            Ok(file) => file,
            Err(err) => return Err(format!("{}", err)),
        };

        let mut buf = [0; Configuration::BUFFER_SIZE];
        loop {
            let read_bytes = match bz2.read(&mut buf) {
                Ok(read_bytes) => read_bytes,
                Err(err) => return Err(format!("{}", err)),
            };

            if read_bytes == 0 {
                break;
            }

            match tar_file.write_all(&buf[0..read_bytes]) {
                Ok(_) => {}
                Err(_) => {
                    info!("failed!");
                    return Err(format!("unable to write tar-file: '{}'", tar_filename));
                }
            }
        }

        info!("completed!");

        self.decompress_tar_file(tar_filename, output_dirs, dbs)
    }

    fn decompress_tar_file<S: AsRef<str>>(
        &self,
        tar_filename: S,
        output_dirs: &Vec<Directory>,
        dbs: &Vec<Database>,
    ) -> Result<(), String> {
        let tar_filename = tar_filename.as_ref();

        info!("extracting tar-file: {}", tar_filename);

        let tar_path = Path::new(&tar_filename);
        let tar_file = match File::open(tar_path) {
            Ok(file) => file,
            Err(err) => return Err(format!("{}", err)),
        };
        let mut tar = tar::Archive::new(tar_file);
        let entries = match tar.entries() {
            Ok(entries) => entries,
            Err(err) => return Err(format!("{}", err)),
        };

        entries.for_each(|e| {
            let mut entry = match e {
                Ok(entry) => entry,
                Err(_) => return,
            };

            let entry_str = match entry.path() {
                Ok(entry_path) => entry_path.to_string_lossy().to_string(),
                Err(_) => return,
            };
            let mut entry_directory_found = false;
            for directory in output_dirs {
                let dir_path = Path::new(&directory.name);
                let dir_name = match dir_path.file_name() {
                    Some(file_name) => file_name,
                    None => continue,
                };

                let dir_name_string = format!("{}/", dir_name.to_string_lossy());
                let dir_name_str = dir_name_string.as_str();
                if !entry_str.starts_with(dir_name_str) {
                    continue;
                }

                let parent_dir = match dir_path.parent() {
                    Some(parent_dir) => parent_dir,
                    None => break,
                };
                let dst = format!("{}/{}", parent_dir.to_string_lossy(), entry_str);
                let dst_path = Path::new(dst.as_str());
                match entry.unpack(dst_path) {
                    Ok(_) => {
                        let uid_opt = match directory.get_uid() {
                            Some(uid) => Some(uid.as_raw()),
                            None => None,
                        };
                        let gid_opt = match directory.get_gid() {
                            Some(gid) => Some(gid.as_raw()),
                            None => None,
                        };
                        if uid_opt.is_some() || gid_opt.is_some() {
                            chown(dst_path, uid_opt, gid_opt).unwrap_or_default();
                        }
                    }
                    Err(err) => {
                        error!("{}", err);
                    }
                }
                entry_directory_found = true;
                break;
            }

            if !entry_directory_found {
                for db in dbs {
                    let expected_string = format!("{}{}", db.name, db.kind.to_extension_string());
                    let expected_str = expected_string.as_str();
                    if expected_str != entry_str {
                        continue;
                    }

                    if let Err(err) = entry.unpack(expected_str) {
                        error!("{:?}", err);
                        continue;
                    };

                    if let Err(err) = db.delete_database() {
                        error!("db-error: {}", err);
                        continue;
                    }
                    if let Err(err) = db.create_database() {
                        error!("db-error: {}", err);
                        continue;
                    }

                    let file = match File::open(expected_str) {
                        Ok(file) => file,
                        Err(err) => {
                            error!("file-error: {}", err);
                            continue;
                        }
                    };
                    if let Err(err) = db.import_database(file) {
                        error!("db-error: {}", err);
                        continue;
                    }
                    if let Err(err) = fs::remove_file(expected_str) {
                        error!(
                            "error removing temporary file: {} => {:?}",
                            tar_filename, err
                        );
                    }
                }
            }
        });

        if let Err(err) = fs::remove_file(&tar_filename) {
            error!(
                "error removing temporary file: {} => {:?}",
                tar_filename, err
            );
        }

        info!("completed!");

        Ok(())
    }

    pub fn to_extension_string(&self) -> String {
        match self {
            Self::None => String::new(),
            Self::Tar => String::from(".tar"),
            Self::TarBZ2 => String::from(".tar.bz2"),
        }
    }
}
