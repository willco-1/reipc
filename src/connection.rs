use alloy_json_rpc::Response;
use bytes::Bytes;
use crossbeam::channel::{Receiver, Sender};

/// Connection to IPC. It allows us to send to and receive from IPC
pub trait Connection {
    //TODO: Maybe send should also work with JSON instead of bytes?
    fn send(&self) -> anyhow::Result<Bytes>;
    fn recv(&self, r: Response) -> anyhow::Result<()>;
}

/// Used by underlying IPC implementation to communicate with Manager
/// It mirrors IpcConnectionHandle
#[derive(Clone)]
pub struct IpcConnection {
    to_send: Receiver<Bytes>,
    to_recv: Sender<Response>,
}

/// Used by Manager to communicate with IPC
/// It mirrors IpcConnection
#[derive(Clone)]
pub struct IpcConnectionHandle {
    to_send: Sender<Bytes>,
    to_recv: Receiver<Response>,
}

impl IpcConnection {
    pub fn new() -> (Self, IpcConnectionHandle) {
        //send_to_ipc is used by Manager to send request to IPC
        //send_to_ipc_rx is how IPC will receive this request
        //(to actually send it to IPC)
        let (send_to_ipc, send_to_ipc_rx) = crossbeam::channel::unbounded();
        // recv_from_ipc used by Manager to receive response from IPC
        // recv_from_ipc_tx is how IPC will send response to Manager once it receives it
        let (recv_from_ipc_tx, recv_from_ipc) = crossbeam::channel::unbounded();

        let ipc_connection_handle = IpcConnectionHandle {
            to_send: send_to_ipc,
            to_recv: recv_from_ipc,
        };

        let ipc_connection = Self {
            to_send: send_to_ipc_rx,
            to_recv: recv_from_ipc_tx,
        };

        (ipc_connection, ipc_connection_handle)
    }
}

impl Connection for IpcConnection {
    fn send(&self) -> anyhow::Result<Bytes> {
        let b = self.to_send.recv()?;
        Ok(b)
    }

    fn recv(&self, r: Response) -> anyhow::Result<()> {
        self.to_recv.send(r)?;
        Ok(())
    }
}

impl IpcConnectionHandle {
    pub(crate) fn send(&self, b: Bytes) -> anyhow::Result<()> {
        self.to_send.send(b)?;
        Ok(())
    }

    pub(crate) fn recv(&self) -> anyhow::Result<Response> {
        let r = self.to_recv.recv()?;
        Ok(r)
    }
}
