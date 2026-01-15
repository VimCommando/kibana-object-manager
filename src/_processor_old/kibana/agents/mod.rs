#[derive(Clone)]
pub struct Agents {
    client: reqwest::Client,
}

impl Agents {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}
