//! Windows Named Pipe bridge transport。

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::adapters::bridge::codec::{
    encode_request_line, encode_response_line, BridgeRequestDecoder, BridgeRequestEnvelope,
    BridgeResponseDecoder, BridgeResponseEnvelope,
};
use crate::adapters::bridge::transport::BridgeTransportError;

/// Windows Named Pipe bridge server。
pub struct WindowsNamedPipeServer {
    /// pipe 名称。
    pipe_name: String,
}

impl WindowsNamedPipeServer {
    /// 创建 Named Pipe server 描述对象。
    pub fn new(pipe_name: impl Into<String>) -> Self {
        Self {
            pipe_name: pipe_name.into(),
        }
    }

    /// 接收单个请求并返回响应。
    pub fn accept_one<F>(&self, handler: F) -> Result<(), BridgeTransportError>
    where
        F: FnOnce(BridgeRequestEnvelope) -> BridgeResponseEnvelope,
    {
        let handle = named_pipe::create_server_pipe(&self.pipe_name)?;
        named_pipe::connect_server_pipe(handle)?;
        let mut pipe = named_pipe::OwnedPipeHandle::new(handle);
        let request = read_request(&mut pipe)?;
        let response = handler(request);
        let response_line =
            encode_response_line(&response).map_err(|_| BridgeTransportError::CodecFailed)?;
        pipe.write_all(&response_line)
            .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
        Ok(())
    }
}

/// 发送 request 并读取 response。
pub fn send_request(
    location: &str,
    request: &BridgeRequestEnvelope,
    timeout: Duration,
) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
    let location = location.to_string();
    let request = request.clone();
    let (sender, receiver) = mpsc::sync_channel(1);

    thread::spawn(move || {
        let result = send_request_blocking(&location, &request);
        let _ = sender.send(result);
    });

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(BridgeTransportError::TimedOut),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(BridgeTransportError::BridgeUnavailable),
    }
}

fn send_request_blocking(
    location: &str,
    request: &BridgeRequestEnvelope,
) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
    let mut pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(location)
        .map_err(|_| BridgeTransportError::BridgeUnavailable)?;
    let request_line =
        encode_request_line(request).map_err(|_| BridgeTransportError::CodecFailed)?;
    pipe.write_all(&request_line)
        .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
    read_response(&mut pipe)
}

fn read_request<R: Read>(reader: &mut R) -> Result<BridgeRequestEnvelope, BridgeTransportError> {
    let mut decoder = BridgeRequestDecoder::new();
    let mut buffer = [0_u8; 4096];

    loop {
        let byte_count = reader
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

fn read_response<R: Read>(
    reader: &mut R,
) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError> {
    let mut decoder = BridgeResponseDecoder::new();
    let mut buffer = [0_u8; 4096];

    loop {
        let byte_count = reader
            .read(&mut buffer)
            .map_err(|error| BridgeTransportError::IoFailed(error.to_string()))?;
        if byte_count == 0 {
            return Ok(None);
        }

        let responses = decoder
            .push_bytes(&buffer[..byte_count])
            .map_err(|_| BridgeTransportError::CodecFailed)?;
        if let Some(response) = responses.into_iter().next() {
            return Ok(Some(response));
        }
    }
}

mod named_pipe {
    use std::ffi::OsStr;
    use std::io::{Read, Result as IoResult, Write};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateNamedPipeW, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    use crate::adapters::bridge::transport::BridgeTransportError;

    pub struct OwnedPipeHandle {
        handle: OwnedHandle,
    }

    impl OwnedPipeHandle {
        pub fn new(handle: HANDLE) -> Self {
            let handle = unsafe { OwnedHandle::from_raw_handle(handle as _) };
            Self { handle }
        }
    }

    impl Read for OwnedPipeHandle {
        fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
            std::fs::File::from(self.handle.try_clone()?).read(buf)
        }
    }

    impl Write for OwnedPipeHandle {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            std::fs::File::from(self.handle.try_clone()?).write(buf)
        }

        fn flush(&mut self) -> IoResult<()> {
            std::fs::File::from(self.handle.try_clone()?).flush()
        }
    }

    pub fn create_server_pipe(pipe_name: &str) -> Result<HANDLE, BridgeTransportError> {
        let pipe_name = to_wide(pipe_name);
        let handle = unsafe {
            CreateNamedPipeW(
                pipe_name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,
                8192,
                8192,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err(BridgeTransportError::BridgeUnavailable);
        }

        Ok(handle)
    }

    pub fn connect_server_pipe(handle: HANDLE) -> Result<(), BridgeTransportError> {
        let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
        if connected != 0 {
            return Ok(());
        }

        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32) {
            return Ok(());
        }

        unsafe {
            CloseHandle(handle);
        }
        Err(BridgeTransportError::IoFailed(error.to_string()))
    }

    fn to_wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }
}
