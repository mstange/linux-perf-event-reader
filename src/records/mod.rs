mod common_data;
mod event_record;
mod parse_info;
mod registers;
mod sample;
mod thread_map;

use byteorder::ByteOrder;
pub use common_data::*;
pub use event_record::*;
pub use parse_info::*;
pub use registers::*;
pub use sample::*;
pub use thread_map::*;

use crate::{RawData, RecordType};

/// Get the ID from an event record, if the sample format includes SampleFormat::IDENTIFIER.
///
/// This can be used if it is not known which `perf_event_attr` describes this record,
/// but only if all potential attrs include `PERF_SAMPLE_IDENTIFIER`.
/// Once the record's ID is known, this ID can be mapped to the right attr,
/// and then the information from the attr can be used to parse the rest of this record.
pub fn get_record_identifier<T: ByteOrder>(
    record_type: RecordType,
    mut data: RawData,
    sample_id_all: bool,
) -> Option<u64> {
    if record_type.is_user_type() {
        None
    } else if record_type == RecordType::SAMPLE {
        // if IDENTIFIER is set, every SAMPLE record starts with the event ID.
        data.read_u64::<T>().ok()
    } else if sample_id_all {
        // if IDENTIFIER and SAMPLE_ID_ALL are set, every non-SAMPLE record ends with the event ID.
        let id_offset_from_start = data.len().checked_sub(8)?;
        data.skip(id_offset_from_start).ok()?;
        data.read_u64::<T>().ok()
    } else {
        None
    }
}

/// Get the ID from an event record, with the help of `RecordIdParseInfo`.
///
/// This can be used if it is not known which `perf_event_attr` describes this record,
/// but only if all potential attrs have the same `RecordIdParseInfo`.
/// Once the record's ID is known, this ID can be mapped to the right attr,
/// and then the information from the attr can be used to parse the rest of this record.
pub fn get_record_id<T: ByteOrder>(
    record_type: RecordType,
    mut data: RawData,
    parse_info: &RecordIdParseInfo,
) -> Option<u64> {
    if record_type.is_user_type() {
        return None;
    }

    if record_type == RecordType::SAMPLE {
        if let Some(id_offset_from_start) = parse_info.sample_record_id_offset_from_start {
            data.skip(id_offset_from_start as usize).ok()?;
            data.read_u64::<T>().ok()
        } else {
            None
        }
    } else if let Some(id_offset_from_end) = parse_info.nonsample_record_id_offset_from_end {
        let id_offset_from_start = data.len().checked_sub(id_offset_from_end as usize)?;
        data.skip(id_offset_from_start).ok()?;
        data.read_u64::<T>().ok()
    } else {
        None
    }
}

/// Get the timestamp from an event record, with the help of `RecordParseInfo`.
///
/// This can be used for record sorting, without having to wrap the record into
/// a `RawRecord`.o
pub fn get_record_timestamp<T: ByteOrder>(
    record_type: RecordType,
    mut data: RawData,
    parse_info: &RecordParseInfo,
) -> Option<u64> {
    if record_type.is_user_type() {
        return None;
    }

    if record_type == RecordType::SAMPLE {
        if let Some(time_offset_from_start) = parse_info.sample_record_time_offset_from_start {
            data.skip(time_offset_from_start as usize).ok()?;
            data.read_u64::<T>().ok()
        } else {
            None
        }
    } else if let Some(time_offset_from_end) = parse_info.nonsample_record_time_offset_from_end {
        let time_offset_from_start = data.len().checked_sub(time_offset_from_end as usize)?;
        data.skip(time_offset_from_start).ok()?;
        data.read_u64::<T>().ok()
    } else {
        None
    }
}
