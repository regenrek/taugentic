use ta_protocol::wire::{
    BrowserActionDecision, BrowserActionKind, BrowserActionRequest, BrowserActionResult,
    BrowserNavigationKind, BrowserNavigationRequest,
};

/// The sole product browser-action policy. Native browser code only waits for
/// this daemon result and never makes a navigation or download decision.
pub fn decide_browser_action(request: &BrowserActionRequest) -> BrowserActionResult {
    let mut result = match request.kind {
        BrowserActionKind::DownloadDestination => deny_browser_download(&request.request_id),
        BrowserActionKind::NavigationIntent => request
            .navigation
            .as_ref()
            .map(decide_browser_navigation)
            .unwrap_or_else(|| deny_browser_unsupported(&request.request_id)),
        BrowserActionKind::NavigationAction => decide_navigation_action(request),
        BrowserActionKind::NavigationResponse => decide_navigation_response(request),
    };
    result.request_id = request.request_id.clone();
    result
}

fn decide_navigation_action(request: &BrowserActionRequest) -> BrowserActionResult {
    let Some(should_perform_download) = request.should_perform_download else {
        return deny_browser_unsupported(&request.request_id);
    };
    let result = request
        .navigation
        .as_ref()
        .map(decide_browser_navigation)
        .unwrap_or_else(|| deny_browser_unsupported(&request.request_id));
    if result.decision == BrowserActionDecision::Allow && should_perform_download {
        BrowserActionResult {
            request_id: String::new(),
            decision: BrowserActionDecision::Download,
            reason: None,
        }
    } else {
        result
    }
}

fn decide_navigation_response(request: &BrowserActionRequest) -> BrowserActionResult {
    let Some(can_show_mime_type) = request.can_show_mime_type else {
        return deny_browser_unsupported(&request.request_id);
    };
    let result = request
        .navigation
        .as_ref()
        .map(decide_browser_navigation)
        .unwrap_or_else(|| deny_browser_unsupported(&request.request_id));
    if result.decision == BrowserActionDecision::Allow && !can_show_mime_type {
        BrowserActionResult {
            request_id: String::new(),
            decision: BrowserActionDecision::Download,
            reason: None,
        }
    } else {
        result
    }
}

fn decide_browser_navigation(request: &BrowserNavigationRequest) -> BrowserActionResult {
    let allowed = match request.kind {
        BrowserNavigationKind::Navigate => request.url.as_deref().is_some_and(allowed_url),
        BrowserNavigationKind::Back
        | BrowserNavigationKind::Forward
        | BrowserNavigationKind::Reload => true,
    };
    BrowserActionResult {
        request_id: String::new(),
        decision: if allowed {
            BrowserActionDecision::Allow
        } else {
            BrowserActionDecision::Cancel
        },
        reason: (!allowed).then(|| {
            "Browser navigation is limited to HTTPS and loopback preview URLs.".to_string()
        }),
    }
}

pub fn deny_browser_download(request_id: impl Into<String>) -> BrowserActionResult {
    BrowserActionResult {
        request_id: request_id.into(),
        decision: BrowserActionDecision::Cancel,
        reason: Some("Downloads are not available yet.".to_string()),
    }
}

/// Returns the one fail-closed result for a browser request whose profile is
/// not authorized for the current principal. The caller must still deliver it
/// to the native surface so its pending handler is resolved exactly once.
pub fn deny_browser_unauthorized(request_id: impl Into<String>) -> BrowserActionResult {
    BrowserActionResult {
        request_id: request_id.into(),
        decision: BrowserActionDecision::Cancel,
        reason: Some("Browser action is not authorized.".to_string()),
    }
}

fn deny_browser_unsupported(request_id: impl Into<String>) -> BrowserActionResult {
    BrowserActionResult {
        request_id: request_id.into(),
        decision: BrowserActionDecision::Cancel,
        reason: Some("This browser action is not available.".to_string()),
    }
}

fn allowed_url(value: &str) -> bool {
    if value.starts_with("https://") {
        return value[8..]
            .split('/')
            .next()
            .is_some_and(|host| !host.is_empty());
    }
    if !value.starts_with("http://") {
        return false;
    }
    matches!(
        value[7..]
            .split('/')
            .next()
            .and_then(|host| host.split(':').next()),
        Some("localhost") | Some("127.0.0.1")
    ) || value[7..].starts_with("[::1]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::{BrowserActionKind, BrowserProfileId};
    fn request(kind: BrowserActionKind, url: Option<&str>) -> BrowserActionRequest {
        BrowserActionRequest {
            request_id: "request".into(),
            profile_id: BrowserProfileId::new("profile").unwrap(),
            kind,
            navigation: url.map(|value| BrowserNavigationRequest {
                kind: BrowserNavigationKind::Navigate,
                url: Some(value.into()),
            }),
            should_perform_download: (kind == BrowserActionKind::NavigationAction).then_some(false),
            can_show_mime_type: (kind == BrowserActionKind::NavigationResponse).then_some(true),
        }
    }
    #[test]
    fn permits_https_and_loopback_only() {
        assert_eq!(
            decide_browser_action(&request(
                BrowserActionKind::NavigationIntent,
                Some("https://example.com")
            ))
            .decision,
            BrowserActionDecision::Allow
        );
        assert_eq!(
            decide_browser_action(&request(
                BrowserActionKind::NavigationAction,
                Some("http://localhost:3000")
            ))
            .decision,
            BrowserActionDecision::Allow
        );
        assert_eq!(
            decide_browser_action(&request(
                BrowserActionKind::NavigationResponse,
                Some("file:///tmp/a")
            ))
            .decision,
            BrowserActionDecision::Cancel
        );
    }
    #[test]
    fn native_download_intent_is_daemon_owned_and_fails_closed() {
        let mut action = request(
            BrowserActionKind::NavigationAction,
            Some("https://example.com/file"),
        );
        action.should_perform_download = Some(true);
        assert_eq!(
            decide_browser_action(&action).decision,
            BrowserActionDecision::Download
        );
        action.navigation.as_mut().expect("navigation").url = Some("file:///tmp/a".into());
        assert_eq!(
            decide_browser_action(&action).decision,
            BrowserActionDecision::Cancel
        );
        action.navigation.as_mut().expect("navigation").url =
            Some("https://example.com/file".into());
        action.should_perform_download = None;
        assert_eq!(
            decide_browser_action(&action).decision,
            BrowserActionDecision::Cancel
        );

        let mut response = request(
            BrowserActionKind::NavigationResponse,
            Some("https://example.com/file"),
        );
        response.can_show_mime_type = Some(false);
        assert_eq!(
            decide_browser_action(&response).decision,
            BrowserActionDecision::Download
        );
        response.can_show_mime_type = None;
        assert_eq!(
            decide_browser_action(&response).decision,
            BrowserActionDecision::Cancel
        );
    }
    #[test]
    fn download_destination_is_explicitly_denied() {
        assert_eq!(
            decide_browser_action(&request(BrowserActionKind::DownloadDestination, None))
                .reason
                .as_deref(),
            Some("Downloads are not available yet.")
        );
    }
    #[test]
    fn unauthorized_actions_are_explicitly_cancelled() {
        let result = deny_browser_unauthorized("stale-request");
        assert_eq!(result.request_id, "stale-request");
        assert_eq!(result.decision, BrowserActionDecision::Cancel);
        assert_eq!(
            result.reason.as_deref(),
            Some("Browser action is not authorized.")
        );
    }
}
