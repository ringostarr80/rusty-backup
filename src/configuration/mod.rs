use std::{
    fs::File,
    io::{BufReader, ErrorKind, Seek, SeekFrom},
};

use rusoto_core::Region;
use xml::reader::{EventReader, XmlEvent as XmlReaderEvent};

pub mod archive;
pub mod compression;
pub mod credential;
pub mod database;
pub mod destination;
pub mod directory;
pub mod encryption;
pub mod program_parameter;

use archive::Archive;
use compression::Compression;
use credential::Credential;
use database::{Database, Kind as DatabaseKind};
use destination::{Destination, Kind as DestinationKind};
use directory::Directory;
use encryption::Encryption;
use program_parameter::ProgramParameter;

pub struct Configuration {
    pub archives: Vec<Archive>,
    pub databases: Vec<Database>,
    pub credentials: Vec<Credential>,
    pub destinations: Vec<Destination>,
    pub encryptions: Vec<Encryption>,
    pub working_directory: String,
}

impl Configuration {
    pub const BUFFER_SIZE: usize = 32576;

    pub fn new() -> Configuration {
        Configuration {
            archives: Vec::new(),
            databases: Vec::new(),
            credentials: Vec::new(),
            destinations: Vec::new(),
            encryptions: Vec::new(),
            working_directory: String::new(),
        }
    }

    pub fn load(filename: &str) -> Result<Configuration, String> {
        let mut configuration = Configuration::new();

        let mut real_filename = String::from(filename);
        match dirs::home_dir() {
            Some(home_dir) => match home_dir.to_str() {
                Some(home) => {
                    real_filename = real_filename.replace("~", home);
                }
                None => {}
            },
            None => {}
        }

        match File::open(real_filename) {
            Ok(mut file) => {
                let mut archive = Archive::new();
                let mut database = Database::new();
                let mut destination = Destination::new();
                let mut encryption = Encryption::new();

                match file.try_clone() {
                    Ok(cloned_file) => {
                        let mut depth = 0;
                        let pre_parser = EventReader::new(BufReader::new(cloned_file));
                        for e in pre_parser {
                            match e {
                                Ok(XmlReaderEvent::StartElement {
                                    name, attributes, ..
                                }) => {
                                    depth += 1;
                                    match name.to_string().as_str() {
                                        "backup-configuration" => {
                                            for attr in attributes {
                                                match attr.name.to_string().as_str() {
                                                    "working-directory" => {
                                                        configuration.working_directory =
                                                            attr.value;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        "database" => {
                                            if depth == 3 {
                                                database = Database::new();

                                                for attr in attributes {
                                                    match attr.name.to_string().as_str() {
                                                        "kind" => match attr.value.as_str() {
                                                            "mongodb" => {
                                                                database.kind =
                                                                    DatabaseKind::MongoDB;
                                                            }
                                                            "mysql" => {
                                                                database.kind = DatabaseKind::MySql;
                                                            }
                                                            "postgresql" => {
                                                                database.kind =
                                                                    DatabaseKind::PostgreSql;
                                                            }
                                                            kind => {
                                                                return Err(format!("invalid database kind value '{}'.", kind));
                                                            }
                                                        },
                                                        "id" => {
                                                            database.id = attr.value;
                                                        }
                                                        "username" => {
                                                            database.credential.username =
                                                                attr.value;
                                                        }
                                                        "password" => {
                                                            database.credential.password =
                                                                attr.value;
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        "destinations" => {}
                                        "destination" => {
                                            destination = Destination::new();

                                            for attr in attributes {
                                                match attr.name.to_string().as_str() {
                                                    "bucket" => {
                                                        destination.s3_bucket = attr.value;
                                                    }
                                                    "kind" => match attr.value.as_str() {
                                                        "none" => {
                                                            destination.kind =
                                                                DestinationKind::None;
                                                        }
                                                        "directory" => {
                                                            destination.kind =
                                                                DestinationKind::Directory;
                                                        }
                                                        "s3" => {
                                                            destination.kind = DestinationKind::S3;
                                                        }
                                                        "ssh" => {
                                                            destination.kind = DestinationKind::SSH;
                                                        }
                                                        kind => {
                                                            return Err(format!("invalid destination kind value '{}'.", kind));
                                                        }
                                                    },
                                                    "max-archive-age" => {
                                                        match parse_duration0::parse(
                                                            attr.value.as_str(),
                                                        ) {
                                                            Ok(duration) => {
                                                                destination.max_archive_age =
                                                                    Some(duration);
                                                            }
                                                            Err(_) => {}
                                                        }
                                                    }
                                                    "password" => {
                                                        destination.password = attr.value;
                                                    }
                                                    "path" => {
                                                        destination.path = attr.value;
                                                    }
                                                    "id" => {
                                                        destination.id = attr.value;
                                                    }
                                                    "region" => match attr.value.as_str() {
                                                        "ap-northeast-1" => {
                                                            destination.s3_region =
                                                                Region::ApNortheast1;
                                                        }
                                                        "ap-northeast-2" => {
                                                            destination.s3_region =
                                                                Region::ApNortheast2;
                                                        }
                                                        "ap-south-1" => {
                                                            destination.s3_region =
                                                                Region::ApSouth1;
                                                        }
                                                        "ap-southeast-1" => {
                                                            destination.s3_region =
                                                                Region::ApSoutheast1;
                                                        }
                                                        "ap-southeast-2" => {
                                                            destination.s3_region =
                                                                Region::ApSoutheast2;
                                                        }
                                                        "ca-central-1" => {
                                                            destination.s3_region =
                                                                Region::CaCentral1;
                                                        }
                                                        "cn-north-1" => {
                                                            destination.s3_region =
                                                                Region::CnNorth1;
                                                        }
                                                        "cn-northwest-1" => {
                                                            destination.s3_region =
                                                                Region::CnNorthwest1;
                                                        }
                                                        "eu-central-1" => {
                                                            destination.s3_region =
                                                                Region::EuCentral1;
                                                        }
                                                        "storj-eu1" => {
                                                            destination.s3_region = Region::Custom {
                                                                name: "StorjEu1".to_string(),
                                                                endpoint:
                                                                    "https://gateway.storjshare.io"
                                                                        .to_string(),
                                                            }
                                                        }
                                                        "eu-west-1" => {
                                                            destination.s3_region = Region::EuWest1;
                                                        }
                                                        "eu-west-2" => {
                                                            destination.s3_region = Region::EuWest2;
                                                        }
                                                        "eu-west-3" => {
                                                            destination.s3_region = Region::EuWest3;
                                                        }
                                                        "sa-east-1" => {
                                                            destination.s3_region = Region::SaEast1;
                                                        }
                                                        "us-east-1" => {
                                                            destination.s3_region = Region::UsEast1;
                                                        }
                                                        "us-east-2" => {
                                                            destination.s3_region = Region::UsEast2;
                                                        }
                                                        "us-gov-west-1" => {
                                                            destination.s3_region =
                                                                Region::UsGovWest1;
                                                        }
                                                        "us-west-1" => {
                                                            destination.s3_region = Region::UsWest1;
                                                        }
                                                        "us-west-2" => {
                                                            destination.s3_region = Region::UsWest2;
                                                        }
                                                        region => {
                                                            return Err(format!("invalid destination region value '{}'.", region));
                                                        }
                                                    },
                                                    "server" => {
                                                        destination.server = attr.value;
                                                    }
                                                    "username" => {
                                                        destination.username = attr.value;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        "encryptions" => {}
                                        "encryption" => {
                                            encryption = Encryption::new();
                                            for attr in attributes {
                                                match attr.name.to_string().as_str() {
                                                    "cipher" => {
                                                        encryption.cipher = attr.value;
                                                    }
                                                    "id" => {
                                                        encryption.id = attr.value;
                                                    }
                                                    "password" => {
                                                        encryption.password = attr.value;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        "parameters" => {}
                                        "parameter" => {
                                            let mut parameter = ProgramParameter::new();

                                            for attr in attributes {
                                                match attr.name.to_string().as_str() {
                                                    "assign-sign" => {
                                                        parameter.assign_sign = attr.value;
                                                    }
                                                    "longname" => {
                                                        parameter.longname = Some(attr.value);
                                                    }
                                                    "shortname" => {
                                                        parameter.shortname = Some(attr.value);
                                                    }
                                                    "value" => {
                                                        parameter.value = Some(attr.value);
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                Ok(XmlReaderEvent::EndElement { name }) => {
                                    match name.to_string().as_str() {
                                        "database" => {
                                            if depth == 3 {
                                                if database.id.len() != 0 {
                                                    for db in &configuration.databases {
                                                        if db.id == database.id {
                                                            return Err(format!("the database-id '{}' already exists", database.id));
                                                        }
                                                    }
                                                    configuration.databases.push(database.clone());
                                                }
                                            }
                                        }
                                        "destination" => {
                                            if destination.kind != DestinationKind::None
                                                && destination.id.len() != 0
                                            {
                                                for dest in &configuration.destinations {
                                                    if dest.id == destination.id {
                                                        return Err(format!("the destination-id '{}' already exists", destination.id));
                                                    }
                                                }
                                                if destination.kind == DestinationKind::S3 {
                                                    if destination.s3_bucket.len() == 0 {
                                                        return Err(format!("the destination-bucket must be set for kind: s3"));
                                                    }
                                                }
                                                configuration
                                                    .destinations
                                                    .push(destination.clone());
                                            }
                                        }
                                        "encryption" => {
                                            if encryption.id.len() > 0
                                                && encryption.password.len() > 0
                                                && encryption.cipher.len() > 0
                                            {
                                                configuration.encryptions.push(encryption.clone());
                                            }
                                        }
                                        _ => {}
                                    }
                                    depth -= 1;
                                }
                                Err(err) => {
                                    return Err(format!("XML-Error: {:?}", err));
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => {}
                }

                match file.seek(SeekFrom::Start(0)) {
                    Ok(_) => {
                        let mut global_db_id = String::new();
                        let mut depth = 0;
                        let parser = EventReader::new(BufReader::new(file));
                        for e in parser {
                            match e {
                                Ok(XmlReaderEvent::StartElement {
                                    name, attributes, ..
                                }) => {
                                    depth += 1;
                                    match name.to_string().as_str() {
                                        "databases" => {
                                            if depth == 4 {
                                                for attr in attributes {
                                                    match attr.name.to_string().as_str() {
                                                        "db-id" => {
                                                            global_db_id = attr.value;
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        "database" => {
                                            if depth == 5 {
                                                let mut database = Database::new();
                                                let mut db_id = global_db_id.clone();
                                                let mut db_name = String::new();
                                                let mut db_name_is_regex = false;
                                                for attr in attributes {
                                                    match attr.name.to_string().as_str() {
                                                        "name" => {
                                                            db_name = attr.value;
                                                        }
                                                        "name-is-regex" => {
                                                            match attr.value.as_str() {
                                                                "1" => {
                                                                    db_name_is_regex = true;
                                                                }
                                                                "true" => {
                                                                    db_name_is_regex = true;
                                                                }
                                                                "yes" => {
                                                                    db_name_is_regex = true;
                                                                }
                                                                "on" => {
                                                                    db_name_is_regex = true;
                                                                }
                                                                "enabled" => {
                                                                    db_name_is_regex = true;
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                        "db-id" => {
                                                            db_id = attr.value;
                                                        }
                                                        _ => {}
                                                    }
                                                }

                                                if db_id.len() == 0 {
                                                    return Err(format!(
                                                        "no db-id was given in configuration"
                                                    ));
                                                }
                                                if db_name.len() == 0 {
                                                    return Err(format!(
                                                        "no db-name was given in configuration"
                                                    ));
                                                }

                                                let mut db_found = false;
                                                for db in &configuration.databases {
                                                    if db.id != db_id {
                                                        continue;
                                                    }

                                                    db_found = true;
                                                    database = db.clone();
                                                    database.name = db_name.clone();
                                                    database.name_is_regex = db_name_is_regex;
                                                }

                                                if !db_found {
                                                    return Err(format!(
                                                        "no database with id '{}' found",
                                                        db_id
                                                    ));
                                                }

                                                archive.databases.push(database);
                                            }
                                        }
                                        "archives" => {}
                                        "archive" => {
                                            archive = Archive::new();

                                            for attr in attributes {
                                                match attr.name.to_string().as_str() {
                                                    "compression" => match attr.value.as_str() {
                                                        "none" => {
                                                            archive.compression = Compression::None;
                                                        }
                                                        "tar" => {
                                                            archive.compression = Compression::Tar;
                                                        }
                                                        "tar.bz2" => {
                                                            archive.compression =
                                                                Compression::TarBZ2;
                                                        }
                                                        compression => {
                                                            return Err(format!(
                                                                "invalid compression value '{}'.",
                                                                compression
                                                            ));
                                                        }
                                                    },
                                                    "destination" => {
                                                        let mut destination_found = false;
                                                        for dest in &configuration.destinations {
                                                            if dest.id == attr.value {
                                                                archive.destination = dest.clone();
                                                                destination_found = true;
                                                                break;
                                                            }
                                                        }

                                                        if !destination_found {
                                                            return Err(format!("destination '{}' not found in configuration.destinations", attr.value));
                                                        }
                                                    }
                                                    "encryption" => {
                                                        let mut encryption_found = false;
                                                        for encryption in &configuration.encryptions
                                                        {
                                                            if encryption.id == attr.value {
                                                                archive.encryption =
                                                                    Some(encryption.clone());
                                                                encryption_found = true;
                                                                break;
                                                            }
                                                        }

                                                        if !encryption_found {
                                                            return Err(format!("encryption '{}' not found in configuration.encryptions", attr.value));
                                                        }
                                                    }
                                                    "name" => {
                                                        archive.name = attr.value;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        "directories" => {}
                                        "directory" => {
                                            let mut dir = Directory::new();

                                            for attr in attributes {
                                                match attr.name.to_string().as_str() {
                                                    "name" => {
                                                        dir.name = attr.value;
                                                    }
                                                    "user" => {
                                                        dir.user = Some(attr.value);
                                                    }
                                                    "group" => {
                                                        dir.group = Some(attr.value);
                                                    }
                                                    _ => {}
                                                }
                                            }

                                            if dir.name.len() > 0 {
                                                archive.directories.push(dir);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                Ok(XmlReaderEvent::EndElement { name }) => {
                                    match name.to_string().as_str() {
                                        "archive" => {
                                            configuration.archives.push(archive.clone());
                                        }
                                        "databases" => {
                                            global_db_id = String::new();
                                        }
                                        _ => {}
                                    }
                                    depth -= 1;
                                }
                                Err(err) => {
                                    return Err(format!("XML-Error: {:?}", err));
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
            Err(why) => match why.kind() {
                ErrorKind::NotFound => {
                    return Err(String::from(format!(
                        "backup_configuration '{}' file does not exists.",
                        filename
                    )));
                }
                _ => {
                    return Err(String::from(format!(
                        "unable to open backup_configuration '{}' file",
                        filename
                    )));
                }
            },
        }

        Ok(configuration)
    }
}
