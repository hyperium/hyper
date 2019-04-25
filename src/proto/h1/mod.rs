use bytes::BytesMut;
use http::{HeaderMap, Method};

use proto::{MessageHead, BodyLength, DecodedLength};

pub(crate) use self::conn::Conn;
pub(crate) use self::dispatch::Dispatcher;
pub use self::decode::Decoder;
pub use self::encode::{EncodedBuf, Encoder};
pub use self::io::Cursor; //TODO: move out of h1::io
pub use self::io::MINIMUM_MAX_BUFFER_SIZE;

mod conn;
pub(super) mod date;
mod decode;
pub(crate) mod dispatch;
mod encode;
mod io;
mod role;


pub(crate) type ServerTransaction = role::Server;
pub(crate) type ClientTransaction = role::Client;

pub(crate) trait Http1Transaction {
    type Incoming;
    type Outgoing: Default;
    const LOG: &'static str;
    fn parse(bytes: &mut BytesMut, ctx: ParseContext) -> ParseResult<Self::Incoming>;
    fn encode(enc: Encode<Self::Outgoing>, dst: &mut Vec<u8>) -> ::Result<Encoder>;

    fn on_error(err: &::Error) -> Option<MessageHead<Self::Outgoing>>;

    fn is_client() -> bool {
        !Self::is_server()
    }

    fn is_server() -> bool {
        !Self::is_client()
    }

    fn should_error_on_parse_eof() -> bool {
        Self::is_client()
    }

    fn should_read_first() -> bool {
        Self::is_server()
    }

    fn update_date() {}
}

/// Result newtype for Http1Transaction::parse.
pub(crate) type ParseResult<T> = Result<Option<ParsedMessage<T>>, ::error::Parse>;

#[derive(Debug)]
pub(crate) struct ParsedMessage<T> {
    head: MessageHead<T>,
    decode: DecodedLength,
    expect_continue: bool,
    keep_alive: bool,
    wants_upgrade: bool,
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

