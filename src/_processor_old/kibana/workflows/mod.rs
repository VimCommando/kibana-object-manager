#[derive(Clone)]
pub struct Workflows {
    client: reqwest::Client,
}

impl Workflows {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}
