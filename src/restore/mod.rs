use std::env;
use std::fs;
use std::time::SystemTime;

use chrono::Weekday;
use log::{info, warn};
use regex::Regex;

use crate::configuration::{destination::Kind as DestinationKind, Configuration};

pub struct Restore {}

impl Restore {
    fn build_possible_archive_names(name: String) -> Vec<String> {
        lazy_static! {
            static ref REGEX_DATE_YEAR: Regex = Regex::new(r"\{date:year\}").unwrap();
            static ref REGEX_DATE_MONTH: Regex = Regex::new(r"\{date:month\}").unwrap();
            static ref REGEX_DATE_DAY: Regex = Regex::new(r"\{date:day\}").unwrap();
            static ref REGEX_DATE_WEEKDAY: Regex = Regex::new(r"\{date:weekday\}").unwrap();
        }

        let name = name.as_str();

        let mut possible_archive_names = Vec::new();

        if let Some(_weekday_caps) = REGEX_DATE_WEEKDAY.captures(name) {
            possible_archive_names.push(
                REGEX_DATE_WEEKDAY
                    .replace(name, Weekday::Mon.to_string())
                    .to_string(),
            );
            possible_archive_names.push(
                REGEX_DATE_WEEKDAY
                    .replace(name, Weekday::Tue.to_string())
                    .to_string(),
            );
            possible_archive_names.push(
                REGEX_DATE_WEEKDAY
                    .replace(name, Weekday::Wed.to_string())
                    .to_string(),
            );
            possible_archive_names.push(
                REGEX_DATE_WEEKDAY
                    .replace(name, Weekday::Thu.to_string())
                    .to_string(),
            );
            possible_archive_names.push(
                REGEX_DATE_WEEKDAY
                    .replace(name, Weekday::Fri.to_string())
                    .to_string(),
            );
            possible_archive_names.push(
                REGEX_DATE_WEEKDAY
                    .replace(name, Weekday::Sat.to_string())
                    .to_string(),
            );
            possible_archive_names.push(
                REGEX_DATE_WEEKDAY
                    .replace(name, Weekday::Sun.to_string())
                    .to_string(),
            );
        }

        if possible_archive_names.len() == 0 {
            possible_archive_names.push(name.to_string());
        }

        possible_archive_names
    }

    fn get_newest_archive_name_in_directory(
        names: Vec<String>,
        archive: &crate::configuration::archive::Archive,
    ) -> Option<String> {
        let mut newest_archive_opt: Option<(String, SystemTime)> = None;
        let directory = archive.destination.path.clone();

        for name in names {
            let mut full_filename = format!(
                "{}/{}{}",
                directory,
                name,
                archive.compression.to_extension_string()
            );
            if let Some(encryption) = &archive.encryption {
                full_filename.push_str(encryption.to_extension_string().as_str());
            }
            if let Ok(metadata) = std::fs::metadata(full_filename) {
                if let Ok(created) = metadata.created() {
                    newest_archive_opt = match newest_archive_opt {
                        Some(newest_archive) => {
                            if created > newest_archive.1 {
                                Some((name, created))
                            } else {
                                Some(newest_archive)
                            }
                        }
                        None => Some((name, created)),
                    }
                }
            }
        }

        match newest_archive_opt {
            Some(newest_archive) => Some(newest_archive.0),
            None => None,
        }
    }

    fn map_error(err: std::io::Error) -> String {
        format!("error: {:?}", err)
    }

    pub async fn start(configuration: Configuration) -> Result<(), String> {
        fs::create_dir_all(&configuration.working_directory).map_err(Restore::map_error)?;
        env::set_current_dir(&configuration.working_directory).map_err(Restore::map_error)?;

        for archive in configuration.archives {
            let mut temporary_files_to_remove: Vec<String> = Vec::new();
            info!("restoring archive: {}", archive.name);
            let archive_filename_opt = match archive.destination.kind {
                DestinationKind::S3 => archive.destination.download_to_tmp(&archive).await?,
                DestinationKind::Directory => {
                    let possible_archive_names =
                        Self::build_possible_archive_names(archive.name.clone());
                    Self::get_newest_archive_name_in_directory(possible_archive_names, &archive)
                }
                DestinationKind::SSH => archive.destination.download_to_tmp(&archive).await?,
                DestinationKind::None => continue,
            };

            if let Some(archive_filename) = archive_filename_opt {
                let full_path = format!(
                    "{}{}",
                    archive_filename,
                    archive.compression.to_extension_string()
                );
                if let Some(encryption) = archive.encryption {
                    let encrypted_filename = format!("{}.enc", full_path);
                    if let Err(err) = encryption.decrypt_file(&encrypted_filename) {
                        return Err(err);
                    }
                    temporary_files_to_remove.push(full_path.clone());
                }
                if let Err(err) = archive.compression.decompress_file(
                    &full_path,
                    &archive.directories,
                    &archive.databases,
                ) {
                    return Err(err);
                }
            }

            for temporary_file in temporary_files_to_remove {
                if let Err(err) = fs::remove_file(&temporary_file) {
                    warn!(
                        "temporary file: '{}' could not been removed => {:?}",
                        temporary_file, err
                    );
                }
            }
        }

        Ok(())
    }
}
