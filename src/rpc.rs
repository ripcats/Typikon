use crate::{WireCodec, WireError, decode_value, encode_value};
use std::future::Future;
use std::pin::Pin;

pub type RpcFuture<'a, E> = Pin<Box<dyn Future<Output = Result<Vec<u8>, E>> + Send + 'a>>;

pub trait RpcTransport {
    type Error;

    fn call<'a>(&'a self, method: &'static str, request: Vec<u8>) -> RpcFuture<'a, Self::Error>;
}

#[derive(Debug)]
pub enum RpcError<E> {
    Encode(WireError),
    Transport(E),
    Decode(WireError),
}

impl<E: std::fmt::Display> std::fmt::Display for RpcError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(error) => write!(f, "RPC request encoding failed: {error:?}"),
            Self::Transport(error) => write!(f, "RPC transport failed: {error}"),
            Self::Decode(error) => write!(f, "RPC response decoding failed: {error:?}"),
        }
    }
}

impl<E: std::fmt::Display + std::fmt::Debug> std::error::Error for RpcError<E> {}

pub struct RpcClient<T> {
    transport: T,
}

impl<T> RpcClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T: RpcTransport> RpcClient<T> {
    pub async fn call<Request, Response>(
        &self,
        method: &'static str,
        request: &Request,
    ) -> Result<Response, RpcError<T::Error>>
    where
        Request: WireCodec,
        Response: WireCodec,
    {
        let encoded = encode_value(request).map_err(RpcError::Encode)?;
        let response = self
            .transport
            .call(method, encoded)
            .await
            .map_err(RpcError::Transport)?;
        decode_value(&response).map_err(RpcError::Decode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    struct Echo;

    impl RpcTransport for Echo {
        type Error = ();

        fn call<'a>(
            &'a self,
            _method: &'static str,
            request: Vec<u8>,
        ) -> RpcFuture<'a, Self::Error> {
            Box::pin(async move { Ok(request) })
        }
    }

    struct Broken;

    impl RpcTransport for Broken {
        type Error = &'static str;

        fn call<'a>(
            &'a self,
            _method: &'static str,
            _request: Vec<u8>,
        ) -> RpcFuture<'a, Self::Error> {
            Box::pin(async { Ok(vec![0xff]) })
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        const VTABLE: RawWakerVTable = RawWakerVTable::new(
            |_| RawWaker::new(std::ptr::null(), &VTABLE),
            |_| {},
            |_| {},
            |_| {},
        );
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
        }
    }

    #[test]
    fn typed_rpc_call_round_trips_through_transport() {
        let client = RpcClient::new(Echo);
        assert_eq!(block_on(client.call::<u8, u8>("echo", &7)).unwrap(), 7);
    }

    #[test]
    fn typed_rpc_call_reports_decode_errors() {
        let client = RpcClient::new(Broken);
        let error = block_on(client.call::<u8, u16>("broken", &7)).unwrap_err();
        assert!(matches!(error, RpcError::Decode(_)));
        assert!(error.to_string().contains("response decoding failed"));
    }
}
