use std::collections::HashMap;
use std::collections::hash_map::Entry;

use super::TokioClient;

pub struct Pool {
    clients: HashMap<String, Vec<TokioClient>>,
}

impl Pool {
    pub fn new() -> Pool {
        Pool {
            clients: HashMap::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&TokioClient> {
        self.clients.find(key).map(|list| &list[0])
    }

    pub fn put(&mut self, key: String, client: TokioClient) {
        self.clients.entry(key)
            .or_insert(Vec::new())
            .push(client);
    }
}
