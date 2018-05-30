use bytes::BytesMut;
use http::{HeaderMap, Method};

use proto::{MessageHead, BodyLength};

pub(crate) use self::conn::Conn;
pub(crate) use self::dispatch::Dispatcher;
pub use self::decode::Decoder;
pub use self::encode::{EncodedBuf, Encoder};
pub use self::io::Cursor; //TODO: move out of h1::io
pub use self::io::MINIMUM_MAX_BUFFER_SIZE;

mod conn;
mod date;
mod decode;
pub(crate) mod dispatch;
mod encode;
mod io;
mod role;


pub(crate) type ServerTransaction = self::role::Server<self::role::YesUpgrades>;
//pub type ServerTransaction = self::role::Server<self::role::NoUpgrades>;
//pub type ServerUpgradeTransaction = self::role::Server<self::role::YesUpgrades>;

pub(crate) type ClientTransaction = self::role::Client<self::role::NoUpgrades>;
pub(crate) type ClientUpgradeTransaction = self::role::Client<self::role::YesUpgrades>;

pub(crate) trait Http1Transaction {
    type Incoming;
    type Outgoing: Default;
    fn parse(bytes: &mut BytesMut, ctx: ParseContext) -> ParseResult<Self::Incoming>;
    fn encode(enc: Encode<Self::Outgoing>, dst: &mut Vec<u8>) -> ::Result<Encoder>;

    fn on_error(err: &::Error) -> Option<MessageHead<Self::Outgoing>>;

    fn should_error_on_parse_eof() -> bool;
    fn should_read_first() -> bool;

    fn update_date() {}
}

pub(crate) type ParseResult<T> = Result<Option<ParsedMessage<T>>, ::error::Parse>;

#[derive(Debug)]
pub(crate) struct ParsedMessage<T> {
    head: MessageHead<T>,
    decode: Decode,
    expect_continue: bool,
    keep_alive: bool,
}

pub(crate) struct ParseContext<'a> {
    cached_headers: &'a mut Option<HeaderMap>,
    req_method: &'a mut Option<Method>,
}

/// Passed to Http1Transaction::encode
pub(crate) struct Encode<'a, T: 'a> {
    head: &'a mut MessageHead<T>,
    body: Option<BodyLength>,
    keep_alive: bool,
    req_method: &'a mut Option<Method>,
    title_case_headers: bool,
}

#[derive(Debug, PartialEq)]
pub enum Decode {
    /// Decode normally.
    Normal(Decoder),
    /// After this decoder is done, HTTP is done.
    Final(Decoder),
    /// A header block that should be ignored, like unknown 1xx responses.
    Ignore,
}
