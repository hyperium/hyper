#[derive(Clone)]
pub struct VecMap<K, V> {
    vec: Vec<(K, V)>,
}

impl<K: PartialEq, V> VecMap<K, V> {
    #[inline]
    pub fn with_capacity(cap: usize) -> VecMap<K, V> {
        VecMap {
            vec: Vec::with_capacity(cap)
        }
    }

    #[inline]
    pub fn insert(&mut self, key: K, value: V) {
        // not using entry or find_mut because of borrowck
        for entry in &mut self.vec {
            if key == entry.0 {
                *entry = (key, value);
                return;
            }
        }
        self.vec.push((key, value));
    }

    #[inline]
    pub fn append(&mut self, key: K, value: V) {
        self.vec.push((key, value));
    }

    #[inline]
    pub fn entry(&mut self, key: K) -> Entry<K, V> {
        match self.pos(&key) {
            Some(pos) => Entry::Occupied(OccupiedEntry {
                vec: &mut self.vec,
                pos: pos,
            }),
            None => Entry::Vacant(VacantEntry {
                vec: &mut self.vec,
                key: key,
            })
        }
    }

    #[inline]
    pub fn get<K2: PartialEq<K> + ?Sized>(&self, key: &K2) -> Option<&V> {
        self.find(key).map(|entry| &entry.1)
    }

    #[inline]
    pub fn get_mut<K2: PartialEq<K> + ?Sized>(&mut self, key: &K2) -> Option<&mut V> {
        self.find_mut(key).map(|entry| &mut entry.1)
    }

    #[inline]
    pub fn contains_key<K2: PartialEq<K> + ?Sized>(&self, key: &K2) -> bool {
        self.find(key).is_some()
    }

    #[inline]
    pub fn len(&self) -> usize { self.vec.len() }

    #[inline]
    pub fn iter(&self) -> ::std::slice::Iter<(K, V)> {
        self.vec.iter()
    }

    #[inline]
    pub fn remove<K2: PartialEq<K> + ?Sized>(&mut self, key: &K2) -> Option<V> {
        self.pos(key).map(|pos| self.vec.remove(pos)).map(|(_, v)| v)
    }

    #[inline]
    pub fn remove_all<K2: PartialEq<K> + ?Sized>(&mut self, key: &K2) {
        let len = self.vec.len();
        for i in (0..len).rev() {
            if key == &self.vec[i].0 {
                self.vec.remove(i);
            }
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.vec.clear();
    }

    #[inline]
    fn find<K2: PartialEq<K> + ?Sized>(&self, key: &K2) -> Option<&(K, V)> {
        for entry in &self.vec {
            if key == &entry.0 {
                return Some(entry);
            }
        }
        None
    }

    #[inline]
    fn find_mut<K2: PartialEq<K> + ?Sized>(&mut self, key: &K2) -> Option<&mut (K, V)> {
        for entry in &mut self.vec {
            if key == &entry.0 {
                return Some(entry);
            }
        }
        None
    }

    #[inline]
    fn pos<K2: PartialEq<K> + ?Sized>(&self, key: &K2) -> Option<usize> {
        self.vec.iter().position(|entry| key == &entry.0)
    }
}

pub enum Entry<'a, K: 'a, V: 'a> {
    Vacant(VacantEntry<'a, K, V>),
    Occupied(OccupiedEntry<'a, K, V>)
}

pub struct VacantEntry<'a, K: 'a, V: 'a> {
    vec: &'a mut Vec<(K, V)>,
    key: K,
}

impl<'a, K, V> VacantEntry<'a, K, V> {
    pub fn insert(self, val: V) -> &'a mut V {
        self.vec.push((self.key, val));
        let pos = self.vec.len() - 1;
        &mut self.vec[pos].1
    }
}

pub struct OccupiedEntry<'a, K: 'a, V: 'a> {
    vec: &'a mut Vec<(K, V)>,
    pos: usize,
}

impl<'a, K, V> OccupiedEntry<'a, K, V> {
    pub fn into_mut(self) -> &'a mut V {
        &mut self.vec[self.pos].1
    }
}
