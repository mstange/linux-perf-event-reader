//! # linux-perf-event-reader
//!
//! This crate lets you parse Linux perf events and associated structures.
//!
//! ## Example
//!
//! ```rust
//! use linux_perf_event_reader::{
//!     CommOrExecRecord, Endianness, EventRecord, PerfEventAttr, RawData, RawEventRecord,
//!     RecordParseInfo, RecordType
//! };
//!
//! # fn it_works() {
//! // Read the perf_event_attr data.
//! let attr_data = vec![
//!     0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 229, 3, 0, 0, 0, 0, 0, 0, 47, 177, 0,
//!     0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 3, 183, 215, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
//!     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 15,
//!     255, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
//!     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 104, 0, 0, 0, 0, 0, 0, 0, 128, 0,
//!     0, 0, 0, 0, 0, 0,
//! ];
//! let (attr, _size) =
//!     PerfEventAttr::parse::<_, byteorder::LittleEndian>(&attr_data[..]).unwrap();
//! let parse_info = RecordParseInfo::new(&attr, Endianness::LittleEndian);
//!
//! let body = b"lG\x08\0lG\x08\0dump_syms\0\0\0\0\0\0\0lG\x08\0lG\x08\08\x1b\xf8\x18hX\x04\0";
//! let body_raw_data = RawData::from(&body[..]);
//! let raw_record = RawEventRecord::new(RecordType::COMM, 0x2000, body_raw_data, parse_info);
//! let parsed_record = raw_record.parse().unwrap();
//!
//! assert_eq!(
//!     parsed_record,
//!     EventRecord::Comm(CommOrExecRecord {
//!         pid: 542572,
//!         tid: 542572,
//!         name: RawData::Single(b"dump_syms"),
//!         is_execve: true
//!     })
//! );
//! # }
//! ```
mod common_data;
pub mod constants;
mod endian;
mod event_record;
mod parse_info;
mod perf_event;
mod raw_data;
mod registers;
mod sample;
mod types;
mod utils;

pub use common_data::*;
pub use endian::*;
pub use event_record::*;
pub use parse_info::*;
pub use perf_event::*;
pub use raw_data::*;
pub use registers::*;
pub use sample::*;
pub use types::*;

#[cfg(test)]
mod test {
    use crate::{
        CommOrExecRecord, Endianness, EventRecord, PerfEventAttr, RawData, RawEventRecord,
        RecordParseInfo, RecordType,
    };

    #[test]
    fn it_works() {
        // Read the perf_event_attr data.
        let attr_data = vec![
            0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 229, 3, 0, 0, 0, 0, 0, 0, 47, 177, 0,
            0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 3, 183, 215, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 15,
            255, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 104, 0, 0, 0, 0, 0, 0, 0, 128, 0,
            0, 0, 0, 0, 0, 0,
        ];
        let (attr, _size) =
            PerfEventAttr::parse::<_, byteorder::LittleEndian>(&attr_data[..]).unwrap();
        let parse_info = RecordParseInfo::new(&attr, Endianness::LittleEndian);

        let body = b"lG\x08\0lG\x08\0dump_syms\0\0\0\0\0\0\0lG\x08\0lG\x08\08\x1b\xf8\x18hX\x04\0";
        let body_raw_data = RawData::from(&body[..]);
        let raw_record = RawEventRecord::new(RecordType::COMM, 0x2000, body_raw_data, parse_info);
        let parsed_record = raw_record.parse().unwrap();

        assert_eq!(
            parsed_record,
            EventRecord::Comm(CommOrExecRecord {
                pid: 542572,
                tid: 542572,
                name: RawData::Single(b"dump_syms"),
                is_execve: true
            })
        );
    }
}
