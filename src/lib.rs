use std::{path::Path, thread::JoinHandle, time::Duration};

use alloy_json_rpc::{Response, SerializedRequest};
use connection::IpcConnection;
use ipc::{Ipc, IpcParallelRW};
use re_manager::ReManager;

pub mod connection;
pub mod ipc;
pub mod re_manager;

pub struct ReIPC {
    manager: ReManager,
    ipc_rw: IpcParallelRW,
    sjh: JoinHandle<anyhow::Result<()>>,
    rjh: JoinHandle<anyhow::Result<()>>,
}

impl ReIPC {
    pub fn try_connect(path: &Path) -> anyhow::Result<ReIPC> {
        let (connection, connection_handle) = IpcConnection::new();
        let ipc_rw = Ipc::try_start(path, connection)?;
        let (manager, rjh, sjh) = ReManager::start(connection_handle);

        //TODO: this is FUGLY fix it
        Ok(Self {
            manager,
            ipc_rw,
            sjh,
            rjh,
        })
    }

    pub fn call(&self, req: SerializedRequest) -> anyhow::Result<Response> {
        let resp = self.manager.send(req)?;
        Ok(resp)
    }

    pub fn call_with_timeout(
        &self,
        req: SerializedRequest,
        timeout: Duration,
    ) -> anyhow::Result<Response> {
        let resp = self.manager.send_with_timeout(req, timeout)?;
        Ok(resp)
    }

    fn close(self) -> anyhow::Result<()> {
        self.manager.close();

        //TODO: this is FUGLY fix it
        self.ipc_rw.0.join().unwrap()?;
        self.ipc_rw.1.join().unwrap()?;

        self.sjh.join().unwrap()?;
        self.rjh.join().unwrap()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_json_rpc::{Request, Response};
    use bytes::{Bytes, BytesMut};
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn test_reipc() -> anyhow::Result<()> {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_socket_reipc");
        let server_jh = spawn_test_server(path.clone());
        let ipc = ReIPC::try_connect(&path)?;

        let resp = ipc.call(make_req(1))?;
        assert_json_resp(&resp, &make_resp(1))?;

        // NOTE: the server is currently stupid so IDs must be sequential
        let resp = ipc.call(make_req(2))?;
        assert_json_resp(&resp, &make_resp(2))?;

        //NOTE: we can add some receive timeout to "oneshot" channel
        // then we can handle server not responding
        let resp = ipc.call_with_timeout(make_req(4), Duration::from_millis(10));
        assert!(resp.is_err());

        ipc.close()?;
        server_jh.join().unwrap()?;
        Ok(())
    }

    fn spawn_test_server(socket_path: PathBuf) -> thread::JoinHandle<anyhow::Result<()>> {
        let server_thread = thread::spawn(move || -> anyhow::Result<()> {
            let listener = UnixListener::bind(&socket_path)?;
            let mut stream = listener
                .incoming()
                .next()
                .ok_or(anyhow::anyhow!("Empty stream"))??;

            let mut buf = BytesMut::zeroed(1024);
            let mut msg_count = 0;
            while let Ok(n) = stream.read(&mut buf) {
                msg_count += 1;
                if n == 0 {
                    break;
                }

                let b = serde_json::to_vec(&make_resp(msg_count))?;
                stream.write_all(&b)?;
            }

            Ok(())
        });

        // Give the server a moment to start up.
        thread::sleep(std::time::Duration::from_millis(50));
        server_thread
    }

    fn make_resp(id: usize) -> Response {
        let response = json!({
            "jsonrpc": "2.0",
            "result": "pong",
            "id": id
        })
        .to_string();

        serde_json::from_str(&response).unwrap()
    }

    fn make_req(id: usize) -> SerializedRequest {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "ping",
            "id": id
        })
        .to_string();

        let req: Request<()> = serde_json::from_str(&request).unwrap();
        req.try_into().unwrap()
    }

    fn assert_json_resp(r1: &Response, r2: &Response) -> anyhow::Result<()> {
        assert_eq!(
            serde_json::to_string_pretty(r1)?,
            serde_json::to_string_pretty(r2)?
        );

        Ok(())
    }
}
