use std::io;

use futures::Poll;
use http::IoBuf;

#[derive(Debug)]
pub struct Txn {
    reading: Reading,
    writing: Writing,
}

impl Txn {
    pub fn read<T>(&mut self, io: &mut IoBuf<T>) -> Poll<(), io::Error> {
        match self.reading {
            Reading::Init => unimplemented!("Reading::Init"),
            Reading::Body => unimplemented!("Reading::Body"),
            Reading::Closed => unimplemented!("Reading::Closed"),
        }
    }
}

#[derive(Debug)]
enum Reading {
    Init,
    Body,
    Closed,
}

#[derive(Debug)]
enum Writing {
    Init,
    Closed,
}
