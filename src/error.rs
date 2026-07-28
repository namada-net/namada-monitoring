use thiserror::Error;
use tokio_retry2::RetryError;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum MainError {
    #[error("No action error")]
    NoAction,
    #[error("RPC error")]
    RpcError,
}

/// The chain has not produced this block yet. Expected during normal operation
/// once the monitor has caught up with the chain head, so it is retried quietly
/// instead of being reported as a failure.
#[derive(Error, Debug, PartialEq, Eq)]
#[error("Block {0} has not been produced yet")]
pub struct BlockNotAvailable(pub u64);

pub trait AsRetryError<T> {
    fn into_retry_error(self) -> Result<T, RetryError<std::io::Error>>;
}

impl<T> AsRetryError<T> for anyhow::Result<T> {
    #[inline]
    fn into_retry_error(self) -> Result<T, RetryError<std::io::Error>> {
        self.map_err(|reason| {
            let err = if reason.downcast_ref::<BlockNotAvailable>().is_some() {
                std::io::Error::new(std::io::ErrorKind::WouldBlock, reason.to_string())
            } else {
                std::io::Error::other(reason.to_string())
            };
            RetryError::Transient {
                err,
                retry_after: None,
            }
        })
    }
}
