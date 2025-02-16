use std::path::Path;

use alloy_json_rpc::{Request, Response, SerializedRequest};
use connection::IpcConnection;
use ipc::{Ipc, IpcParallelRW};
use re_manager::ReManager;

pub mod connection;
pub mod ipc;
pub mod re_manager;

pub struct ReIPC {
    manager: ReManager,
    ipc_rw: IpcParallelRW,
}

impl ReIPC {
    pub fn try_connect(path: &Path) -> anyhow::Result<ReIPC> {
        let (connection, connection_handle) = IpcConnection::new();
        // TODO: what about r/w threads from IPC we need to join on them somewhere
        let ipc_rw = Ipc::try_start(path, connection)?;
        let manager = ReManager::start(connection_handle);

        Ok(Self { manager, ipc_rw })
    }

    pub fn call<P>(&self, req: SerializedRequest) -> anyhow::Result<Response> {
        let resp = self.manager.send(req)?;
        Ok(resp)
    }
}
