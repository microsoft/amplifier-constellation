//! Opt-in synchronous write timing. No paths, request IDs or payloads are recorded.
//!
//! Phase times are inclusive and nested; they must not be summed. One record is
//! emitted after dispatch, so stderr I/O is outside every measured phase.
use std::{cell::RefCell, collections::BTreeMap, io::Write, time::Instant};

struct Record {
    operation: &'static str,
    bytes: usize,
    relative_components: usize,
    started: Instant,
    phases: BTreeMap<&'static str, (u64, u128)>,
}

thread_local! {
    static CURRENT: RefCell<Option<Record>> = const { RefCell::new(None) };
}

/// Owns only the timing record, never an operation or native handle.
pub struct Operation;

impl Operation {
    pub fn begin(operation: &'static str, bytes: usize, relative_components: usize) -> Self {
        CURRENT.with(|current| {
            *current.borrow_mut() = Some(Record {
                operation,
                bytes,
                relative_components,
                started: Instant::now(),
                phases: BTreeMap::new(),
            });
        });
        Self
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        let record = CURRENT.with(|current| current.borrow_mut().take());
        if let Some(record) = record {
            let elapsed_ns = record.started.elapsed().as_nanos();
            let phases: BTreeMap<_, _> = record
                .phases
                .into_iter()
                .map(|(name, (calls, elapsed_ns))| {
                    (
                        name,
                        serde_json::json!({"calls": calls, "inclusive_ns": elapsed_ns}),
                    )
                })
                .collect();
            let output = serde_json::json!({
                "native_write_diagnostic": 1,
                "operation": record.operation,
                "bytes": record.bytes,
                "relative_components": record.relative_components,
                "dispatch_inclusive_ns": elapsed_ns,
                "phases": phases,
            });
            // A diagnostic output failure must not replace the native outcome.
            let _ = writeln!(std::io::stderr().lock(), "{output}");
        }
    }
}

/// Times an existing region only while a diagnostic dispatch owns this thread.
pub struct Phase {
    name: &'static str,
    started: Option<Instant>,
}

impl Phase {
    pub fn begin(name: &'static str) -> Self {
        Self {
            name,
            started: CURRENT.with(|current| current.borrow().as_ref().map(|_| Instant::now())),
        }
    }
}

impl Drop for Phase {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed().as_nanos();
            CURRENT.with(|current| {
                if let Some(record) = current.borrow_mut().as_mut() {
                    let entry = record.phases.entry(self.name).or_default();
                    entry.0 += 1;
                    entry.1 += elapsed;
                }
            });
        }
    }
}
