#[derive(Clone, Debug)]
pub struct ProgramParameter {
    pub longname: Option<String>,
    pub shortname: Option<String>,
    pub value: Option<String>,
    pub assign_sign: String,
}

impl ProgramParameter {
    pub fn new() -> ProgramParameter {
        ProgramParameter {
            longname: None,
            shortname: None,
            value: None,
            assign_sign: String::from("="),
        }
    }
}
