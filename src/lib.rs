//! # linux-perf-event-reader
//!
//! This crate lets you parse Linux perf events and associated structures.
//!
//! ## Example
//!
//! ```rust
//! use linux_perf_event_reader::{PerfEventAttr, RawData, RecordType};
//! use linux_perf_event_reader::records::{CommOrExecRecord, ParsedRecord, RawRecord, RecordParseInfo};
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
//! let attr =
//!     PerfEventAttr::parse::<_, byteorder::LittleEndian>(&attr_data[..], None).unwrap();
//! let parse_info = RecordParseInfo::from_attr(&attr);
//!
//! let body = vec![
//!     108, 71, 8, 0, 108, 71, 8, 0, 100, 117, 109, 112, 95, 115, 121, 109, 115, 0, 0, 0, 0,
//!     0, 0, 0, 108, 71, 8, 0, 108, 71, 8, 0, 56, 27, 248, 24, 104, 88, 4, 0,
//! ];
//! let body_raw_data = RawData::from(&body[..]);
//! let raw_record = RawRecord::new(RecordType(3), 0x2000, body_raw_data);
//! let parsed_record = raw_record
//!     .to_parsed::<byteorder::LittleEndian>(&parse_info)
//!     .unwrap();
//!
//! assert_eq!(
//!     parsed_record,
//!     ParsedRecord::Comm(CommOrExecRecord {
//!         pid: 542572,
//!         tid: 542572,
//!         name: RawData::Single(b"dump_syms"),
//!         is_execve: true
//!     })
//! );
//! # }
//! ```
pub mod consts;
mod perf_event;
mod raw_data;
pub mod records;
mod types;
mod utils;

pub use perf_event::*;
pub use raw_data::*;
pub use types::*;

#[cfg(test)]
mod test {
    use crate::{
        records::{CommOrExecRecord, ParsedRecord, RawRecord, RecordParseInfo},
        PerfEventAttr, RawData, RecordType,
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
        let attr =
            PerfEventAttr::parse::<_, byteorder::LittleEndian>(&attr_data[..], None).unwrap();
        let parse_info = RecordParseInfo::from_attr(&attr);

        let body = vec![
            108, 71, 8, 0, 108, 71, 8, 0, 100, 117, 109, 112, 95, 115, 121, 109, 115, 0, 0, 0, 0,
            0, 0, 0, 108, 71, 8, 0, 108, 71, 8, 0, 56, 27, 248, 24, 104, 88, 4, 0,
        ];
        let body_raw_data = RawData::from(&body[..]);
        let raw_record = RawRecord::new(RecordType(3), 0x2000, body_raw_data);
        let parsed_record = raw_record
            .to_parsed::<byteorder::LittleEndian>(&parse_info)
            .unwrap();

        assert_eq!(
            parsed_record,
            ParsedRecord::Comm(CommOrExecRecord {
                pid: 542572,
                tid: 542572,
                name: RawData::Single(b"dump_syms"),
                is_execve: true
            })
        );
    }
}
