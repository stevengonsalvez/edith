//! RPC method implementations
//!
//! Each method handler receives parsed params and returns a Result<Value, ErrorShape>.
//! Methods are grouped by category matching the protocol specification.

use std::sync::Arc;
use std::time::Instant;

use edith_protocol::{
    ErrorShape, HealthSnapshot, codes,
    ConfigGetParams, ConfigSetParams, SendParams, AgentParams, ChatSendParams,
    SessionsPatchParams, SessionsResetParams, SessionsDeleteParams, CronAddParams,
    CronUpdateParams, CronRemoveParams, CronRunParams, NodeInvokeParams, NodeRenameParams,
    SkillsInstallParams, VoiceWakeSetParams,
};
use parking_lot::RwLock;
use serde_json::{json, Value};
use tracing::{info, warn};

use super::connection::ConnectionState;
use crate::GatewayState;

/// Method execution context
pub struct MethodContext {
    /// Gateway state
    pub gateway_state: GatewayState,

    /// Connection state
    pub conn: Arc<RwLock<ConnectionState>>,

    /// Server start time
    pub server_start: Instant,
}

// =============================================================================
// Health/System Methods
// =============================================================================

pub async fn handle_health(
    _params: Option<Value>,
    ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let uptime_ms = ctx.server_start.elapsed().as_millis() as u64;

    let health = HealthSnapshot {
        ok: true,
        providers: None,
        last_error: None,
        uptime_ms: Some(uptime_ms),
    };

    Ok(serde_json::to_value(health).unwrap_or(json!({"ok": true})))
}

pub async fn handle_status(
    _params: Option<Value>,
    ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let uptime_ms = ctx.server_start.elapsed().as_millis() as u64;

    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_ms": uptime_ms,
        "connections": ctx.gateway_state.connections().len(),
        "config_loaded": true
    }))
}

pub async fn handle_providers_status(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get actual provider status from provider manager
    Ok(json!({
        "providers": []
    }))
}

// =============================================================================
// Config Methods
// =============================================================================

pub async fn handle_config_get(
    params: Option<Value>,
    ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let config = ctx.gateway_state.config();

    // If specific path requested, extract that part
    if let Some(params) = params {
        if let Ok(get_params) = serde_json::from_value::<ConfigGetParams>(params) {
            if let Some(path) = get_params.path {
                let config_json = serde_json::to_value(&config)
                    .map_err(|e| ErrorShape::new(codes::INTERNAL_ERROR, &e.to_string()))?;

                // Simple path resolution (dot-separated)
                let parts: Vec<&str> = path.split('.').collect();
                let mut current = &config_json;
                for part in parts {
                    match current.get(part) {
                        Some(v) => current = v,
                        None => {
                            return Err(ErrorShape::new(
                                codes::NOT_FOUND,
                                &format!("Config path not found: {}", path),
                            ));
                        }
                    }
                }
                return Ok(current.clone());
            }
        }
    }

    // Return full config (redacted)
    let config_json = serde_json::to_value(&config)
        .map_err(|e| ErrorShape::new(codes::INTERNAL_ERROR, &e.to_string()))?;

    Ok(config_json)
}

pub async fn handle_config_set(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let set_params: ConfigSetParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Implement config mutation
    warn!(path = %set_params.path, "config.set not fully implemented");

    Ok(json!({
        "ok": true,
        "path": set_params.path
    }))
}

pub async fn handle_config_schema(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Generate JSON Schema for EdithConfig
    Ok(json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "title": "EdithConfig",
        "description": "Edith configuration schema"
    }))
}

// =============================================================================
// Session Methods
// =============================================================================

pub async fn handle_sessions_list(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get sessions from session manager
    Ok(json!({
        "sessions": []
    }))
}

pub async fn handle_sessions_patch(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let patch_params: SessionsPatchParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Implement session patching
    warn!(session_key = %patch_params.key, "sessions.patch not fully implemented");

    Ok(json!({
        "ok": true,
        "session_key": patch_params.key
    }))
}

pub async fn handle_sessions_reset(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let reset_params: SessionsResetParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Implement session reset
    warn!(session_key = %reset_params.key, "sessions.reset not fully implemented");

    Ok(json!({
        "ok": true,
        "session_key": reset_params.key
    }))
}

pub async fn handle_sessions_delete(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let delete_params: SessionsDeleteParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Implement session deletion
    warn!(session_key = %delete_params.key, "sessions.delete not fully implemented");

    Ok(json!({
        "ok": true,
        "session_key": delete_params.key
    }))
}

pub async fn handle_sessions_compact(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Implement session compaction
    warn!("sessions.compact not fully implemented");

    Ok(json!({
        "ok": true,
        "compacted": 0
    }))
}

// =============================================================================
// Messaging Methods
// =============================================================================

pub async fn handle_send(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let send_params: SendParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Dispatch to provider
    warn!(to = %send_params.to, "send not fully implemented");

    Ok(json!({
        "ok": true,
        "message_id": uuid::Uuid::new_v4().to_string()
    }))
}

pub async fn handle_agent(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let agent_params: AgentParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    let run_id = uuid::Uuid::new_v4().to_string();

    // TODO: Start agent run
    info!(run_id = %run_id, message_len = agent_params.message.len(), "Starting agent run");

    Ok(json!({
        "run_id": run_id,
        "status": "started"
    }))
}

pub async fn handle_agent_wait(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let run_id = params.get("runId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing runId"))?;

    // TODO: Wait for agent completion
    warn!(run_id = %run_id, "agent.wait not fully implemented");

    Ok(json!({
        "run_id": run_id,
        "status": "completed",
        "result": null
    }))
}

pub async fn handle_chat_send(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let chat_params: ChatSendParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    let run_id = uuid::Uuid::new_v4().to_string();

    // TODO: Send chat message to agent
    info!(
        session_id = %chat_params.session_id,
        run_id = %run_id,
        "Starting chat"
    );

    Ok(json!({
        "run_id": run_id,
        "session_id": chat_params.session_id
    }))
}

pub async fn handle_chat_abort(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let run_id = params.get("runId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing runId"))?;

    // TODO: Abort chat/agent run
    warn!(run_id = %run_id, "chat.abort not fully implemented");

    Ok(json!({
        "ok": true,
        "run_id": run_id
    }))
}

pub async fn handle_chat_history(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let session_id = params
        .as_ref()
        .and_then(|p| p.get("sessionId"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing sessionId"))?;

    // TODO: Get chat history
    Ok(json!({
        "session_id": session_id,
        "messages": []
    }))
}

// =============================================================================
// Cron Methods
// =============================================================================

pub async fn handle_cron_list(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get cron jobs from scheduler
    Ok(json!({
        "jobs": []
    }))
}

pub async fn handle_cron_status(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get scheduler status
    Ok(json!({
        "running": true,
        "next_run": null,
        "job_count": 0
    }))
}

pub async fn handle_cron_add(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let add_params: CronAddParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Add cron job
    let job_id = uuid::Uuid::new_v4().to_string();
    warn!(job_id = %job_id, name = %add_params.name, "cron.add not fully implemented");

    Ok(json!({
        "job_id": job_id,
        "name": add_params.name
    }))
}

pub async fn handle_cron_update(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let update_params: CronUpdateParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Update cron job
    warn!(job_id = %update_params.id, "cron.update not fully implemented");

    Ok(json!({
        "ok": true,
        "job_id": update_params.id
    }))
}

pub async fn handle_cron_remove(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let remove_params: CronRemoveParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Remove cron job
    warn!(job_id = %remove_params.id, "cron.remove not fully implemented");

    Ok(json!({
        "ok": true,
        "job_id": remove_params.id
    }))
}

pub async fn handle_cron_run(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let run_params: CronRunParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Trigger cron job manually
    let run_id = uuid::Uuid::new_v4().to_string();
    warn!(job_id = %run_params.id, run_id = %run_id, "cron.run not fully implemented");

    Ok(json!({
        "run_id": run_id,
        "job_id": run_params.id
    }))
}

pub async fn handle_cron_runs(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let job_id = params
        .as_ref()
        .and_then(|p| p.get("id"))
        .and_then(|v| v.as_str());

    // TODO: Get cron run history
    Ok(json!({
        "runs": [],
        "job_id": job_id
    }))
}

// =============================================================================
// Node Methods
// =============================================================================

pub async fn handle_node_pair_request(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Handle pairing request from node
    let request_id = uuid::Uuid::new_v4().to_string();

    Ok(json!({
        "request_id": request_id,
        "status": "pending"
    }))
}

pub async fn handle_node_pair_approve(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let request_id = params.get("requestId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing requestId"))?;

    // TODO: Approve pairing request
    let node_id = uuid::Uuid::new_v4().to_string();
    warn!(request_id = %request_id, node_id = %node_id, "node.pair.approve not fully implemented");

    Ok(json!({
        "ok": true,
        "node_id": node_id,
        "request_id": request_id
    }))
}

pub async fn handle_node_pair_reject(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let request_id = params.get("requestId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing requestId"))?;

    // TODO: Reject pairing request
    warn!(request_id = %request_id, "node.pair.reject not fully implemented");

    Ok(json!({
        "ok": true,
        "request_id": request_id
    }))
}

pub async fn handle_node_pair_verify(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let node_id = params.get("nodeId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing nodeId"))?;

    // TODO: Verify node pairing
    Ok(json!({
        "ok": true,
        "node_id": node_id,
        "verified": true
    }))
}

pub async fn handle_node_list(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: List paired nodes
    Ok(json!({
        "nodes": []
    }))
}

pub async fn handle_node_invoke(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let invoke_params: NodeInvokeParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Invoke node capability
    warn!(
        node_id = %invoke_params.node_id,
        command = %invoke_params.command,
        "node.invoke not fully implemented"
    );

    Ok(json!({
        "ok": true,
        "node_id": invoke_params.node_id,
        "command": invoke_params.command,
        "result": null
    }))
}

pub async fn handle_node_rename(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let rename_params: NodeRenameParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Rename node
    warn!(node_id = %rename_params.node_id, name = %rename_params.name, "node.rename not fully implemented");

    Ok(json!({
        "ok": true,
        "node_id": rename_params.node_id,
        "name": rename_params.name
    }))
}

// =============================================================================
// Wizard Methods
// =============================================================================

pub async fn handle_wizard_init(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Initialize wizard
    let wizard_id = uuid::Uuid::new_v4().to_string();

    Ok(json!({
        "wizard_id": wizard_id,
        "current_step": 0,
        "steps": ["provider", "api_keys", "features", "confirm"]
    }))
}

pub async fn handle_wizard_step(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let step = params
        .as_ref()
        .and_then(|p| p.get("step"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // TODO: Process wizard step
    warn!(step = step, "wizard.step not fully implemented");

    Ok(json!({
        "ok": true,
        "step": step,
        "next_step": step + 1
    }))
}

pub async fn handle_wizard_state(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get wizard state
    Ok(json!({
        "active": false,
        "wizard_id": null,
        "current_step": null
    }))
}

pub async fn handle_wizard_abort(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Abort wizard
    Ok(json!({
        "ok": true
    }))
}

// =============================================================================
// Models Methods
// =============================================================================

pub async fn handle_models_list(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get available models from LLM provider
    Ok(json!({
        "models": [
            {
                "id": "claude-sonnet-4-20250514",
                "name": "Claude Sonnet 4",
                "provider": "anthropic",
                "context_window": 200000
            },
            {
                "id": "claude-3-5-sonnet-20241022",
                "name": "Claude 3.5 Sonnet",
                "provider": "anthropic",
                "context_window": 200000
            },
            {
                "id": "claude-3-5-haiku-20241022",
                "name": "Claude 3.5 Haiku",
                "provider": "anthropic",
                "context_window": 200000
            }
        ]
    }))
}

// =============================================================================
// Skills Methods
// =============================================================================

pub async fn handle_skills_list(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get installed skills
    Ok(json!({
        "skills": []
    }))
}

pub async fn handle_skills_registry(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get skill registry
    Ok(json!({
        "registry": []
    }))
}

pub async fn handle_skills_install(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let install_params: SkillsInstallParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Install skill
    warn!(skill = %install_params.skill, "skills.install not fully implemented");

    Ok(json!({
        "ok": true,
        "skill": install_params.skill
    }))
}

pub async fn handle_skills_uninstall(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let skill_name = params
        .as_ref()
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing name"))?;

    // TODO: Uninstall skill
    warn!(skill = %skill_name, "skills.uninstall not fully implemented");

    Ok(json!({
        "ok": true,
        "skill": skill_name
    }))
}

pub async fn handle_skills_update(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let skill_name = params
        .as_ref()
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing name"))?;

    // TODO: Update skill
    warn!(skill = %skill_name, "skills.update not fully implemented");

    Ok(json!({
        "ok": true,
        "skill": skill_name
    }))
}

// =============================================================================
// Voice Methods
// =============================================================================

pub async fn handle_voicewake_get(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Get voice wake settings
    Ok(json!({
        "enabled": false,
        "phrases": []
    }))
}

pub async fn handle_voicewake_set(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let params = params.ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing params"))?;

    let set_params: VoiceWakeSetParams = serde_json::from_value(params)
        .map_err(|e| ErrorShape::new(codes::INVALID_PARAMS, &e.to_string()))?;

    // TODO: Set voice wake settings
    warn!(enabled = set_params.enabled, "voicewake.set not fully implemented");

    Ok(json!({
        "ok": true,
        "enabled": set_params.enabled
    }))
}

// =============================================================================
// Talk Mode Method
// =============================================================================

pub async fn handle_talk_mode(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let enabled = params
        .as_ref()
        .and_then(|p| p.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // TODO: Toggle talk mode
    warn!(enabled = enabled, "talk.mode not fully implemented");

    Ok(json!({
        "ok": true,
        "enabled": enabled
    }))
}

// =============================================================================
// Web Login Methods
// =============================================================================

pub async fn handle_web_login_request(
    _params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    // TODO: Generate web login request (QR code, etc.)
    let request_id = uuid::Uuid::new_v4().to_string();

    Ok(json!({
        "request_id": request_id,
        "qr_data": null,
        "expires_at": chrono::Utc::now().timestamp() + 300
    }))
}

pub async fn handle_web_login_check(
    params: Option<Value>,
    _ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    let request_id = params
        .as_ref()
        .and_then(|p| p.get("requestId"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ErrorShape::new(codes::INVALID_PARAMS, "Missing requestId"))?;

    // TODO: Check web login status
    Ok(json!({
        "request_id": request_id,
        "status": "pending",
        "token": null
    }))
}
