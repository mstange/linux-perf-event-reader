use byteorder::ByteOrder;

use crate::{RawData, SampleFormat};

use super::RecordParseInfo;

#[derive(Clone, Debug, Default)]
pub struct CommonData {
    pub pid: Option<i32>,
    pub tid: Option<i32>,
    pub timestamp: Option<u64>,
    pub id: Option<u64>,
    pub stream_id: Option<u64>,
    pub cpu: Option<u32>,
}

impl CommonData {
    pub fn parse_sample<T: ByteOrder>(
        data: RawData,
        parse_info: &RecordParseInfo,
    ) -> Result<CommonData, std::io::Error> {
        let sample_format = parse_info.sample_format;

        // { u64 id;       } && PERF_SAMPLE_IDENTIFIER
        // { u64 ip;       } && PERF_SAMPLE_IP
        // { u32 pid, tid; } && PERF_SAMPLE_TID
        // { u64 time;     } && PERF_SAMPLE_TIME
        // { u64 addr;     } && PERF_SAMPLE_ADDR
        // { u64 id;       } && PERF_SAMPLE_ID
        // { u64 stream_id;} && PERF_SAMPLE_STREAM_ID
        // { u32 cpu, res; } && PERF_SAMPLE_CPU
        let mut cur = data;
        let identifier = if sample_format.contains(SampleFormat::IDENTIFIER) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

        if sample_format.contains(SampleFormat::IP) {
            let _ip = cur.read_u64::<T>()?;
        }

        let (pid, tid) = if sample_format.contains(SampleFormat::TID) {
            let pid = cur.read_i32::<T>()?;
            let tid = cur.read_i32::<T>()?;
            (Some(pid), Some(tid))
        } else {
            (None, None)
        };

        let timestamp = if sample_format.contains(SampleFormat::TIME) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

        if sample_format.contains(SampleFormat::ADDR) {
            let _addr = cur.read_u64::<T>()?;
        }

        let id = if sample_format.contains(SampleFormat::ID) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };
        let id = identifier.or(id);

        let stream_id = if sample_format.contains(SampleFormat::STREAM_ID) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

        let cpu = if sample_format.contains(SampleFormat::CPU) {
            let cpu = cur.read_u32::<T>()?;
            let _ = cur.read_u32::<T>()?; // Reserved field; is always zero.
            Some(cpu)
        } else {
            None
        };

        Ok(CommonData {
            pid,
            tid,
            timestamp,
            id,
            stream_id,
            cpu,
        })
    }

    pub fn parse_nonsample<T: ByteOrder>(
        data: RawData,
        parse_info: &RecordParseInfo,
    ) -> Result<CommonData, std::io::Error> {
        if let Some(common_data_offset_from_end) = parse_info.common_data_offset_from_end {
            let sample_format = parse_info.sample_format;

            let mut cur = data;
            let common_data_offset_from_start = cur
                .len()
                .checked_sub(common_data_offset_from_end)
                .ok_or(std::io::ErrorKind::UnexpectedEof)?;
            cur.skip(common_data_offset_from_start)?;

            // struct sample_id {
            //     { u32 pid, tid;  }   /* if PERF_SAMPLE_TID set */
            //     { u64 timestamp; }   /* if PERF_SAMPLE_TIME set */
            //     { u64 id;        }   /* if PERF_SAMPLE_ID set */
            //     { u64 stream_id; }   /* if PERF_SAMPLE_STREAM_ID set  */
            //     { u32 cpu, res;  }   /* if PERF_SAMPLE_CPU set */
            //     { u64 identifier;}   /* if PERF_SAMPLE_IDENTIFIER set */
            // };
            let (pid, tid) = if sample_format.contains(SampleFormat::TID) {
                let pid = cur.read_i32::<T>()?;
                let tid = cur.read_i32::<T>()?;
                (Some(pid), Some(tid))
            } else {
                (None, None)
            };

            let timestamp = if sample_format.contains(SampleFormat::TIME) {
                Some(cur.read_u64::<T>()?)
            } else {
                None
            };

            let id = if sample_format.contains(SampleFormat::ID) {
                Some(cur.read_u64::<T>()?)
            } else {
                None
            };

            let stream_id = if sample_format.contains(SampleFormat::STREAM_ID) {
                Some(cur.read_u64::<T>()?)
            } else {
                None
            };

            let cpu = if sample_format.contains(SampleFormat::CPU) {
                let cpu = cur.read_u32::<T>()?;
                let _ = cur.read_u32::<T>()?; // Reserved field; is always zero.
                Some(cpu)
            } else {
                None
            };

            let identifier = if sample_format.contains(SampleFormat::IDENTIFIER) {
                Some(cur.read_u64::<T>()?)
            } else {
                None
            };
            let id = identifier.or(id);

            Ok(CommonData {
                pid,
                tid,
                timestamp,
                id,
                stream_id,
                cpu,
            })
        } else {
            Ok(Default::default())
        }
    }
}
