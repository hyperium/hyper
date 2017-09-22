use std::fmt;

pub struct Omitted;

impl fmt::Debug for Omitted {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "...")
    }
}
