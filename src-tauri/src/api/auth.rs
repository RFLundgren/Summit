use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub access_token: String,
    pub name: String,
    pub user_email: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiKeyCreatedResponse {
    pub secret: String,
}

pub async fn login(base_url: &str, email: &str, password: &str) -> Result<LoginResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    let url = format!("{}/api/auth/login", base_url);
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await?;

    match resp.status().as_u16() {
        401 => return Err(anyhow!("Invalid email or password")),
        s if s >= 400 => return Err(anyhow!("Login failed ({})", s)),
        _ => {}
    }

    Ok(resp.json::<LoginResponse>().await?)
}

pub async fn create_api_key(base_url: &str, access_token: &str, key_name: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    let url = format!("{}/api/api-keys", base_url);
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&serde_json::json!({ "name": key_name, "permissions": ["all"] }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Failed to create API key: {}", body));
    }

    let result: ApiKeyCreatedResponse = resp.json().await?;
    Ok(result.secret)
}
