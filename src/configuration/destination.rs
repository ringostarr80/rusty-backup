use std::{
    fs::File,
    io::Write,
    ops::Sub,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use chrono::NaiveDateTime;
use log::{info, warn};
use rusoto_core::{Region, RusotoError};
use rusoto_s3::{GetObjectRequest, ListObjectsV2Error, ListObjectsV2Request, S3Client, S3};
use tokio::io::AsyncReadExt;

use crate::configuration::Archive;
use crate::formatter::Formatter;
use crate::helper::ProgressStats;

#[derive(Clone, Debug)]
pub struct Destination {
    pub kind: Kind,
    pub id: String,
    pub path: String,
    pub s3_bucket: String,
    pub s3_region: Region,
    pub max_archive_age: Option<Duration>,
}

impl Destination {
    pub fn new() -> Destination {
        Destination {
            kind: Kind::None,
            id: String::new(),
            path: String::new(),
            s3_bucket: String::new(),
            s3_region: Region::EuCentral1,
            max_archive_age: None,
        }
    }

    pub async fn download_to_tmp(&self, archive: &Archive) -> Result<Option<String>, String> {
        match self.kind {
            Kind::Directory => Ok(None),
            Kind::None => Ok(None),
            Kind::S3 => self.download_from_s3_to_tmp(archive).await,
        }
    }

    async fn download_from_s3_to_tmp(&self, archive: &Archive) -> Result<Option<String>, String> {
        let client = S3Client::new(self.s3_region.clone());

        let prefix_opt = match archive.name.find("{") {
            Some(index) => {
                if index > 0 {
                    let archive_name = archive.name.as_str();
                    Some(String::from(&archive_name[..index]))
                } else {
                    None
                }
            }
            None => None,
        };
        let list_objects_request = ListObjectsV2Request {
            bucket: archive.destination.s3_bucket.clone(),
            prefix: prefix_opt,
            ..Default::default()
        };

        let objects = client
            .list_objects_v2(list_objects_request)
            .await
            .map_err(Self::map_rusoto_list_objects_error)?;

        let contents = match objects.contents {
            Some(contents) => contents,
            None => {
                warn!("no S3 objects found.");
                return Ok(None);
            }
        };

        let mut last_known_key_opt: Option<String> = None;
        let mut last_known_datetime_opt: Option<NaiveDateTime> = None;

        for content in contents {
            let key = match content.key {
                Some(key) => key,
                None => continue,
            };

            let current_datetime_opt = match content.last_modified {
                Some(modified) => {
                    match NaiveDateTime::parse_from_str(modified.as_str(), "%Y-%m-%dT%H:%M:%S%.fZ")
                    {
                        Ok(date) => Some(date),
                        Err(_) => None,
                    }
                }
                None => None,
            };

            match last_known_datetime_opt {
                Some(last_known_datetime) => match current_datetime_opt {
                    Some(current_datetime) => {
                        let datetime_diff = last_known_datetime.sub(current_datetime);
                        if datetime_diff.num_seconds() < 0 {
                            last_known_datetime_opt = Some(current_datetime);
                            last_known_key_opt = Some(key);
                        }
                    }
                    None => {}
                },
                None => match current_datetime_opt {
                    Some(current_datetime) => {
                        last_known_datetime_opt = Some(current_datetime);
                        last_known_key_opt = Some(key);
                    }
                    None => {}
                },
            }
        }

        let key = match last_known_key_opt {
            Some(key) => key,
            None => {
                warn!("no S3-key found.");
                return Ok(None);
            }
        };

        info!("found latest key: {:?}", key);
        let object_request = GetObjectRequest {
            bucket: archive.destination.s3_bucket.clone(),
            key: key.clone(),
            ..Default::default()
        };
        let object = client
            .get_object(object_request)
            .await
            .map_err(Self::map_rusoto_get_object_error)?;
        print!("downloading...");
        let mut download_stats = ProgressStats::new();
        if let Some(content_length) = object.content_length {
            download_stats.total_length = Some(content_length as usize);
        }
        let arc_download_stats = Arc::new(Mutex::new(download_stats));
        let cloned_arc_download_stats = Arc::clone(&arc_download_stats);

        let streaming_body = match object.body {
            Some(streaming_body) => streaming_body,
            None => {
                warn!("no body in S3-object.");
                return Ok(None);
            }
        };

        let mut body = streaming_body.into_async_read();
        let archive_filename = format!("{}", key);
        let mut f = File::create(&archive_filename).map_err(Self::map_error)?;

        let thread = thread::spawn(move || {
            let cloned_arc_download_stats = Arc::clone(&arc_download_stats);
            loop {
                let mut output_string = String::from("downloading... ");
                match cloned_arc_download_stats.lock() {
                    Ok(download_stats) => {
                        output_string.push_str(
                            Formatter::format_size(download_stats.progressed_size, 2).as_str(),
                        );
                        if let Some(content_length) = download_stats.total_length {
                            output_string.push_str(
                                format!(
                                    "/{} ({number:.2}%)",
                                    Formatter::format_size(content_length, 2),
                                    number = download_stats.get_progress_in_percentage().unwrap()
                                )
                                .as_str(),
                            );
                        }
                        output_string.push_str(
                            format!("; runtime: {}", download_stats.get_formatted_runtime())
                                .as_str(),
                        );
                        if let Some(formatted_ete) = download_stats.get_formatted_ete() {
                            output_string.push_str(format!("; ete: {}", formatted_ete).as_str());
                        }
                        output_string.push_str(
                            format!(
                                "; speed: {}/s",
                                Formatter::format_size(download_stats.get_average_speed(), 2)
                            )
                            .as_str(),
                        );
                        output_string.push_str(
                            format!(
                                "; speed (<=1s): {}/s",
                                Formatter::format_size(
                                    download_stats.get_average_speed_for_last_second(),
                                    2
                                )
                            )
                            .as_str(),
                        );
                        output_string.push_str(
                            format!(
                                "; speed (<=10s): {}/s",
                                Formatter::format_size(
                                    download_stats.get_average_speed_for_last_10_seconds(),
                                    2
                                )
                            )
                            .as_str(),
                        );

                        print!("{}\r{}", termion::clear::CurrentLine, output_string);
                        std::io::stdout().flush().unwrap_or_default();

                        if download_stats.is_finished() {
                            break;
                        }
                    }
                    Err(_) => continue,
                }

                thread::sleep(Duration::from_millis(250));
            }
        });

        let mut done = false;
        while !done {
            let mut buffer = vec![];
            let read_bytes = body.read_buf(&mut buffer).await.map_err(Self::map_error)?;
            if read_bytes > 0 {
                f.write_all(&buffer[..read_bytes])
                    .map_err(Self::map_error)?;
                match cloned_arc_download_stats.lock() {
                    Ok(mut download_stats) => {
                        download_stats.add_progressed_size(read_bytes);
                    }
                    Err(_) => {}
                }
            } else {
                done = true;
                match cloned_arc_download_stats.lock() {
                    Ok(mut download_stats) => {
                        download_stats.set_finished();
                    }
                    Err(_) => {}
                }
            }
        }

        thread.join().unwrap_or_default();
        println!();

        let mut archive_name = archive_filename.clone();
        if let Some(encryption) = &archive.encryption {
            let enc_ext = encryption.to_extension_string();
            if archive_name.ends_with(&enc_ext) {
                archive_name = archive_name[..archive_name.len() - enc_ext.len()].to_string();
            }
        }
        let comp_ext = archive.compression.to_extension_string();
        if archive_name.ends_with(&comp_ext) {
            archive_name = archive_name[..archive_name.len() - comp_ext.len()].to_string();
        }

        Ok(Some(archive_name))
    }

    fn map_error(err: std::io::Error) -> String {
        format!("error: {:?}", err)
    }

    fn map_rusoto_get_object_error(
        err: rusoto_core::RusotoError<rusoto_s3::GetObjectError>,
    ) -> String {
        format!("error: {:?}", err)
    }

    fn map_rusoto_list_objects_error(err: RusotoError<ListObjectsV2Error>) -> String {
        format!("error: {:?}", err)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Directory,
    None,
    S3,
}
