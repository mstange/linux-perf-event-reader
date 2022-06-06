use crate::raw_data::RawData;
use crate::types::*;
use crate::utils::HexValue;
use crate::{consts::*, Endianness};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::fmt;

use super::{
    get_record_id, get_record_timestamp, CommonData, RecordParseInfo, SampleRecord, ThreadMap,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ParsedRecord<'a> {
    Sample(SampleRecord<'a>),
    Comm(CommOrExecRecord<'a>),
    Exit(ForkOrExitRecord),
    Fork(ForkOrExitRecord),
    Mmap(MmapRecord<'a>),
    Mmap2(Mmap2Record<'a>),
    Lost(LostRecord),
    Throttle(ThrottleRecord),
    Unthrottle(ThrottleRecord),
    ContextSwitch(ContextSwitchRecord),
    ThreadMap(ThreadMap<'a>),
    Raw(RawRecord<'a>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkOrExitRecord {
    pub pid: i32,
    pub ppid: i32,
    pub tid: i32,
    pub ptid: i32,
    pub timestamp: u64,
}

impl ForkOrExitRecord {
    pub fn parse<T: ByteOrder>(data: RawData) -> Result<Self, std::io::Error> {
        let mut cur = data;

        let pid = cur.read_i32::<T>()?;
        let ppid = cur.read_i32::<T>()?;
        let tid = cur.read_i32::<T>()?;
        let ptid = cur.read_i32::<T>()?;
        let timestamp = cur.read_u64::<T>()?;

        Ok(Self {
            pid,
            ppid,
            tid,
            ptid,
            timestamp,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CommOrExecRecord<'a> {
    pub pid: i32,
    pub tid: i32,
    pub name: RawData<'a>,
    pub is_execve: bool,
}

impl<'a> CommOrExecRecord<'a> {
    pub fn parse<T: ByteOrder>(data: RawData<'a>, misc: u16) -> Result<Self, std::io::Error> {
        let mut cur = data;
        let pid = cur.read_i32::<T>()?;
        let tid = cur.read_i32::<T>()?;
        let name = cur.read_string().unwrap_or(cur); // TODO: return error if no string terminator found

        // TODO: Maybe feature-gate this on 3.16+
        let is_execve = misc & PERF_RECORD_MISC_COMM_EXEC != 0;

        Ok(Self {
            pid,
            tid,
            name,
            is_execve,
        })
    }
}

impl<'a> fmt::Debug for CommOrExecRecord<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        use std::str;

        let mut map = fmt.debug_map();
        map.entry(&"pid", &self.pid).entry(&"tid", &self.tid);

        if let Ok(string) = str::from_utf8(&self.name.as_slice()) {
            map.entry(&"name", &string);
        } else {
            map.entry(&"name", &self.name);
        }

        map.entry(&"is_execve", &self.is_execve);
        map.finish()
    }
}

/// These aren't emitted by the kernel any more - the kernel uses MMAP2 events
/// these days.
/// However, `perf record` still emits synthetic MMAP events (not MMAP2!) for
/// the kernel image. So if you want to symbolicate kernel addresses you still
/// need to process these.
/// The kernel image MMAP events have pid -1.
#[derive(Clone, PartialEq, Eq)]
pub struct MmapRecord<'a> {
    pub pid: i32,
    pub tid: i32,
    pub address: u64,
    pub length: u64,
    pub page_offset: u64,
    pub is_executable: bool,
    pub cpu_mode: CpuMode,
    pub path: RawData<'a>,
}

impl<'a> MmapRecord<'a> {
    pub fn parse<T: ByteOrder>(data: RawData<'a>, misc: u16) -> Result<Self, std::io::Error> {
        let mut cur = data;

        // struct {
        //   struct perf_event_header header;
        //
        //   u32 pid, tid;
        //   u64 addr;
        //   u64 len;
        //   u64 pgoff;
        //   char filename[];
        //   struct sample_id sample_id;
        // };

        let pid = cur.read_i32::<T>()?;
        let tid = cur.read_i32::<T>()?;
        let address = cur.read_u64::<T>()?;
        let length = cur.read_u64::<T>()?;
        let page_offset = cur.read_u64::<T>()?;
        let path = cur.read_string().unwrap_or(cur); // TODO: return error if no string terminator found
        let is_executable = misc & PERF_RECORD_MISC_MMAP_DATA == 0;

        Ok(MmapRecord {
            pid,
            tid,
            address,
            length,
            page_offset,
            is_executable,
            cpu_mode: CpuMode::from_misc(misc),
            path,
        })
    }
}

impl<'a> fmt::Debug for MmapRecord<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_map()
            .entry(&"pid", &self.pid)
            .entry(&"tid", &self.tid)
            .entry(&"address", &HexValue(self.address))
            .entry(&"length", &HexValue(self.length))
            .entry(&"page_offset", &HexValue(self.page_offset))
            .entry(&"cpu_mode", &self.cpu_mode)
            .entry(&"path", &&*String::from_utf8_lossy(&self.path.as_slice()))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mmap2FileId {
    InodeAndVersion(Mmap2InodeAndVersion),
    BuildId(Vec<u8>),
}

#[derive(Clone, PartialEq, Eq)]
pub struct Mmap2Record<'a> {
    pub pid: i32,
    pub tid: i32,
    pub address: u64,
    pub length: u64,
    pub page_offset: u64,
    pub file_id: Mmap2FileId,
    pub protection: u32,
    pub flags: u32,
    pub cpu_mode: CpuMode,
    pub path: RawData<'a>,
}

impl<'a> Mmap2Record<'a> {
    pub fn parse<T: ByteOrder>(data: RawData<'a>, misc: u16) -> Result<Self, std::io::Error> {
        let mut cur = data;

        let pid = cur.read_i32::<T>()?;
        let tid = cur.read_i32::<T>()?;
        let address = cur.read_u64::<T>()?;
        let length = cur.read_u64::<T>()?;
        let page_offset = cur.read_u64::<T>()?;
        let file_id = if misc & PERF_RECORD_MISC_MMAP_BUILD_ID != 0 {
            let build_id_len = cur.read_u8()?;
            assert!(build_id_len <= 20);
            let _align = cur.read_u8()?;
            let _align = cur.read_u16::<T>()?;
            let mut build_id_bytes = [0; 20];
            cur.read_exact(&mut build_id_bytes)?;
            Mmap2FileId::BuildId(build_id_bytes[..build_id_len as usize].to_owned())
        } else {
            let major = cur.read_u32::<T>()?;
            let minor = cur.read_u32::<T>()?;
            let inode = cur.read_u64::<T>()?;
            let inode_generation = cur.read_u64::<T>()?;
            Mmap2FileId::InodeAndVersion(Mmap2InodeAndVersion {
                major,
                minor,
                inode,
                inode_generation,
            })
        };
        let protection = cur.read_u32::<T>()?;
        let flags = cur.read_u32::<T>()?;
        let path = cur.read_string().unwrap_or(cur); // TODO: return error if no string terminator found

        Ok(Mmap2Record {
            pid,
            tid,
            address,
            length,
            page_offset,
            file_id,
            protection,
            flags,
            cpu_mode: CpuMode::from_misc(misc),
            path,
        })
    }
}

impl<'a> fmt::Debug for Mmap2Record<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_map()
            .entry(&"pid", &self.pid)
            .entry(&"tid", &self.tid)
            .entry(&"address", &HexValue(self.address))
            .entry(&"length", &HexValue(self.length))
            .entry(&"page_offset", &HexValue(self.page_offset))
            // .entry(&"major", &self.major)
            // .entry(&"minor", &self.minor)
            // .entry(&"inode", &self.inode)
            // .entry(&"inode_generation", &self.inode_generation)
            .entry(&"protection", &HexValue(self.protection as _))
            .entry(&"flags", &HexValue(self.flags as _))
            .entry(&"cpu_mode", &self.cpu_mode)
            .entry(&"path", &&*String::from_utf8_lossy(&self.path.as_slice()))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mmap2InodeAndVersion {
    pub major: u32,
    pub minor: u32,
    pub inode: u64,
    pub inode_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostRecord {
    pub id: u64,
    pub count: u64,
}

impl LostRecord {
    pub fn parse<T: ByteOrder>(data: RawData) -> Result<Self, std::io::Error> {
        let mut cur = data;

        let id = cur.read_u64::<T>()?;
        let count = cur.read_u64::<T>()?;
        Ok(LostRecord { id, count })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThrottleRecord {
    pub id: u64,
    pub timestamp: u64,
}

impl ThrottleRecord {
    pub fn parse<T: ByteOrder>(data: RawData) -> Result<Self, std::io::Error> {
        let mut cur = data;

        let timestamp = cur.read_u64::<T>()?;
        let id = cur.read_u64::<T>()?;
        Ok(ThrottleRecord { id, timestamp })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextSwitchRecord {
    In {
        prev_pid: Option<i32>,
        prev_tid: Option<i32>,
    },
    Out {
        next_pid: Option<i32>,
        next_tid: Option<i32>,
        preempted: TaskWasPreempted,
    },
}

impl ContextSwitchRecord {
    pub fn from_misc(misc: u16) -> Self {
        Self::from_misc_pid_tid(misc, None, None)
    }

    pub fn parse_cpu_wide<T: ByteOrder>(data: RawData, misc: u16) -> Result<Self, std::io::Error> {
        let mut cur = data;

        let pid = cur.read_i32::<T>()?;
        let tid = cur.read_i32::<T>()?;
        Ok(Self::from_misc_pid_tid(misc, Some(pid), Some(tid)))
    }

    pub fn from_misc_pid_tid(misc: u16, pid: Option<i32>, tid: Option<i32>) -> Self {
        let is_out = misc & PERF_RECORD_MISC_SWITCH_OUT != 0;
        if is_out {
            let is_out_preempt = misc & PERF_RECORD_MISC_SWITCH_OUT_PREEMPT != 0;
            ContextSwitchRecord::Out {
                next_pid: pid,
                next_tid: tid,
                preempted: if is_out_preempt {
                    TaskWasPreempted::Yes
                } else {
                    TaskWasPreempted::No
                },
            }
        } else {
            ContextSwitchRecord::In {
                prev_pid: pid,
                prev_tid: tid,
            }
        }
    }
}

/// Whether a task was in the `TASK_RUNNING` state when it was switched
/// away from.
///
/// This helps understanding whether a workload is CPU or IO bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskWasPreempted {
    /// When switching out, the task was in the `TASK_RUNNING` state.
    Yes,
    /// When switching out, the task was in a non-running state.
    No,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RawRecord<'a> {
    pub record_type: RecordType,
    pub misc: u16,
    pub data: RawData<'a>,
    pub parse_info: RecordParseInfo,
}

impl<'a> RawRecord<'a> {
    pub fn new(
        record_type: RecordType,
        misc: u16,
        data: RawData<'a>,
        parse_info: RecordParseInfo,
    ) -> Self {
        Self {
            record_type,
            misc,
            data,
            parse_info,
        }
    }

    pub fn common_data(&self) -> Result<CommonData, std::io::Error> {
        if self.record_type.is_user_type() {
            return Ok(Default::default());
        }

        if self.record_type == RecordType::SAMPLE {
            CommonData::parse_sample(self.data, &self.parse_info)
        } else {
            CommonData::parse_nonsample(self.data, &self.parse_info)
        }
    }

    pub fn timestamp(&self) -> Option<u64> {
        match self.parse_info.endian {
            Endianness::LittleEndian => self.timestamp_impl::<LittleEndian>(),
            Endianness::BigEndian => self.timestamp_impl::<BigEndian>(),
        }
    }

    pub fn timestamp_impl<T: ByteOrder>(&self) -> Option<u64> {
        get_record_timestamp::<T>(self.record_type, self.data, &self.parse_info)
    }

    pub fn id(&self) -> Option<u64> {
        match self.parse_info.endian {
            Endianness::LittleEndian => self.id_impl::<LittleEndian>(),
            Endianness::BigEndian => self.id_impl::<BigEndian>(),
        }
    }

    pub fn id_impl<T: ByteOrder>(&self) -> Option<u64> {
        get_record_id::<T>(self.record_type, self.data, &self.parse_info.id_parse_info)
    }

    pub fn parse(&self) -> Result<ParsedRecord<'a>, std::io::Error> {
        match self.parse_info.endian {
            Endianness::LittleEndian => self.parse_impl::<LittleEndian>(),
            Endianness::BigEndian => self.parse_impl::<BigEndian>(),
        }
    }

    fn parse_impl<T: ByteOrder>(&self) -> Result<ParsedRecord<'a>, std::io::Error> {
        let parse_info = &self.parse_info;
        let event = match self.record_type {
            // Kernel built-in record types
            RecordType::MMAP => ParsedRecord::Mmap(MmapRecord::parse::<T>(self.data, self.misc)?),
            RecordType::LOST => ParsedRecord::Lost(LostRecord::parse::<T>(self.data)?),
            RecordType::COMM => {
                ParsedRecord::Comm(CommOrExecRecord::parse::<T>(self.data, self.misc)?)
            }
            RecordType::EXIT => ParsedRecord::Exit(ForkOrExitRecord::parse::<T>(self.data)?),
            RecordType::THROTTLE => ParsedRecord::Throttle(ThrottleRecord::parse::<T>(self.data)?),
            RecordType::UNTHROTTLE => {
                ParsedRecord::Unthrottle(ThrottleRecord::parse::<T>(self.data)?)
            }
            RecordType::FORK => ParsedRecord::Fork(ForkOrExitRecord::parse::<T>(self.data)?),
            // READ
            RecordType::SAMPLE => {
                ParsedRecord::Sample(SampleRecord::parse::<T>(self.data, self.misc, parse_info)?)
            }
            RecordType::MMAP2 => {
                ParsedRecord::Mmap2(Mmap2Record::parse::<T>(self.data, self.misc)?)
            }
            // AUX
            // ITRACE_START
            // LOST_SAMPLES
            RecordType::SWITCH => {
                ParsedRecord::ContextSwitch(ContextSwitchRecord::from_misc(self.misc))
            }
            RecordType::SWITCH_CPU_WIDE => ParsedRecord::ContextSwitch(
                ContextSwitchRecord::parse_cpu_wide::<T>(self.data, self.misc)?,
            ),
            // NAMESPACES
            // KSYMBOL
            // BPF_EVENT
            // CGROUP
            // TEXT_POKE
            // AUX_OUTPUT_HW_ID

            // Record types added by the `perf` tool from user space

            // HEADER_ATTR
            // HEADER_EVENT_TYPE
            // HEADER_TRACING_DATA
            // HEADER_BUILD_ID
            // FINISHED_ROUND
            // ID_INDEX
            // AUXTRACE_INFO
            // AUXTRACE
            // AUXTRACE_ERROR
            RecordType::THREAD_MAP => ParsedRecord::ThreadMap(ThreadMap::parse::<T>(self.data)?),
            // CPU_MAP
            // STAT_CONFIG
            // STAT
            // STAT_ROUND
            // EVENT_UPDATE
            // TIME_CONV
            // HEADER_FEATURE
            // COMPRESSED
            _ => ParsedRecord::Raw(self.clone()),
        };
        Ok(event)
    }
}

impl<'a> fmt::Debug for RawRecord<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_map()
            .entry(&"record_type", &self.record_type)
            .entry(&"misc", &self.misc)
            .entry(&"data.len", &self.data.len())
            .finish()
    }
}
