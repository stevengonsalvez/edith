//! Request handler and method dispatch

use std::sync::Arc;
use std::time::Instant;

use edith_protocol::{
    DedupeCache, DedupeResult, ErrorShape, GatewayFrame, IdempotencyHandler, RequestFrame,
    ResponseFrame, codes, methods,
};
use parking_lot::RwLock;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use super::connection::ConnectionState;
use super::methods::MethodContext;
use crate::GatewayState;

/// Request handler for processing incoming frames
pub struct RequestHandler {
    /// Deduplication cache
    dedupe: Arc<DedupeCache>,

    /// Gateway state
    gateway_state: GatewayState,

    /// Server start time (for uptime)
    server_start: Instant,
}

impl RequestHandler {
    /// Create a new request handler
    pub fn new(gateway_state: GatewayState, server_start: Instant) -> Self {
        Self {
            dedupe: Arc::new(DedupeCache::new()),
            gateway_state,
            server_start,
        }
    }

    /// Handle an incoming request frame
    pub async fn handle_request(
        &self,
        request: RequestFrame,
        conn: &Arc<RwLock<ConnectionState>>,
    ) -> ResponseFrame {
        let request_id = request.id.clone();
        let method = request.method.clone();

        debug!(request_id = %request_id, method = %method, "Handling request");

        // Check for idempotency key
        let idempotency_key = request.idempotency_key.as_ref();

        // Check deduplication for side-effecting methods
        if let Some(key) = idempotency_key {
            if is_side_effecting_method(&method) {
                let handler = IdempotencyHandler::new(self.dedupe.clone());
                match handler.check(&method, key).await {
                    DedupeResult::Cached(entry) => {
                        debug!(
                            request_id = %request_id,
                            method = %method,
                            idempotency_key = %key,
                            "Returning cached response"
                        );
                        return ResponseFrame {
                            id: request_id,
                            ok: entry.ok,
                            payload: entry.payload,
                            error: entry.error,
                        };
                    }
                    DedupeResult::New => {
                        // Continue processing
                    }
                }
            }
        }

        // Build method context
        let ctx = MethodContext {
            gateway_state: self.gateway_state.clone(),
            conn: conn.clone(),
            server_start: self.server_start,
        };

        // Dispatch to method handler
        let result = dispatch_method(&method, request.params, &ctx).await;

        // Cache result if idempotent
        if let Some(key) = idempotency_key {
            if is_side_effecting_method(&method) {
                let handler = IdempotencyHandler::new(self.dedupe.clone());
                match &result {
                    Ok(payload) => {
                        handler
                            .record_success(&method, key, payload.clone())
                            .await;
                    }
                    Err(error) => {
                        handler.record_failure(&method, key, error.clone()).await;
                    }
                }
            }
        }

        // Build response
        match result {
            Ok(payload) => ResponseFrame {
                id: request_id,
                ok: true,
                payload: Some(payload),
                error: None,
            },
            Err(error) => ResponseFrame {
                id: request_id,
                ok: false,
                payload: None,
                error: Some(error),
            },
        }
    }

    /// Get deduplication cache (for stats/admin)
    pub fn dedupe_cache(&self) -> &DedupeCache {
        &self.dedupe
    }
}

/// Check if a method has side effects and should be deduplicated
fn is_side_effecting_method(method: &str) -> bool {
    matches!(
        method,
        methods::SEND
            | methods::AGENT
            | methods::CHAT_SEND
            | methods::NODE_INVOKE
            | methods::SESSIONS_PATCH
            | methods::SESSIONS_RESET
            | methods::SESSIONS_DELETE
            | methods::SESSIONS_COMPACT
            | methods::CONFIG_SET
            | methods::CRON_ADD
            | methods::CRON_UPDATE
            | methods::CRON_REMOVE
            | methods::CRON_RUN
            | methods::NODE_PAIR_APPROVE
            | methods::NODE_PAIR_REJECT
            | methods::NODE_RENAME
            | methods::SKILLS_INSTALL
            | methods::SKILLS_UNINSTALL
            | methods::SKILLS_UPDATE
            | methods::WIZARD_INIT
            | methods::WIZARD_STEP
    )
}

/// Dispatch request to appropriate method handler
async fn dispatch_method(
    method: &str,
    params: Option<Value>,
    ctx: &MethodContext,
) -> Result<Value, ErrorShape> {
    use super::methods::*;

    match method {
        // Health/System
        methods::HEALTH => handle_health(params, ctx).await,
        methods::STATUS => handle_status(params, ctx).await,
        methods::PROVIDERS_STATUS => handle_providers_status(params, ctx).await,

        // Config
        methods::CONFIG_GET => handle_config_get(params, ctx).await,
        methods::CONFIG_SET => handle_config_set(params, ctx).await,
        methods::CONFIG_SCHEMA => handle_config_schema(params, ctx).await,

        // Sessions
        methods::SESSIONS_LIST => handle_sessions_list(params, ctx).await,
        methods::SESSIONS_PATCH => handle_sessions_patch(params, ctx).await,
        methods::SESSIONS_RESET => handle_sessions_reset(params, ctx).await,
        methods::SESSIONS_DELETE => handle_sessions_delete(params, ctx).await,
        methods::SESSIONS_COMPACT => handle_sessions_compact(params, ctx).await,

        // Messaging
        methods::SEND => handle_send(params, ctx).await,
        methods::AGENT => handle_agent(params, ctx).await,
        methods::AGENT_WAIT => handle_agent_wait(params, ctx).await,
        methods::CHAT_SEND => handle_chat_send(params, ctx).await,
        methods::CHAT_ABORT => handle_chat_abort(params, ctx).await,
        methods::CHAT_HISTORY => handle_chat_history(params, ctx).await,

        // Cron
        methods::CRON_LIST => handle_cron_list(params, ctx).await,
        methods::CRON_STATUS => handle_cron_status(params, ctx).await,
        methods::CRON_ADD => handle_cron_add(params, ctx).await,
        methods::CRON_UPDATE => handle_cron_update(params, ctx).await,
        methods::CRON_REMOVE => handle_cron_remove(params, ctx).await,
        methods::CRON_RUN => handle_cron_run(params, ctx).await,
        methods::CRON_RUNS => handle_cron_runs(params, ctx).await,

        // Node
        methods::NODE_PAIR_REQUEST => handle_node_pair_request(params, ctx).await,
        methods::NODE_PAIR_APPROVE => handle_node_pair_approve(params, ctx).await,
        methods::NODE_PAIR_REJECT => handle_node_pair_reject(params, ctx).await,
        methods::NODE_PAIR_VERIFY => handle_node_pair_verify(params, ctx).await,
        methods::NODE_LIST => handle_node_list(params, ctx).await,
        methods::NODE_INVOKE => handle_node_invoke(params, ctx).await,
        methods::NODE_RENAME => handle_node_rename(params, ctx).await,

        // Wizard
        methods::WIZARD_INIT => handle_wizard_init(params, ctx).await,
        methods::WIZARD_STEP => handle_wizard_step(params, ctx).await,
        methods::WIZARD_STATE => handle_wizard_state(params, ctx).await,
        methods::WIZARD_ABORT => handle_wizard_abort(params, ctx).await,

        // Models
        methods::MODELS_LIST => handle_models_list(params, ctx).await,

        // Skills
        methods::SKILLS_LIST => handle_skills_list(params, ctx).await,
        methods::SKILLS_REGISTRY => handle_skills_registry(params, ctx).await,
        methods::SKILLS_INSTALL => handle_skills_install(params, ctx).await,
        methods::SKILLS_UNINSTALL => handle_skills_uninstall(params, ctx).await,
        methods::SKILLS_UPDATE => handle_skills_update(params, ctx).await,

        // Voice
        methods::VOICEWAKE_GET => handle_voicewake_get(params, ctx).await,
        methods::VOICEWAKE_SET => handle_voicewake_set(params, ctx).await,

        // Talk mode
        methods::TALK_MODE => handle_talk_mode(params, ctx).await,

        // Web login
        methods::WEB_LOGIN_REQUEST => handle_web_login_request(params, ctx).await,
        methods::WEB_LOGIN_CHECK => handle_web_login_check(params, ctx).await,

        // Unknown method
        _ => {
            warn!(method = %method, "Unknown method");
            Err(ErrorShape::new(
                codes::METHOD_NOT_FOUND,
                &format!("Unknown method: {}", method),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_effecting_methods() {
        assert!(is_side_effecting_method(methods::SEND));
        assert!(is_side_effecting_method(methods::AGENT));
        assert!(is_side_effecting_method(methods::CHAT_SEND));
        assert!(!is_side_effecting_method(methods::HEALTH));
        assert!(!is_side_effecting_method(methods::STATUS));
        assert!(!is_side_effecting_method(methods::SESSIONS_LIST));
    }
}
