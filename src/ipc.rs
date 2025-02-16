use bytes::{Buf, BytesMut};
use std::{
    io::{Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::Path,
    thread::JoinHandle,
};

use crate::connection::Connection;

/// Indicates closing of the IPC stream
const EOF: usize = 0;

pub(crate) type IpcParallelRW = (
    JoinHandle<anyhow::Result<()>>,
    JoinHandle<anyhow::Result<()>>,
);

pub struct Ipc<T> {
    connection: T,
    stream: UnixStream,
}

impl<T> Ipc<T>
where
    T: Connection + Send + Clone + 'static,
{
    pub fn try_start(path: &Path, connection: T) -> anyhow::Result<IpcParallelRW> {
        let ipc = Self::try_connect(path, connection)?;
        ipc.start()
    }

    pub fn try_connect(path: &Path, connection: T) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(path)?;

        Ok(Self { stream, connection })
    }

    pub fn start(self) -> anyhow::Result<IpcParallelRW> {
        const INTERNAL_READ_BUF_CAPACITY: usize = 4096;

        let (mut ipc_writer, mut ipc_reader) = (self.stream.try_clone()?, self.stream);
        let (connection_w, connection_r) = (self.connection.clone(), self.connection);

        //Inspired by  alloy.rs async transport IPC implementation
        //https://github.com/alloy-rs/alloy/blob/main/crates/transport-ipc/src/lib.rs
        let read_jh = std::thread::spawn(move || -> anyhow::Result<()> {
            let mut buf = BytesMut::zeroed(INTERNAL_READ_BUF_CAPACITY);

            while let Ok(n) = ipc_reader.read(&mut buf) {
                if n == EOF {
                    break;
                }

                let mut de = serde_json::Deserializer::from_slice(&buf)
                    .into_iter::<alloy_json_rpc::Response>();
                let maybe_fully_de_json = de.next();

                // advance the buffer
                buf.advance(de.byte_offset());

                match maybe_fully_de_json {
                    Some(Ok(response)) => {
                        connection_r.recv(Some(response))?;
                    }
                    Some(Err(err)) => {
                        let unrecoverable_err = !(err.is_eof() || err.is_data());
                        if unrecoverable_err {
                            break;
                        }
                    }
                    // nothing deserialized
                    None => {}
                }
            }

            //let the manager threads know server exited
            connection_r.recv(None)?;

            // The intention of this lib is to mimic request - response pattern
            // If we cannot receive any more responses, we close IPC completely
            // Will error if socket is no longer (or never was) connected, we don't care
            let _ = ipc_reader.shutdown(Shutdown::Both);
            Ok(())
        });

        let write_jh = std::thread::spawn(move || -> anyhow::Result<()> {
            while let Ok(Some(msg)) = connection_w.send() {
                ipc_writer.write_all(&msg)?;
            }

            // The intention of this lib is to mimic request - response pattern
            // If we cannot send any more requests, we close IPC completely
            // Will error if socket is no longer(or never was) connected, we don't care
            let _ = ipc_writer.shutdown(Shutdown::Both);

            Ok(())
        });

        Ok((read_jh, write_jh))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_json_rpc::Response;
    use bytes::{Bytes, BytesMut};
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::thread;
    use tempfile::tempdir;

    #[derive(Clone)]
    struct MockConnection {
        to_send: crossbeam::channel::Receiver<Bytes>,
        to_recv: crossbeam::channel::Sender<Response>,
    }

    impl MockConnection {
        fn new(
            to_send: crossbeam::channel::Receiver<Bytes>,
            to_recv: crossbeam::channel::Sender<Response>,
        ) -> Self {
            Self { to_send, to_recv }
        }
    }

    impl Connection for MockConnection {
        fn send(&self) -> anyhow::Result<Option<Bytes>> {
            let b = self.to_send.recv()?;
            Ok(Some(b))
        }

        fn recv(&self, r: Option<Response>) -> anyhow::Result<()> {
            if let Some(r) = r {
                self.to_recv.send(r)?;
            }
            Ok(())
        }
    }

    #[test]
    fn test_ipc_send() -> anyhow::Result<()> {
        // Crates temp socket, deleted after test
        let dir = tempdir()?;
        let socket_path = dir.path().join("test_socket");

        //channels for server to communicate results
        let (server_tx, server_rx) = crossbeam::channel::unbounded();
        let server_thread = spawn_test_server(socket_path.clone(), server_tx);

        let (send_to_ipc, send_to_ipc_rx) = crossbeam::channel::unbounded();
        let (recv_from_ipc_tx, recv_from_ipc) = crossbeam::channel::unbounded();

        let ipc = Ipc::try_connect(
            socket_path.as_path(),
            MockConnection::new(send_to_ipc_rx, recv_from_ipc_tx),
        )?;
        let (ipc_r_jh, ipc_w_jh) = ipc.start()?;

        send_to_ipc.send(Bytes::from_static(b"ping_1"))?;
        assert_eq!(server_rx.recv()?, Bytes::from_static(b"ping_1"));
        assert_eq!(
            serde_json::to_string_pretty(&recv_from_ipc.recv()?)?,
            serde_json::to_string_pretty(&make_resp(1))?
        );

        send_to_ipc.send(Bytes::from_static(b"ping_2"))?;
        assert_eq!(server_rx.recv()?, Bytes::from_static(b"ping_2"));
        assert_eq!(
            serde_json::to_string_pretty(&recv_from_ipc.recv()?)?,
            serde_json::to_string_pretty(&make_resp(2))?
        );

        // closes communication channels, effectively closing IPC connection
        // unless this is done, test hangs because server thread doesn't exit
        drop(send_to_ipc);

        ipc_r_jh.join().unwrap()?;
        ipc_w_jh.join().unwrap()?;
        server_thread.join().unwrap()?;

        Ok(())
    }

    fn spawn_test_server(
        socket_path: PathBuf,
        tx: crossbeam::channel::Sender<Bytes>,
    ) -> thread::JoinHandle<anyhow::Result<()>> {
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
                if n == EOF {
                    break;
                }

                tx.send(buf.split_to(n).freeze())?;
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
}
