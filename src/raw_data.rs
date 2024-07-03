use crate::utils::HexValue;
use byteorder::{ByteOrder, NativeEndian};
use std::borrow::Cow;
use std::cmp::min;
use std::ops::Range;
use std::{fmt, mem};

/// A slice of u8 data that can have non-contiguous backing storage split
/// into two pieces, and abstracts that split away so that users can pretend
/// to deal with a contiguous slice.
///
/// When reading perf events from the mmap'd fd that contains the perf event
/// stream, it often happens that a single record straddles the boundary between
/// two mmap chunks, or is wrapped from the end to the start of a chunk.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RawData<'a> {
    Single(&'a [u8]),
    Split(&'a [u8], &'a [u8]),
}

impl<'a> From<&'a Cow<'a, [u8]>> for RawData<'a> {
    fn from(data: &'a Cow<'a, [u8]>) -> Self {
        match *data {
            Cow::Owned(ref bytes) => RawData::Single(bytes.as_slice()),
            Cow::Borrowed(bytes) => RawData::Single(bytes),
        }
    }
}

impl<'a> From<&'a [u8]> for RawData<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        RawData::Single(bytes)
    }
}

/// A helper which prints out byte slices but limits the output to 20 elements.
struct DisplayableSlice<'a>(&'a [u8]);

impl<'a> fmt::Display for DisplayableSlice<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let len = self.0.len();
        if len == 0 {
            return fmt.write_str("[]");
        }

        const MAX_PRINT_COUNT: usize = 20;
        let need_ellipsis = len > MAX_PRINT_COUNT;
        let print_count = len.min(MAX_PRINT_COUNT);
        let last_printed_index = print_count - 1;

        fmt.write_str("[")?;
        for b in self.0.iter().take(last_printed_index) {
            write!(fmt, "{b}, ")?;
        }
        write!(fmt, "{}", self.0[last_printed_index])?;
        if need_ellipsis {
            write!(
                fmt,
                ", ... (and {} more, total length {})",
                len - MAX_PRINT_COUNT,
                len
            )?;
        }
        fmt.write_str("]")?;
        Ok(())
    }
}

impl<'a> fmt::Debug for RawData<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            RawData::Single(buffer) => {
                write!(fmt, "RawData::Single({})", &DisplayableSlice(buffer))
            }
            RawData::Split(left, right) => write!(
                fmt,
                "RawData::Split({}, {})",
                &DisplayableSlice(left),
                &DisplayableSlice(right),
            ),
        }
    }
}

impl<'a> RawData<'a> {
    #[inline]
    pub fn empty() -> Self {
        RawData::Single(&[])
    }

    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), std::io::Error> {
        let buf_len = buf.len();
        *self = match *self {
            RawData::Single(single) => {
                if single.len() < buf_len {
                    return Err(std::io::ErrorKind::UnexpectedEof.into());
                }
                buf.copy_from_slice(&single[..buf_len]);
                RawData::Single(&single[buf_len..])
            }
            RawData::Split(left, right) => {
                let left_len = left.len();
                if buf_len <= left_len {
                    buf.copy_from_slice(&left[..buf_len]);
                    if buf_len < left_len {
                        RawData::Split(&left[buf_len..], right)
                    } else {
                        RawData::Single(right)
                    }
                } else {
                    let remainder_len = buf_len - left_len;
                    if remainder_len > right.len() {
                        return Err(std::io::ErrorKind::UnexpectedEof.into());
                    }
                    buf[..left_len].copy_from_slice(left);
                    buf[left_len..].copy_from_slice(&right[..remainder_len]);
                    RawData::Single(&right[remainder_len..])
                }
            }
        };
        Ok(())
    }

    pub fn read_u64<T: ByteOrder>(&mut self) -> Result<u64, std::io::Error> {
        let mut b = [0; 8];
        self.read_exact(&mut b)?;
        Ok(T::read_u64(&b))
    }

    pub fn read_u32<T: ByteOrder>(&mut self) -> Result<u32, std::io::Error> {
        let mut b = [0; 4];
        self.read_exact(&mut b)?;
        Ok(T::read_u32(&b))
    }

    pub fn read_i32<T: ByteOrder>(&mut self) -> Result<i32, std::io::Error> {
        let mut b = [0; 4];
        self.read_exact(&mut b)?;
        Ok(T::read_i32(&b))
    }

    pub fn read_u16<T: ByteOrder>(&mut self) -> Result<u16, std::io::Error> {
        let mut b = [0; 2];
        self.read_exact(&mut b)?;
        Ok(T::read_u16(&b))
    }

    pub fn read_u8(&mut self) -> Result<u8, std::io::Error> {
        let mut b = [0; 1];
        self.read_exact(&mut b)?;
        Ok(b[0])
    }

    /// Finds the first nul byte. Returns everything before that nul byte.
    /// Sets self to everything after the nul byte.
    pub fn read_string(&mut self) -> Option<RawData<'a>> {
        let (rv, new_self) = match *self {
            RawData::Single(single) => {
                let n = memchr::memchr(0, single)?;
                (
                    RawData::Single(&single[..n]),
                    RawData::Single(&single[n + 1..]),
                )
            }
            RawData::Split(left, right) => {
                if let Some(n) = memchr::memchr(0, left) {
                    (
                        RawData::Single(&left[..n]),
                        if n + 1 < left.len() {
                            RawData::Split(&left[n + 1..], right)
                        } else {
                            RawData::Single(right)
                        },
                    )
                } else if let Some(n) = memchr::memchr(0, right) {
                    (
                        RawData::Split(left, &right[..n]),
                        RawData::Single(&right[n + 1..]),
                    )
                } else {
                    return None;
                }
            }
        };
        *self = new_self;
        Some(rv)
    }

    /// Returns the first `n` bytes, and sets self to the remainder.
    pub fn split_off_prefix(&mut self, n: usize) -> Result<Self, std::io::Error> {
        let (rv, new_self) = match *self {
            RawData::Single(single) => {
                if single.len() < n {
                    return Err(std::io::ErrorKind::UnexpectedEof.into());
                }
                (RawData::Single(&single[..n]), RawData::Single(&single[n..]))
            }
            RawData::Split(left, right) => {
                if n <= left.len() {
                    (
                        RawData::Single(&left[..n]),
                        if n < left.len() {
                            RawData::Split(&left[n..], right)
                        } else {
                            RawData::Single(right)
                        },
                    )
                } else {
                    let remainder_len = n - left.len();
                    if remainder_len > right.len() {
                        return Err(std::io::ErrorKind::UnexpectedEof.into());
                    }
                    (
                        RawData::Split(left, &right[..remainder_len]),
                        RawData::Single(&right[remainder_len..]),
                    )
                }
            }
        };
        *self = new_self;
        Ok(rv)
    }

    pub fn skip(&mut self, n: usize) -> Result<(), std::io::Error> {
        *self = match *self {
            RawData::Single(single) => {
                if single.len() < n {
                    return Err(std::io::ErrorKind::UnexpectedEof.into());
                }
                RawData::Single(&single[n..])
            }
            RawData::Split(left, right) => {
                if n < left.len() {
                    RawData::Split(&left[n..], right)
                } else {
                    let remainder_len = n - left.len();
                    if remainder_len > right.len() {
                        return Err(std::io::ErrorKind::UnexpectedEof.into());
                    }
                    RawData::Single(&right[remainder_len..])
                }
            }
        };
        Ok(())
    }

    #[inline]
    fn write_into(&self, target: &mut Vec<u8>) {
        target.clear();
        match *self {
            RawData::Single(slice) => target.extend_from_slice(slice),
            RawData::Split(first, second) => {
                target.reserve(first.len() + second.len());
                target.extend_from_slice(first);
                target.extend_from_slice(second);
            }
        }
    }

    pub fn as_slice(&self) -> Cow<'a, [u8]> {
        match *self {
            RawData::Single(buffer) => buffer.into(),
            RawData::Split(..) => {
                let mut vec = Vec::new();
                self.write_into(&mut vec);
                vec.into()
            }
        }
    }

    pub fn get(&self, range: Range<usize>) -> Option<RawData<'a>> {
        Some(match self {
            RawData::Single(buffer) => RawData::Single(buffer.get(range)?),
            RawData::Split(left, right) => {
                if range.start >= left.len() {
                    RawData::Single(right.get(range.start - left.len()..range.end - left.len())?)
                } else if range.end <= left.len() {
                    RawData::Single(left.get(range)?)
                } else {
                    let left = left.get(range.start..)?;
                    let right = right.get(..min(range.end - left.len(), right.len()))?;
                    RawData::Split(left, right)
                }
            }
        })
    }

    pub fn is_empty(&self) -> bool {
        match *self {
            RawData::Single(buffer) => buffer.is_empty(),
            RawData::Split(left, right) => left.is_empty() && right.is_empty(),
        }
    }

    pub fn len(&self) -> usize {
        match *self {
            RawData::Single(buffer) => buffer.len(),
            RawData::Split(left, right) => left.len() + right.len(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RawDataU64<'a> {
    swapped_endian: bool,
    raw_data: RawData<'a>,
}

pub fn is_swapped_endian<T: ByteOrder>() -> bool {
    let mut buf = [0; 2];
    T::write_u16(&mut buf, 0x1234);
    u16::from_ne_bytes(buf) != 0x1234
}

impl<'a> RawDataU64<'a> {
    #[inline]
    pub fn from_raw_data<T: ByteOrder>(raw_data: RawData<'a>) -> Self {
        RawDataU64 {
            raw_data,
            swapped_endian: is_swapped_endian::<T>(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.raw_data.is_empty()
    }

    pub fn len(&self) -> usize {
        self.raw_data.len() / mem::size_of::<u64>()
    }

    pub fn get(&self, index: usize) -> Option<u64> {
        let offset = index * mem::size_of::<u64>();
        let mut data = self.raw_data;
        data.skip(offset).ok()?;
        let value = data.read_u64::<NativeEndian>().ok()?;
        Some(if self.swapped_endian {
            value.swap_bytes()
        } else {
            value
        })
    }
}

impl<'a> std::fmt::Debug for RawDataU64<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let mut list = fmt.debug_list();
        let mut data = self.raw_data;
        while let Ok(value) = data.read_u64::<NativeEndian>() {
            let value = if self.swapped_endian {
                value.swap_bytes()
            } else {
                value
            };
            list.entry(&HexValue(value));
        }

        list.finish()
    }
}

#[cfg(test)]
mod test {
    use super::RawData;

    #[test]
    fn test_reading_from_split() {
        let full = b"CDEF===AB"; // 0123___78"
        assert_eq!(full.len(), 9);
        let mut split = RawData::Split(&full[7..9], &full[0..4]);
        let mut dest = vec![0; 6];
        split.read_exact(&mut dest).unwrap();
        assert_eq!(&dest, b"ABCDEF");
    }
}
