use std::{sync::Arc, thread};

use alloy_json_rpc::{Id, Response, SerializedRequest};
use crossbeam::channel::{self, Sender};
use dashmap::DashMap;

use crate::connection::IpcConnectionHandle;

//TODO:
// This is more or less how I see this, probably missing some details, I think I got it like 80%
// right
//1. Keep track of all requests HashMap<Id, OneShotChannel>
//2. To send request we:
//  2.1 Using channel send part from Manager send data to IPC
//  2.2 Which means IPC needs to have receiver part of this channel
//  2.3 We then crate (sender, receiver) = OneShotChannel::new()
//  2.3 We insert to hashmap <Id, OneShotChannel::Sender>
//  2.4 Wait on OneShotChannel::Receiver to get resposnse
//  2.5 Return response
//  2.6 Maybe timeout, I think on timeout we just remove from the map
//3. To receive response we:
//  3.1 need to have loop which listens to new responses from IPC
//  3.2 this means that Manager holds receiver part to get data from IPC and IPC holds sender part
//    (inverse from the above)
//  3.2 when response is received we check the HashMap and try to get response by id, on success
//  3.3 se send response to one shot channel
//  It looks to me I'll need one more layer here one "real" manager  and smht. I inject to IPC
//  (currently incorrectly called Manager)

#[derive(Clone)]
pub struct ReManager {
    requests: Arc<DashMap<Id, Sender<Response>>>,
    connection: IpcConnectionHandle,

    to_send: Sender<SerializedRequest>,
}

impl ReManager {
    fn new(connection: IpcConnectionHandle, send: Sender<SerializedRequest>) -> Self {
        Self {
            connection,
            to_send: send,
            requests: Arc::new(DashMap::new()),
        }
    }

    pub(crate) fn start(connection: IpcConnectionHandle) -> Self {
        let (sender, receiver) = channel::unbounded();
        let manager = ReManager::new(connection, sender);

        let (rec, send) = (manager.clone(), manager.clone());

        //TODO: this needs better handling I ended up heare because ReManager nees new to_send
        let rec_jh = thread::spawn(move || -> anyhow::Result<()> { rec.receive_loop() });
        let send_jh = thread::spawn(move || -> anyhow::Result<()> { send.send_loop(receiver) });

        manager
    }

    pub(crate) fn send(&self, req: SerializedRequest) -> anyhow::Result<Response> {
        let (s, r) = channel::bounded::<Response>(1);
        let id = req.id().clone();

        self.to_send.send(req)?;
        // Only insert after we are sure that it was sent (at least) to the channel
        self.requests.insert(id, s);

        //TODO: mabye timeout?
        let r = r.recv()?;
        Ok(r)
    }

    fn send_loop(&self, to_send: channel::Receiver<SerializedRequest>) -> anyhow::Result<()> {
        while let Ok(req) = to_send.recv() {
            let req = req.serialized().get().to_owned().into();
            self.connection.send(req)?;
        }

        Ok(())
    }
    fn receive_loop(&self) -> anyhow::Result<()> {
        while let Ok(r) = self.connection.recv() {
            if let Some((_, pending_req)) = self.requests.remove(&r.id) {
                pending_req.send(r)?;
            }
        }

        Ok(())
    }
}
