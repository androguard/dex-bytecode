use alloc::borrow::Cow;
use core::fmt;

use std::error::Error;

#[derive(Debug, Clone)]

pub struct DexError {
    error: Cow<'static, str>,
}

impl Error for DexError {}

impl fmt::Display for DexError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.error)
    }
}
