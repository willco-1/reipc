use std::{
    borrow::Cow,
    fmt::Debug,
    ops::Deref,
    path::Path,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use alloy_json_rpc::{
    ErrorPayload, Request, Response, ResponsePayload, RpcSend, SerializedRequest,
};

use crate::{errors::RpcError, ipc_transport::ReIPC};

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
    ) -> Result<Self, RpcError> {
        let ipc = ReIPC::try_connect(path)?;

        let rpc_provider = RpcProviderInner {
            ipc,
            default_request_timeout,
            id: Default::default(),
        };

        Ok(Self(Arc::new(rpc_provider)))
    }

    pub fn close(&self) -> Result<(), RpcError> {
        let _ = self.ipc.close()?;
        Ok(())
    }

    pub fn call<ReqParams, Resp>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: ReqParams,
    ) -> Result<Resp, RpcError>
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

    pub fn call_no_params<Resp>(
        &self,
        method: impl Into<Cow<'static, str>>,
    ) -> Result<Resp, RpcError>
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

    pub fn parse_response<T>(resp: Response) -> Result<T, RpcError>
    where
        T: Debug + serde::de::DeserializeOwned,
    {
        match resp.payload {
            ResponsePayload::Success(_) => {
                match resp.try_success_as::<T>() {
                    Some(Ok(r)) => Ok(r),
                    Some(Err(e)) => Err(e.into()),
                    // the response was received successfully, but it contains Err payload
                    // we shouldn't have  ended up here
                    None => Err(RpcError::JsonErrPayloadMisinterpretedAsSuccess),
                }
            }
            ResponsePayload::Failure(_) => {
                match resp.try_error_as::<ErrorPayload>() {
                    Some(Ok(err_payload)) => Err(RpcError::ServerError(err_payload.into())),
                    Some(Err(e)) => Err(e.into()),
                    // the response was received successfully, but it contains Success payload
                    // we shouldn't have ended up here
                    None => Err(RpcError::JsonSuccessPayloadMisinterpretedAsError),
                }
            }
        }
    }
}

impl Deref for RpcProvider {
    type Target = RpcProviderInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
