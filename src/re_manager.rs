use alloy_json_rpc::Response;
use bytes::Bytes;

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

//TODO: improve naming
pub trait Manager {
    //TODO: Maybe send should also work with JSON instead of bytes?
    fn send(&self) -> Option<Bytes>;
    fn recv(&self, b: Response) -> anyhow::Result<()>;
}
