# linux-perf-event-reader

This crate lets you parse Linux perf events and associated structures.

## Example

```rust
use linux_perf_event_reader::{Endianness, PerfEventAttr, RawData, RecordType};
use linux_perf_event_reader::records::{CommOrExecRecord, EventRecord, RawEventRecord, RecordParseInfo};

// Read the perf_event_attr data.
let attr_data = vec![
    0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 229, 3, 0, 0, 0, 0, 0, 0, 47, 177, 0,
    0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 3, 183, 215, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 15,
    255, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 104, 0, 0, 0, 0, 0, 0, 0, 128, 0,
    0, 0, 0, 0, 0, 0,
];
let (attr, _size) =
    PerfEventAttr::parse::<_, byteorder::LittleEndian>(&attr_data[..]).unwrap();
let parse_info = RecordParseInfo::new(&attr, Endianness::LittleEndian);

let body = b"lG\x08\0lG\x08\0dump_syms\0\0\0\0\0\0\0lG\x08\0lG\x08\08\x1b\xf8\x18hX\x04\0";
let body_raw_data = RawData::from(&body[..]);
let raw_record = RawEventRecord::new(RecordType::COMM, 0x2000, body_raw_data, parse_info);
let parsed_record = raw_record.parse().unwrap();

assert_eq!(
    parsed_record,
    EventRecord::Comm(CommOrExecRecord {
        pid: 542572,
        tid: 542572,
        name: RawData::Single(b"dump_syms"),
        is_execve: true
    })
);
```

## Acknowledgements

Some of the code in this repo was based on [**@koute**'s `not-perf` project](https://github.com/koute/not-perf/tree/20e4ddc2bf8895d96664ab839a64c36f416023c8/perf_event_open/src).

## License

Licensed under either of

  * Apache License, Version 2.0 ([`LICENSE-APACHE`](./LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
  * MIT license ([`LICENSE-MIT`](./LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
