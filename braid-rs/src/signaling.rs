use crate::manifest::GameManifest;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePayload {
    pub state: String,
}

pub async fn post_state(
    client: &Client,
    base_url: &str,
    session_id: &str,
    state_bytes: &[u8],
) -> Result<(), String> {
    let url = format!("{}/state/{}", base_url.trim_end_matches('/'), session_id);
    let state_b64 = general_purpose::STANDARD.encode(state_bytes);
    let payload = StatePayload { state: state_b64 };

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("failed to contact signaling server: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "failed to push state blob to signaling server (HTTP {})",
            resp.status()
        ));
    }

    Ok(())
}

pub async fn get_state(
    client: &Client,
    base_url: &str,
    session_id: &str,
) -> Result<Vec<u8>, String> {
    let url = format!("{}/state/{}", base_url.trim_end_matches('/'), session_id);

    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("error contacting signaling server: {e}"))?;

    if resp.status().as_u16() == 404 {
        return Err("no save-state found for this session (404)".to_string());
    }

    if !resp.status().is_success() {
        return Err(format!(
            "failed to fetch state blob from signaling server (HTTP {})",
            resp.status()
        ));
    }

    let payload: StatePayload = resp
        .json()
        .await
        .map_err(|e| format!("failed to decode state payload: {e}"))?;

    general_purpose::STANDARD
        .decode(payload.state.as_bytes())
        .map_err(|e| format!("failed to decode base64 state blob: {e}"))
}
