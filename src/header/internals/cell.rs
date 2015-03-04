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
    fn deref<'a>(&'a self) -> &'a Option<T> {
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
        let mut map = unsafe { &mut *self.0.get() };
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
        let mut map = &mut *self.0.get();
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
