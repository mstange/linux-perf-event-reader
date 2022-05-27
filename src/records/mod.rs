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

/// Get the event ID from an event record, if the sample format includes SampleFormat::IDENTIFIER.
///
/// This can be used if it is not known which `perf_event_attr` describes this record,
/// but only if all potential attrs include `PERF_SAMPLE_IDENTIFIER`.
/// Once the record's event ID is known, this event ID can be mapped to the right attr,
/// and then the information from the attr can be used to parse the rest of this record.
pub fn get_record_event_identifier<T: ByteOrder>(
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
