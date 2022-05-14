use crate::consts::*;
use crate::types::*;
use byteorder::{ByteOrder, ReadBytesExt};
use std::io;
use std::io::Read;

/// `perf_event_header`
#[derive(Debug, Clone, Copy)]
pub struct PerfEventHeader {
    pub type_: u32,
    pub misc: u16,
    pub size: u16,
}

impl PerfEventHeader {
    pub const STRUCT_SIZE: usize = 4 + 2 + 2;

    pub fn parse<R: Read, T: ByteOrder>(reader: &mut R) -> Result<Self, std::io::Error> {
        let type_ = reader.read_u32::<T>()?;
        let misc = reader.read_u16::<T>()?;
        let size = reader.read_u16::<T>()?;
        Ok(Self { type_, misc, size })
    }
}

/// `perf_event_attr`
#[derive(Debug, Clone, Copy)]
pub struct PerfEventAttr {
    /// Major type: hardware/software/tracepoint/etc.
    pub type_: u32,
    /// Size of the attr structure, for fwd/bwd compat.
    pub size: u32,
    /// Type-specific configuration information.
    pub config: u64,

    /// If AttrFlags::FREQ is set in `flags`, this is the sample frequency,
    /// otherwise it is the sample period.
    ///
    /// ```c
    /// union {
    ///     /// Period of sampling
    ///     __u64 sample_period;
    ///     /// Frequency of sampling
    ///     __u64 sample_freq;
    /// };
    /// ```
    pub sampling_period_or_frequency: u64,

    /// Specifies values included in sample. (original name `sample_type`)
    pub sample_format: SampleFormat,

    /// Specifies the structure values returned by read() on a perf event fd,
    /// see [`ReadFormat`].
    pub read_format: ReadFormat,

    /// Bitset of flags.
    pub flags: AttrFlags,

    /// If AttrFlags::WATERMARK is set in `flags`, this is the watermark,
    /// otherwise it is the event count after which to wake up.
    ///
    /// ```c
    /// union {
    ///     /// wakeup every n events
    ///     __u32 wakeup_events;
    ///     /// bytes before wakeup
    ///     __u32 wakeup_watermark;
    /// };
    /// ```
    pub wakeup_events_or_watermark: u32,

    /// breakpoint type
    pub bp_type: HwBreakpointType,

    /// Union discriminator is ???
    ///
    /// ```c
    /// union {
    ///     __u64 bp_addr;
    ///     __u64 kprobe_func; /* for perf_kprobe */
    ///     __u64 uprobe_path; /* for perf_uprobe */
    ///     __u64 config1; /* extension of config */
    /// };
    /// ```
    pub bp_addr_or_kprobe_func_or_uprobe_func_or_config1: u64,

    /// Union discriminator is ???
    ///
    /// ```c
    /// union {
    ///     __u64 bp_len; /* breakpoint length, uses HW_BREAKPOINT_LEN_* constants */
    ///     __u64 kprobe_addr; /* when kprobe_func == NULL */
    ///     __u64 probe_offset; /* for perf_[k,u]probe */
    ///     __u64 config2; /* extension of config1 */
    /// };
    pub bp_len_or_kprobe_addr_or_probe_offset_or_config2: u64,

    /// Branch-sample specific flags.
    pub branch_sample_format: BranchSampleFormat,

    /// Defines set of user regs to dump on samples.
    /// See asm/perf_regs.h for details.
    pub sample_regs_user: u64,

    /// Defines size of the user stack to dump on samples.
    pub sample_stack_user: u32,

    /// The clock ID.
    pub clockid: ClockId,

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
        reader: &mut R,
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

        Ok(Self {
            type_,
            size,
            config,
            sampling_period_or_frequency,
            sample_format: SampleFormat::from_bits_truncate(sample_type),
            read_format: ReadFormat::from_bits_truncate(read_format),
            flags: AttrFlags::from_bits_truncate(flags),
            wakeup_events_or_watermark,
            bp_type: HwBreakpointType::from_bits_truncate(bp_type),
            bp_addr_or_kprobe_func_or_uprobe_func_or_config1,
            bp_len_or_kprobe_addr_or_probe_offset_or_config2,
            branch_sample_format: BranchSampleFormat::from_bits_truncate(branch_sample_type),
            sample_regs_user,
            sample_stack_user,
            clockid: ClockId::from_u32(clockid).ok_or(io::ErrorKind::InvalidInput)?,
            sample_regs_intr,
            aux_watermark,
            sample_max_stack,
            aux_sample_size,
            sig_data,
        })
    }
}
