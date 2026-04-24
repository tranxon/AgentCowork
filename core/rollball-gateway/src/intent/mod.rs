//! Intent routing module
//!
//! Routes Intent messages between Agents and applies privacy filters
//! to responses before cross-agent forwarding.

pub mod privacy;

use rollball_core::Intent;
use serde_json::Value;

/// Intent router for cross-agent messaging.
///
/// Responsible for:
/// - Routing Intent requests to target Agents
/// - Filtering sensitive content from responses before forwarding
#[derive(Debug, Clone)]
pub struct IntentRouter;

impl IntentRouter {
    /// Create a new IntentRouter
    pub fn new() -> Self {
        Self
    }

    /// Apply privacy filtering to an intent response before forwarding.
    ///
    /// Strips any memory nodes marked as `Sensitive` from the response payload.
    pub fn filter_response(&self, response: Value) -> Value {
        privacy::filter_sensitive_content(response)
    }

    /// Route an intent to its target and return the (filtered) response.
    ///
    /// In the full implementation this would dispatch to the target Agent's
    /// runtime and wait for a response. For now the filtering stage is
    /// wired in as a transparent pass-through.
    pub fn route_and_filter(&self, _intent: &Intent, response: Value) -> Value {
        self.filter_response(response)
    }
}

impl Default for IntentRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_intent_router_filters_sensitive_in_response() {
        let router = IntentRouter::new();
        let response = json!({
            "memories": [
                { "id": "1", "content": "safe", "metadata": null, "zone": "semantic", "privacy_level": "Public" },
                { "id": "2", "content": "secret", "metadata": null, "zone": "semantic", "privacy_level": "Sensitive" }
            ]
        });

        let filtered = router.filter_response(response);
        let memories = filtered.get("memories").unwrap().as_array().unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].get("id").unwrap().as_str().unwrap(), "1");
    }

    #[test]
    fn test_intent_router_route_and_filter() {
        let router = IntentRouter::new();
        let intent = Intent {
            target: "com.example.target".to_string(),
            action: "query".to_string(),
            params: json!({}),
            async_: false,
            message_id: None,
            source: Some("com.example.source".to_string()),
        };

        let response = json!({
            "memories": [
                { "id": "1", "content": "safe", "metadata": null, "zone": "semantic", "privacy_level": "Public" },
                { "id": "2", "content": "secret", "metadata": null, "zone": "semantic", "privacy_level": "Sensitive" }
            ]
        });

        let filtered = router.route_and_filter(&intent, response);
        let memories = filtered.get("memories").unwrap().as_array().unwrap();
        assert_eq!(memories.len(), 1);
    }

    #[test]
    fn test_intent_router_passes_through_non_memory_responses() {
        let router = IntentRouter::new();
        let response = json!({
            "status": "ok",
            "result": 42
        });

        let filtered = router.filter_response(response.clone());
        assert_eq!(filtered, response);
    }
}
