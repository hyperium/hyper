use std::any::{Any, TypeId};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt;
use std::mem;
use std::ops::Deref;

pub struct OptCell<T>(UnsafeCell<Option<T>>);

impl<T> OptCell<T> {
    #[inline]
    pub fn new(val: Option<T>) -> OptCell<T> {
        OptCell(UnsafeCell::new(val))
    }

    #[inline]
    pub fn set(&self, val: T) {
        unsafe {
            let opt = self.0.get();
            debug_assert!((*opt).is_none());
            *opt = Some(val)
        }
    }

    #[inline]
    pub unsafe fn get_mut(&mut self) -> &mut T {
        let opt = &mut *self.0.get();
        opt.as_mut().unwrap()
    }
}

impl<T> Deref for OptCell<T> {
    type Target = Option<T>;
    #[inline]
    fn deref(&self) -> &Option<T> {
        unsafe { &*self.0.get() }
    }
}

impl<T: Clone> Clone for OptCell<T> {
    #[inline]
    fn clone(&self) -> OptCell<T> {
        OptCell::new((**self).clone())
    }
}

pub struct PtrMapCell<V: ?Sized>(UnsafeCell<PtrMap<Box<V>>>);

#[derive(Clone, Debug)]
enum PtrMap<T> {
    Empty,
    One(TypeId, T),
    Many(HashMap<TypeId, T>)
}

impl<V: ?Sized + fmt::Debug + Any + 'static> PtrMapCell<V> {
    #[inline]
    pub fn new() -> PtrMapCell<V> {
        PtrMapCell(UnsafeCell::new(PtrMap::Empty))
    }

    #[inline]
    pub fn get(&self, key: TypeId) -> Option<&V> {
        let map = unsafe { &*self.0.get() };
        match *map {
            PtrMap::Empty => None,
            PtrMap::One(id, ref v) => if id == key {
                Some(v)
            } else {
                None
            },
            PtrMap::Many(ref hm) => hm.get(&key)
        }.map(|val| &**val)
    }

    #[inline]
    pub fn get_mut(&mut self, key: TypeId) -> Option<&mut V> {
        let map = unsafe { &mut *self.0.get() };
        match *map {
            PtrMap::Empty => None,
            PtrMap::One(id, ref mut v) => if id == key {
                Some(v)
            } else {
                None
            },
            PtrMap::Many(ref mut hm) => hm.get_mut(&key)
        }.map(|val| &mut **val)
    }

    #[inline]
    pub unsafe fn insert(&self, key: TypeId, val: Box<V>) {
        let map = &mut *self.0.get();
        match *map {
            PtrMap::Empty => *map = PtrMap::One(key, val),
            PtrMap::One(..) => {
                let one = mem::replace(map, PtrMap::Empty);
                match one {
                    PtrMap::One(id, one) => {
                        debug_assert!(id != key);
                        let mut hm = HashMap::with_capacity(2);
                        hm.insert(id, one);
                        hm.insert(key, val);
                        mem::replace(map, PtrMap::Many(hm));
                    },
                    _ => unreachable!()
                }
            },
            PtrMap::Many(ref mut hm) => { hm.insert(key, val); }
        }
    }

    #[inline]
    pub unsafe fn one(&self) -> &V {
        let map = &*self.0.get();
        match *map {
            PtrMap::One(_, ref one) => one,
            _ => panic!("not PtrMap::One value, {:?}", *map)
        }
    }
}

impl<V: ?Sized + fmt::Debug + Any + 'static> Clone for PtrMapCell<V> where Box<V>: Clone {
    #[inline]
    fn clone(&self) -> PtrMapCell<V> {
        let cell = PtrMapCell::new();
        unsafe {
            *cell.0.get() = (&*self.0.get()).clone()
        }
        cell
    }
}

#[cfg(test)]
mod test {
    use std::any::TypeId;
    use super::*;

    #[test]
    fn test_opt_cell_set() {
        let one:OptCell<u32> = OptCell::new(None);
        one.set(1);
        assert_eq!(*one,Some(1));
    }

    #[test]
    fn test_opt_cell_clone() {
        let one:OptCell<u32> = OptCell::new(Some(3));
        let stored = *one.clone();
        assert_eq!(stored,Some(3));
    }


    #[test]
    fn test_ptr_map_cell_none() {
        let type_id = TypeId::of::<u32>();
        let pm:PtrMapCell<u32> = PtrMapCell::new();
        assert_eq!(pm.get(type_id),None);
    }

    #[test]
    fn test_ptr_map_cell_one() {
        let type_id = TypeId::of::<String>();
        let pm:PtrMapCell<String> = PtrMapCell::new();
        unsafe { pm.insert(type_id, Box::new("a".to_string())); }
        assert_eq!(pm.get(type_id), Some(&"a".to_string()));
        assert_eq!(unsafe {pm.one()}, "a");
    }

    #[test]
    fn test_ptr_map_cell_two() {
        let type_id = TypeId::of::<String>();
        let type_id2 = TypeId::of::<Vec<u8>>();
        let pm:PtrMapCell<String> = PtrMapCell::new();
        unsafe { pm.insert(type_id, Box::new("a".to_string())); }
        unsafe { pm.insert(type_id2, Box::new("b".to_string())); }
        assert_eq!(pm.get(type_id), Some(&"a".to_string()));
        assert_eq!(pm.get(type_id2), Some(&"b".to_string()));
    }

    #[test]
    fn test_ptr_map_cell_many() {
        let id1 = TypeId::of::<String>();
        let id2 = TypeId::of::<Vec<u8>>();
        let id3 = TypeId::of::<OptCell<String>>();
        let pm:PtrMapCell<String> = PtrMapCell::new();
        unsafe { pm.insert(id1, Box::new("a".to_string())); }
        unsafe { pm.insert(id2, Box::new("b".to_string())); }
        unsafe { pm.insert(id3, Box::new("c".to_string())); }
        assert_eq!(pm.get(id1), Some(&"a".to_string()));
        assert_eq!(pm.get(id2), Some(&"b".to_string()));
        assert_eq!(pm.get(id3), Some(&"c".to_string()));
    }


    #[test]
    fn test_ptr_map_cell_clone() {
        let type_id = TypeId::of::<String>();
        let pm:PtrMapCell<String> = PtrMapCell::new();
        unsafe { pm.insert(type_id, Box::new("a".to_string())); }
        let cloned = pm.clone();
        assert_eq!(cloned.get(type_id), Some(&"a".to_string()));
    }

}
