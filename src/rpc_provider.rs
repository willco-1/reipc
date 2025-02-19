use std::{
    borrow::Cow,
    fmt::Debug,
    ops::Deref,
    path::Path,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use alloy_json_rpc::{Request, Response, ResponsePayload, RpcSend, SerializedRequest};

use crate::ipc_transport::ReIPC;

#[derive(Clone)]
pub struct RpcProvider(Arc<RpcProviderInner>);

pub struct RpcProviderInner {
    id: AtomicU64,
    ipc: ReIPC,
    default_request_timeout: Option<Duration>,
}

impl RpcProvider {
    pub fn try_connect(
        path: &Path,
        default_request_timeout: Option<Duration>,
    ) -> anyhow::Result<Self> {
        let ipc = ReIPC::try_connect(path)?;

        let rpc_provider = RpcProviderInner {
            ipc,
            default_request_timeout,
            id: Default::default(),
        };

        Ok(Self(Arc::new(rpc_provider)))
    }

    pub fn close(&self) -> anyhow::Result<()> {
        self.ipc.close()
    }

    pub fn call<ReqParams, Resp>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: ReqParams,
    ) -> anyhow::Result<Resp>
    where
        ReqParams: RpcSend,
        Resp: Debug + serde::de::DeserializeOwned,
    {
        let req = self.make_request(method, params);
        let resp = match self.default_request_timeout {
            Some(d) => self.ipc.call_with_timeout(req, d)?,
            None => self.ipc.call(req)?,
        };

        RpcProvider::parse_response(resp)
    }

    pub fn call_no_params<Resp>(&self, method: impl Into<Cow<'static, str>>) -> anyhow::Result<Resp>
    where
        Resp: Debug + serde::de::DeserializeOwned,
    {
        self.call(method, ())
    }

    fn make_request<P: RpcSend>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: P,
    ) -> SerializedRequest {
        // Relaxed is ok, because we are not syncronizing anything,
        // atomicitiy&fetch_add guarantees that numbers will be in stricly increasing order
        // thusly unique
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let req = Request::new(method, id.into(), params);
        req.try_into().unwrap()
    }

    fn parse_response<T>(resp: Response) -> anyhow::Result<T>
    where
        T: Debug + serde::de::DeserializeOwned,
    {
        if let ResponsePayload::Success(_) = resp.payload {
            if let Some(Ok(data)) = resp.try_success_as::<T>() {
                return Ok(data);
            } else {
                //TODO: add tracing crate
                //println!("Failed to deserialize success payload");
            }
        };

        anyhow::bail!("Failed to get response")
    }
}

impl Deref for RpcProvider {
    type Target = RpcProviderInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
