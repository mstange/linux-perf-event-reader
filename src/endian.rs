/// An enum for little or big endian.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    LittleEndian,
    BigEndian,
}

impl Endianness {
    #[cfg(target_endian = "little")]
    pub const NATIVE: Self = Self::LittleEndian;

    #[cfg(target_endian = "big")]
    pub const NATIVE: Self = Self::BigEndian;
}
