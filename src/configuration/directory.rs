use nix::unistd::{Gid, Uid, User};

#[derive(Clone, Debug)]
pub struct Directory {
    pub name: String,
    pub user: Option<String>,
    pub group: Option<String>,
}

impl Directory {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            user: None,
            group: None,
        }
    }

    pub fn get_gid(&self) -> Option<Gid> {
        match &self.group {
            Some(group) => match User::from_name(group.as_str()) {
                Ok(group_opt) => match group_opt {
                    Some(group) => Some(group.gid),
                    None => None,
                },
                Err(_) => None,
            },
            None => None,
        }
    }

    pub fn get_uid(&self) -> Option<Uid> {
        match &self.user {
            Some(user) => match User::from_name(user.as_str()) {
                Ok(user_opt) => match user_opt {
                    Some(user) => Some(user.uid),
                    None => None,
                },
                Err(_) => None,
            },
            None => None,
        }
    }
}
