use crate::agent::Agent;
use crate::database::{Conversation, DatabaseManager};
use crate::mcp::{McpManager, McpServerConfig};
use crate::subagent::{SubagentConfig, SubagentManager};
use anyhow::Result;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::Utc;

#[derive(Clone)]
pub struct WebState {
    pub agent: Arc<Mutex<Agent>>,
    pub database: Arc<DatabaseManager>,
    pub mcp_manager: Arc<McpManager>,
    pub subagent_manager: Arc<Mutex<SubagentManager>>,
}

#[derive(Serialize)]
struct ConversationListItem {
    id: String,
    created_at: String,
    updated_at: String,
    model: String,
    subagent: Option<String>,
    total_tokens: i32,
    request_count: i32,
    last_message: Option<String>,
    message_count: usize,
}

#[derive(Serialize)]
struct MessageDto {
    id: String,
    role: String,
    content: String,
    created_at: String,
}

#[derive(Serialize)]
struct ConversationDetail {
    conversation: ConversationMeta,
    messages: Vec<MessageDto>,
}

#[derive(Serialize)]
struct ConversationMeta {
    id: String,
    created_at: String,
    updated_at: String,
    system_prompt: Option<String>,
    model: String,
    subagent: Option<String>,
    total_tokens: i32,
    request_count: i32,
}

#[derive(Deserialize)]
struct NewConversationRequest {
    system_prompt: Option<String>,
}

#[derive(Deserialize)]
struct MessageRequest {
    message: String,
}

#[derive(Serialize)]
struct PlanDto {
    id: String,
    conversation_id: Option<String>,
    title: Option<String>,
    user_request: String,
    plan_markdown: String,
    created_at: String,
}

#[derive(Deserialize)]
struct PlanUpdateRequest {
    title: Option<String>,
    user_request: Option<String>,
    plan_markdown: Option<String>,
}

#[derive(Deserialize)]
struct PlanCreateRequest {
    title: Option<String>,
    user_request: String,
    plan_markdown: String,
    conversation_id: Option<String>,
}

#[derive(Deserialize)]
struct UpsertServerRequest {
    name: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    url: Option<String>,
    env: Option<HashMap<String, String>>,
    enabled: Option<bool>,
}

#[derive(Serialize)]
struct ServerDto {
    name: String,
    config: McpServerConfig,
    connected: bool,
}

#[derive(Serialize)]
struct AgentDto {
    name: String,
    allowed_tools: Vec<String>,
    denied_tools: Vec<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    model: Option<String>,
    created_at: String,
    updated_at: String,
    system_prompt: String,
    active: bool,
}

#[derive(Deserialize)]
struct AgentUpdateRequest {
    system_prompt: String,
    allowed_tools: Vec<String>,
    denied_tools: Vec<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct NewAgentRequest {
    name: String,
    system_prompt: String,
    allowed_tools: Vec<String>,
    denied_tools: Vec<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct ActivateAgentRequest {
    name: Option<String>,
}

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");

pub async fn launch_web_ui(state: WebState, port: u16) -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!(
        "?? Web UI starting on http://{} (Ctrl+C to stop)",
        addr
    );

    let router = Router::new()
        .route("/", get(serve_index))
        .route("/app.js", get(serve_app_js))
        .route("/api/health", get(health))
        .route(
            "/api/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route("/api/conversations/:id", get(get_conversation))
        .route(
            "/api/conversations/:id/message",
            post(send_message_to_conversation),
        )
        .route("/api/plans", get(list_plans).post(create_plan))
        .route(
            "/api/plans/:id",
            get(get_plan).put(update_plan).delete(delete_plan),
        )
        .route(
            "/api/mcp/servers",
            get(list_mcp_servers).post(upsert_mcp_server),
        )
        .route(
            "/api/mcp/servers/:name",
            get(get_mcp_server)
                .put(upsert_mcp_server_named)
                .delete(delete_mcp_server),
        )
        .route(
            "/api/mcp/servers/:name/connect",
            post(connect_mcp_server),
        )
        .route(
            "/api/mcp/servers/:name/disconnect",
            post(disconnect_mcp_server),
        )
        .route("/api/agents", get(list_agents).post(create_agent))
        .route(
            "/api/agents/:name",
            get(get_agent).put(update_agent).delete(delete_agent),
        )
        .route("/api/agents/active", get(get_active_agent).post(set_active_agent))
        .with_state(state);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, router).await?;
    Ok(())
}

async fn serve_index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn serve_app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        APP_JS,
    )
}

async fn health() -> impl IntoResponse {
    Json(HashMap::from([("status", "ok")]))
}

async fn list_conversations(State(state): State<WebState>) -> impl IntoResponse {
    let db = state.database.clone();
    let result = db.get_recent_conversations(100, None).await;

    match result {
        Ok(conversations) => {
            let mut items = Vec::new();
            for conversation in conversations {
                let messages = db
                    .get_conversation_messages(&conversation.id)
                    .await
                    .unwrap_or_default();
                let last_message = messages.last().map(|m| m.content.clone());
                let item = ConversationListItem {
                    id: conversation.id.clone(),
                    created_at: conversation.created_at.to_rfc3339(),
                    updated_at: conversation.updated_at.to_rfc3339(),
                    model: conversation.model.clone(),
                    subagent: conversation.subagent.clone(),
                    total_tokens: conversation.total_tokens,
                    request_count: conversation.request_count,
                    last_message,
                    message_count: messages.len(),
                };
                items.push(item);
            }
            Json(items).into_response()
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list conversations: {}", e),
            )
                .into_response()
        }
    }
}

fn conversation_to_meta(conversation: &Conversation) -> ConversationMeta {
    ConversationMeta {
        id: conversation.id.clone(),
        created_at: conversation.created_at.to_rfc3339(),
        updated_at: conversation.updated_at.to_rfc3339(),
        system_prompt: conversation.system_prompt.clone(),
        model: conversation.model.clone(),
        subagent: conversation.subagent.clone(),
        total_tokens: conversation.total_tokens,
        request_count: conversation.request_count,
    }
}

async fn get_conversation(
    State(state): State<WebState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let db = state.database.clone();
    let conversation = db.get_conversation(&id).await;

    match conversation {
        Ok(Some(conversation)) => {
            let messages = match db.get_conversation_messages(&id).await {
                Ok(messages) => messages
                    .into_iter()
                    .map(|m| MessageDto {
                        id: m.id,
                        role: m.role,
                        content: m.content,
                        created_at: m.created_at.to_rfc3339(),
                    })
                    .collect(),
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to load messages: {}", e),
                    )
                        .into_response()
                }
            };

            Json(ConversationDetail {
                conversation: conversation_to_meta(&conversation),
                messages,
            })
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Conversation not found".to_string()).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to load conversation: {}", e),
        )
            .into_response(),
    }
}

async fn create_conversation(
    State(state): State<WebState>,
    Json(payload): Json<NewConversationRequest>,
) -> impl IntoResponse {
    let mut agent = state.agent.lock_owned().await;

    if let Some(prompt) = payload.system_prompt {
        agent.set_system_prompt(prompt);
    }

    let result = agent.clear_conversation_keep_agents_md().await;

    match result {
        Ok(_) => match agent.current_conversation_id() {
            Some(id) => Json(HashMap::from([("id", id)])).into_response(),
            None => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Conversation ID missing".to_string(),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create conversation: {}", e),
        )
            .into_response(),
    }
}

#[axum::debug_handler]
async fn send_message_to_conversation(
    State(state): State<WebState>,
    Path(id): Path<String>,
    Json(payload): Json<MessageRequest>,
) -> impl IntoResponse {
    let mut agent = state.agent.lock_owned().await;

    if agent.current_conversation_id() != Some(id.clone()) {
        if let Err(e) = agent.resume_conversation(&id).await {
            return (
                StatusCode::BAD_REQUEST,
                format!("Failed to load conversation: {}", e),
            )
                .into_response();
        }
    }

    let cancellation_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    match agent
        .process_message(&payload.message, cancellation_flag)
        .await
    {
        Ok(response) => Json(HashMap::from([("response", response)])).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to process message: {}", e),
        )
            .into_response(),
    }
}

async fn list_plans(State(state): State<WebState>) -> impl IntoResponse {
    match state.database.list_plans(None).await {
        Ok(plans) => {
            let list: Vec<PlanDto> = plans
                .into_iter()
                .map(|p| PlanDto {
                    id: p.id,
                    conversation_id: p.conversation_id,
                    title: p.title,
                    user_request: p.user_request,
                    plan_markdown: p.plan_markdown,
                    created_at: p.created_at.to_rfc3339(),
                })
                .collect();
            Json(list).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list plans: {}", e),
        )
            .into_response(),
    }
}

async fn get_plan(State(state): State<WebState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.database.get_plan(&id).await {
        Ok(Some(plan)) => Json(PlanDto {
            id: plan.id,
            conversation_id: plan.conversation_id,
            title: plan.title,
            user_request: plan.user_request,
            plan_markdown: plan.plan_markdown,
            created_at: plan.created_at.to_rfc3339(),
        })
        .into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Plan not found".to_string()).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch plan: {}", e),
        )
            .into_response(),
    }
}

async fn update_plan(
    State(state): State<WebState>,
    Path(id): Path<String>,
    Json(payload): Json<PlanUpdateRequest>,
) -> impl IntoResponse {
    match state
        .database
        .update_plan(&id, payload.title, payload.user_request, payload.plan_markdown)
        .await
    {
        Ok(plan) => Json(PlanDto {
            id: plan.id,
            conversation_id: plan.conversation_id,
            title: plan.title,
            user_request: plan.user_request,
            plan_markdown: plan.plan_markdown,
            created_at: plan.created_at.to_rfc3339(),
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update plan: {}", e),
            )
                .into_response(),
    }
}

async fn delete_plan(State(state): State<WebState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.database.delete_plan(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete plan: {}", e),
        )
            .into_response(),
    }
}

async fn create_plan(
    State(state): State<WebState>,
    Json(payload): Json<PlanCreateRequest>,
) -> impl IntoResponse {
    match state
        .database
        .create_plan(
            payload.conversation_id.as_deref(),
            payload.title.as_deref(),
            &payload.user_request,
            &payload.plan_markdown,
        )
        .await
    {
        Ok(id) => Json(HashMap::from([("id", id)])).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create plan: {}", e),
        )
            .into_response(),
    }
}

async fn list_mcp_servers(State(state): State<WebState>) -> impl IntoResponse {
    match state.mcp_manager.list_servers().await {
        Ok(servers) => {
            let list: Vec<ServerDto> = servers
                .into_iter()
                .map(|(name, config, connected)| ServerDto {
                    name,
                    config,
                    connected,
                })
                .collect();
            Json(list).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list MCP servers: {}", e),
        )
            .into_response(),
    }
}

async fn get_mcp_server(
    State(state): State<WebState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.mcp_manager.get_server(&name).await {
        Some(config) => Json(ServerDto {
            name: name.clone(),
            config,
            connected: state.mcp_manager.is_connected(&name).await,
        })
        .into_response(),
        None => (StatusCode::NOT_FOUND, "Server not found".to_string()).into_response(),
    }
}

async fn upsert_mcp_server(
    State(state): State<WebState>,
    Json(payload): Json<UpsertServerRequest>,
) -> impl IntoResponse {
    let name = match payload.name.clone() {
        Some(name) => name,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                "Name is required to create a server".to_string(),
            )
                .into_response()
        }
    };
    upsert_mcp_server_inner(state, name, payload).await
}

async fn upsert_mcp_server_named(
    State(state): State<WebState>,
    Path(name): Path<String>,
    Json(payload): Json<UpsertServerRequest>,
) -> impl IntoResponse {
    upsert_mcp_server_inner(state, name, payload).await
}

async fn upsert_mcp_server_inner(
    state: WebState,
    name: String,
    payload: UpsertServerRequest,
) -> Response {
    let enabled = payload.enabled.unwrap_or(true);

    if payload.command.is_none() && payload.url.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            "Either command or url is required".to_string(),
        )
            .into_response();
    }

    let config = McpServerConfig {
        name: name.clone(),
        command: payload.command,
        args: payload.args,
        url: payload.url,
        env: payload.env,
        enabled,
    };

    match state.mcp_manager.upsert_server(&name, config).await {
        Ok(_) => Json(HashMap::from([("name", name)])).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save MCP server: {}", e),
        )
            .into_response(),
    }
}

async fn delete_mcp_server(
    State(state): State<WebState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.mcp_manager.remove_server(&name).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete MCP server: {}", e),
        )
            .into_response(),
    }
}

async fn connect_mcp_server(
    State(state): State<WebState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.mcp_manager.connect_server(&name).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to connect MCP server: {}", e),
        )
            .into_response(),
    }
}

async fn disconnect_mcp_server(
    State(state): State<WebState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.mcp_manager.disconnect_server(&name).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to disconnect MCP server: {}", e),
        )
            .into_response(),
    }
}

async fn list_agents(State(state): State<WebState>) -> impl IntoResponse {
    let active = state.agent.lock().await.active_subagent_name();
    let mut manager = state.subagent_manager.lock().await;
    if let Err(e) = manager.load_all_subagents().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to load agents: {}", e),
        )
            .into_response();
    }

    let list: Vec<AgentDto> = manager
        .list_subagents()
        .into_iter()
        .map(|a| AgentDto {
            name: a.name.clone(),
            allowed_tools: a.allowed_tools.iter().cloned().collect(),
            denied_tools: a.denied_tools.iter().cloned().collect(),
            max_tokens: a.max_tokens,
            temperature: a.temperature,
            model: a.model.clone(),
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
            system_prompt: a.system_prompt.clone(),
            active: active.as_ref() == Some(&a.name),
        })
        .collect();

    Json(list).into_response()
}

async fn get_agent(State(state): State<WebState>, Path(name): Path<String>) -> impl IntoResponse {
    let active = state.agent.lock().await.active_subagent_name();
    let manager = state.subagent_manager.lock().await;
    match manager.get_subagent(&name) {
        Some(agent) => Json(AgentDto {
            name: agent.name.clone(),
            allowed_tools: agent.allowed_tools.iter().cloned().collect(),
            denied_tools: agent.denied_tools.iter().cloned().collect(),
            max_tokens: agent.max_tokens,
            temperature: agent.temperature,
            model: agent.model.clone(),
            created_at: agent.created_at.to_rfc3339(),
            updated_at: agent.updated_at.to_rfc3339(),
            system_prompt: agent.system_prompt.clone(),
            active: active.as_ref() == Some(&agent.name),
        })
        .into_response(),
        None => (StatusCode::NOT_FOUND, "Agent not found".to_string()).into_response(),
    }
}

async fn create_agent(
    State(state): State<WebState>,
    Json(payload): Json<NewAgentRequest>,
) -> impl IntoResponse {
    let mut manager = state.subagent_manager.lock().await;
    let now = Utc::now();

    let config = SubagentConfig {
        name: payload.name.clone(),
        system_prompt: payload.system_prompt,
        allowed_tools: payload.allowed_tools.into_iter().collect(),
        denied_tools: payload.denied_tools.into_iter().collect(),
        max_tokens: payload.max_tokens,
        temperature: payload.temperature,
        model: payload.model,
        created_at: now,
        updated_at: now,
    };

    let save_result = manager.save_subagent(&config).await;
    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save agent: {}", e),
        )
            .into_response();
    }

    // Refresh in-memory list
    if let Err(e) = manager.load_all_subagents().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to reload agents: {}", e),
        )
            .into_response();
    }

    Json(HashMap::from([("name", payload.name)])).into_response()
}

async fn update_agent(
    State(state): State<WebState>,
    Path(name): Path<String>,
    Json(payload): Json<AgentUpdateRequest>,
) -> impl IntoResponse {
    let mut manager = state.subagent_manager.lock().await;
    let existing = match manager.get_subagent(&name).cloned() {
        Some(agent) => agent,
        None => return (StatusCode::NOT_FOUND, "Agent not found".to_string()).into_response(),
    };

    let config = SubagentConfig {
        name: existing.name.clone(),
        system_prompt: payload.system_prompt,
        allowed_tools: payload.allowed_tools.into_iter().collect(),
        denied_tools: payload.denied_tools.into_iter().collect(),
        max_tokens: payload.max_tokens,
        temperature: payload.temperature,
        model: payload.model,
        created_at: existing.created_at,
        updated_at: existing.updated_at,
    };

    match manager.update_subagent(&config).await {
        Ok(_) => Json(HashMap::from([("name", name)])).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update agent: {}", e),
        )
            .into_response(),
    }
}

async fn delete_agent(
    State(state): State<WebState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let mut manager = state.subagent_manager.lock().await;
    match manager.delete_subagent(&name).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete agent: {}", e),
        )
            .into_response(),
    }
}

async fn get_active_agent(State(state): State<WebState>) -> impl IntoResponse {
    let name = state.agent.lock().await.active_subagent_name();
    Json(HashMap::from([("active", name)])).into_response()
}

async fn set_active_agent(
    State(state): State<WebState>,
    Json(payload): Json<ActivateAgentRequest>,
) -> impl IntoResponse {
    if let Some(name) = payload.name {
        let config = {
            let manager = state.subagent_manager.lock().await;
            manager.get_subagent(&name).cloned()
        };
        let config = match config {
            Some(cfg) => cfg,
            None => return (StatusCode::NOT_FOUND, "Agent not found".to_string()).into_response(),
        };

        match state
            .agent
            .lock_owned()
            .await
            .switch_to_subagent(&config)
            .await
        {
            Ok(_) => {
                let mut manager = state.subagent_manager.lock().await;
                manager.set_active_subagent(Some(name.clone()));
                Json(HashMap::from([("active", Some(name))])).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to activate agent: {}", e),
            )
                .into_response(),
        }
    } else {
        match state.agent.lock().await.exit_subagent().await {
            Ok(_) => {
                let mut manager = state.subagent_manager.lock().await;
                manager.set_active_subagent(None);
                Json(HashMap::from([("active", Option::<String>::None)])).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to exit agent: {}", e),
            )
                .into_response(),
        }
    }
}
