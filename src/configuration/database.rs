use std::{
    fs::File,
    io::{Read, Write},
    process::{Command, Stdio},
};

use log::info;

use crate::configuration::{Configuration, Credential};

#[derive(Clone, Debug)]
pub struct Database {
    pub credential: Credential,
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub name_is_regex: bool,
}

impl Database {
    pub fn new() -> Database {
        Database {
            credential: Credential::new(),
            id: String::new(),
            kind: Kind::MySql,
            name: String::new(),
            name_is_regex: false,
        }
    }

    pub fn build_dump_filename(&self) -> String {
        let mut db_filename = format!("{}.sql", self.name);

        match self.kind {
            Kind::MongoDB => {
                db_filename = format!("{}.bson", self.name);
            }
            _ => {}
        }

        db_filename
    }

    pub fn build_dump_command(&self) -> Command {
        match self.kind {
            Kind::MongoDB => {
                let mut cmd = Command::new("mongodump");
                cmd.arg("--archive");

                if self.name != "*" {
                    cmd.arg(format!("--db='{}'", self.name));
                    info!("dumping mongodb-database: {}", self.name);
                } else {
                    info!("dumping all mongodb-databases");
                }

                cmd
            }
            Kind::MySql => {
                let mut cmd = Command::new("mysqldump");
                if self.credential.username.len() > 0 {
                    cmd.arg(format!("-u"));
                    cmd.arg(&self.credential.username);
                    if self.credential.password.len() > 0 {
                        cmd.arg(format!("-p{}", self.credential.password));
                    }
                }
                cmd.arg("--databases");

                if self.name_is_regex {
                } else {
                    cmd.arg(&self.name);
                }

                info!("dumping mysql-database: {}", self.name);
                cmd
            }
            Kind::PostgreSql => {
                let mut cmd = Command::new("pg_dump");
                if self.credential.username.len() > 0 {
                    cmd.arg(format!("--username={}", self.credential.username));
                    if self.credential.password.len() > 0 {
                        cmd.env("PGPASSWORD", &self.credential.password);
                    }
                }
                cmd.arg("--host=localhost");
                cmd.arg(format!("--dbname={}", self.name));

                info!("dumping postgresql-database: {}", self.name);
                cmd
            }
        }
    }

    pub fn build_create_db_command(&self) -> Command {
        match self.kind {
            Kind::MongoDB => Command::new("echo"),
            Kind::MySql => {
                let mut cmd = Command::new("mysql");
                cmd.arg("-u")
                    .arg(self.credential.username.clone())
                    .arg(format!("-p{}", self.credential.password))
                    .arg("-e")
                    .arg(format!("CREATE DATABASE IF NOT EXISTS `{}`", self.name));

                cmd
            }
            Kind::PostgreSql => Command::new("echo"),
        }
    }

    pub fn build_delete_command(&self) -> Command {
        match self.kind {
            Kind::MongoDB => Command::new("echo"),
            Kind::MySql => {
                let mut cmd = Command::new("mysql");
                cmd.arg("-u")
                    .arg(self.credential.username.clone())
                    .arg(format!("-p{}", self.credential.password))
                    .arg("-e")
                    .arg(format!("DROP DATABASE IF EXISTS `{}`", self.name));

                cmd
            }
            Kind::PostgreSql => Command::new("echo"),
        }
    }

    pub fn build_import_command(&self) -> Command {
        match self.kind {
            Kind::MongoDB => {
                let mut cmd = Command::new("mongorestore");
                cmd.arg("--archive");
                cmd.arg("--drop");
                cmd.arg("--preserveUUID");

                cmd
            }
            Kind::MySql => {
                let mut cmd = Command::new("mysql");
                cmd.arg("-u")
                    .arg(self.credential.username.clone())
                    .arg(format!("-p{}", self.credential.password))
                    .arg(self.name.clone());

                cmd
            }
            Kind::PostgreSql => {
                let cmd = Command::new("echo");

                cmd
            }
        }
    }

    pub fn create_database(&self) -> Result<(), String> {
        let mut create_db_command = self.build_create_db_command();
        let child = match create_db_command.spawn() {
            Ok(child) => child,
            Err(err) => return Err(format!("{}", err)),
        };
        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(err) => return Err(format!("{}", err)),
        };
        if !output.status.success() {
            return Err(format!(
                "error while executing create-command: {:?}",
                create_db_command
            ));
        }

        Ok(())
    }

    pub fn delete_database(&self) -> Result<(), String> {
        let mut db_delete_command = self.build_delete_command();
        let child = match db_delete_command.spawn() {
            Ok(child) => child,
            Err(err) => return Err(format!("{}", err)),
        };
        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(err) => return Err(format!("{}", err)),
        };
        if !output.status.success() {
            return Err(format!(
                "error while executing delete-command: {:?}",
                db_delete_command
            ));
        }

        Ok(())
    }

    pub fn import_database(&self, mut file: File) -> Result<(), String> {
        let mut db_import_command = self.build_import_command();
        db_import_command.stdin(Stdio::piped());
        let child = match db_import_command.spawn() {
            Ok(child) => child,
            Err(err) => return Err(format!("{}", err)),
        };
        if let Some(mut stdin) = child.stdin.as_ref() {
            let mut buf = [0; Configuration::BUFFER_SIZE];
            loop {
                let read_bytes = match file.read(&mut buf) {
                    Ok(read_bytes) => read_bytes,
                    Err(err) => return Err(format!("{:?}", err)),
                };
                if read_bytes == 0 {
                    break;
                }
                match stdin.write(&buf[0..read_bytes]) {
                    Ok(_) => {}
                    Err(err) => return Err(format!("{:?}", err)),
                };
            }
        }
        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(err) => return Err(format!("{}", err)),
        };
        if !output.status.success() {
            return Err(format!(
                "error while executing delete-command: {:?}",
                db_import_command
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    MongoDB,
    MySql,
    PostgreSql,
}

impl Kind {
    pub fn to_extension_string(&self) -> String {
        match self {
            Kind::MongoDB => String::from(".bson"),
            Kind::MySql => String::from(".sql"),
            Kind::PostgreSql => String::from(".sql"),
        }
    }
}
