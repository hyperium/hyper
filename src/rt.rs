//! Default runtime
//!
//! By default, hyper includes the [tokio](https://tokio.rs) runtime. To ease
//! using it, several types are re-exported here.
//!
//! The inclusion of a default runtime can be disabled by turning off hyper's
//! `runtime` Cargo feature.

pub use futures::{Future, Stream};
pub use futures::future::{lazy, poll_fn};
pub use tokio::{run, spawn};
