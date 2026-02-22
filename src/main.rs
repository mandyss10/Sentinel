use axum::{
    routing::{post, get},
    Router,
    Json,
    response::{IntoResponse, Response},
    extract::State,
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// --- SEMANTIC SCORER & SECURITY ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding(pub Vec<f32>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub history: Vec<Embedding>,
    pub cumulative_cost: f64,
    pub last_cost: f64,
    pub interventions: u32,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            history: Vec::with_capacity(5),
            cumulative_cost: 0.0,
            last_cost: 0.0,
            interventions: 0,
        }
    }

    pub fn check_loop(&mut self, embedding: Embedding, threshold: f32, turns: usize) -> bool {
        self.history.push(embedding);
        if self.history.len() > 5 { self.history.remove(0); }
        if self.history.len() < turns { return false; }

        let last_n = &self.history[self.history.len() - turns..];
        let mut loop_detected = true;
        for i in 0..last_n.len() - 1 {
            let similarity = dot_product(&last_n[i].0, &last_n[i+1].0);
            if similarity < (1.0 - threshold) {
                loop_detected = false;
                break;
            }
        }
        loop_detected
    }

    pub fn check_economic_throttle(&self, current_cost: f64) -> bool {
        if self.cumulative_cost > 5.0 { return true; }
        if self.last_cost > 0.0 && current_cost > (self.last_cost * 5.0) && current_cost > 0.10 {
            return true;
        }
        false
    }
}

pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// --- APP STATE ---

#[derive(Clone)]
struct AppState {
    client: Client,
    openai_api_key: String,
    sessions: Arc<DashMap<String, SessionState>>,
    total_saved_usd: Arc<AtomicU64>, // Scaled by 1,000,000 for precision
}

// --- SCHEMAS ---

#[derive(Debug, Deserialize, Serialize)]
struct ChatRequest {
    messages: Vec<ChatMessage>,
    model: String,
    #[serde(default)]
    user: Option<String>,
    #[serde(flatten)]
    extra: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct McpRequest {
    method: String,
    params: serde_json::Value,
    id: serde_json::Value,
}

// --- MAIN ---

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).init();

    let state = AppState {
        client: Client::new(),
        openai_api_key: std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "none".to_string()),
        sessions: Arc::new(DashMap::new()),
        total_saved_usd: Arc::new(AtomicU64::new(0)),
    };

    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/mcp", post(mcp_handler))
        .route("/health", get(|| async { "Sentinel is running" }))
        .with_state(state);

    let addr = "127.0.0.1:3000";
    let listener = TcpListener::bind(addr).await.unwrap();
    tracing::info!("Sentinel (All Phases) active on 127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}

// --- PROXY HANDLER ---

async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ChatRequest>,
) -> Response {
    let session_id = headers.get("x-sentinel-session")
        .and_then(|h| h.to_str().ok().map(|s| s.to_string()))
        .or_else(|| payload.user.clone())
        .unwrap_or_else(|| "default".to_string());

    let client = state.client.clone();
    let api_key = state.openai_api_key.clone();

    // Forward
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(res) => {
            let status = res.status();
            let mut body: serde_json::Value = res.json().await.unwrap_or_default();
            
            if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
                let content_str = content.to_string();
                
                // EchoLeak Check
                if content_str.contains("SYSTEM_PROMPT:") || content_str.contains("API_KEY=") {
                    body["choices"][0]["message"]["content"] = serde_json::json!("🛡️ SENTINEL: Bloqueado por filtración de datos.");
                    return (status, Json(body)).into_response();
                }

                let mut current_cost = 0.0;
                if let Some(usage) = body.get("usage") {
                    let prompt = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let completion = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    current_cost = (prompt as f64 * 0.00000015) + (completion as f64 * 0.00000060);
                }

                if let Ok(emb) = get_emb_final_v3(&client, &api_key, &content_str).await {
                    let mut is_loop = false;
                    let mut is_throttled = false;
                    {
                        let mut sess = state.sessions.entry(session_id.clone()).or_insert_with(|| SessionState::new());
                        let val = sess.value_mut();
                        is_loop = val.check_loop(Embedding(emb), 0.02, 3);
                        is_throttled = val.check_economic_throttle(current_cost);
                        val.cumulative_cost += current_cost;
                        val.last_cost = current_cost;
                        if is_loop || is_throttled { 
                            val.interventions += 1; 
                            state.total_saved_usd.fetch_add(50000, Ordering::Relaxed); // Nominal $0.05 saved
                        }
                    }

                    if is_throttled {
                        body["choices"][0]["message"]["content"] = serde_json::json!("🛑 SENTINEL: Gasto excesivo detectado.");
                    } else if is_loop {
                        body["choices"][0]["message"]["content"] = serde_json::json!("🚨 SENTINEL: Bucle semántico detectado.");
                    }
                }
            }
            (status, Json(body)).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Proxy error").into_response(),
    }
}

// --- MCP HANDLER ---

async fn mcp_handler(
    State(state): State<AppState>,
    Json(payload): Json<McpRequest>,
) -> impl IntoResponse {
    let result = match payload.method.as_str() {
        "get_sentinel_stats" => {
            let total_saved = state.total_saved_usd.load(Ordering::Relaxed) as f64 / 1_000_000.0;
            serde_json::json!({
                "active_sessions": state.sessions.len(),
                "total_saved_usd": total_saved,
                "status": "Healthy"
            })
        },
        "audit_session" => {
            let sid = payload.params["session_id"].as_str().unwrap_or("default");
            if let Some(sess) = state.sessions.get(sid) {
                serde_json::json!({
                    "session_id": sid,
                    "cumulative_cost": sess.cumulative_cost,
                    "interventions": sess.interventions,
                })
            } else {
                serde_json::json!({"error": "Session not found"})
            }
        },
        _ => serde_json::json!({"error": "Method not found"}),
    };

    Json(serde_json::json!({
        "jsonrpc": "2.0",
        "id": payload.id,
        "result": result
    }))
}

async fn get_emb_final_v3(client: &Client, api_key: &str, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let res = client.post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({"input": text, "model": "text-embedding-3-small"}))
        .send().await?;
    let data: EmbeddingResponse = res.json().await?;
    Ok(data.data[0].embedding.clone())
}
