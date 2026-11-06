//! Complete public commentary, separate from mutable activity and final-answer
//! selection. ACP v1 deltas alone do not prove an utterance is complete.
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const META: &str = "com.microsoft.amplifier.message";
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

pub fn advertise(capabilities: &mut Value) {
    capabilities["_meta"][META]["completion"] = json!({"version":1});
}
pub fn supported(capabilities: &Value) -> bool {
    capabilities["_meta"][META]["completion"]["version"] == 1
}
pub fn completion(id: &str, phase: Option<&str>, text: &str) -> Value {
    let mut value = json!({"sessionUpdate":"agent_message_chunk","messageId":id,
        "content":{"type":"text","text":""},"_meta":{META:{"completed":{
            "version":1,"utf8Bytes":text.len(),"sha256":format!("{:x}",Sha256::digest(text.as_bytes()))}}}});
    if let Some(phase) = phase {
        value["_meta"][META]["phase"] = phase.into();
    }
    value
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Utterance {
    pub id: String,
    pub text: String,
}
#[derive(Default)]
pub struct Commentary {
    parts: Vec<Part>,
    bytes: usize,
}
#[derive(Default)]
struct Part {
    id: String,
    text: String,
    commentary: bool,
    emitted: bool,
}
impl Commentary {
    pub fn observe(&mut self, update: &Value, negotiated: bool) -> Result<Option<Utterance>> {
        if update["sessionUpdate"] != "agent_message_chunk" || update["content"]["type"] != "text" {
            return Ok(None);
        }
        let Some(text) = update["content"]["text"].as_str() else {
            return Ok(None);
        };
        let id = update["messageId"].as_str().unwrap_or("");
        let completed = &update["_meta"][META]["completed"];
        // Empty control envelopes cannot create/reorder drafts or alter phases.
        if text.is_empty() {
            if !negotiated || id.is_empty() || completed["version"] != 1 {
                return Ok(None);
            }
            let Some(part) = self.parts.iter_mut().find(|p| p.id == id) else {
                return Ok(None);
            };
            if part.emitted || !part.commentary || part.text.trim().is_empty() {
                return Ok(None);
            }
            ensure!(
                completed["utf8Bytes"].as_u64() == Some(part.text.len() as u64)
                    && completed["sha256"].as_str()
                        == Some(format!("{:x}", Sha256::digest(part.text.as_bytes())).as_str()),
                "Public message completion does not match its content"
            );
            part.emitted = true;
            return Ok(Some(Utterance {
                id: part.id.clone(),
                text: part.text.clone(),
            }));
        }
        ensure!(
            id.len() <= 1024 && self.bytes + text.len() <= MAX_BYTES,
            "Public messages exceed bounds"
        );
        let index = if let Some(i) = self.parts.iter().position(|p| p.id == id) {
            i
        } else {
            ensure!(self.parts.len() < 256, "Too many public messages");
            self.parts.push(Part {
                id: id.into(),
                ..Default::default()
            });
            self.parts.len() - 1
        };
        let part = &mut self.parts[index];
        ensure!(
            !part.emitted,
            "Agent appended to a completed public message"
        );
        if let Some(phase) = update["_meta"][META]["phase"].as_str() {
            match phase {
                "commentary" => part.commentary = true,
                "final_answer" => part.commentary = false,
                _ => {}
            }
        }
        part.text.push_str(text);
        self.bytes += text.len();
        Ok(None)
    }
    /// Plain ACP remains usable. Only a successful prompt terminal can settle
    /// drafts without negotiated completion. Final answers use their own path.
    pub fn terminal(&mut self, successful: bool) -> Vec<Utterance> {
        if !successful {
            return vec![];
        }
        self.parts
            .iter_mut()
            .filter_map(|p| {
                if p.emitted || !p.commentary || p.text.trim().is_empty() {
                    return None;
                }
                p.emitted = true;
                Some(Utterance {
                    id: p.id.clone(),
                    text: p.text.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(id: &str, text: &str) -> Value {
        json!({"sessionUpdate":"agent_message_chunk","messageId":id,"content":{"type":"text","text":text},"_meta":{META:{"phase":"commentary"}}})
    }
    #[test]
    fn interleaved_completed_comments_append_once_without_exposing_private_thought() {
        let mut c = Commentary::default();
        assert!(c.observe(&chunk("a", "First "), true).unwrap().is_none());
        c.observe(&chunk("b", "Second."), true).unwrap();
        c.observe(&chunk("a", "point."), true).unwrap();
        let b = c
            .observe(&completion("b", Some("commentary"), "Second."), true)
            .unwrap()
            .unwrap();
        assert_eq!(b.text, "Second.");
        assert!(
            c.observe(&completion("b", None, "Second."), true)
                .unwrap()
                .is_none()
        );
        let mut thought = chunk("x", "private");
        thought["sessionUpdate"] = "agent_thought_chunk".into();
        c.observe(&thought, true).unwrap();
        assert_eq!(
            c.terminal(true),
            vec![Utterance {
                id: "a".into(),
                text: "First point.".into()
            }]
        );
        assert!(c.observe(&chunk("b", " alteration"), true).is_err());
    }
    #[test]
    fn empty_receipts_unknown_completion_and_failures_never_publish_drafts() {
        let mut c = Commentary::default();
        c.observe(&chunk("a", "Draft"), true).unwrap();
        let mut empty = chunk("b", "");
        empty["_meta"] = json!({"inputReceipt":{"state":"received"}});
        c.observe(&empty, true).unwrap();
        assert!(
            c.observe(&completion("unknown", Some("commentary"), "payload"), true)
                .unwrap()
                .is_none()
        );
        assert!(c.observe(&completion("a", None, "wrong"), true).is_err());
        assert!(c.terminal(false).is_empty());
    }
    #[test]
    fn unnegotiated_signal_waits_for_success_and_does_not_publish_final_control_json() {
        let mut c = Commentary::default();
        c.observe(&chunk("a", "Checking."), false).unwrap();
        assert!(
            c.observe(&completion("a", None, "Checking."), false)
                .unwrap()
                .is_none()
        );
        let mut final_chunk = chunk("b", r#"{"decision":"pass"}"#);
        final_chunk["_meta"][META]["phase"] = "final_answer".into();
        c.observe(&final_chunk, false).unwrap();
        assert_eq!(c.terminal(true).len(), 1);
        assert!(c.terminal(true).is_empty());
    }
}
