use std::cell::UnsafeCell;
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
