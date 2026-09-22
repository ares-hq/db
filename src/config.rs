use anyhow::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub supabase_url: String,
    pub supabase_key: String,
    pub first_username: String,
    pub first_password: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            supabase_url: env::var("SUPABASE_URL").context("SUPABASE_URL not set")?,
            supabase_key: env::var("SUPABASE_KEY").context("SUPABASE_KEY not set")?,
            first_username: env::var("FIRST_USERNAME").context("FIRST_USERNAME not set")?,
            first_password: env::var("FIRST_PASS").context("FIRST_PASS not set")?,
        })
    }
}
