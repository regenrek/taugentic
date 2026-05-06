use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use ta_auth_openai::browser::BrowserLaunch;
use ta_auth_openai::{
    CredentialKey, CredentialStore, CredentialStoreError, OAuthConfig, StoredCredentials,
    TokenLifecycleEvent,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use url::Url;

pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

pub async fn expect_lifecycle(
    receiver: &mut tokio::sync::broadcast::Receiver<TokenLifecycleEvent>,
    label: &'static str,
    matches_event: impl Fn(&TokenLifecycleEvent) -> bool,
) -> TestResult {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = receiver
                .recv()
                .await
                .map_err(|error| test_error(format!("lifecycle channel closed: {error}")))?;
            if matches_event(&event) {
                return Ok(());
            }
        }
    })
    .await
    .map_err(|_| test_error(format!("timed out waiting for {label} lifecycle event")))?
}

fn test_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}

#[derive(Clone)]
pub struct MockBrowserLauncher {
    result_tx: Arc<Mutex<Option<oneshot::Sender<TestResult>>>>,
    launches: Arc<AtomicUsize>,
}

impl MockBrowserLauncher {
    pub fn new() -> (Self, oneshot::Receiver<TestResult>) {
        let (sender, receiver) = oneshot::channel();
        (
            Self {
                result_tx: Arc::new(Mutex::new(Some(sender))),
                launches: Arc::new(AtomicUsize::new(0)),
            },
            receiver,
        )
    }

    pub fn launcher(&self) -> Arc<dyn Fn(&Url) -> BrowserLaunch + Send + Sync> {
        let launcher = self.clone();
        Arc::new(move |authorize_url: &Url| {
            launcher.launches.fetch_add(1, Ordering::SeqCst);
            let authorize_url = authorize_url.clone();
            let result_tx = launcher
                .result_tx
                .lock()
                .expect("browser result lock")
                .take();
            tokio::spawn(async move {
                let result = follow_authorize_redirect(authorize_url).await;
                if let Some(sender) = result_tx {
                    let _ = sender.send(result);
                }
            });
            BrowserLaunch::Opened
        })
    }

    pub fn launch_count(&self) -> usize {
        self.launches.load(Ordering::SeqCst)
    }
}

async fn follow_authorize_redirect(authorize_url: Url) -> TestResult {
    let response = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()?
        .get(authorize_url)
        .send()
        .await?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(test_error(format!(
            "authorize redirect ended with {}",
            response.status()
        )))
    }
}

pub struct MockOpenAiServer {
    address: SocketAddr,
    state: Arc<MockServerState>,
    shutdown: CancellationToken,
    task: JoinHandle<()>,
}

impl MockOpenAiServer {
    pub async fn start() -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let state = Arc::new(MockServerState::default());
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(run_mock_server(
            listener,
            Arc::clone(&state),
            shutdown.clone(),
        ));
        Ok(Self {
            address,
            state,
            shutdown,
            task,
        })
    }

    pub fn oauth_config(&self) -> OAuthConfig {
        OAuthConfig {
            auth_url: self.url("/oauth/authorize"),
            token_url: self.url("/oauth/token"),
            revoke_url: self.url("/oauth/revoke"),
            client_id: "test-client".to_string(),
            scopes: vec!["openid".to_string(), "offline_access".to_string()],
            redirect_uri_template: "http://localhost:{port}/auth/callback".to_string(),
            callback_ports: vec![0],
            callback_timeout: Duration::from_secs(5),
            originator: None,
            allowed_workspace_id: None,
        }
    }

    pub fn base_url(&self, prefix: &str) -> String {
        format!("http://{}{}", self.address, prefix)
    }

    pub fn snapshot(&self) -> MockServerSnapshot {
        MockServerSnapshot {
            token_grants: self.state.token_grants.lock().expect("grant lock").clone(),
            response_bearers: self
                .state
                .response_bearers
                .lock()
                .expect("bearer lock")
                .clone(),
            revoke_count: self.state.revoke_count.load(Ordering::SeqCst),
            failures: self.state.failures.lock().expect("failure lock").clone(),
        }
    }

    fn url(&self, path: &str) -> Url {
        Url::parse(&self.base_url(path)).expect("mock URL")
    }
}

impl Drop for MockOpenAiServer {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.task.abort();
    }
}

#[derive(Default)]
struct MockServerState {
    token_grants: Mutex<Vec<String>>,
    response_bearers: Mutex<Vec<String>>,
    revoke_count: AtomicUsize,
    failures: Mutex<Vec<String>>,
}

pub struct MockServerSnapshot {
    pub token_grants: Vec<String>,
    pub response_bearers: Vec<String>,
    pub revoke_count: usize,
    pub failures: Vec<String>,
}

async fn run_mock_server(
    listener: TcpListener,
    state: Arc<MockServerState>,
    shutdown: CancellationToken,
) {
    loop {
        tokio::select! {
            () = shutdown.cancelled() => break,
            accepted = listener.accept() => {
                let Ok((stream, _peer)) = accepted else {
                    break;
                };
                tokio::spawn(handle_mock_connection(stream, Arc::clone(&state)));
            }
        }
    }
}

async fn handle_mock_connection(mut stream: TcpStream, state: Arc<MockServerState>) {
    let response = match read_request(&mut stream).await {
        Ok(request) => route_request(request, &state),
        Err(error) => http_response(400, "Bad Request", &error.to_string()),
    };
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn route_request(request: HttpRequest, state: &MockServerState) -> String {
    match (request.method.as_str(), request.url.path()) {
        ("GET", "/oauth/authorize") => authorize_response(&request),
        ("POST", "/oauth/token") => token_response(&request, state),
        ("POST", "/oauth/revoke") => {
            state.revoke_count.fetch_add(1, Ordering::SeqCst);
            http_response(200, "OK", "{}")
        }
        ("POST", "/v1/responses") => responses_response(&request, state),
        _ => http_response(404, "Not Found", "not found"),
    }
}

fn authorize_response(request: &HttpRequest) -> String {
    let redirect_uri = query_value(&request.url, "redirect_uri");
    let state = query_value(&request.url, "state");
    match (redirect_uri, state) {
        (Some(redirect_uri), Some(state)) => {
            redirect_response(&format!("{redirect_uri}?code=mock-auth-code&state={state}"))
        }
        _ => http_response(400, "Bad Request", "missing redirect parameters"),
    }
}

fn token_response(request: &HttpRequest, state: &MockServerState) -> String {
    let fields = request.fields();
    let grant = fields.get("grant_type").cloned().unwrap_or_default();
    state
        .token_grants
        .lock()
        .expect("grant lock")
        .push(grant.clone());
    match grant.as_str() {
        "authorization_code"
            if fields
                .get("code")
                .is_some_and(|code| code == "mock-auth-code") =>
        {
            http_json(&format!(
                r#"{{"access_token":"mock-access","refresh_token":"mock-refresh","id_token":"{}","expires_in":3600}}"#,
                id_token_with_organization()
            ))
        }
        "urn:ietf:params:oauth:grant-type:token-exchange"
            if fields
                .get("requested_token")
                .is_some_and(|token| token == "openai-api-key") =>
        {
            http_json(r#"{"access_token":"mock-api-access"}"#)
        }
        "refresh_token"
            if fields
                .get("refresh_token")
                .is_some_and(|token| token == "mock-refresh") =>
        {
            http_json(
                r#"{"access_token":"fresh-access","refresh_token":"mock-refresh-2","api_access_token":"fresh-api-access","expires_in":3600}"#,
            )
        }
        _ => {
            state
                .failures
                .lock()
                .expect("failure lock")
                .push(format!("unexpected token grant: {grant}"));
            http_response(400, "Bad Request", "unexpected token request")
        }
    }
}

fn responses_response(request: &HttpRequest, state: &MockServerState) -> String {
    let bearer = request
        .headers
        .get("authorization")
        .cloned()
        .unwrap_or_default();
    state
        .response_bearers
        .lock()
        .expect("bearer lock")
        .push(bearer.clone());
    match bearer.as_str() {
        "Bearer mock-api-access" => http_response(401, "Unauthorized", "expired"),
        "Bearer fresh-api-access" => http_sse("data: [DONE]\n\n"),
        _ => http_response(500, "Internal Server Error", "unexpected bearer"),
    }
}

fn id_token_with_organization() -> &'static str {
    "e30.eyJleHAiOjE4MDAwMDAwMDAsImVtYWlsIjoidXNlckBleGFtcGxlLmNvbSIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY2NfMTIzIiwiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIiwiY2hhdGdwdF9hY2NvdW50X2lzX2ZlZHJhbXAiOmZhbHNlLCJvcmdhbml6YXRpb25faWQiOiJvcmdfMTIzIn19.sig"
}

struct HttpRequest {
    method: String,
    url: Url,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl HttpRequest {
    fn fields(&self) -> HashMap<String, String> {
        if self
            .headers
            .get("content-type")
            .is_some_and(|content_type| content_type.starts_with("application/json"))
        {
            return serde_json::from_slice::<HashMap<String, String>>(&self.body)
                .unwrap_or_default();
        }
        url::form_urlencoded::parse(&self.body)
            .into_owned()
            .collect()
    }
}

async fn read_request(stream: &mut TcpStream) -> std::io::Result<HttpRequest> {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 1024];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed before headers",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };
    let headers_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers_text.lines();
    let request_line = lines.next().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing request line")
    })?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let url = request_url(parts.next().unwrap_or_default())?;
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let mut chunk = [0_u8; 1024];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body = buffer[body_start..body_start + content_length].to_vec();
    Ok(HttpRequest {
        method,
        url,
        headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn request_url(target: &str) -> std::io::Result<Url> {
    let raw_url = if target.starts_with("http://") || target.starts_with("https://") {
        target.to_string()
    } else {
        format!("http://localhost{target}")
    };
    Url::parse(&raw_url)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn query_value(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

fn redirect_response(location: &str) -> String {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}

fn http_json(body: &str) -> String {
    http_response_with_type(200, "OK", "application/json", body)
}

fn http_sse(body: &str) -> String {
    http_response_with_type(200, "OK", "text/event-stream", body)
}

fn http_response(status: u16, reason: &str, body: &str) -> String {
    http_response_with_type(status, reason, "text/plain; charset=utf-8", body)
}

fn http_response_with_type(status: u16, reason: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[derive(Default)]
pub struct TestCredentialStore {
    credentials: Mutex<Option<StoredCredentials>>,
}

impl CredentialStore for TestCredentialStore {
    fn store(
        &self,
        _key: &CredentialKey,
        credentials: &StoredCredentials,
    ) -> Result<(), CredentialStoreError> {
        *self.credentials.lock().expect("store lock") = Some(credentials.clone());
        Ok(())
    }

    fn load(
        &self,
        _key: &CredentialKey,
    ) -> Result<Option<StoredCredentials>, CredentialStoreError> {
        Ok(self.credentials.lock().expect("store lock").clone())
    }

    fn delete(&self, _key: &CredentialKey) -> Result<(), CredentialStoreError> {
        *self.credentials.lock().expect("store lock") = None;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "test-memory"
    }
}
