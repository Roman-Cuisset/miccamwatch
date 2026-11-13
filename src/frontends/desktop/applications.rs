//! One observed process snapshot, all resource/device accesses, bounded pages.
//! Read cursors are not process authority; termination always prepares afresh.
use anyhow::{Context as _, Result, ensure};
use crate::model::Access;
use serde_json::Value;
use std::io::{self, Write};

pub(super) struct Browser {
    remaining: std::vec::IntoIter<Access>,
    count: usize,
    position: usize,
    cursor: Option<String>,
    observed_at: chrono::DateTime<chrono::Utc>,
    identity_confirmed: bool,
}
impl Browser {
    pub fn new(accesses: Vec<Access>) -> Result<Self> {
        let first = accesses.first().context("No current observed access for this process")?;
        let pid = first.pid.context("No attributed process ID")?;
        let identity = first.process.as_ref().map(|p| &p.instance_id);
        ensure!(accesses.iter().all(|a| a.pid == Some(pid) && a.process.as_ref().map(|p| &p.instance_id) == identity),
            "Observed process generation is ambiguous; no accesses combined");
        let identity_confirmed = identity.is_some_and(|identity| !identity.is_empty());
        Ok(Self { count: accesses.len(), remaining: accesses.into_iter(), position: 0, cursor: None,
            observed_at: chrono::Utc::now(), identity_confirmed })
    }
    pub fn page(&mut self, supplied: Option<&str>) -> Result<Value> {
        let expected = self.cursor.take();
        ensure!((self.position == 0 && supplied.is_none()) ||
            (expected.is_some() && expected.as_deref() == supplied), "Application cursor stale or already used; refresh the process snapshot");
        let access = self.remaining.next().context("No remaining observed application access")?;
        let mut data = super::tools::application_details(access);
        bound_text(&mut data)?;
        self.position += 1;
        if self.remaining.len() > 0 { self.cursor = Some(super::new_private_token()?); }
        let object = data.as_object_mut().context("Invalid native application projection")?;
        object.insert("access_index".into(), self.position.into());
        object.insert("access_count".into(), self.count.into());
        object.insert("snapshot_read_at".into(), self.observed_at.to_rfc3339().into());
        object.insert("snapshot_not_live".into(), true.into());
        object.insert("process_identity_confirmed".into(), self.identity_confirmed.into());
        object.insert("next_cursor".into(), self.cursor.clone().into());
        Ok(data)
    }
}

// Count serialized bytes without making a second JSON buffer. Escaping matters:
// a short control-character string can be six times larger on the wire.
struct Budget(usize);
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.checked_sub(bytes.len()).ok_or_else(|| io::Error::other("application page byte budget exceeded"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
fn bound_text(data: &mut Value) -> Result<()> {
    if serde_json::to_writer(Budget(96 * 1024), &data).is_ok() { return Ok(()); }
    clip_long_text(data);
    data["truncated"] = true.into();
    serde_json::to_writer(Budget(96 * 1024), &data).context("Native application page exceeds its explicit text budget")?;
    Ok(())
}
fn clip_long_text(value: &mut Value) {
    match value {
        Value::String(text) if text.len() > 128 => {
            let mut end = 128;
            while !text.is_char_boundary(end) { end -= 1; }
            text.truncate(end);
        }
        Value::Array(items) => { for value in items { clip_long_text(value); } }
        Value::Object(items) => { for value in items.values_mut() { clip_long_text(value); } }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Activity, Confidence, EnforcementDecision, Evidence, EvidenceKind, ProcessContext, Resource, Risk};
    fn access(resource: Resource, device: &str, identity: &str) -> Access {
        Access { key: format!("fixture:{device}"), resource, activity: Activity::Active,
            confidence: Confidence::High, enforcement: EnforcementDecision::Alert,
            application: "Fixture recorder".into(), pid: Some(12345), parent_pid: None,
            parent_name: None, executable: Some("fixture-recorder".into()), signature: None,
            device: Some(device.into()), started_at: None, modules: vec![], evidence: vec![],
            risk: Risk::Unexplained, process: Some(ProcessContext { instance_id: identity.into(),
                ancestry: vec![], user: None, session_id: None, integrity: None }) }
    }
    #[test]
    fn all_simultaneous_resources_remain_available_without_reusing_a_cursor() {
        let mut browser = Browser::new(vec![access(Resource::Microphone,"input-one","one"),
            access(Resource::Microphone,"input-two","one"), access(Resource::Camera,"camera-one","one")]).unwrap();
        let first = browser.page(None).unwrap();
        assert_eq!(first["access_count"], 3);
        assert_eq!(first["device"], "input-one");
        let cursor = first["next_cursor"].as_str().unwrap();
        let second = browser.page(Some(cursor)).unwrap();
        assert_eq!(second["device"], "input-two");
        let next = second["next_cursor"].as_str().unwrap();
        let third = browser.page(Some(next)).unwrap();
        assert_eq!(third["resource"], "camera");
        assert_eq!(third["device"], "camera-one");
        assert_eq!(third["access_index"], 3);
        assert!(third["next_cursor"].is_null());
        assert!(browser.page(Some(cursor)).is_err());
        assert_eq!(first["snapshot_read_at"], third["snapshot_read_at"]);
    }
    #[test]
    fn different_process_generations_and_foreign_cursors_are_not_combined() {
        assert!(Browser::new(vec![access(Resource::Microphone,"input","old"), access(Resource::Camera,"camera","replacement")]).is_err());
        let mut old = Browser::new(vec![access(Resource::Microphone,"input","old"), access(Resource::Camera,"camera","old")]).unwrap();
        let first = old.page(None).unwrap();
        let mut replacement = Browser::new(vec![access(Resource::Camera,"camera-new","replacement")]).unwrap();
        assert!(replacement.page(Some(first["next_cursor"].as_str().unwrap())).is_err());
        assert_eq!(replacement.page(None).unwrap()["device"], "camera-new");
    }
    #[test]
    fn worst_case_escaped_evidence_stays_within_the_wire_budget_and_reports_omission() {
        let mut access = access(Resource::Camera,"camera","one");
        access.modules = vec!["\u{0001}".repeat(4096); 32];
        access.evidence = (0..32).map(|_| Evidence::new(EvidenceKind::CaptureModule,"fixture","\u{0001}".repeat(4096))).collect();
        let page = Browser::new(vec![access]).unwrap().page(None).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() < 100 * 1024);
        assert_eq!(page["truncated"], true);
        assert_eq!(page["resource"], "camera");
        assert_eq!(page["activity"], "active");
    }
}
