use std::{borrow::Cow, fmt::Debug, sync::atomic::AtomicU64};

use alloy_json_rpc::{
    ErrorPayload, Request, Response, ResponsePayload, RpcSend, SerializedRequest,
};

#[derive(Default)]
pub struct RpcRequest {
    id: AtomicU64,
}

impl RpcRequest {
    pub fn make_request<P: RpcSend>(
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

    pub fn make_request_empty_params(
        &self,
        method: impl Into<Cow<'static, str>>,
    ) -> SerializedRequest {
        self.make_request(method, ())
    }

    pub fn parse_response<T>(resp: Response) -> anyhow::Result<T>
    where
        T: Debug + serde::de::DeserializeOwned,
    {
        match resp.payload {
            ResponsePayload::Success(_) => {
                if let Some(Ok(data)) = resp.try_success_as::<T>() {
                    return Ok(data);
                } else {
                    println!("Failed to deserialize success payload");
                }
            }
            ResponsePayload::Failure(_) => {
                if let Some(Ok(err_data)) = resp.try_error_as::<ErrorPayload>() {
                    println!("Error: {:?}", err_data);
                } else {
                    println!("Failed to deserialize error payload");
                }
            }
        };

        anyhow::bail!("Failed to get response")
    }
}
