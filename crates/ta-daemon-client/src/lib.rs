mod credential_store;
mod persistent;

use ta_jsonrpc::{
    ClientConfig, JsonRpcClient, JsonRpcClientError, SocketAddress, resolve_local_endpoint_name,
};
use ta_protocol::wire::{
    DAEMON_DEFAULT_SOCKET_NAME, DAEMON_SOCKET_NAME_ENV_VAR, DaemonControlStatusResult,
    DaemonStatusParams, DaemonStatusResult, METHOD_DAEMON_CONTROL_STATUS, METHOD_DAEMON_STATUS,
};

use crate::credential_store::{
    load_client_credential, remove_client_session_authorities, store_client_credential,
};

const DEFAULT_CLIENT_NAME: &str = "ta-cli";

pub use persistent::PersistentDaemonClient;

#[derive(Debug, Clone)]
pub struct DaemonClient {
    inner: JsonRpcClient,
}

impl DaemonClient {
    pub fn new(socket_override: Option<&str>) -> Self {
        Self::with_client_name(DEFAULT_CLIENT_NAME, socket_override)
    }

    pub fn with_client_name(client_name: &str, socket_override: Option<&str>) -> Self {
        let resolved_socket_name = resolved_socket_name(
            socket_override,
            &resolve_local_endpoint_name(DAEMON_DEFAULT_SOCKET_NAME, DAEMON_SOCKET_NAME_ENV_VAR),
        );
        Self {
            inner: JsonRpcClient::local_default(client_name, &resolved_socket_name),
        }
    }

    pub fn status(&self) -> Result<DaemonStatusResult, JsonRpcClientError> {
        self.inner
            .call(METHOD_DAEMON_STATUS, &DaemonStatusParams {})
    }

    pub fn control_status(&self) -> Result<DaemonControlStatusResult, JsonRpcClientError> {
        self.inner
            .call(METHOD_DAEMON_CONTROL_STATUS, &DaemonStatusParams {})
    }

    pub fn connect_persistent(
        &self,
        client_name: &str,
        client_version: &str,
    ) -> Result<PersistentDaemonClient, JsonRpcClientError> {
        let mut client =
            PersistentDaemonClient::connect(self.inner.config().clone(), client_name.to_string())?;
        let stored_credential = load_client_credential(self.inner.config(), client_name);
        let initialize =
            client.initialize(client_name, client_version, stored_credential.clone())?;
        if stored_credential.as_deref() != Some(initialize.client_credential.as_str()) {
            remove_client_session_authorities(self.inner.config(), client_name)?;
        }
        store_client_credential(
            self.inner.config(),
            client_name,
            &initialize.client_credential,
        )?;
        Ok(client)
    }

    pub fn socket_address(&self) -> &SocketAddress {
        &self.inner.config().socket_address
    }

    pub fn config(&self) -> &ClientConfig {
        self.inner.config()
    }

    pub fn socket_display_string(&self) -> String {
        self.socket_address().to_string()
    }

    pub fn socket_name(&self) -> Option<String> {
        match self.socket_address() {
            SocketAddress::Unix(path) => path
                .file_stem()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned),
            SocketAddress::NamedPipe(name) => Some(name.clone()),
        }
    }
}

fn resolved_socket_name(socket_override: Option<&str>, resolved_default: &str) -> String {
    socket_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| resolved_default.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::thread;

    use ta_jsonrpc::{
        JsonLineCodec, JsonRpcClientError, JsonRpcMessage, JsonRpcResponse, bind_listener,
    };
    use ta_protocol::wire::{
        DAEMON_PROTOCOL_VERSION, DaemonInitializeResult, DaemonServerCapabilities,
        SessionAuthority, SessionId,
    };

    use super::{DaemonClient, resolved_socket_name};
    use crate::credential_store::{
        load_client_credential, load_session_authority, store_client_credential,
        store_session_authority,
    };

    #[test]
    fn prefers_non_empty_cli_socket_override() {
        let socket_name = resolved_socket_name(Some("ta-daemon-cli"), "ta-daemon-env");

        assert_eq!(socket_name, "ta-daemon-cli");
    }

    #[test]
    fn falls_back_to_resolved_default_when_override_is_missing() {
        let socket_name = resolved_socket_name(None, "ta-daemon-env");

        assert_eq!(socket_name, "ta-daemon-env");
    }

    #[test]
    fn ignores_empty_cli_socket_override() {
        let socket_name = resolved_socket_name(Some("   "), "ta-daemon-env");

        assert_eq!(socket_name, "ta-daemon-env");
    }

    #[test]
    fn connect_persistent_purges_all_client_session_authorities_after_credential_rotation() {
        let socket_name = format!("ta-daemon-client-init-{}", unique_id_suffix());
        let client = DaemonClient::with_client_name("ta-cli", Some(&socket_name));
        let config = client.config().clone();
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id_a = SessionId::new("session-1").expect("session id");
        let session_id_b = SessionId::new("session-2").expect("session id");
        let session_authority_a =
            SessionAuthority::new("session-authority-1session-authority-1".to_string())
                .expect("session authority");
        let session_authority_b =
            SessionAuthority::new("session-authority-2session-authority-2".to_string())
                .expect("session authority");
        store_client_credential(
            &config,
            "ta-cli",
            "credential-oldcredential-oldcredential-old",
        )
        .expect("client credential should persist");
        store_session_authority(&config, "ta-cli", &session_id_a, &session_authority_a)
            .expect("first session authority should persist");
        store_session_authority(&config, "ta-cli", &session_id_b, &session_authority_b)
            .expect("session authority should persist");

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("listener should accept");
            let mut reader = BufReader::new(&mut stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("request should read");
            let request = match JsonLineCodec
                .decode_message(&request_line)
                .expect("request should decode")
            {
                JsonRpcMessage::Request(request) => request,
                other => panic!("expected request, got {other:?}"),
            };
            assert_eq!(request.method, ta_protocol::wire::METHOD_DAEMON_INITIALIZE);
            let response_line = JsonLineCodec
                .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                    request.id,
                    serde_json::to_value(DaemonInitializeResult {
                        daemon_instance_id: "daemon-1".to_string(),
                        daemon_version: "0.0.1".to_string(),
                        client_credential: "credential-newcredential-newcredential-new".to_string(),
                        protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                        capabilities: DaemonServerCapabilities {
                            notifications: true,
                            event_subscriptions: true,
                        },
                    })
                    .expect("result should serialize"),
                )))
                .expect("response should encode");
            reader
                .get_mut()
                .write_all(response_line.as_bytes())
                .expect("response should write");
            reader.get_mut().flush().expect("response should flush");
        });

        let _persistent = client
            .connect_persistent("ta-cli", "0.0.1")
            .expect("connect_persistent should succeed");

        assert_eq!(
            load_client_credential(&config, "ta-cli").as_deref(),
            Some("credential-newcredential-newcredential-new")
        );
        assert_eq!(
            load_session_authority(&config, "ta-cli", &session_id_a),
            None
        );
        assert_eq!(
            load_session_authority(&config, "ta-cli", &session_id_b),
            None
        );

        server.join().expect("server thread should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn connect_persistent_ignores_invalid_persisted_client_credential() {
        use std::path::PathBuf;

        let socket_name = format!("ta-daemon-client-init-invalid-{}", unique_id_suffix());
        let client = DaemonClient::with_client_name("ta-cli", Some(&socket_name));
        let config = client.config().clone();
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        store_client_credential(
            &config,
            "ta-cli",
            "credential-oldcredential-oldcredential-old",
        )
        .expect("client credential should persist");
        let credential_path = credential_path(&config, "ta-cli");
        fs::write(&credential_path, "short").expect("invalid credential should persist");

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("listener should accept");
            let mut reader = BufReader::new(&mut stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("request should read");
            let request = match JsonLineCodec
                .decode_message(&request_line)
                .expect("request should decode")
            {
                JsonRpcMessage::Request(request) => request,
                other => panic!("expected request, got {other:?}"),
            };
            assert_eq!(request.method, ta_protocol::wire::METHOD_DAEMON_INITIALIZE);
            let params = request.params.expect("initialize params should exist");
            assert!(
                params.get("clientCredential").is_none()
                    || params.get("clientCredential") == Some(&serde_json::Value::Null)
            );
            let response_line = JsonLineCodec
                .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                    request.id,
                    serde_json::to_value(DaemonInitializeResult {
                        daemon_instance_id: "daemon-1".to_string(),
                        daemon_version: "0.0.1".to_string(),
                        client_credential: "credential-newcredential-newcredential-new".to_string(),
                        protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                        capabilities: DaemonServerCapabilities {
                            notifications: true,
                            event_subscriptions: true,
                        },
                    })
                    .expect("result should serialize"),
                )))
                .expect("response should encode");
            reader
                .get_mut()
                .write_all(response_line.as_bytes())
                .expect("response should write");
            reader.get_mut().flush().expect("response should flush");
        });

        let _persistent = client
            .connect_persistent("ta-cli", "0.0.1")
            .expect("connect_persistent should succeed");

        assert_eq!(
            load_client_credential(&config, "ta-cli").as_deref(),
            Some("credential-newcredential-newcredential-new")
        );

        server.join().expect("server thread should complete");
        cleanup_socket_address(&config.socket_address);

        fn credential_path(config: &ta_jsonrpc::ClientConfig, client_name: &str) -> PathBuf {
            use sha2::{Digest, Sha256};

            let socket_name = match &config.socket_address {
                ta_jsonrpc::SocketAddress::Unix(path) => path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .expect("socket name should exist")
                    .to_string(),
                ta_jsonrpc::SocketAddress::NamedPipe(name) => name.clone(),
            };
            let base_dir = match &config.socket_address {
                ta_jsonrpc::SocketAddress::Unix(path) => path
                    .parent()
                    .expect("socket parent")
                    .join("taugentic-client-credentials"),
                ta_jsonrpc::SocketAddress::NamedPipe(_) => {
                    std::env::temp_dir().join("taugentic-client-credentials")
                }
            };
            let mut hasher = Sha256::new();
            hasher.update(client_name.trim().as_bytes());
            base_dir
                .join(socket_name)
                .join(format!("{:x}.credential", hasher.finalize()))
        }
    }

    #[test]
    fn connect_persistent_purges_orphaned_session_authorities_when_client_credential_is_missing() {
        let socket_name = format!("ta-daemon-client-init-{}", unique_id_suffix());
        let client = DaemonClient::with_client_name("ta-cli", Some(&socket_name));
        let config = client.config().clone();
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-1").expect("session id");
        let session_authority =
            SessionAuthority::new("session-authority-1session-authority-1".to_string())
                .expect("session authority");
        store_session_authority(&config, "ta-cli", &session_id, &session_authority)
            .expect("session authority should persist");

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("listener should accept");
            let mut reader = BufReader::new(&mut stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("request should read");
            let request = match JsonLineCodec
                .decode_message(&request_line)
                .expect("request should decode")
            {
                JsonRpcMessage::Request(request) => request,
                other => panic!("expected request, got {other:?}"),
            };
            assert_eq!(request.method, ta_protocol::wire::METHOD_DAEMON_INITIALIZE);
            let response_line = JsonLineCodec
                .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                    request.id,
                    serde_json::to_value(DaemonInitializeResult {
                        daemon_instance_id: "daemon-1".to_string(),
                        daemon_version: "0.0.1".to_string(),
                        client_credential: "credential-1credential-1credential-1".to_string(),
                        protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                        capabilities: DaemonServerCapabilities {
                            notifications: true,
                            event_subscriptions: true,
                        },
                    })
                    .expect("result should serialize"),
                )))
                .expect("response should encode");
            reader
                .get_mut()
                .write_all(response_line.as_bytes())
                .expect("response should write");
            reader.get_mut().flush().expect("response should flush");
        });

        let mut persistent = client
            .connect_persistent("ta-cli", "0.0.1")
            .expect("connect_persistent should succeed");

        assert_eq!(
            load_client_credential(&config, "ta-cli").as_deref(),
            Some("credential-1credential-1credential-1")
        );
        assert_eq!(load_session_authority(&config, "ta-cli", &session_id), None);

        let error = persistent
            .attach_session(session_id.clone())
            .expect_err("orphaned session authority should be purged before attach");
        assert!(
            matches!(&error, JsonRpcClientError::Read(read) if read
                .to_string()
                .contains("missing local session authority for session-1")),
            "expected missing local session authority error, got {error:?}"
        );

        server.join().expect("server thread should complete");
        cleanup_socket_address(&config.socket_address);
    }

    fn cleanup_socket_address(socket_address: &ta_jsonrpc::SocketAddress) {
        if let ta_jsonrpc::SocketAddress::Unix(path) = socket_address {
            let _ = fs::remove_file(path);
        }
    }

    fn unique_id_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    }
}
