//! Client Connection Pooling
use std::borrow::ToOwned;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, Shutdown};
use std::sync::{Arc, Mutex};

use mio::{Evented, Selector, Token, EventSet, PollOpt};


use http;
use net;

pub struct Pool {
    _inner: ()
}

/// Config options for the `Pool`.
#[derive(Debug)]
pub struct Config {
    /// The maximum idle connections *per host*.
    pub max_idle: usize,
    pub max_connections: usize
}

impl Default for Config {
    #[inline]
    fn default() -> Config {
        Config {
            max_idle: 5,
            max_connections: 8_192
        }
    }
}

impl Pool {
    /// Creates a `Pool` with a specified `NetworkConnector`.
    #[inline]
    pub fn new(config: Config) -> Pool {
        Pool {
            _inner: ()
        }
    }
}

type Key = (String, u16, Scheme);

fn key<T: Into<Scheme>>(host: &str, port: u16, scheme: T) -> Key {
    (host.to_owned(), port, scheme.into())
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
enum Scheme {
    Http,
    Https,
    Other(String)
}

impl<'a> From<&'a str> for Scheme {
    fn from(s: &'a str) -> Scheme {
        match s {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            s => Scheme::Other(String::from(s))
        }
    }
}

