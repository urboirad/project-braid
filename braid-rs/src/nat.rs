use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PunchRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PunchResponse {
    pub peer: Option<String>,
}

/// Minimal UDP-based signaling service for hole punching experiments.
pub async fn run_nat_signaling_server(bind: &str) -> Result<(), String> {
    let socket = UdpSocket::bind(bind)
        .await
        .map_err(|e| format!("failed to bind UDP socket: {e}"))?;

    eprintln!("[braid-rs] NAT signaling server listening on {bind}");

    let sessions: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mut buf = [0u8; 1024];

    loop {
        let (len, addr) = socket
            .recv_from(&mut buf)
            .await
            .map_err(|e| format!("recv_from failed: {e}"))?;

        let data = &buf[..len];
        let req: PunchRequest = match serde_json::from_slice(data) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[braid-rs] invalid PunchRequest from {addr}: {e}");
                continue;
            }
        };

        let mut map = sessions
            .lock()
            .map_err(|_| "sessions map poisoned".to_string())?;

        let entry = map.entry(req.session_id.clone()).or_default();
        if !entry.contains(&addr) {
            entry.push(addr);
        }

        if entry.len() >= 2 {
            let peers = entry.clone();
            for &p in &peers {
                if let Some(other) = peers.iter().copied().find(|x| x != &p) {
                    let resp = PunchResponse {
                        peer: Some(other.to_string()),
                    };
                    if let Ok(body) = serde_json::to_vec(&resp) {
                        let _ = socket.send_to(&body, p).await;
                    }
                }
            }
        } else {
            let resp = PunchResponse { peer: None };
            if let Ok(body) = serde_json::to_vec(&resp) {
                let _ = socket.send_to(&body, addr).await;
            }
        }
    }
}

/// Client helper: ask a remote NAT signaling server for a peer address and
/// perform basic heartbeats to open NAT mappings.
pub async fn negotiate_peer(
    server_addr: &str,
    session_id: &str,
) -> Result<Option<SocketAddr>, String> {
    let socket = Arc::new(
        UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| format!("failed to bind local UDP socket: {e}"))?,
    );

    let server: SocketAddr = server_addr
        .parse()
        .map_err(|e| format!("invalid server address: {e}"))?;

    let req = PunchRequest {
        session_id: session_id.to_string(),
    };
    let body = serde_json::to_vec(&req).map_err(|e| format!("encode error: {e}"))?;

    socket
        .send_to(&body, server)
        .await
        .map_err(|e| format!("send_to failed: {e}"))?;

    let mut buf = [0u8; 1024];

    for _ in 0..10 {
        tokio::select! {
            res = socket.recv_from(&mut buf) => {
                let (len, _addr) = res.map_err(|e| format!("recv_from failed: {e}"))?;
                let data = &buf[..len];
                let resp: PunchResponse = serde_json::from_slice(data)
                    .map_err(|e| format!("decode error: {e}"))?;
                if let Some(peer_str) = resp.peer {
                    let peer: SocketAddr = peer_str
                        .parse()
                        .map_err(|e| format!("invalid peer address: {e}"))?;

                    // Basic heartbeats in the background to keep mappings open.
                    let hb_socket = Arc::clone(&socket);
                    tokio::spawn(async move {
                        let msg = b"ping";
                        loop {
                            let _ = hb_socket.send_to(msg, peer).await;
                            sleep(Duration::from_secs(2)).await;
                        }
                    });

                    return Ok(Some(peer));
                }
            }
            _ = sleep(Duration::from_secs(1)) => {
                // Retry request periodically until a peer appears.
                socket
                    .send_to(&body, server)
                    .await
                    .map_err(|e| format!("send_to failed: {e}"))?;
            }
        }
    }

    Ok(None)
}
