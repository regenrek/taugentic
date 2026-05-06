use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener as StdTcpListener};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

use crate::config::OAuthConfig;
use crate::error::OAuthError;
use crate::oauth::endpoints::OPENAI_CHATGPT_CALLBACK_PATH;

#[derive(Clone, Debug)]
pub struct CallbackServerConfig {
    pub expected_state: String,
    pub ports: Vec<u16>,
    pub timeout: Duration,
    pub redirect_uri_template: String,
}

impl CallbackServerConfig {
    pub fn from_oauth_config(config: &OAuthConfig, expected_state: impl Into<String>) -> Self {
        Self {
            expected_state: expected_state.into(),
            ports: config.callback_ports.clone(),
            timeout: config.callback_timeout,
            redirect_uri_template: config.redirect_uri_template.clone(),
        }
    }
}

pub struct CallbackServer {
    listener: StdTcpListener,
    local_addr: SocketAddr,
    redirect_uri: String,
    expected_state: String,
    timeout: Duration,
}

impl CallbackServer {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn port(&self) -> u16 {
        self.local_addr.port()
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub async fn wait_for_code(self) -> Result<String, OAuthError> {
        let listener = TcpListener::from_std(self.listener)?;
        run_server(listener, self.expected_state, self.timeout).await
    }
}

pub async fn start_callback_server(
    config: CallbackServerConfig,
) -> Result<CallbackServer, OAuthError> {
    let listener = bind_first_available(&config.ports).await?;
    let local_addr = listener.local_addr()?;
    let redirect_uri = build_redirect_uri(&config.redirect_uri_template, local_addr.port())?;

    Ok(CallbackServer {
        listener,
        local_addr,
        redirect_uri,
        expected_state: config.expected_state,
        timeout: config.timeout,
    })
}

async fn bind_first_available(ports: &[u16]) -> Result<StdTcpListener, OAuthError> {
    if ports.is_empty() {
        return Err(OAuthError::InvalidConfig(
            "callback port list must not be empty".to_string(),
        ));
    }

    let mut last_error = None;
    for port in ports {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, *port));
        match StdTcpListener::bind(addr).and_then(|listener| {
            listener.set_nonblocking(true)?;
            Ok(listener)
        }) {
            Ok(listener) => return Ok(listener),
            Err(error) => last_error = Some(error),
        }
    }

    Err(OAuthError::CallbackBindFailed {
        ports: ports.to_vec(),
        source: last_error.unwrap_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "no ports configured")
        }),
    })
}

fn build_redirect_uri(template: &str, port: u16) -> Result<String, OAuthError> {
    if !template.contains("{port}") {
        return Err(OAuthError::InvalidConfig(
            "redirect_uri_template must contain `{port}`".to_string(),
        ));
    }
    Ok(template.replace("{port}", &port.to_string()))
}

async fn run_server(
    listener: TcpListener,
    expected_state: String,
    timeout: Duration,
) -> Result<String, OAuthError> {
    tokio::select! {
        callback = accept_until_callback(listener, &expected_state) => callback,
        () = tokio::time::sleep(timeout) => Err(OAuthError::CallbackTimeout),
    }
}

async fn accept_until_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, OAuthError> {
    loop {
        let (stream, _peer_addr) = listener.accept().await?;
        match handle_connection(stream, expected_state).await? {
            ConnectionOutcome::Continue => continue,
            ConnectionOutcome::Complete(result) => return result,
        }
    }
}

enum ConnectionOutcome {
    Continue,
    Complete(Result<String, OAuthError>),
}

async fn handle_connection(
    mut stream: TcpStream,
    expected_state: &str,
) -> Result<ConnectionOutcome, OAuthError> {
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer).await?;
    if read == 0 {
        return Ok(ConnectionOutcome::Continue);
    }

    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(request_line) = request.lines().next() else {
        write_response(&mut stream, 400, "Bad Request", ERROR_HTML).await?;
        return Ok(ConnectionOutcome::Continue);
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next();
    let target = parts.next();
    if method != Some("GET") {
        write_response(&mut stream, 405, "Method Not Allowed", ERROR_HTML).await?;
        return Ok(ConnectionOutcome::Continue);
    }

    let Some(target) = target else {
        write_response(&mut stream, 400, "Bad Request", ERROR_HTML).await?;
        return Ok(ConnectionOutcome::Continue);
    };
    let parsed_url = parse_request_target(target)?;
    if parsed_url.path() != OPENAI_CHATGPT_CALLBACK_PATH {
        write_response(&mut stream, 404, "Not Found", "Not Found").await?;
        return Ok(ConnectionOutcome::Continue);
    }

    let outcome = parse_callback_query(&parsed_url, expected_state);
    match &outcome {
        Ok(_) => write_response(&mut stream, 200, "OK", SUCCESS_HTML).await?,
        Err(error) => {
            let status = match error {
                OAuthError::StateMismatch
                | OAuthError::MissingAuthorizationCode
                | OAuthError::AuthorizationError { .. } => 400,
                _ => 500,
            };
            write_response(&mut stream, status, "OAuth Error", ERROR_HTML).await?;
        }
    }

    Ok(ConnectionOutcome::Complete(outcome))
}

fn parse_request_target(target: &str) -> Result<Url, OAuthError> {
    if target.starts_with("http://") || target.starts_with("https://") {
        return Url::parse(target).map_err(|source| OAuthError::InvalidUrl {
            field: "callback_request_target",
            source,
        });
    }

    Url::parse(&format!("http://localhost{target}")).map_err(|source| OAuthError::InvalidUrl {
        field: "callback_request_target",
        source,
    })
}

fn parse_callback_query(url: &Url, expected_state: &str) -> Result<String, OAuthError> {
    let mut code = None;
    let mut state = None;
    let mut error_code = None;
    let mut error_description = None;

    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error_code = Some(value.into_owned()),
            "error_description" => error_description = Some(value.into_owned()),
            _ => {}
        }
    }

    if state.as_deref() != Some(expected_state) {
        return Err(OAuthError::StateMismatch);
    }
    if let Some(code) = error_code {
        return Err(OAuthError::AuthorizationError {
            code,
            description: error_description,
        });
    }

    code.filter(|value| !value.is_empty())
        .ok_or(OAuthError::MissingAuthorizationCode)
}

async fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &str,
) -> Result<(), OAuthError> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

const SUCCESS_HTML: &str = r#"<!doctype html><html><body><h1>Sign-in complete</h1><p>You can return to Taugentic.</p></body></html>"#;
const ERROR_HTML: &str = r#"<!doctype html><html><body><h1>Sign-in failed</h1><p>Return to Taugentic and try again.</p></body></html>"#;

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use super::{CallbackServerConfig, start_callback_server};
    use crate::error::OAuthError;

    #[tokio::test]
    async fn callback_receives_code() -> Result<(), Box<dyn std::error::Error>> {
        let server = start_callback_server(config_for_tests("expected", vec![0])).await?;
        let port = server.port();
        let code_task = tokio::spawn(async move { server.wait_for_code().await });

        let response = send_get(port, "/auth/callback?code=auth-code&state=expected").await?;
        let code = code_task.await??;

        assert!(response.starts_with("HTTP/1.1 200"));
        assert_eq!(code, "auth-code");
        Ok(())
    }

    #[tokio::test]
    async fn state_mismatch_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let server = start_callback_server(config_for_tests("expected", vec![0])).await?;
        let port = server.port();
        let code_task = tokio::spawn(async move { server.wait_for_code().await });

        let response = send_get(port, "/auth/callback?code=auth-code&state=wrong").await?;
        let result = code_task.await?;

        assert!(response.starts_with("HTTP/1.1 400"));
        assert!(matches!(result, Err(OAuthError::StateMismatch)));
        Ok(())
    }

    #[tokio::test]
    async fn timeout_returns_error() -> Result<(), Box<dyn std::error::Error>> {
        let server = start_callback_server(CallbackServerConfig {
            expected_state: "expected".to_string(),
            ports: vec![0],
            timeout: Duration::from_millis(20),
            redirect_uri_template: "http://localhost:{port}/auth/callback".to_string(),
        })
        .await?;

        let result = server.wait_for_code().await;

        assert!(matches!(result, Err(OAuthError::CallbackTimeout)));
        Ok(())
    }

    #[tokio::test]
    async fn port_in_use_returns_error() -> Result<(), Box<dyn std::error::Error>> {
        let occupied = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).await?;
        let port = occupied.local_addr()?.port();

        let result = start_callback_server(config_for_tests("expected", vec![port])).await;

        assert!(matches!(result, Err(OAuthError::CallbackBindFailed { .. })));
        Ok(())
    }

    fn config_for_tests(expected_state: &str, ports: Vec<u16>) -> CallbackServerConfig {
        CallbackServerConfig {
            expected_state: expected_state.to_string(),
            ports,
            timeout: Duration::from_secs(5),
            redirect_uri_template: "http://localhost:{port}/auth/callback".to_string(),
        }
    }

    async fn send_get(port: u16, target: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut stream = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).await?;
        let request =
            format!("GET {target} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await?;

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;
        Ok(String::from_utf8(response)?)
    }
}
