//! 本地 bridge 传输层。

use std::env;
use std::time::Duration;

use crate::adapters::bridge::codec::{BridgeRequestEnvelope, BridgeResponseEnvelope};

/// bridge 传输错误。
#[derive(Debug)]
pub enum BridgeTransportError {
    /// 本地 bridge 连接不可用。
    BridgeUnavailable,
    /// 读写超时。
    TimedOut,
    /// codec 失败。
    CodecFailed,
    /// IO 失败。
    IoFailed(String),
}

/// 读取当前平台默认 bridge 位置。
pub fn default_bridge_location() -> String {
    if let Ok(value) = env::var("BUILDER_PANEL_BRIDGE_PATH") {
        if !value.trim().is_empty() {
            return value;
        }
    }

    platform_default_bridge_location()
}

/// 发送 bridge request。
pub fn send_bridge_request(
    request: &BridgeRequestEnvelope,
    timeout: Duration,
) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
    send_bridge_request_to(&default_bridge_location(), request, timeout)
}

/// 向指定 bridge 位置发送 request。
pub fn send_bridge_request_to(
    location: &str,
    request: &BridgeRequestEnvelope,
    timeout: Duration,
) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
    platform_send_bridge_request_to(location, request, timeout)
}

#[cfg(unix)]
fn platform_default_bridge_location() -> String {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{home}/Library/Application Support/BuilderPanel/bridge.sock")
}

#[cfg(windows)]
fn platform_default_bridge_location() -> String {
    r"\\.\pipe\builder-panel-bridge".to_string()
}

#[cfg(unix)]
fn platform_send_bridge_request_to(
    location: &str,
    request: &BridgeRequestEnvelope,
    timeout: Duration,
) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
    unix_transport::send_request(location, request, timeout)
}

#[cfg(windows)]
fn platform_send_bridge_request_to(
    location: &str,
    request: &BridgeRequestEnvelope,
    timeout: Duration,
) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
    windows_transport::send_request(location, request, timeout)
}

#[cfg(unix)]
pub mod unix_transport {
    //! Unix Domain Socket bridge transport。

    use std::fs;
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::Duration;

    use crate::adapters::bridge::codec::{
        encode_request_line, encode_response_line, BridgeRequestDecoder, BridgeRequestEnvelope,
        BridgeResponseDecoder, BridgeResponseEnvelope,
    };
    use crate::adapters::bridge::transport::BridgeTransportError;

    /// Unix Domain Socket bridge server。
    pub struct UnixBridgeServer {
        /// socket 路径。
        socket_path: PathBuf,
        /// Unix listener。
        listener: UnixListener,
    }

    impl UnixBridgeServer {
        /// 绑定 socket 并清理旧 socket 文件。
        pub fn bind(socket_path: impl Into<PathBuf>) -> Result<Self, BridgeTransportError> {
            let socket_path = socket_path.into();
            if let Some(parent) = socket_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
            }

            remove_stale_socket(&socket_path)?;
            let listener = UnixListener::bind(&socket_path)
                .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;

            Ok(Self {
                socket_path,
                listener,
            })
        }

        /// 接收单个请求并返回响应，测试和 APP 调度均可复用。
        pub fn accept_one<F>(&self, handler: F) -> Result<(), BridgeTransportError>
        where
            F: FnOnce(BridgeRequestEnvelope) -> BridgeResponseEnvelope,
        {
            let (stream, _) = self
                .listener
                .accept()
                .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
            handle_stream(stream, handler)
        }

        /// 接收单个请求，并在线程中处理该连接。
        pub fn accept_one_on_thread<F>(&self, handler: F) -> Result<(), BridgeTransportError>
        where
            F: FnOnce(BridgeRequestEnvelope) -> BridgeResponseEnvelope + Send + 'static,
        {
            let (stream, _) = self
                .listener
                .accept()
                .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
            thread::spawn(move || {
                let _ = handle_stream(stream, handler);
            });
            Ok(())
        }
    }

    impl Drop for UnixBridgeServer {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.socket_path);
        }
    }

    /// 发送 request 并读取 response。
    pub fn send_request(
        location: &str,
        request: &BridgeRequestEnvelope,
        timeout: Duration,
    ) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
        let mut stream =
            UnixStream::connect(location).map_err(|_| BridgeTransportError::BridgeUnavailable)?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;

        let request_line =
            encode_request_line(request).map_err(|_| BridgeTransportError::CodecFailed)?;
        stream
            .write_all(&request_line)
            .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;

        read_response(&mut stream)
    }

    fn read_request(
        stream: &mut UnixStream,
    ) -> Result<BridgeRequestEnvelope, BridgeTransportError> {
        let mut decoder = BridgeRequestDecoder::new();
        let mut buffer = [0_u8; 4096];

        loop {
            let byte_count = stream
                .read(&mut buffer)
                .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
            if byte_count == 0 {
                return Err(BridgeTransportError::BridgeUnavailable);
            }

            let requests = decoder
                .push_bytes(&buffer[..byte_count])
                .map_err(|_| BridgeTransportError::CodecFailed)?;
            if let Some(request) = requests.into_iter().next() {
                return Ok(request);
            }
        }
    }

    fn handle_stream<F>(mut stream: UnixStream, handler: F) -> Result<(), BridgeTransportError>
    where
        F: FnOnce(BridgeRequestEnvelope) -> BridgeResponseEnvelope,
    {
        let request = read_request(&mut stream)?;
        let response = handler(request);
        let response_line =
            encode_response_line(&response).map_err(|_| BridgeTransportError::CodecFailed)?;
        stream
            .write_all(&response_line)
            .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
        Ok(())
    }

    fn read_response(
        stream: &mut UnixStream,
    ) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
        let mut decoder = BridgeResponseDecoder::new();
        let mut buffer = [0_u8; 4096];

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => return Ok(None),
                Ok(byte_count) => {
                    let responses = decoder
                        .push_bytes(&buffer[..byte_count])
                        .map_err(|_| BridgeTransportError::CodecFailed)?;
                    if let Some(response) = responses.into_iter().next() {
                        return Ok(Some(response));
                    }
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return Err(BridgeTransportError::TimedOut);
                }
                Err(error) => return Err(BridgeTransportError::IoFailed(error.to_string())),
            }
        }
    }

    fn remove_stale_socket(path: &Path) -> Result<(), BridgeTransportError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(BridgeTransportError::IoFailed(error.to_string())),
        }
    }
}

#[cfg(windows)]
pub mod windows_transport;

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use crate::adapters::bridge::codec::{
        BridgeHookEventName, BridgeRequestEnvelope, BridgeResponseEnvelope, ValidatedHookPayload,
    };
    use crate::adapters::bridge::transport::send_bridge_request_to;
    use crate::domain::agent_session::AgentKind;

    fn request() -> BridgeRequestEnvelope {
        request_with_id("req-transport")
    }

    fn request_with_id(request_id: &str) -> BridgeRequestEnvelope {
        BridgeRequestEnvelope::process_agent_hook(
            request_id.to_string(),
            ValidatedHookPayload {
                agent_kind: AgentKind::CodexCli,
                hook_event_name: BridgeHookEventName::SessionStart,
                cwd: "/tmp/project".to_string(),
                session_id: "session-1".to_string(),
                model: None,
                permission_mode: None,
                transcript_path: None,
                terminal_app: None,
                terminal_session_id: None,
                terminal_tty: None,
                terminal_title: None,
                turn_id: None,
                tool_name: None,
                tool_input: None,
                prompt: None,
                last_assistant_message: None,
                permission_suggestions: None,
            },
        )
    }

    #[cfg(unix)]
    #[test]
    fn unix_bridge_round_trips_one_request() {
        use crate::adapters::bridge::transport::unix_transport::UnixBridgeServer;

        let socket_path =
            std::env::temp_dir().join(format!("builder-panel-test-{}.sock", std::process::id()));
        let server = UnixBridgeServer::bind(&socket_path).expect("server should bind");
        let server_thread = thread::spawn(move || {
            server
                .accept_one(|request| BridgeResponseEnvelope::ack(request.request_id))
                .expect("server should accept one request");
        });

        let response = send_bridge_request_to(
            socket_path.to_str().expect("socket path should be utf8"),
            &request(),
            Duration::from_secs(2),
        )
        .expect("request should succeed")
        .expect("response should exist");

        server_thread.join().expect("server thread should join");
        assert_eq!(
            response,
            BridgeResponseEnvelope::ack("req-transport".to_string())
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_bridge_accepts_second_request_while_first_handler_waits() {
        use crate::adapters::bridge::transport::unix_transport::UnixBridgeServer;

        let socket_path = std::env::temp_dir().join(format!(
            "builder-panel-concurrent-test-{}.sock",
            std::process::id()
        ));
        let server = UnixBridgeServer::bind(&socket_path).expect("server should bind");
        let (first_started_sender, first_started_receiver) = mpsc::channel();
        let (release_first_sender, release_first_receiver) = mpsc::channel();
        let server_thread = thread::spawn(move || {
            server
                .accept_one_on_thread(move |request| {
                    first_started_sender
                        .send(())
                        .expect("started signal should send");
                    release_first_receiver
                        .recv()
                        .expect("release signal should arrive");
                    BridgeResponseEnvelope::ack(request.request_id)
                })
                .expect("first accept should succeed");
            server
                .accept_one_on_thread(|request| BridgeResponseEnvelope::ack(request.request_id))
                .expect("second accept should succeed");
        });
        let first_socket_path = socket_path.clone();
        let first_client = thread::spawn(move || {
            send_bridge_request_to(
                first_socket_path
                    .to_str()
                    .expect("socket path should be utf8"),
                &request_with_id("req-first"),
                Duration::from_secs(2),
            )
            .expect("first request should succeed")
            .expect("first response should exist")
        });

        first_started_receiver
            .recv()
            .expect("first handler should start");
        let second_response = send_bridge_request_to(
            socket_path.to_str().expect("socket path should be utf8"),
            &request_with_id("req-second"),
            Duration::from_secs(2),
        )
        .expect("second request should succeed")
        .expect("second response should exist");

        assert_eq!(second_response.request_id, "req-second");
        release_first_sender
            .send(())
            .expect("release signal should send");
        assert_eq!(
            first_client
                .join()
                .expect("first client should join")
                .request_id,
            "req-first"
        );
        server_thread.join().expect("server thread should join");
    }

    #[cfg(unix)]
    #[test]
    fn unix_bridge_unavailable_returns_fail_open_error() {
        let socket_path =
            std::env::temp_dir().join(format!("builder-panel-missing-{}.sock", std::process::id()));

        let result = send_bridge_request_to(
            socket_path.to_str().expect("socket path should be utf8"),
            &request(),
            Duration::from_millis(20),
        );

        assert!(result.is_err());
    }
}
