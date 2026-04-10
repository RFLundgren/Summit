use crate::api::{types::ConnectionTestResult, ImmichClient};

/// Test a connection using a URL and API key directly.
/// Used in the pre-login flow to verify server reachability.
#[tauri::command]
pub async fn test_connection(url: String, key: String) -> Result<ConnectionTestResult, String> {
    let client = ImmichClient::new(&url, &key).map_err(|e| e.to_string())?;
    Ok(client.test_connection().await)
}
