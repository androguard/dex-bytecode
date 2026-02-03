use crate::error::DexError;

pub struct Decoder<'a>
where
    Self: Send + Sync,
{
    // Current RIP value
    ip: u64,
    // Input data provided by the user. When there's no more bytes left to read we'll return a NoMoreBytes error
    data: &'a [u8],
}

impl<'a> Decoder<'a> {
    pub fn with_ip(data: &'a [u8], ip: u64, options: u32) -> Decoder<'a> {
        Decoder::try_with_ip(data, ip, options).unwrap()
    }

    pub fn try_with_ip(data: &'a [u8], ip: u64, options: u32) -> Result<Decoder<'a>, DexError> {
        Ok(Decoder { ip, data })
    }

    #[inline]
    pub const fn ip(&self) -> u64 {
        self.ip
    }
}
