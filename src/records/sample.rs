use byteorder::ByteOrder;

use crate::{BranchSampleFormat, RawData, RawDataU64, ReadFormat, SampleFormat};

use super::{RecordParseInfo, Regs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleRecord<'a> {
    pub id: Option<u64>,
    pub addr: Option<u64>,
    pub stream_id: Option<u64>,
    pub raw: Option<RawData<'a>>,
    pub ip: Option<u64>,
    pub timestamp: Option<u64>,
    pub pid: Option<i32>,
    pub tid: Option<i32>,
    pub cpu: Option<u32>,
    pub period: Option<u64>,
    pub user_regs: Option<Regs<'a>>,
    pub user_stack: Option<(RawData<'a>, u64)>,
    pub callchain: Option<RawDataU64<'a>>,
    pub phys_addr: Option<u64>,
    pub data_page_size: Option<u64>,
    pub code_page_size: Option<u64>,
}

impl<'a> SampleRecord<'a> {
    pub fn parse<T: ByteOrder>(
        data: RawData<'a>,
        parse_info: &RecordParseInfo,
    ) -> Result<Self, std::io::Error> {
        let sample_format = parse_info.sample_format;
        let branch_sample_format = parse_info.branch_sample_format;
        let read_format = parse_info.read_format;
        let regs_count = parse_info.regs_count;
        let sample_regs_user = parse_info.sample_regs_user;
        let mut cur = data;

        let identifier = if sample_format.contains(SampleFormat::IDENTIFIER) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

        let ip = if sample_format.contains(SampleFormat::IP) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

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

        let addr = if sample_format.contains(SampleFormat::ADDR) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

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
            let _reserved = cur.read_u32::<T>()?;
            Some(cpu)
        } else {
            None
        };

        let period = if sample_format.contains(SampleFormat::PERIOD) {
            let period = cur.read_u64::<T>()?;
            Some(period)
        } else {
            None
        };

        if sample_format.contains(SampleFormat::READ) {
            if read_format.contains(ReadFormat::GROUP) {
                let _value = cur.read_u64::<T>()?;
                if read_format.contains(ReadFormat::TOTAL_TIME_ENABLED) {
                    let _time_enabled = cur.read_u64::<T>()?;
                }
                if read_format.contains(ReadFormat::TOTAL_TIME_RUNNING) {
                    let _time_running = cur.read_u64::<T>()?;
                }
                if read_format.contains(ReadFormat::ID) {
                    let _id = cur.read_u64::<T>()?;
                }
            } else {
                let nr = cur.read_u64::<T>()?;
                if read_format.contains(ReadFormat::TOTAL_TIME_ENABLED) {
                    let _time_enabled = cur.read_u64::<T>()?;
                }
                if read_format.contains(ReadFormat::TOTAL_TIME_RUNNING) {
                    let _time_running = cur.read_u64::<T>()?;
                }
                for _ in 0..nr {
                    let _value = cur.read_u64::<T>()?;
                    if read_format.contains(ReadFormat::ID) {
                        let _id = cur.read_u64::<T>()?;
                    }
                }
            }
        }

        let callchain = if sample_format.contains(SampleFormat::CALLCHAIN) {
            let callchain_length = cur.read_u64::<T>()?;
            let callchain =
                cur.split_off_prefix(callchain_length as usize * std::mem::size_of::<u64>())?;
            Some(RawDataU64::from_raw_data::<T>(callchain))
        } else {
            None
        };

        let raw = if sample_format.contains(SampleFormat::RAW) {
            let size = cur.read_u32::<T>()?;
            Some(cur.split_off_prefix(size as usize)?)
        } else {
            None
        };

        if sample_format.contains(SampleFormat::BRANCH_STACK) {
            let nr = cur.read_u64::<T>()?;
            if branch_sample_format.contains(BranchSampleFormat::HW_INDEX) {
                let _hw_idx = cur.read_u64::<T>()?;
            }
            for _ in 0..nr {
                let _from = cur.read_u64::<T>()?;
                let _to = cur.read_u64::<T>()?;
                let _flags = cur.read_u64::<T>()?;
            }
        }

        let user_regs = if sample_format.contains(SampleFormat::REGS_USER) {
            let regs_abi = cur.read_u64::<T>()?;
            if regs_abi == 0 {
                None
            } else {
                let regs_data = cur.split_off_prefix(regs_count * std::mem::size_of::<u64>())?;
                let raw_regs = RawDataU64::from_raw_data::<T>(regs_data);
                let user_regs = Regs::new(sample_regs_user, raw_regs);
                Some(user_regs)
            }
        } else {
            None
        };

        let user_stack = if sample_format.contains(SampleFormat::STACK_USER) {
            let stack_size = cur.read_u64::<T>()?;
            let stack = cur.split_off_prefix(stack_size as usize)?;

            let dynamic_size = if stack_size != 0 {
                cur.read_u64::<T>()?
            } else {
                0
            };
            Some((stack, dynamic_size))
        } else {
            None
        };

        if sample_format.contains(SampleFormat::WEIGHT) {
            let _weight = cur.read_u64::<T>()?;
        }

        if sample_format.contains(SampleFormat::DATA_SRC) {
            let _data_src = cur.read_u64::<T>()?;
        }

        if sample_format.contains(SampleFormat::TRANSACTION) {
            let _transaction = cur.read_u64::<T>()?;
        }

        if sample_format.contains(SampleFormat::REGS_INTR) {
            let regs_abi = cur.read_u64::<T>()?;
            if regs_abi != 0 {
                cur.skip(regs_count * std::mem::size_of::<u64>())?;
            }
        }

        let phys_addr = if sample_format.contains(SampleFormat::PHYS_ADDR) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

        if sample_format.contains(SampleFormat::AUX) {
            let size = cur.read_u64::<T>()?;
            cur.skip(size as usize)?;
        }

        let data_page_size = if sample_format.contains(SampleFormat::DATA_PAGE_SIZE) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

        let code_page_size = if sample_format.contains(SampleFormat::CODE_PAGE_SIZE) {
            Some(cur.read_u64::<T>()?)
        } else {
            None
        };

        Ok(Self {
            id,
            ip,
            addr,
            stream_id,
            raw,
            user_regs,
            user_stack,
            callchain,
            cpu,
            timestamp,
            pid,
            tid,
            period,
            phys_addr,
            data_page_size,
            code_page_size,
        })
    }
}
