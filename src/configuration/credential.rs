#[derive(Clone, Debug)]
pub struct Credential {
    pub username: String,
    pub password: String,
}

impl Credential {
    pub fn new() -> Credential {
        Credential {
            username: String::new(),
            password: String::new(),
        }
    }
}
