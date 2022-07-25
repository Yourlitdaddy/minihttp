//#![warn(missing_docs)]

pub mod config;
pub mod request;
pub(crate) mod response;
pub mod thread_pool;

#[cfg(not(feature = "async"))]
pub mod http;

#[cfg(feature = "async")]
pub mod async_http;

pub mod http2;
