use std::process::Command;

use log::info;
use regex::Regex;

#[derive(Clone, Debug)]
pub struct Encryption {
    pub id: String,
    pub cipher: String,
    pub password: String,
}

impl Encryption {
    pub fn new() -> Self {
        Self {
            id: String::new(),
            cipher: String::new(),
            password: String::new(),
        }
    }

    pub fn decrypt_file<S: AsRef<str>>(&self, input_filename: S) -> Result<(), String> {
        let input_filename = input_filename.as_ref();

        lazy_static! {
            static ref REGEX_ENC_EXT: Regex = Regex::new(r"\.enc$").unwrap();
        }

        let output_filename = REGEX_ENC_EXT.replace(input_filename, "");

        let mut cmd = Command::new("openssl");
        cmd.arg(&self.cipher)
            .arg("-d")
            .arg("-pbkdf2")
            .arg("-in")
            .arg(input_filename)
            .arg("-out")
            .arg(output_filename.as_ref())
            .arg("-k")
            .arg(&self.password);

        info!("decryption command: {:?}", cmd);
        let child = match cmd.spawn() {
            Ok(child) => child,
            Err(_) => return Err(format!("error while spawning decryption-program.")),
        };
        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(_) => return Err(format!("error while waiting for decryption-program.")),
        };
        match output.status.code() {
            Some(0) => {
                info!("decryption successfully finished.");
                Ok(())
            }
            Some(code) => Err(format!("error program exit-code: {}.", code)),
            None => Err(format!("no output status.")),
        }
    }

    pub fn encrypt_file<S: AsRef<str>>(&self, input_filename: S) -> Result<(), String> {
        let input_filename = input_filename.as_ref();
        let output_filename = format!("{}.enc", input_filename);

        let mut cmd = Command::new("openssl");
        cmd.arg(&self.cipher)
            .arg("-pbkdf2")
            .arg("-in")
            .arg(input_filename)
            .arg("-out")
            .arg(output_filename)
            .arg("-k")
            .arg(&self.password);

        info!("encryption command: {:?}", cmd);
        let child = match cmd.spawn() {
            Ok(child) => child,
            Err(_) => return Err(format!("error while spawning openssl-program.")),
        };
        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(_) => return Err(format!("error while waiting for openssl-program.")),
        };
        match output.status.code() {
            Some(0) => {
                info!("encryption successfully finished.");
                Ok(())
            }
            Some(code) => Err(format!("error program exit-code: {}.", code)),
            None => Err(format!("no output status.")),
        }
    }

    pub fn to_extension_string(&self) -> String {
        String::from(".enc")
    }
}
