#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process;

use clap::{Arg, Command};
use log::{error, info, LevelFilter};
use log4rs::{
    append::console::{ConsoleAppender, Target},
    config::{Appender, Config, Root},
    filter::threshold::ThresholdFilter,
};

mod backup;
mod configuration;
mod formatter;
mod helper;
mod restore;

struct Arguments {
    backup_settings_file: String,
    mode: String,
}

async fn start_main() {
    let arguments = get_arguments();

    let backup_configuration =
        match configuration::Configuration::load(arguments.backup_settings_file.as_str()) {
            Ok(backup_configuration) => backup_configuration,
            Err(message) => {
                error!("Error: {}", message);
                return;
            }
        };

    match arguments.mode.as_str() {
        "backup" => match backup::Backup::start(backup_configuration).await {
            Ok(_) => {}
            Err(why) => {
                error!("{}", why);
                return;
            }
        },
        "restore" => match restore::Restore::start(backup_configuration).await {
            Ok(_) => {}
            Err(why) => {
                error!("{}", why);
                return;
            }
        },
        mode => {
            error!("invalid mode: {}", mode);
            return;
        }
    }
}

#[tokio::main]
async fn main() {
    let level = LevelFilter::Debug;
    let stderr = ConsoleAppender::builder().target(Target::Stderr).build();

    let config = Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(level)))
                .build("stderr", Box::new(stderr)),
        )
        .build(Root::builder().appender("stderr").build(LevelFilter::Info))
        .unwrap();
    log4rs::init_config(config).unwrap();

    start_main().await;
}

fn get_arguments() -> Arguments {
    let matches = Command::new("rusty-backup")
        .version(crate_version!())
        .author(crate_authors!())
        .about(
            "An app to backup/restore files/folders/databases to internal and external storages.",
        )
        .arg(
            Arg::new("backup-settings-file")
                .short('s')
                .long("backup-settings-file")
                .value_name("FILE")
                .help("The file, where to load the backup settings (default: backup_settings.xml)"),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Specify a config file from where to read settings"),
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .value_name("MODE")
                .help("backup (default) oder restore"),
        )
        .get_matches();

    let home_dir = dirs::home_dir()
        .unwrap_or(PathBuf::from("~"))
        .to_str()
        .unwrap()
        .to_owned();
    let config_file_name = match matches.get_one::<String>("config") {
        Some(file) => {
            let normalized_file = file.replace("~", &home_dir);
            if !Path::new(&normalized_file).exists() {
                error!("config file doesn't exists.");
                process::exit(1);
            };
            Some(String::from(file))
        }
        None => {
            let normalized = "~/rusty-backup.conf".replace("~", &home_dir);
            if Path::new("rusty-backup.conf").exists() {
                Some(String::from("rusty-backup.conf"))
            } else if Path::new(normalized.as_str()).exists() {
                Some(normalized)
            } else if Path::new("/etc/rusty-backup.conf").exists() {
                Some(String::from("/etc/rusty-backup.conf"))
            } else {
                None
            }
        }
    };

    let mut backup_settings_file = String::from("backup_settings.xml");
    let mut mode = String::from("backup");

    match config_file_name {
        Some(file_name) => {
            info!("read setting from {}", file_name);
            match File::open(file_name) {
                Ok(file) => {
                    let buffer = BufReader::new(&file);
                    for line in buffer.lines() {
                        if line.is_err() {
                            continue;
                        }
                        let l = line.unwrap();
                        let position_of_equal_sign = l.find("=");
                        if position_of_equal_sign.is_none() {
                            continue;
                        }
                        let position_of_equal_sign = position_of_equal_sign.unwrap();
                        let key = String::from(&l[..position_of_equal_sign])
                            .trim()
                            .to_string();
                        let value = String::from(&l[(position_of_equal_sign + 1)..])
                            .trim()
                            .to_string();

                        match key.as_str() {
                            "backup_settings_file" => {
                                backup_settings_file = value;
                            }
                            "mode" => {
                                mode = value;
                            }
                            _ => (),
                        }
                    }
                }
                Err(_) => (),
            }
        }
        None => (),
    }

    backup_settings_file = matches
        .get_one("backup-settings-file")
        .unwrap_or(&backup_settings_file)
        .to_string();
    mode = matches.get_one("mode").unwrap_or(&mode).to_string();

    Arguments {
        backup_settings_file: backup_settings_file,
        mode: mode,
    }
}
