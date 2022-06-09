use crate::raw_data::RawData;
use crate::utils::HexValue;
use crate::{
    constants, CommonData, CpuMode, Endianness, RecordIdParseInfo, RecordParseInfo, RecordType,
    SampleRecord,
};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::fmt;

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

/// A fully parsed event record.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum EventRecord<'a> {
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
    Raw(RawEventRecord<'a>),
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
        let is_execve = misc & constants::PERF_RECORD_MISC_COMM_EXEC != 0;

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
        let is_executable = misc & constants::PERF_RECORD_MISC_MMAP_DATA == 0;

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
        let file_id = if misc & constants::PERF_RECORD_MISC_MMAP_BUILD_ID != 0 {
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
        let is_out = misc & constants::PERF_RECORD_MISC_SWITCH_OUT != 0;
        if is_out {
            let is_out_preempt = misc & constants::PERF_RECORD_MISC_SWITCH_OUT_PREEMPT != 0;
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

/// An unparsed event record.
///
/// This can be converted into a parsed record by calling `.parse()`.
///
/// The raw record also provides access to "common data" like the ID, timestamp,
/// tid etc., i.e. the information that was requested with [`SampleFormat`](crate::SampleFormat) and
/// [`AttrFlags::SAMPLE_ID_ALL`](crate::AttrFlags::SAMPLE_ID_ALL).
#[derive(Clone, PartialEq, Eq)]
pub struct RawEventRecord<'a> {
    /// The record type. Must be a builtin type, i.e. not a user type.
    pub record_type: RecordType,
    /// The `misc` value on this record.
    pub misc: u16,
    /// The raw bytes in the body of this record.
    pub data: RawData<'a>,
    /// The parse info from our corresponding evnt's attr.
    pub parse_info: RecordParseInfo,
}

impl<'a> RawEventRecord<'a> {
    /// Create a new `RawEventRecord`. Must only be called if `record_type.is_builtin_type()` is `true`.
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

    /// Parse "common data" on this record, see [`CommonData`].
    ///
    /// The available information is determined by the event attr, specifically
    /// by the requested [`SampleFormat`](crate::SampleFormat) and by the
    /// presence of the [`AttrFlags::SAMPLE_ID_ALL`](crate::AttrFlags::SAMPLE_ID_ALL)
    /// flag: The `SampleFormat` determines the available fields, and the
    /// `SAMPLE_ID_ALL` flag determines the record types on which these fields
    /// are available. If `SAMPLE_ID_ALL` is set, the requested fields are
    /// available on all records, otherwise only on sample records
    /// ([`RecordType::SAMPLE`]).
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

    /// The record timestamp, if available.
    pub fn timestamp(&self) -> Option<u64> {
        match self.parse_info.endian {
            Endianness::LittleEndian => self.timestamp_impl::<LittleEndian>(),
            Endianness::BigEndian => self.timestamp_impl::<BigEndian>(),
        }
    }

    fn timestamp_impl<T: ByteOrder>(&self) -> Option<u64> {
        get_record_timestamp::<T>(self.record_type, self.data, &self.parse_info)
    }

    /// The ID, if available.
    pub fn id(&self) -> Option<u64> {
        match self.parse_info.endian {
            Endianness::LittleEndian => self.id_impl::<LittleEndian>(),
            Endianness::BigEndian => self.id_impl::<BigEndian>(),
        }
    }

    fn id_impl<T: ByteOrder>(&self) -> Option<u64> {
        get_record_id::<T>(self.record_type, self.data, &self.parse_info.id_parse_info)
    }

    /// Parses this raw record into an [`EventRecord`].
    pub fn parse(&self) -> Result<EventRecord<'a>, std::io::Error> {
        match self.parse_info.endian {
            Endianness::LittleEndian => self.parse_impl::<LittleEndian>(),
            Endianness::BigEndian => self.parse_impl::<BigEndian>(),
        }
    }

    fn parse_impl<T: ByteOrder>(&self) -> Result<EventRecord<'a>, std::io::Error> {
        let parse_info = &self.parse_info;
        let event = match self.record_type {
            // Kernel built-in record types
            RecordType::MMAP => EventRecord::Mmap(MmapRecord::parse::<T>(self.data, self.misc)?),
            RecordType::LOST => EventRecord::Lost(LostRecord::parse::<T>(self.data)?),
            RecordType::COMM => {
                EventRecord::Comm(CommOrExecRecord::parse::<T>(self.data, self.misc)?)
            }
            RecordType::EXIT => EventRecord::Exit(ForkOrExitRecord::parse::<T>(self.data)?),
            RecordType::THROTTLE => EventRecord::Throttle(ThrottleRecord::parse::<T>(self.data)?),
            RecordType::UNTHROTTLE => {
                EventRecord::Unthrottle(ThrottleRecord::parse::<T>(self.data)?)
            }
            RecordType::FORK => EventRecord::Fork(ForkOrExitRecord::parse::<T>(self.data)?),
            // READ
            RecordType::SAMPLE => {
                EventRecord::Sample(SampleRecord::parse::<T>(self.data, self.misc, parse_info)?)
            }
            RecordType::MMAP2 => EventRecord::Mmap2(Mmap2Record::parse::<T>(self.data, self.misc)?),
            // AUX
            // ITRACE_START
            // LOST_SAMPLES
            RecordType::SWITCH => {
                EventRecord::ContextSwitch(ContextSwitchRecord::from_misc(self.misc))
            }
            RecordType::SWITCH_CPU_WIDE => EventRecord::ContextSwitch(
                ContextSwitchRecord::parse_cpu_wide::<T>(self.data, self.misc)?,
            ),
            // NAMESPACES
            // KSYMBOL
            // BPF_EVENT
            // CGROUP
            // TEXT_POKE
            // AUX_OUTPUT_HW_ID
            _ => EventRecord::Raw(self.clone()),
        };
        Ok(event)
    }
}

impl<'a> fmt::Debug for RawEventRecord<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_map()
            .entry(&"record_type", &self.record_type)
            .entry(&"misc", &self.misc)
            .entry(&"data.len", &self.data.len())
            .finish()
    }
}
