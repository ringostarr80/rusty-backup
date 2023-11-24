use crate::configuration::{Compression, Database, Destination, Directory, Encryption};

#[derive(Clone, Debug)]
pub struct Archive {
    pub compression: Compression,
    pub databases: Vec<Database>,
    pub destination: Destination,
    pub directories: Vec<Directory>,
    pub encryption: Option<Encryption>,
    pub name: String,
}

impl Archive {
    pub fn new() -> Archive {
        Archive {
            compression: Compression::None,
            databases: Vec::new(),
            destination: Destination::new(),
            directories: Vec::new(),
            encryption: None,
            name: String::new(),
        }
    }
}
