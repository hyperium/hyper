#[derive(Clone)]
pub struct VecMap<K, V> {
    vec: Vec<(K, V)>,
}

impl<K: PartialEq, V> VecMap<K, V> {
    pub fn new() -> VecMap<K, V> {
        VecMap {
            vec: Vec::new()
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        match self.find(&key) {
            Some(pos) => self.vec[pos] = (key, value),
            None => self.vec.push((key, value))
        }
    }

    pub fn entry(&mut self, key: K) -> Entry<K, V> {
        match self.find(&key) {
            Some(pos) => Entry::Occupied(OccupiedEntry {
                vec: self,
                pos: pos,
            }),
            None => Entry::Vacant(VacantEntry {
                vec: self,
                key: key,
            })
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.find(key).map(move |pos| &self.vec[pos].1)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.find(key).map(move |pos| &mut self.vec[pos].1)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.find(key).is_some()
    }

    pub fn len(&self) -> usize { self.vec.len() }
    pub fn iter(&self) -> ::std::slice::Iter<(K, V)> {
        self.vec.iter()
    }
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.find(key).map(|pos| self.vec.remove(pos)).map(|(_, v)| v)
    }
    pub fn clear(&mut self) {
        self.vec.clear();
    }

    fn find(&self, key: &K) -> Option<usize> {
        self.vec.iter().position(|entry| key == &entry.0)
    }
}

pub enum Entry<'a, K: 'a, V: 'a> {
    Vacant(VacantEntry<'a, K, V>),
    Occupied(OccupiedEntry<'a, K, V>)
}

pub struct VacantEntry<'a, K: 'a, V: 'a> {
    vec: &'a mut VecMap<K, V>,
    key: K,
}

impl<'a, K, V> VacantEntry<'a, K, V> {
    pub fn insert(self, val: V) -> &'a mut V {
        let vec = self.vec;
        vec.vec.push((self.key, val));
        let pos = vec.vec.len() - 1;
        &mut vec.vec[pos].1
    }
}

pub struct OccupiedEntry<'a, K: 'a, V: 'a> {
    vec: &'a mut VecMap<K, V>,
    pos: usize,
}

impl<'a, K, V> OccupiedEntry<'a, K, V> {
    pub fn into_mut(self) -> &'a mut V {
        &mut self.vec.vec[self.pos].1
    }
}
