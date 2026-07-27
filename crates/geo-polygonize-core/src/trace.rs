//! Internal bounded topology trace schema.

use crate::PolygonizerOptions;
use serde::Serialize;

pub const TOPOLOGY_TRACE_V1_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceLevelV1 {
    Summary,
    Noding,
    Graph,
    Rings,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStageV1 {
    Summary,
    Noding,
    Graph,
    Rings,
    Output,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TraceEventV1 {
    pub sequence: usize,
    pub stage: TraceStageV1,
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopologyTraceV1 {
    pub schema_version: u32,
    pub library_version: String,
    pub level: TraceLevelV1,
    pub byte_limit: usize,
    pub bytes_used: usize,
    pub truncated: bool,
    pub options: serde_json::Value,
    pub events: Vec<TraceEventV1>,
}

/// Collects serialized trace events up to an exact byte budget.
///
/// Callers hold this as an `Option`; `None` is the disabled fast path.
pub struct TraceRecorderV1 {
    trace: TopologyTraceV1,
}

impl TraceRecorderV1 {
    pub fn new(
        level: Option<TraceLevelV1>,
        byte_limit: usize,
        options: &PolygonizerOptions,
    ) -> Option<Self> {
        level.map(|level| Self {
            trace: TopologyTraceV1 {
                schema_version: TOPOLOGY_TRACE_V1_SCHEMA_VERSION,
                library_version: env!("CARGO_PKG_VERSION").to_string(),
                level,
                byte_limit,
                bytes_used: 0,
                truncated: false,
                options: serde_json::to_value(options).expect("validated options serialize"),
                events: Vec::new(),
            },
        })
    }

    /// Records an allowed event, returning false when the byte budget is exhausted.
    pub fn record(
        &mut self,
        stage: TraceStageV1,
        kind: impl Into<String>,
        payload: serde_json::Value,
    ) -> bool {
        if self.trace.truncated || !self.trace.level.allows(stage) {
            return !self.trace.truncated;
        }
        let event = TraceEventV1 {
            sequence: self.trace.events.len(),
            stage,
            kind: kind.into(),
            payload,
        };
        let event_bytes = serde_json::to_vec(&event)
            .expect("trace event serializes")
            .len();
        let Some(bytes_used) = self.trace.bytes_used.checked_add(event_bytes) else {
            self.trace.truncated = true;
            return false;
        };
        if bytes_used > self.trace.byte_limit {
            self.trace.truncated = true;
            return false;
        }
        self.trace.bytes_used = bytes_used;
        self.trace.events.push(event);
        true
    }

    pub fn finish(self) -> TopologyTraceV1 {
        self.trace
    }
}

impl TraceLevelV1 {
    fn allows(self, stage: TraceStageV1) -> bool {
        stage == TraceStageV1::Summary
            || matches!(
                (self, stage),
                (Self::Noding, TraceStageV1::Noding)
                    | (Self::Graph, TraceStageV1::Graph)
                    | (Self::Rings, TraceStageV1::Rings)
                    | (Self::Full, _)
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn disabled_trace_allocates_no_recorder_and_enabled_trace_is_bounded() {
        let options = PolygonizerOptions::default();
        assert!(TraceRecorderV1::new(None, 1024, &options).is_none());

        let mut recorder = TraceRecorderV1::new(Some(TraceLevelV1::Noding), 120, &options).unwrap();
        assert!(recorder.record(
            TraceStageV1::Graph,
            "ignored",
            json!({"large": "x".repeat(500)})
        ));
        assert!(recorder.record(TraceStageV1::Noding, "candidate", json!({"pair": [1, 2]})));
        assert!(!recorder.record(
            TraceStageV1::Noding,
            "oversized",
            json!({"large": "x".repeat(500)})
        ));

        let trace = recorder.finish();
        assert_eq!(trace.schema_version, TOPOLOGY_TRACE_V1_SCHEMA_VERSION);
        assert_eq!(trace.events.len(), 1);
        assert_eq!(trace.events[0].sequence, 0);
        assert!(trace.bytes_used <= trace.byte_limit);
        assert!(trace.truncated);
        assert!(trace.options.is_object());
    }
}
