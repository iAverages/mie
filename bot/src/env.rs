use std::env;

use dotenvy::Error as DotEnvError;

#[derive(Clone, Debug)]
pub struct Config {
    pub discord_token: String,
    pub cdn_url: String,
    pub b2_key_id: String,
    pub b2_application_key: String,
    pub b2_bucket_path_prefix: String,
    pub b2_bucket_id: String,
}

pub fn load_env() -> Result<(), anyhow::Error> {
    if let Err(DotEnvError::Io(err)) = dotenvy::dotenv() {
        if cfg!(debug_assertions) {
            tracing::warn!("error while loading .env file, {}", err);
        }
    }
    Ok(())
}

pub fn create_config() -> Config {
    Config {
        discord_token: env::var("DISCORD_TOKEN").expect("No DISCORD_TOKEN provided"),
        cdn_url: env::var("CDN_URL").expect("No CDN_URL provided"),
        b2_key_id: env::var("B2_KEY_ID").expect("No B2_KEY_ID provided"),
        b2_application_key: env::var("B2_APPLICATION_KEY").expect("No B2_APPLICATION_KEY provided"),
        b2_bucket_path_prefix: env::var("B2_BUCKET_PATH_PREFIX")
            .expect("No B2_BUCKET_PATH_PREFIX provided"),

        b2_bucket_id: env::var("B2_BUCKET_ID").expect("No B2_BUCKET_ID provided"),
    }
}
