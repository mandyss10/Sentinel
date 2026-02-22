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
use std::collections::VecDeque;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

// --- SEMANTIC SCORER & SECURITY ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding(pub Vec<f32>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub history: Vec<Embedding>,
    pub history_text: Vec<String>,
    pub cumulative_cost: f64,
    pub last_cost: f64,
    pub interventions: u32,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            history: Vec::with_capacity(5),
            history_text: Vec::with_capacity(5),
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

    pub fn check_basic_loop(&mut self, text: String, threshold: f32, turns: usize) -> bool {
        self.history_text.push(text);
        if self.history_text.len() > 5 { self.history_text.remove(0); }
        if self.history_text.len() < turns { return false; }

        let last_n = &self.history_text[self.history_text.len() - turns..];
        let mut loop_detected = true;
        for i in 0..last_n.len() - 1 {
            let similarity = word_overlap_similarity(&last_n[i], &last_n[i+1]);
            if similarity < (1.0 - threshold) { 
                loop_detected = false;
                break;
            }
        }
        loop_detected
    }

    pub fn check_economic_throttle(&self, current_cost: f64) -> bool {
        if self.cumulative_cost > 10.0 { return true; }
        if self.last_cost > 0.0 && current_cost > (self.last_cost * 5.0) && current_cost > 0.10 {
            return true;
        }
        false
    }
}

pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn word_overlap_similarity(s1: &str, s2: &str) -> f32 {
    let w1: std::collections::HashSet<_> = s1.split_whitespace().map(|s| s.to_lowercase()).collect();
    let w2: std::collections::HashSet<_> = s2.split_whitespace().map(|s| s.to_lowercase()).collect();
    if w1.is_empty() || w2.is_empty() { return 0.0; }
    let intersection = w1.intersection(&w2).count();
    let union = w1.union(&w2).count();
    intersection as f32 / union as f32
}

// --- AUDIT LOGS ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterventionLog {
    timestamp: u64,
    session_id: String,
    reason: String,
    content_snippet: String,
    savings_est: f64,
}

// --- APP STATE ---

#[derive(Clone)]
struct AppState {
    client: Client,
    openai_api_key: String,
    groq_api_key: String,
    sessions: Arc<DashMap<String, SessionState>>,
    total_saved_usd: Arc<AtomicU64>,
    audit_logs: Arc<Mutex<VecDeque<InterventionLog>>>,
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
    #[serde(default)]
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
        groq_api_key: std::env::var("GROQ_API_KEY").unwrap_or_else(|_| "none".to_string()),
        sessions: Arc::new(DashMap::new()),
        total_saved_usd: Arc::new(AtomicU64::new(0)),
        audit_logs: Arc::new(Mutex::new(VecDeque::with_capacity(50))),
    };

    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/mcp", post(mcp_handler))
        .route("/api/stats", get(get_stats))
        .route("/api/logs", get(get_logs))
        .route("/health", get(|| async { "Sentinel is running" }))
        .fallback_service(tower_http::services::ServeDir::new(".").fallback(tower_http::services::ServeFile::new("index.html")))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "127.0.0.1:3000";
    let listener = TcpListener::bind(addr).await.unwrap();
    tracing::info!("🛡️ Sentinel SaaS active on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

// --- HANDLERS ---

async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    let total = state.total_saved_usd.load(Ordering::Relaxed) as f64 / 100.0;
    Json(serde_json::json!({
        "active_sessions": state.sessions.len(),
        "total_saved_usd": total,
        "interventions": state.sessions.iter().map(|s| s.interventions).sum::<u32>(),
        "status": "Healthy"
    }))
}

async fn get_logs(State(state): State<AppState>) -> impl IntoResponse {
    let logs = state.audit_logs.lock().await;
    Json(logs.clone())
}

async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ChatRequest>,
) -> impl IntoResponse {
    let session_id = headers.get("x-sentinel-session")
        .and_then(|h| h.to_str().ok().map(|s| s.to_string()))
        .or_else(|| payload.user.clone())
        .unwrap_or_else(|| "default".to_string());

    let provider = headers.get("x-sentinel-provider")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_else(|| {
            if payload.model.contains("llama") || payload.model.contains("mixtral") || payload.model.contains("gemma") {
                "groq"
            } else {
                "openai"
            }
        });

    let (url, api_key) = match provider {
        "groq" => ("https://api.groq.com/openai/v1/chat/completions", state.groq_api_key.clone()),
        _ => ("https://api.openai.com/v1/chat/completions", state.openai_api_key.clone()),
    };

    let prompt_to_check = payload.messages.last()
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // 1. Loop Detection
    let mut is_loop = false;
    let mut reason = String::new();
    let emb_result = get_emb_final_v4(&state.client, &state.openai_api_key, &prompt_to_check).await;
    
    {
        let mut sess = state.sessions.entry(session_id.clone()).or_insert_with(|| SessionState::new());
        let val = sess.value_mut();
        
        if let Ok(emb) = emb_result {
            if val.check_loop(Embedding(emb), 0.20, 3) {
                is_loop = true;
                reason = "Semantic Loop Detected (Vector Similarity)".to_string();
            }
        }
        
        if !is_loop {
            if val.check_basic_loop(prompt_to_check.clone(), 0.80, 3) {
                is_loop = true;
                reason = "Fuzzy Overlap Detected (String Repetition)".to_string();
            }
        }
    }

    if is_loop {
        state.total_saved_usd.fetch_add(50, Ordering::Relaxed);
        
        // Log intervention
        let mut logs = state.audit_logs.lock().await;
        logs.push_back(InterventionLog {
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            session_id: session_id.clone(),
            reason: reason.clone(),
            content_snippet: prompt_to_check.chars().take(50).collect::<String>() + "...",
            savings_est: 0.50,
        });
        if logs.len() > 50 { logs.pop_front(); }

        let error_body = serde_json::json!({
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": format!("🚨 SENTINEL: Bloqueado. Motivo: {}", reason)
                },
                "finish_reason": "stop"
            }]
        });
        return (StatusCode::OK, Json(error_body)).into_response();
    }

    // 2. Forward
    let response = state.client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(res) => {
            let status = res.status();
            let mut body: serde_json::Value = res.json().await.unwrap_or_default();
            
            if !status.is_success() {
                return (status, Json(body)).into_response();
            }

            if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
                let content_str = content.to_string();
                
                if content_str.contains("SYSTEM_PROMPT:") || content_str.contains("API_KEY=") {
                    body["choices"][0]["message"]["content"] = serde_json::json!("🛡️ SENTINEL: Bloqueado por filtración de datos.");
                    
                    let mut logs = state.audit_logs.lock().await;
                    logs.push_back(InterventionLog {
                        timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                        session_id: session_id.clone(),
                        reason: "Sensitive Data Leak (EchoLeak)".to_string(),
                        content_snippet: "[REDACTED SENSITIVE DATA]".to_string(),
                        savings_est: 0.10,
                    });
                    
                    return (status, Json(body)).into_response();
                }

                if let Some(mut sess) = state.sessions.get_mut(&session_id) {
                    let mut cost = 0.0;
                    if let Some(usage) = body.get("usage") {
                        let p = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let c = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        cost = (p as f64 * 0.00000015) + (c as f64 * 0.00000060);
                    }
                    
                    if sess.check_economic_throttle(cost) {
                        body["choices"][0]["message"]["content"] = serde_json::json!("🛑 SENTINEL: Gasto excesivo detectado.");
                        
                        let mut logs = state.audit_logs.lock().await;
                        logs.push_back(InterventionLog {
                            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                            session_id: session_id.clone(),
                            reason: "Economic Throttling (Cost Spike)".to_string(),
                            content_snippet: format!("Cost: ${:.4}", cost),
                            savings_est: 1.00,
                        });
                    }
                    sess.cumulative_cost += cost;
                    sess.last_cost = cost;
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
            let total = state.total_saved_usd.load(Ordering::Relaxed) as f64 / 100.0;
            serde_json::json!({
                "active_sessions": state.sessions.len(),
                "total_saved_usd": total,
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

async fn get_emb_final_v4(client: &Client, api_key: &str, text: &str) -> Result<Vec<f32>, String> {
    if api_key == "none" || api_key.contains("xxxx") {
        return Err("No Key".to_string());
    }
    let res = client.post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({"input": text, "model": "text-embedding-3-small"}))
        .send().await.map_err(|e| e.to_string())?;
    
    let data: EmbeddingResponse = res.json().await.map_err(|e| e.to_string())?;
    if let Some(first) = data.data.get(0) {
        Ok(first.embedding.clone())
    } else {
        Err("No embedding".to_string())
    }
}
