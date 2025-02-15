use std::{
    io::{Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::Path,
    thread::JoinHandle,
};

use bytes::{Bytes, BytesMut};

/// Indicates closing of the IPC stream
const EOF: usize = 0;

pub struct Ipc<T> {
    manager: T,
    stream: UnixStream,
}

impl<T> Ipc<T>
where
    T: Manager + Send + Clone + 'static,
{
    pub fn try_connect(path: &Path, manager: T) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(path)?;

        Ok(Self { stream, manager })
    }

    pub fn start(
        self,
    ) -> anyhow::Result<(
        JoinHandle<anyhow::Result<()>>,
        JoinHandle<anyhow::Result<()>>,
    )> {
        let (mut ipc_writer, mut ipc_reader) = (self.stream.try_clone()?, self.stream);
        let (manager_reader, manager_writer) = (self.manager.clone(), self.manager.clone());

        let read_jh = std::thread::spawn(move || -> anyhow::Result<()> {
            let mut buf = BytesMut::zeroed(1024);
            while let Ok(n) = ipc_reader.read(&mut buf) {
                if n == EOF {
                    break;
                }

                manager_writer.recv(buf.split_to(n).freeze())?;
            }

            // The intention of this lib is to mimic request - response pattern
            // If we cannot receive any more responses, we close IPC completely
            // Will error if socket is not connected, we don't care
            let _ = ipc_reader.shutdown(Shutdown::Both);
            Ok(())
        });

        let write_jh = std::thread::spawn(move || -> anyhow::Result<()> {
            while let Some(msg) = manager_reader.send() {
                ipc_writer.write_all(&msg)?;
            }

            // The intention of this lib is to mimic request - response pattern
            // If we cannot send any more requests, we close IPC completely
            // Will error if socket is not connected, we don't care
            let _ = ipc_writer.shutdown(Shutdown::Both);

            Ok(())
        });

        Ok((read_jh, write_jh))
    }
}

pub trait Manager {
    fn send(&self) -> Option<Bytes>;
    fn recv(&self, b: Bytes) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{Bytes, BytesMut};
    use pretty_assertions::assert_eq;
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::thread;
    use tempfile::tempdir;

    #[derive(Clone)]
    struct TestManager {
        to_send: crossbeam::channel::Receiver<Bytes>,
        to_recv: crossbeam::channel::Sender<Bytes>,
    }

    impl TestManager {
        fn new(
            to_send: crossbeam::channel::Receiver<Bytes>,
            to_recv: crossbeam::channel::Sender<Bytes>,
        ) -> Self {
            Self { to_send, to_recv }
        }
    }

    impl Manager for TestManager {
        fn send(&self) -> Option<Bytes> {
            self.to_send.recv().ok()
        }

        fn recv(&self, b: Bytes) -> anyhow::Result<()> {
            self.to_recv.send(b)?;
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
            TestManager::new(send_to_ipc_rx, recv_from_ipc_tx),
        )?;
        let (ipc_r_jh, ipc_w_jh) = ipc.start()?;

        send_to_ipc.send(Bytes::from_static(b"ping_1"))?;
        assert_eq!(server_rx.recv()?, Bytes::from_static(b"ping_1"));
        assert_eq!(recv_from_ipc.recv()?, Bytes::from_static(b"pong_1"));

        send_to_ipc.send(Bytes::from_static(b"ping_2"))?;
        assert_eq!(server_rx.recv()?, Bytes::from_static(b"ping_2"));
        assert_eq!(recv_from_ipc.recv()?, Bytes::from_static(b"pong_2"));

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
                let msg = format!("pong_{msg_count}");
                stream.write_all(&Bytes::from(msg))?;
            }

            Ok(())
        });

        // Give the server a moment to start up.
        thread::sleep(std::time::Duration::from_millis(50));
        server_thread
    }
}
