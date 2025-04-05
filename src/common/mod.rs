#[cfg(any(http1_client, http1_server))]
pub(crate) mod buf;
#[cfg(http_server)]
pub(crate) mod date;
pub(crate) mod io;
#[cfg(any(http1_client, http1_server))]
pub(crate) mod task;
#[cfg(any(http_server, http2_client))]
pub(crate) mod time;
#[cfg(any(http1_client, http1_server))]
pub(crate) mod watch;
