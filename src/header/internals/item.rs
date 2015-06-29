use std::any::Any;
use std::any::TypeId;
use std::fmt;
use std::str::from_utf8;

use typeable::Typeable;

use super::cell::{OptCell, PtrMapCell};
use header::{Header, HeaderFormat};


#[derive(Clone)]
pub struct Item {
    raw: OptCell<Vec<Vec<u8>>>,
    typed: PtrMapCell<HeaderFormat + Send + Sync>
}

impl Item {
    #[inline]
    pub fn new_raw(data: Vec<Vec<u8>>) -> Item {
        Item {
            raw: OptCell::new(Some(data)),
            typed: PtrMapCell::new(),
        }
    }

    #[inline]
    pub fn new_typed(ty: Box<HeaderFormat + Send + Sync>) -> Item {
        let map = PtrMapCell::new();
        unsafe { map.insert((*ty).get_type(), ty); }
        Item {
            raw: OptCell::new(None),
            typed: map,
        }
    }

    #[inline]
    pub fn mut_raw(&mut self) -> &mut Vec<Vec<u8>> {
        self.typed = PtrMapCell::new();
        unsafe {
            self.raw.get_mut()
        }
    }

    pub fn raw(&self) -> &[Vec<u8>] {
        if let Some(ref raw) = *self.raw {
            return &raw[..];
        }

        let raw = vec![unsafe { self.typed.one() }.to_string().into_bytes()];
        self.raw.set(raw);

        let raw = self.raw.as_ref().unwrap();
        &raw[..]
    }

    pub fn typed<H: Header + HeaderFormat + Any>(&self) -> Option<&H> {
        let tid = TypeId::of::<H>();
        match self.typed.get(tid) {
            Some(val) => Some(val),
            None => {
                match parse::<H>(self.raw.as_ref().expect("item.raw must exist")) {
                    Ok(typed) => {
                        unsafe { self.typed.insert(tid, typed); }
                        self.typed.get(tid)
                    },
                    Err(_) => None
                }
            }
        }.map(|typed| unsafe { typed.downcast_ref_unchecked() })
    }

    pub fn typed_mut<H: Header + HeaderFormat>(&mut self) -> Option<&mut H> {
        let tid = TypeId::of::<H>();
        if self.typed.get_mut(tid).is_none() {
            match parse::<H>(self.raw.as_ref().expect("item.raw must exist")) {
                Ok(typed) => {
                    unsafe { self.typed.insert(tid, typed); }
                },
                Err(_) => ()
            }
        }
        self.typed.get_mut(tid).map(|typed| unsafe { typed.downcast_mut_unchecked() })
    }
}

#[inline]
fn parse<H: Header + HeaderFormat>(raw: &Vec<Vec<u8>>) ->
        ::Result<Box<HeaderFormat + Send + Sync>> {
    Header::parse_header(&raw[..]).map(|h: H| {
        // FIXME: Use Type ascription
        let h: Box<HeaderFormat + Send + Sync> = Box::new(h);
        h
    })
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.raw {
            Some(ref raw) => {
                for part in raw.iter() {
                    match from_utf8(&part[..]) {
                        Ok(s) => try!(f.write_str(s)),
                        Err(e) => {
                            error!("raw header value is not utf8. header={:?}, error={:?}",
                                part, e);
                            return Err(fmt::Error);
                        }
                    }
                }
                Ok(())
            },
            None => fmt::Display::fmt(&unsafe { self.typed.one() }, f)
        }
    }
}
