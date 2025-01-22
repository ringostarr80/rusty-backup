use std::env;
use std::fs;
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Stdio;

use bytes::Bytes;
use bzip2::write::BzEncoder;
use chrono::{Datelike, Utc};
use futures::{FutureExt, TryStreamExt};
use futures_fs::FsPool;
use log::{error, info};
use regex::Regex;
use rusoto_s3::{PutObjectRequest, S3Client, StreamingBody, S3};
use ssh2::Session;
use tar::Builder;

use crate::configuration::{
    compression::Compression, database::Database, destination::Kind as DestinationKind,
    directory::Directory, Configuration,
};

pub struct Backup {}

impl Backup {
    fn map_error(err: std::io::Error) -> String {
        format!("error: {:?}", err)
    }

    fn build_real_archive_name(mut name: String) -> String {
        lazy_static! {
            static ref REGEX_DATE_YEAR: Regex = Regex::new(r"\{date:year\}").unwrap();
            static ref REGEX_DATE_MONTH: Regex = Regex::new(r"\{date:month\}").unwrap();
            static ref REGEX_DATE_DAY: Regex = Regex::new(r"\{date:day\}").unwrap();
            static ref REGEX_DATE_WEEKDAY: Regex = Regex::new(r"\{date:weekday\}").unwrap();
        }
        let now = Utc::now();
        name = REGEX_DATE_YEAR
            .replace_all(
                name.as_str(),
                format!("{:0width$}", now.year(), width = 4).as_str(),
            )
            .into_owned();
        name = REGEX_DATE_MONTH
            .replace_all(
                name.as_str(),
                format!("{:0width$}", now.month(), width = 2).as_str(),
            )
            .into_owned();
        name = REGEX_DATE_DAY
            .replace_all(
                name.as_str(),
                format!("{:0width$}", now.day(), width = 2).as_str(),
            )
            .into_owned();
        name = REGEX_DATE_WEEKDAY
            .replace_all(name.as_str(), format!("{:?}", now.weekday()).as_str())
            .into_owned();

        name
    }

    pub async fn start(configuration: Configuration) -> Result<(), String> {
        fs::create_dir_all(&configuration.working_directory).map_err(Backup::map_error)?;
        env::set_current_dir(&configuration.working_directory).map_err(Backup::map_error)?;

        for archive in configuration.archives {
            let mut real_archive_name = Backup::build_real_archive_name(archive.name);

            info!("creating archive: {}", real_archive_name);
            let mut files_to_move_to_destination: Vec<String> = Vec::new();
            let mut temporary_files: Vec<String> = Vec::new();

            if archive.compression == Compression::Tar || archive.compression == Compression::TarBZ2
            {
                match Backup::tar_archive(
                    &real_archive_name,
                    &archive.directories,
                    &archive.databases,
                ) {
                    Ok(tar_file) => {
                        real_archive_name = tar_file.clone();
                        if archive.compression == Compression::Tar {
                            files_to_move_to_destination.push(tar_file);
                        }
                    }
                    Err(error) => {
                        return Err(error);
                    }
                }

                if archive.compression == Compression::TarBZ2 {
                    temporary_files.push(real_archive_name.clone());
                    match Backup::bz2_archive(&real_archive_name) {
                        Ok(bz2_file) => {
                            files_to_move_to_destination.push(bz2_file);
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
            }

            match archive.encryption {
                Some(encryption) => {
                    for file in &files_to_move_to_destination {
                        if let Err(err) = encryption.encrypt_file(file) {
                            return Err(err);
                        }
                    }

                    let cloned = files_to_move_to_destination.clone();
                    files_to_move_to_destination.clear();
                    for mut cloned_element in cloned {
                        cloned_element.push_str(".enc");
                        files_to_move_to_destination.push(cloned_element);
                    }
                }
                None => {}
            }

            match archive.destination.kind {
                DestinationKind::Directory => {
                    let mut archive_path = archive.destination.path;
                    archive_path.push_str("/");
                    if fs::create_dir_all(&archive_path).is_err() {
                        return Err(format!("unable to create archive_path: '{}'", archive_path));
                    }

                    for file in files_to_move_to_destination {
                        let mut new_archive_name = archive_path.clone();
                        new_archive_name.push_str(file.as_str());
                        match fs::rename(&file, &new_archive_name) {
                            Ok(_) => {}
                            Err(_rename_err) => match fs::copy(&file, &new_archive_name) {
                                Ok(_) => match fs::remove_file(&file) {
                                    Ok(_) => {}
                                    Err(_) => {}
                                },
                                Err(copy_err) => {
                                    return Err(format!(
                                        "unable to rename and copy '{}' to '{}'! error: {:?}",
                                        file, new_archive_name, copy_err
                                    ));
                                }
                            },
                        }
                    }
                }
                DestinationKind::S3 => {
                    let fs = FsPool::default();
                    let client = S3Client::new(archive.destination.s3_region);

                    for file in files_to_move_to_destination {
                        match fs::metadata(&file) {
                            Ok(meta) => {
                                info!("uploading file: {}", &file);
                                let object_key = file.clone();
                                let read_stream = tokio::fs::read(file)
                                    .into_stream()
                                    .map_ok(|b| Bytes::from(b));

                                let put_object_request = PutObjectRequest {
                                    bucket: archive.destination.s3_bucket.clone(),
                                    key: object_key.clone(),
                                    content_length: Some(meta.len() as i64),
                                    body: Some(StreamingBody::new(read_stream)),
                                    server_side_encryption: Some(String::from("AES256")),
                                    ..Default::default()
                                };
                                match client.put_object(put_object_request).await {
                                    Ok(foo) => {
                                        info!("put_object ok: {:?}", foo);
                                        fs.delete(object_key.clone());
                                    }
                                    Err(err) => {
                                        error!("put_object err: {:?}", err);
                                    }
                                }
                            }
                            Err(err) => {
                                error!("fs::metadata({}) err: {}", file, err);
                            }
                        }
                    }
                }
                DestinationKind::SSH => {
                    let addr = format!("{}:22", archive.destination.server);
                    let tcp = TcpStream::connect(addr).unwrap();
                    let mut ssh2_session = Session::new().unwrap();
                    ssh2_session.set_tcp_stream(tcp);
                    ssh2_session.handshake().unwrap();
                    ssh2_session.userauth_password(&archive.destination.username, &archive.destination.password).unwrap();

                    for filename in files_to_move_to_destination {
                        match fs::metadata(&filename) {
                            Ok(meta) => {
                                info!("uploading file: {}", &filename);

                                let mut remote_file = ssh2_session.scp_send(Path::new(&filename), 0o644, meta.size(), None).unwrap();

                                let mut file = fs::File::open(&filename).unwrap();
                                // more than 32KB seems to be too much for the buffer, so that not the complete file is transferred.
                                let mut buf = [0; 32 * 1_024]; // 32KB
                                let mut read_bytes = file.read(&mut buf).unwrap();
                                while read_bytes > 0 {
                                    remote_file.write(&buf).unwrap();
                                    read_bytes = file.read(&mut buf).unwrap();
                                }

                                remote_file.send_eof().unwrap();
                                remote_file.wait_eof().unwrap();
                                remote_file.close().unwrap();
                                remote_file.wait_close().unwrap();
                            }
                            Err(err) => {
                                error!("fs::metadata({}) err: {}", filename, err);
                            }
                        }
                    }
                }
                DestinationKind::None => {}
            }

            for file in temporary_files {
                if fs::remove_file(&file).is_err() {
                    return Err(format!("unable to remove temporary file: '{}'", file));
                }
            }
        }

        Ok(())
    }

    fn bz2_archive(archive_name: &String) -> Result<String, String> {
        let bz2_archive_name = format!("{}.bz2", archive_name);
        match File::create(&bz2_archive_name) {
            Ok(file) => {
                let mut bz2 = BzEncoder::new(file, bzip2::Compression::best());
                info!("bzip file: '{}' ...", &archive_name);
                match File::open(&archive_name) {
                    Ok(mut tar_file) => {
                        let mut done = false;
                        let mut buf = [0; Configuration::BUFFER_SIZE];
                        while !done {
                            match tar_file.read(&mut buf) {
                                Ok(read_bytes) => {
                                    if read_bytes > 0 {
                                        match bz2.write_all(&buf[0..read_bytes]) {
                                            Ok(_) => {}
                                            Err(_) => {
                                                info!(" failed!");
                                                return Err(format!(
                                                    "unable to write bz2-file: '{}.bz2'",
                                                    archive_name
                                                ));
                                            }
                                        }
                                    } else {
                                        done = true;
                                    }
                                }
                                Err(_) => {
                                    info!(" failed!");
                                    return Err(format!(
                                        "unable to read tar-file: '{}'",
                                        archive_name
                                    ));
                                }
                            }
                        }

                        match bz2.finish() {
                            Ok(_) => {
                                info!(" completed!");
                            }
                            Err(_) => {
                                info!(" failed!");
                                return Err(format!("unable to finish bz2 stream."));
                            }
                        }
                    }
                    Err(_) => {
                        return Err(format!("unable to open tar-file: '{}'", archive_name));
                    }
                }
            }
            Err(_) => {
                return Err(format!("unable to create file '{}'", archive_name));
            }
        }

        Ok(bz2_archive_name)
    }

    fn tar_archive(
        archive_name: &String,
        directories: &Vec<Directory>,
        databases: &Vec<Database>,
    ) -> Result<String, String> {
        let archive_name = format!("{}.tar", archive_name);
        lazy_static! {
            static ref REGEX_PATH: Regex = Regex::new(r".*/").unwrap();
        }

        match File::create(&archive_name) {
            Ok(file) => {
                let mut tar = Builder::new(file);
                for directory in directories {
                    let archive_directory_string = REGEX_PATH
                        .replace_all(directory.name.as_str(), "")
                        .into_owned();
                    match tar.append_dir_all(archive_directory_string, &directory.name) {
                        Ok(_) => {}
                        Err(error) => {
                            return Err(format!(
                                "tar.append_dir_all: unable to append directory: {}\nerror: {:?}",
                                directory.name, error
                            ));
                        }
                    }
                }

                for database in databases {
                    let mut dump_command = database.build_dump_command();
                    let db_filename = database.build_dump_filename();

                    File::create(&db_filename).and_then(|dump_output| {
						dump_command
							.stdout(Stdio::from(dump_output))
							.spawn()
					}).map_err(|_err| {
						format!("error while executing command")
					}).and_then(|child| {
						child.wait_with_output().map_err(|_err| {
							format!("error while executing command")
						})
					}).and_then(|output| {
						match output.status.code() {
							Some(0) => {
								info!("tar file: '{}' ...", &db_filename);
								match File::open(&db_filename) {
									Ok(mut db_file) => {
										match tar.append_file(&db_filename, &mut db_file) {
											Ok(_) => {

											},
											Err(_) => {
												return Err(format!("tar.append_file: unable to append directory: {}", db_filename));
											}
										}
										if fs::remove_file(&db_filename).is_err() {
											return Err(format!("unable to remove sql file: {}", db_filename));
										}
									},
									Err(_) => {
										error!(" failed!");
										return Err(format!("unable to open sql file: {}", db_filename));
									}
								}
							},
							Some(_) => {

							},
							None => {

							}
						}

						Ok(())
					}).unwrap();
                }
            }
            Err(why) => match why.kind() {
                ErrorKind::AlreadyExists => {
                    return Err(format!(
                        "unable to create file: {}.tar => already exists",
                        archive_name
                    ));
                }
                ErrorKind::PermissionDenied => {
                    return Err(format!(
                        "unable to create file: {}.tar => permission denied",
                        archive_name
                    ));
                }
                _ => {
                    return Err(format!("unable to create file: {}.tar", archive_name));
                }
            },
        }

        Ok(archive_name)
    }
}
