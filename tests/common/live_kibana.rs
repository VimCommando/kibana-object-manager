use eyre::{Context, Result, bail};
use kibana_object_manager::{
    client::{Auth, KibanaClient},
    etl::Loader,
    kibana::spaces::{SpaceEntry, SpacesLoader, SpacesManifest},
};
use serde_json::json;
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::TempDir;
use url::Url;

pub struct LiveKibana {
    pub client: KibanaClient,
    pub run_id: String,
    _temp_dir: TempDir,
}

impl LiveKibana {
    pub async fn new(extra_spaces: &[String]) -> Result<Self> {
        require_live_tests_enabled()?;

        let run_id = unique_run_id();
        let temp_dir = TempDir::new().context("failed to create live Kibana temp project")?;
        let mut spaces = vec![SpaceEntry::new(
            "default".to_string(),
            "Default".to_string(),
        )];
        for space in extra_spaces {
            spaces.push(SpaceEntry::new(space.clone(), space.clone()));
        }
        SpacesManifest::with_spaces(spaces).write(temp_dir.path().join("spaces.yml"))?;

        let url = Url::parse(&env_var(
            "KIBANA_TEST_URL",
            "KIBANA_URL",
            "http://localhost:15601",
        ))?;
        let auth = auth_from_env();
        let max_requests = env::var("KIBANA_TEST_MAX_REQUESTS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(8);

        let client = KibanaClient::try_new(url, auth, temp_dir.path(), max_requests)?;
        let response = client.test_connection().await?;
        if !response.status().is_success() {
            bail!("live Kibana connection failed: {}", response.status());
        }

        Ok(Self {
            client,
            run_id,
            _temp_dir: temp_dir,
        })
    }

    pub async fn ensure_space(&self, space_id: &str) -> Result<()> {
        let space = json!({
            "id": space_id,
            "name": space_id,
            "description": format!("kibana-object-manager live test space {}", self.run_id),
            "disabledFeatures": []
        });
        SpacesLoader::new(self.client.clone())
            .with_overwrite(true)
            .load(vec![space])
            .await?;
        Ok(())
    }

    pub async fn delete_space(&self, space_id: &str) -> Result<()> {
        if space_id == "default" {
            return Ok(());
        }

        let path = format!("/api/spaces/space/{}", space_id);
        let response = self
            .client
            .request(reqwest::Method::DELETE, &Default::default(), &path, None)
            .await?;
        if response.status().is_success() || response.status().as_u16() == 404 {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("failed to delete live test space {space_id} ({status}): {body}");
        }
    }
}

pub fn test_space_id(suffix: &str) -> String {
    let prefix = env::var("KIBANA_TEST_SPACE_PREFIX").unwrap_or_else(|_| "kibob-live".to_string());
    format!("{}-{}-{}", prefix, unique_run_id(), suffix)
}

fn require_live_tests_enabled() -> Result<()> {
    if env::var("KIBOB_LIVE_KIBANA_TESTS").as_deref() == Ok("1") {
        return Ok(());
    }

    bail!(
        "live Kibana tests are disabled; run scripts/live-kibana-tests.sh test or set KIBOB_LIVE_KIBANA_TESTS=1"
    );
}

fn auth_from_env() -> Auth {
    if let Ok(apikey) = env::var("KIBANA_TEST_APIKEY").or_else(|_| env::var("KIBANA_APIKEY")) {
        return Auth::Apikey(apikey);
    }

    let username = env::var("KIBANA_TEST_USERNAME")
        .or_else(|_| env::var("KIBANA_USERNAME"))
        .unwrap_or_else(|_| "elastic".to_string());
    let password = env::var("KIBANA_TEST_PASSWORD")
        .or_else(|_| env::var("KIBANA_PASSWORD"))
        .unwrap_or_else(|_| "changeme".to_string());

    if username.is_empty() || password.is_empty() {
        Auth::None
    } else {
        Auth::Basic(username, password)
    }
}

fn env_var(primary: &str, fallback: &str, default: &str) -> String {
    env::var(primary)
        .or_else(|_| env::var(fallback))
        .unwrap_or_else(|_| default.to_string())
}

fn unique_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{}-{}", std::process::id(), millis)
}
