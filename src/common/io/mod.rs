mod rewind;

pub(crate) use self::rewind::Rewind;
pub(crate) const MAX_WRITEV_BUFS: usize = 64;
