use crate::manifest::GameManifest;
use reqwest::Client;
use std::time::Duration;

pub async fn post_manifest(
    client: &Client,
    base_url: &str,
    session_id: &str,
    manifest: &GameManifest,
) -> Result<(), String> {
    let url = format!("{}/sessions/{}", base_url.trim_end_matches('/'), session_id);
    let body = manifest
        .to_json()
        .map_err(|e| format!("failed to serialize manifest: {e}"))?;

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("failed to contact signaling server: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("signaling server returned HTTP {}", resp.status()));
    }

    Ok(())
}

pub async fn get_manifest(
    client: &Client,
    base_url: &str,
    session_id: &str,
) -> Result<String, String> {
    let url = format!("{}/sessions/{}", base_url.trim_end_matches('/'), session_id);

    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("error contacting signaling server: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "failed to fetch manifest from signaling server (HTTP {})",
            resp.status()
        ));
    }

    resp
        .text()
        .await
        .map_err(|e| format!("failed to read manifest body: {e}"))
}
