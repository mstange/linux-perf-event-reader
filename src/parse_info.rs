use crate::{AttrFlags, BranchSampleFormat, Endianness, PerfEventAttr, ReadFormat, SampleFormat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordParseInfo {
    pub endian: Endianness,
    pub sample_format: SampleFormat,
    pub branch_sample_format: BranchSampleFormat,
    pub read_format: ReadFormat,
    pub common_data_offset_from_end: Option<u8>, // 0..=48
    pub sample_regs_user: u64,
    pub user_regs_count: u8, // 0..=64
    pub sample_regs_intr: u64,
    pub intr_regs_count: u8, // 0..=64
    pub id_parse_info: RecordIdParseInfo,
    pub nonsample_record_time_offset_from_end: Option<u8>, // 0..=40
    pub sample_record_time_offset_from_start: Option<u8>,  // 0..=32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordIdParseInfo {
    pub nonsample_record_id_offset_from_end: Option<u8>, // 0..=32
    pub sample_record_id_offset_from_start: Option<u8>,  // 0..=24
}

impl RecordParseInfo {
    pub fn new(attr: &PerfEventAttr, endian: Endianness) -> Self {
        let sample_format = attr.sample_format;
        let branch_sample_format = attr.branch_sample_format;
        let read_format = attr.read_format;

        // struct sample_id {
        //     { u32 pid, tid; }   /* if PERF_SAMPLE_TID set */
        //     { u64 time;     }   /* if PERF_SAMPLE_TIME set */
        //     { u64 id;       }   /* if PERF_SAMPLE_ID set */
        //     { u64 stream_id;}   /* if PERF_SAMPLE_STREAM_ID set  */
        //     { u32 cpu, res; }   /* if PERF_SAMPLE_CPU set */
        //     { u64 id;       }   /* if PERF_SAMPLE_IDENTIFIER set */
        // };
        let common_data_offset_from_end = if attr.flags.contains(AttrFlags::SAMPLE_ID_ALL) {
            Some(
                sample_format
                    .intersection(
                        SampleFormat::TID
                            | SampleFormat::TIME
                            | SampleFormat::ID
                            | SampleFormat::STREAM_ID
                            | SampleFormat::CPU
                            | SampleFormat::IDENTIFIER,
                    )
                    .bits()
                    .count_ones() as u8
                    * 8,
            )
        } else {
            None
        };
        let sample_regs_user = attr.sample_regs_user;
        let user_regs_count = sample_regs_user.count_ones() as u8;
        let sample_regs_intr = attr.sample_regs_intr;
        let intr_regs_count = sample_regs_intr.count_ones() as u8;
        let nonsample_record_time_offset_from_end = if attr.flags.contains(AttrFlags::SAMPLE_ID_ALL)
            && sample_format.contains(SampleFormat::TIME)
        {
            Some(
                sample_format
                    .intersection(
                        SampleFormat::TIME
                            | SampleFormat::ID
                            | SampleFormat::STREAM_ID
                            | SampleFormat::CPU
                            | SampleFormat::IDENTIFIER,
                    )
                    .bits()
                    .count_ones() as u8
                    * 8,
            )
        } else {
            None
        };

        // { u64 id;           } && PERF_SAMPLE_IDENTIFIER
        // { u64 ip;           } && PERF_SAMPLE_IP
        // { u32 pid; u32 tid; } && PERF_SAMPLE_TID
        // { u64 time;         } && PERF_SAMPLE_TIME
        // { u64 addr;         } && PERF_SAMPLE_ADDR
        // { u64 id;           } && PERF_SAMPLE_ID
        let sample_record_time_offset_from_start = if sample_format.contains(SampleFormat::TIME) {
            Some(
                sample_format
                    .intersection(SampleFormat::IDENTIFIER | SampleFormat::IP | SampleFormat::TID)
                    .bits()
                    .count_ones() as u8
                    * 8,
            )
        } else {
            None
        };

        Self {
            endian,
            sample_format,
            branch_sample_format,
            read_format,
            common_data_offset_from_end,
            sample_regs_user,
            user_regs_count,
            sample_regs_intr,
            intr_regs_count,
            nonsample_record_time_offset_from_end,
            sample_record_time_offset_from_start,
            id_parse_info: RecordIdParseInfo::new(attr),
        }
    }
}

impl RecordIdParseInfo {
    pub fn new(attr: &PerfEventAttr) -> Self {
        let sample_format = attr.sample_format;
        let nonsample_record_id_offset_from_end = if attr.flags.contains(AttrFlags::SAMPLE_ID_ALL)
            && sample_format.intersects(SampleFormat::ID | SampleFormat::IDENTIFIER)
        {
            if sample_format.contains(SampleFormat::IDENTIFIER) {
                Some(8)
            } else {
                Some(
                    sample_format
                        .intersection(
                            SampleFormat::ID
                                | SampleFormat::STREAM_ID
                                | SampleFormat::CPU
                                | SampleFormat::IDENTIFIER,
                        )
                        .bits()
                        .count_ones() as u8
                        * 8,
                )
            }
        } else {
            None
        };

        // { u64 id;           } && PERF_SAMPLE_IDENTIFIER
        // { u64 ip;           } && PERF_SAMPLE_IP
        // { u32 pid; u32 tid; } && PERF_SAMPLE_TID
        // { u64 time;         } && PERF_SAMPLE_TIME
        // { u64 addr;         } && PERF_SAMPLE_ADDR
        // { u64 id;           } && PERF_SAMPLE_ID
        let sample_record_id_offset_from_start = if sample_format.contains(SampleFormat::IDENTIFIER)
        {
            Some(0)
        } else if sample_format.contains(SampleFormat::ID) {
            Some(
                sample_format
                    .intersection(
                        SampleFormat::IP
                            | SampleFormat::TID
                            | SampleFormat::TIME
                            | SampleFormat::ADDR,
                    )
                    .bits()
                    .count_ones() as u8
                    * 8,
            )
        } else {
            None
        };

        Self {
            nonsample_record_id_offset_from_end,
            sample_record_id_offset_from_start,
        }
    }
}
