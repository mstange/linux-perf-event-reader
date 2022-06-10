use crate::constants::*;
use crate::types::*;
use byteorder::{ByteOrder, ReadBytesExt};
use std::io;
use std::io::Read;
use std::num::NonZeroU64;

/// `perf_event_header`
#[derive(Debug, Clone, Copy)]
pub struct PerfEventHeader {
    pub type_: u32,
    pub misc: u16,
    pub size: u16,
}

impl PerfEventHeader {
    pub const STRUCT_SIZE: usize = 4 + 2 + 2;

    pub fn parse<R: Read, T: ByteOrder>(mut reader: R) -> Result<Self, std::io::Error> {
        let type_ = reader.read_u32::<T>()?;
        let misc = reader.read_u16::<T>()?;
        let size = reader.read_u16::<T>()?;
        Ok(Self { type_, misc, size })
    }
}

/// `perf_event_attr`
#[derive(Debug, Clone, Copy)]
pub struct PerfEventAttr {
    /// The type of the perf event.
    pub type_: PerfEventType,

    /// The sampling policy.
    pub sampling_policy: SamplingPolicy,

    /// Specifies values included in sample. (original name `sample_type`)
    pub sample_format: SampleFormat,

    /// Specifies the structure values returned by read() on a perf event fd,
    /// see [`ReadFormat`].
    pub read_format: ReadFormat,

    /// Bitset of flags.
    pub flags: AttrFlags,

    /// The wake-up policy.
    pub wakeup_policy: WakeupPolicy,

    /// Branch-sample specific flags.
    pub branch_sample_format: BranchSampleFormat,

    /// Defines set of user regs to dump on samples.
    /// See asm/perf_regs.h for details.
    pub sample_regs_user: u64,

    /// Defines size of the user stack to dump on samples.
    pub sample_stack_user: u32,

    /// The clock ID.
    pub clock: PerfClock,

    /// Defines set of regs to dump for each sample
    /// state captured on:
    ///  - precise = 0: PMU interrupt
    ///  - precise > 0: sampled instruction
    ///
    /// See asm/perf_regs.h for details.
    pub sample_regs_intr: u64,

    /// Wakeup watermark for AUX area
    pub aux_watermark: u32,

    /// When collecting stacks, this is the maximum number of stack frames
    /// (user + kernel) to collect.
    pub sample_max_stack: u16,

    /// When sampling AUX events, this is the size of the AUX sample.
    pub aux_sample_size: u32,

    /// User provided data if sigtrap=1, passed back to user via
    /// siginfo_t::si_perf_data, e.g. to permit user to identify the event.
    /// Note, siginfo_t::si_perf_data is long-sized, and sig_data will be
    /// truncated accordingly on 32 bit architectures.
    pub sig_data: u64,
}

impl PerfEventAttr {
    pub fn parse<R: Read, T: ByteOrder>(
        mut reader: R,
        size: Option<u32>,
    ) -> Result<Self, std::io::Error> {
        let type_ = reader.read_u32::<T>()?;
        let self_described_size = reader.read_u32::<T>()?;
        let config = reader.read_u64::<T>()?;

        let size = size.unwrap_or(self_described_size);
        if size < PERF_ATTR_SIZE_VER0 {
            return Err(io::ErrorKind::InvalidInput.into());
        }

        let sampling_period_or_frequency = reader.read_u64::<T>()?;
        let sample_type = reader.read_u64::<T>()?;
        let read_format = reader.read_u64::<T>()?;
        let flags = reader.read_u64::<T>()?;
        let wakeup_events_or_watermark = reader.read_u32::<T>()?;
        let bp_type = reader.read_u32::<T>()?;
        let bp_addr_or_kprobe_func_or_uprobe_func_or_config1 = reader.read_u64::<T>()?;

        let bp_len_or_kprobe_addr_or_probe_offset_or_config2 = if size >= PERF_ATTR_SIZE_VER1 {
            reader.read_u64::<T>()?
        } else {
            0
        };

        let branch_sample_type = if size >= PERF_ATTR_SIZE_VER2 {
            reader.read_u64::<T>()?
        } else {
            0
        };

        let (sample_regs_user, sample_stack_user, clockid) = if size >= PERF_ATTR_SIZE_VER3 {
            let sample_regs_user = reader.read_u64::<T>()?;
            let sample_stack_user = reader.read_u32::<T>()?;
            let clockid = reader.read_u32::<T>()?;

            (sample_regs_user, sample_stack_user, clockid)
        } else {
            (0, 0, 0)
        };

        let sample_regs_intr = if size >= PERF_ATTR_SIZE_VER4 {
            reader.read_u64::<T>()?
        } else {
            0
        };

        let (aux_watermark, sample_max_stack) = if size >= PERF_ATTR_SIZE_VER5 {
            let aux_watermark = reader.read_u32::<T>()?;
            let sample_max_stack = reader.read_u16::<T>()?;
            let __reserved_2 = reader.read_u16::<T>()?;
            (aux_watermark, sample_max_stack)
        } else {
            (0, 0)
        };

        let aux_sample_size = if size >= PERF_ATTR_SIZE_VER6 {
            let aux_sample_size = reader.read_u32::<T>()?;
            let __reserved_3 = reader.read_u32::<T>()?;
            aux_sample_size
        } else {
            0
        };

        let sig_data = if size >= PERF_ATTR_SIZE_VER7 {
            reader.read_u64::<T>()?
        } else {
            0
        };

        // Consume any remaining bytes.
        if size > PERF_ATTR_SIZE_VER7 {
            let remaining = size - PERF_ATTR_SIZE_VER7;
            io::copy(&mut reader.by_ref().take(remaining.into()), &mut io::sink())?;
        }

        let flags = AttrFlags::from_bits_truncate(flags);
        let type_ = PerfEventType::parse(
            type_,
            bp_type,
            config,
            bp_addr_or_kprobe_func_or_uprobe_func_or_config1,
            bp_len_or_kprobe_addr_or_probe_offset_or_config2,
        )
        .ok_or(io::ErrorKind::InvalidInput)?;

        // If AttrFlags::FREQ is set in `flags`, this is the sample frequency,
        // otherwise it is the sample period.
        //
        // ```c
        // union {
        //     /// Period of sampling
        //     __u64 sample_period;
        //     /// Frequency of sampling
        //     __u64 sample_freq;
        // };
        // ```
        let sampling_policy = if flags.contains(AttrFlags::FREQ) {
            SamplingPolicy::Frequency(sampling_period_or_frequency)
        } else if let Some(period) = NonZeroU64::new(sampling_period_or_frequency) {
            SamplingPolicy::Period(period)
        } else {
            SamplingPolicy::NoSampling
        };

        let wakeup_policy = if flags.contains(AttrFlags::WATERMARK) {
            WakeupPolicy::Watermark(wakeup_events_or_watermark)
        } else {
            WakeupPolicy::EventCount(wakeup_events_or_watermark)
        };

        let clock = if flags.contains(AttrFlags::USE_CLOCKID) {
            let clockid = ClockId::from_u32(clockid).ok_or(io::ErrorKind::InvalidInput)?;
            PerfClock::ClockId(clockid)
        } else {
            PerfClock::Default
        };

        Ok(Self {
            type_,
            sampling_policy,
            sample_format: SampleFormat::from_bits_truncate(sample_type),
            read_format: ReadFormat::from_bits_truncate(read_format),
            flags,
            wakeup_policy,
            branch_sample_format: BranchSampleFormat::from_bits_truncate(branch_sample_type),
            sample_regs_user,
            sample_stack_user,
            clock,
            sample_regs_intr,
            aux_watermark,
            sample_max_stack,
            aux_sample_size,
            sig_data,
        })
    }
}

/// The type of perf event
#[derive(Debug, Clone, Copy)]
pub enum PerfEventType {
    /// A hardware perf event. (`PERF_TYPE_HARDWARE`)
    Hardware(HardwareEventId, PmuTypeId),
    /// A software perf event. (`PERF_TYPE_SOFTWARE`)
    ///
    /// Special "software" events provided by the kernel, even if the hardware
    /// does not support performance events. These events measure various
    /// physical and sw events of the kernel (and allow the profiling of them as
    /// well).
    Software(SoftwareCounterType),
    /// A tracepoint perf event. (`PERF_TYPE_TRACEPOINT`)
    Tracepoint(u64),
    /// A hardware cache perf event. (`PERF_TYPE_HW_CACHE`)
    ///
    /// Selects a certain combination of CacheId, CacheOp, CacheOpResult, PMU type ID.
    ///
    /// ```plain
    /// { L1-D, L1-I, LLC, ITLB, DTLB, BPU, NODE } x
    /// { read, write, prefetch } x
    /// { accesses, misses }
    /// ```
    HwCache(
        HardwareCacheId,
        HardwareCacheOp,
        HardwareCacheOpResult,
        PmuTypeId,
    ),
    /// A hardware breakpoint perf event. (`PERF_TYPE_BREAKPOINT`)
    ///
    /// Breakpoints can be read/write accesses to an address as well as
    /// execution of an instruction address.
    Breakpoint(HwBreakpointType, HwBreakpointAddr, HwBreakpointLen),
    /// Dynamic PMU
    ///
    /// `(pmu, config, config1, config2)`
    ///
    /// Acceptable values for each of `config`, `config1` and `config2`
    /// parameters are defined by corresponding entries in
    /// `/sys/bus/event_source/devices/<pmu>/format/*`.
    ///
    /// From the `perf_event_open` man page:
    /// > Since Linux 2.6.38, perf_event_open() can support multiple PMUs.  To
    /// > enable this, a value exported by the kernel can be used in the type
    /// > field to indicate which PMU to use.  The value to use can be found in
    /// > the sysfs filesystem: there is a subdirectory per PMU instance under
    /// > /sys/bus/event_source/devices.  In each subdirectory there is a type
    /// > file whose content is an integer that can be used in the type field.
    /// > For instance, /sys/bus/event_source/devices/cpu/type contains the
    /// > value for the core CPU PMU, which is usually 4.
    ///
    /// (I don't fully understand this - the value 4 also means `PERF_TYPE_RAW`.
    /// Maybe the type `Raw` is just one of those dynamic PMUs, usually "core"?)
    ///
    /// Among the "dynamic PMU" values, there are two special values for
    /// kprobes and uprobes:
    ///
    /// > kprobe and uprobe (since Linux 4.17)
    /// > These two dynamic PMUs create a kprobe/uprobe and attach it to the
    /// > file descriptor generated by perf_event_open.  The kprobe/uprobe will
    /// > be destroyed on the destruction of the file descriptor.  See fields
    /// > kprobe_func, uprobe_path, kprobe_addr, and probe_offset for more details.
    ///
    /// ```c
    /// union {
    ///     __u64 kprobe_func; /* for perf_kprobe */
    ///     __u64 uprobe_path; /* for perf_uprobe */
    ///     __u64 config1; /* extension of config */
    /// };
    ///
    /// union {
    ///     __u64 kprobe_addr; /* when kprobe_func == NULL */
    ///     __u64 probe_offset; /* for perf_[k,u]probe */
    ///     __u64 config2; /* extension of config1 */
    /// };
    /// ```
    DynamicPmu(u32, u64, u64, u64),
}

/// PMU type ID
///
/// The PMU type ID allows selecting whether to observe only "atom", only "core",
/// or both. If the PMU type ID is zero, both "atom" and "core" are observed.
/// To observe just one of them, the PMU type ID needs to be set to the value of
/// `/sys/devices/cpu_atom/type` or of `/sys/devices/cpu_core/type`.
#[derive(Debug, Clone, Copy)]
pub struct PmuTypeId(pub u32);

/// The address of the breakpoint.
///
/// For execution breakpoints, this is the memory address of the instruction
/// of interest; for read and write breakpoints, it is the memory address of
/// the memory location of interest.
#[derive(Debug, Clone, Copy)]
pub struct HwBreakpointAddr(pub u64);

/// The length of the breakpoint being measured.
///
/// Options are `HW_BREAKPOINT_LEN_1`, `HW_BREAKPOINT_LEN_2`,
/// `HW_BREAKPOINT_LEN_4`, and `HW_BREAKPOINT_LEN_8`.  For an
/// execution breakpoint, set this to sizeof(long).
#[derive(Debug, Clone, Copy)]
pub struct HwBreakpointLen(pub u64);

impl PerfEventType {
    pub fn parse(
        type_: u32,
        bp_type: u32,
        config: u64,
        config1: u64,
        config2: u64,
    ) -> Option<Self> {
        let t = match type_ {
            PERF_TYPE_HARDWARE => {
                // Config format: 0xEEEEEEEE000000AA
                //
                //  - AA: hardware event ID
                //  - EEEEEEEE: PMU type ID
                let hardware_event_id = (config & 0xff) as u8;
                let pmu_type = PmuTypeId((config >> 32) as u32);
                Self::Hardware(HardwareEventId::parse(hardware_event_id)?, pmu_type)
            }
            PERF_TYPE_SOFTWARE => Self::Software(SoftwareCounterType::parse(config)?),
            PERF_TYPE_TRACEPOINT => Self::Tracepoint(config),
            PERF_TYPE_HW_CACHE => {
                // Config format: 0xEEEEEEEE00DDCCBB
                //
                //  - BB: hardware cache ID
                //  - CC: hardware cache op ID
                //  - DD: hardware cache op result ID
                //  - EEEEEEEE: PMU type ID
                let cache_id = config as u8;
                let cache_op_id = (config >> 8) as u8;
                let cache_op_result = (config >> 16) as u8;
                let pmu_type = PmuTypeId((config >> 32) as u32);
                Self::HwCache(
                    HardwareCacheId::parse(cache_id)?,
                    HardwareCacheOp::parse(cache_op_id)?,
                    HardwareCacheOpResult::parse(cache_op_result)?,
                    pmu_type,
                )
            }
            PERF_TYPE_BREAKPOINT => {
                let bp_type = HwBreakpointType::from_bits_truncate(bp_type);
                Self::Breakpoint(bp_type, HwBreakpointAddr(config1), HwBreakpointLen(config2))
            }
            _ => Self::DynamicPmu(type_, config, config1, config2),
            // PERF_TYPE_RAW is handled as part of DynamicPmu.
        };
        Some(t)
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum HardwareEventId {
    /// `PERF_COUNT_HW_CPU_CYCLES`
    CpuCycles,
    /// `PERF_COUNT_HW_INSTRUCTIONS`
    Instructions,
    /// `PERF_COUNT_HW_CACHE_REFERENCES`
    CacheReferences,
    /// `PERF_COUNT_HW_CACHE_MISSES`
    CacheMisses,
    /// `PERF_COUNT_HW_BRANCH_INSTRUCTIONS`
    BranchInstructions,
    /// `PERF_COUNT_HW_BRANCH_MISSES`
    BranchMisses,
    /// `PERF_COUNT_HW_BUS_CYCLES`
    BusCycles,
    /// `PERF_COUNT_HW_STALLED_CYCLES_FRONTEND`
    StalledCyclesFrontend,
    /// `PERF_COUNT_HW_STALLED_CYCLES_BACKEND`
    StalledCyclesBackend,
    /// `PERF_COUNT_HW_REF_CPU_CYCLES`
    RefCpuCycles,
}

impl HardwareEventId {
    pub fn parse(hardware_event_id: u8) -> Option<Self> {
        let t = match hardware_event_id {
            PERF_COUNT_HW_CPU_CYCLES => Self::CpuCycles,
            PERF_COUNT_HW_INSTRUCTIONS => Self::Instructions,
            PERF_COUNT_HW_CACHE_REFERENCES => Self::CacheReferences,
            PERF_COUNT_HW_CACHE_MISSES => Self::CacheMisses,
            PERF_COUNT_HW_BRANCH_INSTRUCTIONS => Self::BranchInstructions,
            PERF_COUNT_HW_BRANCH_MISSES => Self::BranchMisses,
            PERF_COUNT_HW_BUS_CYCLES => Self::BusCycles,
            PERF_COUNT_HW_STALLED_CYCLES_FRONTEND => Self::StalledCyclesFrontend,
            PERF_COUNT_HW_STALLED_CYCLES_BACKEND => Self::StalledCyclesBackend,
            PERF_COUNT_HW_REF_CPU_CYCLES => Self::RefCpuCycles,
            _ => return None,
        };
        Some(t)
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SoftwareCounterType {
    /// `PERF_COUNT_SW_CPU_CLOCK`
    CpuClock,
    /// `PERF_COUNT_SW_TASK_CLOCK`
    TaskClock,
    /// `PERF_COUNT_SW_PAGE_FAULTS`
    PageFaults,
    /// `PERF_COUNT_SW_CONTEXT_SWITCHES`
    ContextSwitches,
    /// `PERF_COUNT_SW_CPU_MIGRATIONS`
    CpuMigrations,
    /// `PERF_COUNT_SW_PAGE_FAULTS_MIN`
    PageFaultsMin,
    /// `PERF_COUNT_SW_PAGE_FAULTS_MAJ`
    PageFaultsMaj,
    /// `PERF_COUNT_SW_ALIGNMENT_FAULTS`
    AlignmentFaults,
    /// `PERF_COUNT_SW_EMULATION_FAULTS`
    EmulationFaults,
    /// `PERF_COUNT_SW_DUMMY`
    Dummy,
    /// `PERF_COUNT_SW_BPF_OUTPUT`
    BpfOutput,
    /// `PERF_COUNT_SW_CGROUP_SWITCHES`
    CgroupSwitches,
}

impl SoftwareCounterType {
    pub fn parse(config: u64) -> Option<Self> {
        let t = match config {
            PERF_COUNT_SW_CPU_CLOCK => Self::CpuClock,
            PERF_COUNT_SW_TASK_CLOCK => Self::TaskClock,
            PERF_COUNT_SW_PAGE_FAULTS => Self::PageFaults,
            PERF_COUNT_SW_CONTEXT_SWITCHES => Self::ContextSwitches,
            PERF_COUNT_SW_CPU_MIGRATIONS => Self::CpuMigrations,
            PERF_COUNT_SW_PAGE_FAULTS_MIN => Self::PageFaultsMin,
            PERF_COUNT_SW_PAGE_FAULTS_MAJ => Self::PageFaultsMaj,
            PERF_COUNT_SW_ALIGNMENT_FAULTS => Self::AlignmentFaults,
            PERF_COUNT_SW_EMULATION_FAULTS => Self::EmulationFaults,
            PERF_COUNT_SW_DUMMY => Self::Dummy,
            PERF_COUNT_SW_BPF_OUTPUT => Self::BpfOutput,
            PERF_COUNT_SW_CGROUP_SWITCHES => Self::CgroupSwitches,
            _ => return None,
        };
        Some(t)
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum HardwareCacheId {
    /// `PERF_COUNT_HW_CACHE_L1D`
    L1d,
    /// `PERF_COUNT_HW_CACHE_L1I`
    L1i,
    /// `PERF_COUNT_HW_CACHE_LL`
    Ll,
    /// `PERF_COUNT_HW_CACHE_DTLB`
    Dtlb,
    /// `PERF_COUNT_HW_CACHE_ITLB`
    Itlb,
    /// `PERF_COUNT_HW_CACHE_BPU`
    Bpu,
    /// `PERF_COUNT_HW_CACHE_NODE`
    Node,
}

impl HardwareCacheId {
    pub fn parse(cache_id: u8) -> Option<Self> {
        let rv = match cache_id {
            PERF_COUNT_HW_CACHE_L1D => Self::L1d,
            PERF_COUNT_HW_CACHE_L1I => Self::L1i,
            PERF_COUNT_HW_CACHE_LL => Self::Ll,
            PERF_COUNT_HW_CACHE_DTLB => Self::Dtlb,
            PERF_COUNT_HW_CACHE_ITLB => Self::Itlb,
            PERF_COUNT_HW_CACHE_BPU => Self::Bpu,
            PERF_COUNT_HW_CACHE_NODE => Self::Node,
            _ => return None,
        };
        Some(rv)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HardwareCacheOp {
    /// `PERF_COUNT_HW_CACHE_OP_READ`
    Read,
    /// `PERF_COUNT_HW_CACHE_OP_WRITE`
    Write,
    /// `PERF_COUNT_HW_CACHE_OP_PREFETCH`
    Prefetch,
}

impl HardwareCacheOp {
    pub fn parse(cache_op: u8) -> Option<Self> {
        match cache_op {
            PERF_COUNT_HW_CACHE_OP_READ => Some(Self::Read),
            PERF_COUNT_HW_CACHE_OP_WRITE => Some(Self::Write),
            PERF_COUNT_HW_CACHE_OP_PREFETCH => Some(Self::Prefetch),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HardwareCacheOpResult {
    /// `PERF_COUNT_HW_CACHE_RESULT_ACCESS`
    Access,
    /// `PERF_COUNT_HW_CACHE_RESULT_MISS`
    Miss,
}

impl HardwareCacheOpResult {
    pub fn parse(cache_op_result: u8) -> Option<Self> {
        match cache_op_result {
            PERF_COUNT_HW_CACHE_RESULT_ACCESS => Some(Self::Access),
            PERF_COUNT_HW_CACHE_RESULT_MISS => Some(Self::Miss),
            _ => None,
        }
    }
}

/// Sampling Policy
///
/// > Events can be set to notify when a threshold is crossed,
/// > indicating an overflow. [...]
/// >
/// > Overflows are generated only by sampling events (sample_period
/// > must have a nonzero value).
#[derive(Debug, Clone, Copy)]
pub enum SamplingPolicy {
    /// `NoSampling` means that the event is a count and not a sampling event.
    NoSampling,
    /// Sets a fixed sampling period for a sampling event, in the unit of the
    /// observed count / event.
    ///
    /// A "sampling" event is one that generates an overflow notification every
    /// N events, where N is given by the sampling period. A sampling event has
    /// a sampling period greater than zero.
    ///
    /// When an overflow occurs, requested data is recorded in the mmap buffer.
    /// The `SampleFormat` bitfield controls what data is recorded on each overflow.
    Period(NonZeroU64),
    /// Sets a frequency for a sampling event, in "samples per (wall-clock) second".
    ///
    /// This uses a dynamic period which is adjusted by the kernel to hit the
    /// desired frequency. The rate of adjustment is a timer tick.
    ///
    /// If `SampleFormat::PERIOD` is requested, the current period at the time of
    /// the sample is stored in the sample.
    Frequency(u64),
}

/// Wakeup policy for "overflow notifications". This controls the point at
/// which the `read` call completes. (TODO: double check this)
///
/// > There are two ways to generate overflow notifications.
/// >
/// > The first is to set a `WakeupPolicy`
/// > that will trigger if a certain number of samples or bytes have
/// > been written to the mmap ring buffer.
/// >
/// > The other way is by use of the PERF_EVENT_IOC_REFRESH ioctl.
/// > This ioctl adds to a counter that decrements each time the event
/// > overflows.  When nonzero, POLLIN is indicated, but once the
/// > counter reaches 0 POLLHUP is indicated and the underlying event
/// > is disabled.
#[derive(Debug, Clone, Copy)]
pub enum WakeupPolicy {
    /// Wake up every time N records of type `RecordType::SAMPLE` have been
    /// written to the mmap ring buffer.
    EventCount(u32),
    /// Wake up after N bytes of any record type have been written to the mmap
    /// ring buffer.
    ///
    /// To receive a wakeup after every single record, choose `Watermark(1)`.
    /// `Watermark(0)` is treated the same as `Watermark(1)`.
    Watermark(u32),
}

/// This allows selecting which internal Linux clock to use when generating
/// timestamps.
///
/// Setting a specific ClockId can make it easier to correlate perf sample
/// times with timestamps generated by other tools. For example, when sampling
/// applications which emit JITDUMP information, you'll usually select the
/// moonotonic clock. This makes it possible to correctly order perf event
/// records and JITDUMP records - those also usually use the monotonic clock.
#[derive(Debug, Clone, Copy)]
pub enum PerfClock {
    /// The default clock. If this is used, the timestamps in event records
    /// are obtained with `local_clock()` which is a hardware timestamp if
    /// available and the jiffies value if not.
    ///
    /// In practice, on x86_64 this seems to use ktime_get_ns() which is the
    /// number of nanoseconds since boot.
    Default,

    /// A specific clock.
    ClockId(ClockId),
}
