use std::{
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};

use alloy_json_rpc::{Id, Response, SerializedRequest};
use crossbeam::channel::{self, Sender};
use dashmap::DashMap;

use crate::{connection::IpcConnectionHandle, errors::TransportError};

#[derive(Clone, Debug)]
pub(crate) struct ReManager {
    requests: Arc<DashMap<Id, Sender<Response>>>,
    connection: IpcConnectionHandle,

    to_send: Sender<Option<SerializedRequest>>,
}

impl ReManager {
    fn new(connection: IpcConnectionHandle, send: Sender<Option<SerializedRequest>>) -> Self {
        Self {
            connection,
            to_send: send,
            requests: Arc::new(DashMap::new()),
        }
    }

    pub(crate) fn close(&self) {
        self.connection.close();
        let _ = self.to_send.send(None);
    }

    pub(crate) fn start(
        connection: IpcConnectionHandle,
    ) -> (
        Self,
        JoinHandle<Result<(), TransportError>>,
        JoinHandle<Result<(), TransportError>>,
    ) {
        let (sender, receiver) = channel::unbounded();
        let manager = ReManager::new(connection, sender);

        let (rec, send) = (manager.clone(), manager.clone());

        //TODO: this needs better handling I ended up heare because ReManager needs new to_send
        let rec_jh = thread::spawn(move || -> Result<(), TransportError> {
            rec.receive_loop()?;
            Ok(())
        });

        let send_jh = thread::spawn(move || -> Result<(), TransportError> {
            send.send_loop(receiver)?;
            Ok(())
        });

        //TODO: this is FUGLY fix it
        (manager, rec_jh, send_jh)
    }

    pub(crate) fn send(&self, req: SerializedRequest) -> Result<Response, TransportError> {
        let (s, r) = channel::bounded::<Response>(1);
        let id = req.id().clone();

        self.to_send.send(Some(req))?;
        // Only insert after we are sure that it was sent (at least) to the channel
        self.requests.insert(id, s);

        let r = r.recv()?;
        Ok(r)
    }

    pub(crate) fn send_with_timeout(
        &self,
        req: SerializedRequest,
        timeout: Duration,
    ) -> Result<Response, TransportError> {
        let (s, r) = channel::bounded::<Response>(1);
        let id = req.id().clone();
        let del_id = req.id().clone();

        self.to_send.send(Some(req))?;
        // Only insert after we are sure that it was sent (at least) to the channel
        self.requests.insert(id, s);

        let r = match r.recv_timeout(timeout) {
            Ok(r) => r,
            Err(e) => {
                //TODO: add retry logic
                //In case of timeout drop the request
                self.requests.remove(&del_id);
                return Err(e.into());
            }
        };

        Ok(r)
    }

    fn send_loop(
        &self,
        to_send: channel::Receiver<Option<SerializedRequest>>,
    ) -> Result<(), TransportError> {
        while let Ok(Some(req)) = to_send.recv() {
            let req = req.serialized().get().to_owned().into();
            self.connection.send(Some(req))?;
        }

        Ok(())
    }
    fn receive_loop(&self) -> Result<(), TransportError> {
        while let Ok(Some(resp)) = self.connection.recv() {
            if let Some((_, pending_req)) = self.requests.remove(&resp.id) {
                pending_req.send(resp)?;
            }
        }

        self.drop_all_pending_requests();
        Ok(())
    }

    fn drop_all_pending_requests(&self) {
        // DashMap doesn't have drain, this mimics it
        // More info: https://github.com/xacrimon/dashmap/issues/141
        for k in self
            .requests
            .iter()
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>()
        {
            if let Some((_, pending_req)) = self.requests.remove(&k) {
                drop(pending_req);
            }
        }
    }
}
