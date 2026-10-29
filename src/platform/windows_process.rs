//! Fresh process inventory; only immutable birth times may be reused.
//!
//! Microsoft documents SystemBasicProcessInformation on Windows 11 26100.4770+:
//! https://learn.microsoft.com/windows/win32/api/winternl/nf-winternl-ntquerysysteminformation
//! Its kernel SequenceNumber detects PID reuse. Older/unsupported/malformed replies
//! fall back to the existing Toolhelp implementation. No executable, signature,
//! token, policy, module or authorization decision is cached here.
use super::{ProcessNode, ProcessTable, process_creation_time, profile};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::c_void,
    mem::size_of,
    ops::Range,
};
use windows::Win32::Foundation::UNICODE_STRING;

// The current windows crate predates this documented information class/header.
const SYSTEM_BASIC_PROCESS_INFORMATION: u32 = 252;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xc0000004u32 as i32;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_PROCESSES: usize = 65_536;
#[repr(C)]
#[derive(Clone, Copy)]
struct Header {
    next: u32,
    pid: usize,
    parent: usize,
    sequence: u64,
    name: UNICODE_STRING,
}
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQuerySystemInformation(class: u32, buffer: *mut c_void, size: u32, used: *mut u32) -> i32;
}
#[derive(Clone, Debug)]
struct Record {
    pid: u32,
    parent: u32,
    sequence: u64,
    name: Range<usize>,
}
#[derive(Clone, Copy)]
struct Birth {
    sequence: u64,
    time: u64,
    seen: u64,
}
#[derive(Default)]
struct Births {
    generation: u64,
    entries: HashMap<u32, Birth>,
}
impl Births {
    fn known(&self, record: &Record) -> Option<u64> {
        self.entries
            .get(&record.pid)
            .filter(|b| b.sequence == record.sequence)
            .map(|b| b.time)
    }
    fn covered(&self, record: &Record, sampled: &HashMap<u32, (u64, Option<u64>)>) -> bool {
        record.pid == 0
            || self.known(record).is_some()
            || sampled
                .get(&record.pid)
                .is_some_and(|(sequence, _)| *sequence == record.sequence)
    }
    fn begin(&mut self) {
        if self.generation == u64::MAX {
            self.entries.clear();
            self.generation = 0;
        }
        self.generation += 1;
    }
    fn resolve(
        &mut self,
        record: &Record,
        sampled: &HashMap<u32, (u64, Option<u64>)>,
    ) -> Option<u64> {
        let time = self.known(record).or_else(|| {
            sampled
                .get(&record.pid)
                .filter(|(seq, _)| *seq == record.sequence)
                .and_then(|(_, t)| *t)
        });
        if let Some(time) = time {
            self.entries.insert(
                record.pid,
                Birth {
                    sequence: record.sequence,
                    time,
                    seen: self.generation,
                },
            );
        } else {
            self.entries.remove(&record.pid);
        }
        time
    }
    fn finish(&mut self) {
        self.entries.retain(|_, b| b.seen == self.generation);
    }
}
#[derive(Default)]
struct Inventory {
    buffer: Vec<u8>,
    records: Vec<Record>,
    seen: HashSet<u32>,
    births: Births,
    sampled: HashMap<u32, (u64, Option<u64>)>,
}
thread_local! {
    static INVENTORY: RefCell<Inventory> = RefCell::new(Inventory::default());
}

pub(super) fn load() -> Option<ProcessTable> {
    INVENTORY.with(|inventory| {
        let mut inventory = inventory.try_borrow_mut().ok()?;
        let table = inventory.load();
        if table.is_none() {
            // A failed/changed interface cannot leave a partially reconciled cache.
            inventory.births.entries.clear();
        }
        table
    })
}
impl Inventory {
    fn query(&mut self) -> Option<()> {
        if self.buffer.is_empty() {
            self.buffer.resize(64 * 1024, 0);
        }
        for _ in 0..8 {
            let mut used = 0;
            // The kernel writes only within this initialized, caller-owned buffer.
            let status = unsafe {
                NtQuerySystemInformation(
                    SYSTEM_BASIC_PROCESS_INFORMATION,
                    self.buffer.as_mut_ptr().cast(),
                    self.buffer.len().try_into().ok()?,
                    &mut used,
                )
            };
            if status == STATUS_INFO_LENGTH_MISMATCH {
                let needed = (used as usize).max(self.buffer.len().checked_mul(2)?);
                if needed > MAX_BYTES {
                    return None;
                }
                self.buffer.resize(needed, 0);
                continue;
            }
            if status < 0 || used as usize > self.buffer.len() {
                return None;
            }
            return parse(
                &self.buffer[..used as usize],
                &mut self.records,
                &mut self.seen,
            );
        }
        None
    }
    fn load(&mut self) -> Option<ProcessTable> {
        self.query()?;
        self.sampled.clear();
        let mut hits = 0;
        for record in &self.records {
            if self.births.known(record).is_some() {
                hits += 1;
            } else if record.pid != 0 {
                self.sampled.insert(
                    record.pid,
                    (record.sequence, process_creation_time(record.pid)),
                );
            }
        }
        if !self.sampled.is_empty() {
            // The PID could have exited/reused while GetProcessTimes ran. Publish
            // a sampled birth ONLY when a second fresh kernel snapshot confirms
            // the same generation. Unknown/denied values are never retained.
            self.query()?;
        }
        // Generations born/reused between the two snapshots have no birth-time
        // sample. Do not publish weaker ancestry than Toolhelp: use that complete
        // compatibility path for this cycle and retry the optimization next time.
        if self
            .records
            .iter()
            .any(|record| !self.births.covered(record, &self.sampled))
        {
            return None;
        }
        self.births.begin();
        let mut by_name: HashMap<String, Vec<u32>> = HashMap::with_capacity(self.records.len());
        let mut by_pid = HashMap::with_capacity(self.records.len());
        for record in &self.records {
            let name = if record.pid == 0 && record.name.is_empty() {
                "[System Process]".to_owned()
            } else {
                // Validated ranges/alignment, decoded only once on the final snapshot.
                let units = unsafe {
                    std::slice::from_raw_parts(
                        self.buffer.as_ptr().add(record.name.start).cast::<u16>(),
                        record.name.len() / 2,
                    )
                };
                String::from_utf16_lossy(units)
            };
            let created_at_filetime = self.births.resolve(record, &self.sampled);
            by_name
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(record.pid);
            by_pid.insert(
                record.pid,
                ProcessNode {
                    parent_pid: record.parent,
                    name,
                    created_at_filetime,
                },
            );
        }
        self.births.finish();
        profile::inventory_work(
            "basic_process",
            self.sampled.len(),
            hits,
            self.births.entries.len(),
            self.buffer.capacity(),
        );
        Some(ProcessTable { by_name, by_pid })
    }
}

fn parse(buffer: &[u8], records: &mut Vec<Record>, seen: &mut HashSet<u32>) -> Option<()> {
    records.clear();
    seen.clear();
    let base = buffer.as_ptr() as usize;
    let mut offset = 0usize;
    loop {
        let header_end = offset.checked_add(size_of::<Header>())?;
        if header_end > buffer.len() || records.len() >= MAX_PROCESSES {
            return None;
        }
        // NT buffers are bytes; unaligned reads avoid relying on Vec<u8>'s alignment.
        let header = unsafe {
            buffer
                .as_ptr()
                .add(offset)
                .cast::<Header>()
                .read_unaligned()
        };
        let end = if header.next == 0 {
            buffer.len()
        } else {
            if (header.next as usize) < size_of::<Header>()
                || !(header.next as usize).is_multiple_of(size_of::<usize>())
            {
                return None;
            }
            offset
                .checked_add(header.next as usize)
                .filter(|end| *end <= buffer.len())?
        };
        let pid = header.pid.try_into().ok()?;
        let parent = header.parent.try_into().ok()?;
        if pid != 0 && header.sequence == 0 {
            return None;
        }
        let length = header.name.Length as usize;
        if !length.is_multiple_of(2) || header.name.MaximumLength < header.name.Length {
            return None;
        }
        let name = if length == 0 {
            if pid != 0 {
                return None;
            }
            header_end..header_end
        } else {
            let pointer = header.name.Buffer.as_ptr() as usize;
            if !pointer.is_multiple_of(std::mem::align_of::<u16>()) {
                return None;
            }
            let start = pointer.checked_sub(base)?;
            let name_end = start.checked_add(length)?;
            if start < header_end || name_end > end {
                return None;
            }
            start..name_end
        };
        if !seen.insert(pid) {
            return None;
        }
        records.push(Record {
            pid,
            parent,
            sequence: header.sequence,
            name,
        });
        if header.next == 0 {
            break;
        }
        offset = end;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::PWSTR;
    fn record(pid: u32, sequence: u64) -> Record {
        Record {
            pid,
            parent: 0,
            sequence,
            name: 0..0,
        }
    }
    fn buffer() -> Vec<u8> {
        let mut data = vec![0u8; size_of::<Header>() + 4];
        let header = Header {
            next: 0,
            pid: 12,
            parent: 4,
            sequence: 27,
            name: UNICODE_STRING {
                Length: 4,
                MaximumLength: 4,
                Buffer: PWSTR(unsafe { data.as_mut_ptr().add(size_of::<Header>()).cast() }),
            },
        };
        unsafe {
            data.as_mut_ptr().cast::<Header>().write_unaligned(header);
        }
        data[size_of::<Header>()..].copy_from_slice(&[b'a', 0, b'b', 0]);
        data
    }
    fn mutate(data: &mut [u8], change: impl FnOnce(&mut Header)) {
        let mut header = unsafe { data.as_ptr().cast::<Header>().read_unaligned() };
        change(&mut header);
        unsafe {
            data.as_mut_ptr().cast::<Header>().write_unaligned(header);
        }
    }
    #[test]
    fn immutable_birth_reused_only_for_same_kernel_generation() {
        let mut cache = Births::default();
        cache.begin();
        assert_eq!(
            cache.resolve(&record(12, 27), &HashMap::from([(12, (27, Some(100)))])),
            Some(100)
        );
        cache.finish();
        cache.begin();
        assert_eq!(cache.resolve(&record(12, 27), &HashMap::new()), Some(100));
        assert_eq!(
            cache.resolve(&record(12, 28), &HashMap::from([(12, (27, Some(100)))])),
            None
        );
        assert_eq!(cache.known(&record(12, 28)), None);
        assert_eq!(
            cache.resolve(&record(12, 28), &HashMap::from([(12, (28, Some(200)))])),
            Some(200)
        );
    }
    #[test]
    fn newly_arrived_and_reused_generations_require_a_matching_sample() {
        let mut cache = Births::default();
        let sampled = HashMap::from([(12, (27, Some(100)))]);
        cache.begin();
        cache.resolve(&record(12, 27), &sampled);
        assert!(cache.covered(&record(12, 27), &HashMap::new()));
        assert!(!cache.covered(&record(12, 28), &sampled));
        assert!(!cache.covered(&record(14, 30), &sampled));
        // Access denied is a genuine observation, not an unsampled arrival.
        assert!(cache.covered(&record(14, 30), &HashMap::from([(14, (30, None))])));
    }
    #[test]
    fn disappeared_and_denied_births_are_not_retained() {
        let mut cache = Births::default();
        cache.begin();
        cache.resolve(&record(12, 27), &HashMap::from([(12, (27, Some(100)))]));
        cache.finish();
        cache.begin();
        cache.resolve(&record(14, 30), &HashMap::from([(14, (30, None))]));
        cache.finish();
        assert!(cache.entries.is_empty());
    }
    #[test]
    fn parses_bounded_header_and_name_without_pointer_escape() {
        let data = buffer();
        let mut records = Vec::new();
        assert_eq!(parse(&data, &mut records, &mut HashSet::new()), Some(()));
        assert_eq!(
            (records[0].pid, records[0].parent, records[0].sequence),
            (12, 4, 27)
        );
        assert_eq!(&data[records[0].name.clone()], &[b'a', 0, b'b', 0]);
    }
    #[test]
    fn malformed_offsets_lengths_and_foreign_pointers_fail_closed() {
        for case in 0..7 {
            let mut data = buffer();
            mutate(&mut data, |h| match case {
                0 => h.next = 1,
                1 => h.next = u32::MAX,
                2 => h.name.Length = 3,
                3 => h.name.MaximumLength = 0,
                4 => h.name.Buffer = PWSTR(std::ptr::null_mut()),
                5 => h.name.Buffer = PWSTR(usize::MAX as *mut u16),
                _ => h.name.Buffer = PWSTR(std::ptr::dangling_mut()),
            });
            assert_eq!(
                parse(&data, &mut Vec::new(), &mut HashSet::new()),
                None,
                "case {case}"
            );
        }
        assert_eq!(parse(&[0u8; 1], &mut Vec::new(), &mut HashSet::new()), None);
    }
}
