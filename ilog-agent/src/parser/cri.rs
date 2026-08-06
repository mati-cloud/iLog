//! CRI log format parser.
//!
//! The kubelet does not hand application output to disk verbatim. Every line
//! written to a container's stdout/stderr arrives in `/var/log/pods/...` wrapped
//! in the CRI envelope:
//!
//! ```text
//! 2026-08-06T10:00:00.123456789Z stdout F {"level":"info","msg":"hello"}
//! <RFC3339Nano>                  <stream> <tags> <content>
//! ```
//!
//! `tags` is a comma-separated list whose first entry is the partial indicator:
//! `F` for a full line, `P` when the runtime split an oversized line (the CRI
//! limit is 16KiB). A logical line therefore spans N `P` records followed by one
//! `F`, and only the `F` completes it.
//!
//! This is the container-runtime framing, not a vendor log format -- reading a
//! pod log without it is like reading a `.gz` without gunzip. Once the envelope
//! is off, the payload is handed to the inner parser (JSON by default), so
//! structured application logs keep their fields instead of collapsing into one
//! opaque string.
//!
//! ## Statefulness
//!
//! Reassembly means this parser holds a buffer across calls, so **one instance
//! per file**. [`crate::providers::file`] builds parsers per tailed path rather
//! than sharing one across a glob expansion, which is what makes the `Mutex`
//! here uncontended and correct: two files interleaving into one buffer would
//! splice unrelated lines together.

use super::{FieldMapping, LogParser};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// Upper bound on a reassembled line, in bytes.
///
/// A container emitting `P` records with no terminating `F` would otherwise grow
/// the buffer without limit -- a malformed or truncated log becomes an agent
/// OOM. At the CRI 16KiB split size this allows ~64 continuation records, far
/// past any real log line, so hitting it means the stream is broken rather than
/// merely verbose. On overflow the buffer is flushed as-is: emitting a truncated
/// line loses trailing bytes, dropping it loses the whole event.
const MAX_REASSEMBLED: usize = 1024 * 1024;

pub struct CriParser {
    /// Inner parser for the unwrapped payload. `None` treats the payload as
    /// plain text and stores it under `message`.
    inner: Option<Box<dyn LogParser>>,
    fields: Vec<FieldMapping>,
    /// Accumulated `P` records awaiting their `F`. Empty in the common case.
    partial: Mutex<String>,
}

impl CriParser {
    pub fn new(inner: Option<Box<dyn LogParser>>, fields: Vec<FieldMapping>) -> Self {
        Self {
            inner,
            fields,
            partial: Mutex::new(String::new()),
        }
    }

    /// Split a CRI record into `(timestamp, stream, tags, content)`.
    ///
    /// Returns `None` when the line does not carry the envelope at all, which
    /// the caller treats as a plain-text line rather than an error -- a file
    /// configured as `cri` that turns out not to be a pod log should still
    /// produce readable events.
    fn split_envelope(line: &str) -> Option<(&str, &str, &str, &str)> {
        let (timestamp, rest) = line.split_once(' ')?;
        let (stream, rest) = rest.split_once(' ')?;

        // Only stdout/stderr are valid CRI streams. Checking this is what keeps
        // an arbitrary space-delimited log line from being mistaken for an
        // envelope and having its first two words eaten.
        if stream != "stdout" && stream != "stderr" {
            return None;
        }

        // The content itself may be empty (a bare newline from the container),
        // in which case there is no separator after the tags.
        let (tags, content) = match rest.split_once(' ') {
            Some((tags, content)) => (tags, content),
            None => (rest, ""),
        };

        Some((timestamp, stream, tags, content))
    }

    /// Parse the payload and merge it into `out`.
    ///
    /// A payload that the inner parser rejects is not dropped: it lands in
    /// `message` verbatim. Container stdout routinely mixes structured JSON with
    /// startup banners and panics, and losing the panic would be worse than
    /// losing its structure.
    fn merge_payload(&self, content: &str, out: &mut HashMap<String, Value>) {
        match self.inner.as_ref().and_then(|p| p.parse(content)) {
            Some(parsed) => out.extend(parsed),
            None => {
                out.insert("message".to_string(), Value::String(content.to_string()));
            }
        }
    }
}

impl LogParser for CriParser {
    fn parse(&self, line: &str) -> Option<HashMap<String, Value>> {
        let Some((timestamp, stream, tags, content)) = Self::split_envelope(line) else {
            // Not CRI-framed. Fall through to the inner parser so the line is
            // still usable.
            let mut out = HashMap::new();
            self.merge_payload(line, &mut out);
            return Some(out);
        };

        // The partial flag is the first tag; the rest are reserved by the CRI
        // spec and ignored here.
        let is_partial = tags.split(',').next() == Some("P");

        let mut buf = self
            .partial
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if is_partial {
            buf.push_str(content);
            if buf.len() < MAX_REASSEMBLED {
                // Incomplete logical line: no event yet.
                return None;
            }
            // Overflow -- emit what we have rather than growing further.
        } else if buf.is_empty() {
            // Fast path: a complete line with nothing buffered, no allocation.
            drop(buf);
            let mut out = HashMap::new();
            out.insert("k8s.stream".to_string(), Value::String(stream.to_string()));
            out.insert("k8s.time".to_string(), Value::String(timestamp.to_string()));
            self.merge_payload(content, &mut out);
            return Some(apply_mappings(out, &self.fields));
        } else {
            buf.push_str(content);
        }

        let assembled = std::mem::take(&mut *buf);
        drop(buf);

        let mut out = HashMap::new();
        out.insert("k8s.stream".to_string(), Value::String(stream.to_string()));
        out.insert("k8s.time".to_string(), Value::String(timestamp.to_string()));
        self.merge_payload(&assembled, &mut out);
        Some(apply_mappings(out, &self.fields))
    }
}

/// Rename/select fields per the source's `fields:` block.
///
/// Empty mappings mean "keep everything", matching [`super::JsonParser`]. When
/// mappings are present they select rather than augment, so a config that maps
/// only `message` gets only `message`.
fn apply_mappings(
    parsed: HashMap<String, Value>,
    fields: &[FieldMapping],
) -> HashMap<String, Value> {
    if fields.is_empty() {
        return parsed;
    }

    let mut result = HashMap::new();
    for field in fields {
        if let Some(value) = parsed.get(&field.from) {
            result.insert(field.name.clone(), value.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::JsonParser;

    fn json_cri() -> CriParser {
        CriParser::new(Some(Box::new(JsonParser::new(vec![]))), vec![])
    }

    #[test]
    fn full_line_json_payload_keeps_structure() {
        let p = json_cri();
        let got = p
            .parse(r#"2026-08-06T10:00:00.123456789Z stdout F {"level":"info","msg":"hello"}"#)
            .expect("full line yields an event");

        assert_eq!(got.get("k8s.stream").unwrap(), "stdout");
        assert_eq!(got.get("k8s.time").unwrap(), "2026-08-06T10:00:00.123456789Z");
        // The point of the nested parse: inner fields survive as fields.
        assert_eq!(got.get("level").unwrap(), "info");
        assert_eq!(got.get("msg").unwrap(), "hello");
    }

    #[test]
    fn stderr_stream_is_recorded() {
        let p = json_cri();
        let got = p.parse("2026-08-06T10:00:00Z stderr F boom").unwrap();
        assert_eq!(got.get("k8s.stream").unwrap(), "stderr");
    }

    #[test]
    fn partial_records_reassemble_into_one_event() {
        let p = json_cri();

        // P records are incomplete -- they must not emit.
        assert!(p.parse(r#"2026-08-06T10:00:00Z stdout P {"msg":"hel"#).is_none());
        assert!(p.parse(r#"2026-08-06T10:00:00Z stdout P lo wor"#).is_none());

        let got = p
            .parse(r#"2026-08-06T10:00:00Z stdout F ld"}"#)
            .expect("the F record completes the line");

        // Reassembled payload parses as JSON; splitting would have broken it.
        assert_eq!(got.get("msg").unwrap(), "hello world");
    }

    #[test]
    fn buffer_is_clear_after_reassembly() {
        let p = json_cri();
        p.parse(r#"2026-08-06T10:00:00Z stdout P {"msg":"a"#);
        p.parse(r#"2026-08-06T10:00:00Z stdout F "}"#);

        // A following independent line must not carry leftovers.
        let got = p
            .parse(r#"2026-08-06T10:00:01Z stdout F {"msg":"b"}"#)
            .unwrap();
        assert_eq!(got.get("msg").unwrap(), "b");
    }

    #[test]
    fn plain_text_payload_falls_back_to_message() {
        let p = json_cri();
        let got = p
            .parse("2026-08-06T10:00:00Z stdout F Server listening on :8080")
            .unwrap();

        // Not JSON, but the envelope was still stripped.
        assert_eq!(got.get("message").unwrap(), "Server listening on :8080");
        assert_eq!(got.get("k8s.stream").unwrap(), "stdout");
    }

    #[test]
    fn non_cri_line_is_not_mangled() {
        let p = json_cri();
        // A plain line whose first two words must not be eaten as an envelope.
        let got = p.parse("some arbitrary log line").unwrap();
        assert_eq!(got.get("message").unwrap(), "some arbitrary log line");
        assert!(!got.contains_key("k8s.stream"));
    }

    #[test]
    fn empty_content_is_handled() {
        let p = json_cri();
        // A bare newline from the container: no separator after the tags.
        let got = p.parse("2026-08-06T10:00:00Z stdout F").unwrap();
        assert_eq!(got.get("message").unwrap(), "");
    }

    #[test]
    fn multi_tag_partial_flag_is_read() {
        let p = json_cri();
        // First tag is the partial indicator; later tags are reserved.
        assert!(p.parse("2026-08-06T10:00:00Z stdout P,extra chunk").is_none());
        let got = p.parse("2026-08-06T10:00:00Z stdout F,extra done").unwrap();
        assert_eq!(got.get("message").unwrap(), "chunkdone");
    }

    /// A verbatim line from `/var/log/pods/kube-system_traefik-.../traefik/0.log`
    /// on the k3s/containerd cluster this agent targets. Pins the envelope format
    /// to bytes observed on a real node rather than to the spec as remembered.
    #[test]
    fn real_containerd_traefik_line() {
        let line = r#"2026-08-06T07:05:49.598410896Z stdout F {"ClientAddr":"104.23.199.232:10427","DownstreamStatus":307,"Duration":13191322,"RequestHost":"app.monops.dev"}"#;

        let got = json_cri().parse(line).expect("real line parses");

        assert_eq!(got.get("k8s.stream").unwrap(), "stdout");
        assert_eq!(got.get("k8s.time").unwrap(), "2026-08-06T07:05:49.598410896Z");
        // Nested fields must survive as typed fields -- this is the whole reason
        // `format: cri` exists instead of a regex that flattens to `message`.
        assert_eq!(got.get("RequestHost").unwrap(), "app.monops.dev");
        assert_eq!(got.get("DownstreamStatus").unwrap(), 307);
        assert_eq!(got.get("Duration").unwrap(), 13191322);
    }

    #[test]
    fn field_mappings_select_and_rename() {
        let p = CriParser::new(
            Some(Box::new(JsonParser::new(vec![]))),
            vec![FieldMapping {
                name: "stream".to_string(),
                from: "k8s.stream".to_string(),
                field_type: crate::parser::config::FieldType::String,
            }],
        );
        let got = p
            .parse(r#"2026-08-06T10:00:00Z stdout F {"msg":"x"}"#)
            .unwrap();

        assert_eq!(got.get("stream").unwrap(), "stdout");
        // Mappings select rather than augment.
        assert!(!got.contains_key("msg"));
    }
}
