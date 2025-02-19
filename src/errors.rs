use std::fmt::Display;

use alloy_json_rpc::ErrorPayload;
use crossbeam::channel::{RecvError, RecvTimeoutError, SendError};
use thiserror::Error;

//TODO: add more context here, like did the error occur:
// 1. IPC -> upper layers (e.g. manager)
// 2. upper layers -> IPC
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Could not connect to UDS: {0}")]
    Connect(#[from] std::io::Error),
    #[error("Could not parse received bytes into JSON:{0}")]
    JsonParseErr(#[from] serde_json::error::Error),
    #[error("Send to closed channel")]
    SendToClosedChannel,
    #[error("Send to closed channel {0}")]
    ChannelReceive(#[from] RecvError),
}

impl<T> From<SendError<T>> for ConnectionError {
    fn from(_val: SendError<T>) -> Self {
        ConnectionError::SendToClosedChannel
    }
}

#[derive(Error, Debug)]
pub enum TransportError {
    #[error(transparent)]
    Connection(#[from] ConnectionError),
    #[error("Request timed out")]
    RequestTimeout(#[from] RecvTimeoutError),
}

impl<T> From<SendError<T>> for TransportError {
    fn from(err: SendError<T>) -> Self {
        ConnectionError::from(err).into()
    }
}

impl From<RecvError> for TransportError {
    fn from(err: RecvError) -> Self {
        ConnectionError::from(err).into()
    }
}

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("Transport error {0}")]
    TransportError(#[from] TransportError),
    #[error("Could not parse received response payload into JSON:{0}")]
    JsonParseErr(#[from] serde_json::error::Error),
    #[error("Received error payload from server, but it was read as success")]
    JsonErrPayloadMisinterpretedAsSuccess,
    #[error("Received success payload from server, but it was read as error")]
    JsonSuccessPayloadMisinterpretedAsError,
    #[error("Server error: {0}")]
    ServerError(ResponseErrorPayload),
}

//TODO: check if there is more concise way of doing tis
#[derive(Debug)]
pub struct ResponseErrorPayload {
    code: i64,
    message: String,
}

impl Display for ResponseErrorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "code {}, message {}", self.code, self.message)
    }
}

impl From<ErrorPayload> for ResponseErrorPayload {
    fn from(e: ErrorPayload) -> Self {
        Self {
            code: e.code,
            message: e.message.into_owned(),
        }
    }
}
