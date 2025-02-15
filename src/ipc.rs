use std::{
    io::{Read, Write},
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
    ) -> (
        JoinHandle<anyhow::Result<()>>,
        JoinHandle<anyhow::Result<()>>,
    ) {
        let mut writer = self.stream.try_clone().expect("Could not clone stream");
        let _reader = self.stream;

        let ms = self.manager.clone();
        let _mr = self.manager.clone();

        //TODO: implement receiving
        let read_jh = std::thread::spawn(move || -> anyhow::Result<()> {
            //let mut buf = BytesMut::zeroed(1024);
            //let r = reader.read(&mut buf);
            Ok(())
        });

        let write_jh = std::thread::spawn(move || -> anyhow::Result<()> {
            while let Some(msg) = ms.send() {
                writer.write_all(&msg)?;
            }

            Ok(())
        });

        (read_jh, write_jh)
    }
}

pub trait Manager {
    fn send(&self) -> Option<Bytes>;
    fn recv(&self);
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
        r: crossbeam::channel::Receiver<Bytes>,
    }

    impl TestManager {
        fn new(r: crossbeam::channel::Receiver<Bytes>) -> Self {
            Self { r }
        }
    }

    impl Manager for TestManager {
        fn send(&self) -> Option<Bytes> {
            self.r.recv().ok()
        }

        fn recv(&self) {
            // No-op for now.
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

        let (send_to_ipc, receive_on_ipc) = crossbeam::channel::unbounded();
        let ipc = Ipc::try_connect(socket_path.as_path(), TestManager::new(receive_on_ipc))?;
        let (ipc_r_jh, ipc_w_jh) = ipc.start();

        send_to_ipc.send(Bytes::from_static(b"hello_1"))?;
        assert_eq!(server_rx.recv()?, Bytes::from_static(b"hello_1"));

        send_to_ipc.send(Bytes::from_static(b"hello_2"))?;
        assert_eq!(server_rx.recv()?, Bytes::from_static(b"hello_2"));

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
            while let Ok(n) = stream.read(&mut buf) {
                if n == EOF {
                    break;
                }

                tx.send(buf.split_to(n).freeze())?;
            }

            Ok(())
        });

        // Give the server a moment to start up.
        thread::sleep(std::time::Duration::from_millis(50));
        server_thread
    }
}
