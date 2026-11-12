//! Send feedback (roadmap 65): a message, an optional reply address, an
//! optional screenshot or video and, if the person agrees, technical
//! details, sent to the wugs feedback service (`services/feedback/`).
//!
//! Everything goes through an outbox in app data first, so a report written
//! offline, or cut off halfway, is sent again later instead of lost. Each
//! report keeps the id it was given here; the service files a report once per
//! id, so sending one again after a lost answer never makes a second issue.
//! Online.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wugs_model::ulid;

/// The feedback service. Filled in once it is deployed (services/feedback/README.md).
pub const ENDPOINT: &str = "";
/// The service refuses larger uploads (Cloudflare's free plan stops at 100 MB).
pub const MAX_ATTACHMENT_BYTES: u64 = 95 * 1024 * 1024;
pub const MAX_MESSAGE_CHARS: usize = 10_000;

/// What the person wrote, plus the technical details they agreed to send.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Report {
    pub message: String,
    #[serde(default)]
    pub contact: String,
    pub client: String,
    #[serde(default)]
    pub details: Option<Value>,
    #[serde(default)]
    pub attachment: Option<AttachmentInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AttachmentInfo {
    pub name: String,
    pub bytes: u64,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// This report reached the service, attachment included.
    pub sent: bool,
    /// Reports still waiting in the outbox, this one included if not sent.
    pub waiting: usize,
    /// Why sending stopped, in words, when it did.
    pub problem: Option<String>,
    /// This build knows where to send feedback.
    pub configured: bool,
}

/// One answer from the service.
pub struct Reply {
    pub status: u16,
    pub body: String,
}

/// How reports travel; the real one is HTTPS, tests use a fake.
pub trait Transport {
    fn post_report(&self, id: &str, report: &str) -> Result<Reply, String>;
    fn put_attachment(&self, id: &str, token: &str, file: &Path, content_type: &str) -> Result<Reply, String>;
}

/// Only one send runs at a time, so a startup retry and a new report never
/// send the same item twice at once.
#[derive(Default)]
pub struct Sending(pub Mutex<()>);

fn outbox(app_data: &Path) -> PathBuf {
    app_data.join("feedback").join("outbox")
}

/// The technical details this computer adds to the page's own (version,
/// system), shown to the person before anything is sent.
pub fn details(page: Value) -> Value {
    let mut d = json!({
        "wugs": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    });
    if let (Some(d), Value::Object(page)) = (d.as_object_mut(), page) {
        for (k, v) in page {
            d.entry(k).or_insert(v);
        }
    }
    d
}

/// Check a report and put it in the outbox. Returns its id.
pub fn enqueue(app_data: &Path, message: &str, contact: &str, details: Option<Value>, attachment: Option<&Path>) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("Write what happened first.".into());
    }
    if message.chars().count() > MAX_MESSAGE_CHARS {
        return Err(format!("The message is longer than {MAX_MESSAGE_CHARS} characters."));
    }
    let contact = contact.trim();
    if !contact.is_empty() && !looks_like_email(contact) {
        return Err("The reply address does not look like an email address.".into());
    }
    if let Some(path) = attachment {
        let meta = fs::metadata(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        if !meta.is_file() || meta.len() == 0 {
            return Err("The attachment is empty or not a file.".into());
        }
        too_big(meta.len())?;
    }
    let id = ulid().to_string();
    let dir = outbox(app_data).join(&id);
    let tmp = outbox(app_data).join(format!(".{id}.tmp"));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let fill = || -> Result<(), String> {
        let attached = match attachment {
            None => None,
            Some(path) => {
                // A copy: the person may move or delete their recording meanwhile.
                let copy = tmp.join("attachment");
                fs::copy(path, &copy).map_err(|e| format!("Could not copy the attachment: {e}"))?;
                // Sized from the copy: a recording still being written grows.
                let bytes = fs::metadata(&copy).map_err(|e| e.to_string())?.len();
                too_big(bytes)?;
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "attachment".into());
                Some(AttachmentInfo { name, bytes })
            }
        };
        let report = Report {
            message: message.to_owned(),
            contact: contact.to_owned(),
            client: format!("wugs {} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS),
            details,
            attachment: attached,
        };
        fs::write(tmp.join("report.json"), serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        // Whole or not at all: a half-written report is never sent.
        fs::rename(&tmp, &dir).map_err(|e| e.to_string())
    };
    fill().inspect_err(|_| {
        let _ = fs::remove_dir_all(&tmp);
    })?;
    Ok(id)
}

fn too_big(bytes: u64) -> Result<(), String> {
    if bytes > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "The attachment is {} MB; the most that can be sent is 95 MB. A shorter clip, or a screenshot, will do.",
            bytes / 1_000_000
        ));
    }
    Ok(())
}

/// The same test the service applies, so the form never keeps an address
/// the service would refuse.
pub fn looks_like_email(s: &str) -> bool {
    let mut parts = s.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let fine = |p: &str, max: usize| !p.is_empty() && p.chars().count() <= max && !p.contains(|c: char| c.is_whitespace() || c == '<' || c == '>');
    fine(local, 64) && fine(domain, 190)
}

/// Reports in the outbox, oldest first (ids sort by time).
pub fn waiting(app_data: &Path) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(outbox(app_data))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("report.json").is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    ids.sort();
    ids
}

/// Send every waiting report, oldest first. A report the service cannot
/// take yet is skipped, so it never holds up the ones behind it; no
/// connection at all stops the round. `focus` is the report the person just
/// wrote, whose fate is reported.
pub fn send_waiting(app_data: &Path, net: &dyn Transport, focus: Option<&str>) -> Outcome {
    sweep_unfinished(app_data);
    let mut offline = None;
    let mut focus_problem = None;
    let mut tried = std::collections::BTreeSet::new();
    // Looked up again after each report, so one written while this runs goes too.
    while let Some(id) = waiting(app_data).into_iter().find(|id| !tried.contains(id)) {
        tried.insert(id.clone());
        match send_one(app_data, net, &id) {
            Ok(()) => {}
            Err(Failure::Unreachable(why)) => {
                offline = Some(why);
                break;
            }
            Err(Failure::Later(why)) => {
                if focus == Some(id.as_str()) {
                    focus_problem = Some(why);
                }
            }
            Err(Failure::Refused(why)) => {
                if focus == Some(id.as_str()) {
                    focus_problem = Some(why.clone());
                }
                // The service will never take this one as it is: keep it aside
                // with the reason rather than retrying it forever.
                let refused = refused_dir(app_data).join(&id);
                let _ = fs::create_dir_all(refused_dir(app_data));
                if fs::rename(outbox(app_data).join(&id), &refused).is_ok() {
                    let _ = fs::write(refused.join("reason.txt"), &why);
                }
            }
        }
    }
    let left = waiting(app_data);
    let refused_reason = focus.and_then(|f| fs::read_to_string(refused_dir(app_data).join(f).join("reason.txt")).ok());
    let sent = focus.is_some_and(|f| !left.iter().any(|w| w == f) && !refused_dir(app_data).join(f).exists());
    let problem = if sent { None } else { focus_problem.or(refused_reason).or(offline) };
    Outcome { sent, waiting: left.len(), problem, configured: true }
}

fn refused_dir(app_data: &Path) -> PathBuf {
    app_data.join("feedback").join("refused")
}

/// A report half-written when the app stopped never became one; its folder
/// goes after an hour.
fn sweep_unfinished(app_data: &Path) {
    let hour = Duration::from_secs(60 * 60);
    for e in fs::read_dir(outbox(app_data)).into_iter().flatten().flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > hour);
        if e.file_name().to_string_lossy().starts_with('.') && old {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

enum Failure {
    /// No answer at all (offline): stop this round.
    Unreachable(String),
    /// The service answered but cannot take this report yet: try it later.
    Later(String),
    /// The service refused the report itself.
    Refused(String),
}

fn send_one(app_data: &Path, net: &dyn Transport, id: &str) -> Result<(), Failure> {
    let dir = outbox(app_data).join(id);
    let report_text = fs::read_to_string(dir.join("report.json")).map_err(|e| Failure::Later(e.to_string()))?;
    let report: Report = serde_json::from_str(&report_text).map_err(|e| Failure::Refused(e.to_string()))?;
    // Posted on every try: the service files it once per id, and its answer
    // says whether it still needs the attachment, so a file it already has is
    // never uploaded again, and a fresh upload token comes with each answer.
    let reply = net.post_report(id, &report_text).map_err(Failure::Unreachable)?;
    classify(&reply)?;
    let answer: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
    // Only the service's own answer about this report counts as received.
    if answer.get("id").and_then(Value::as_str) != Some(id) {
        return Err(Failure::Later("The feedback service gave an unexpected answer. It will be sent later.".into()));
    }
    if let (Some(info), Some(token)) = (&report.attachment, answer.pointer("/upload/token").and_then(Value::as_str)) {
        let file = dir.join("attachment");
        if !file.is_file() {
            // Removed from the outbox (by an antivirus, say): it can never go.
            return Err(Failure::Refused("The attachment kept for this report is gone.".into()));
        }
        let reply = net.put_attachment(id, token, &file, content_type(&info.name)).map_err(Failure::Unreachable)?;
        classify(&reply)?;
    }
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

fn classify(reply: &Reply) -> Result<(), Failure> {
    if (200..=299).contains(&reply.status) {
        return Ok(());
    }
    let answer = serde_json::from_str::<Value>(&reply.body).unwrap_or(Value::Null);
    let said = answer.get("error").and_then(Value::as_str).map(str::to_owned);
    // Only the service's own checks refuse a report (they say `refused`).
    // Anything else (a proxy, a Cloudflare page, a redeploy, an old address)
    // means try again later.
    if answer.get("refused").and_then(Value::as_bool) == Some(true) {
        return Err(Failure::Refused(said.unwrap_or_else(|| "The feedback service refused the report.".into())));
    }
    Err(Failure::Later(said.unwrap_or_else(|| format!("The feedback service could not take it just now (it answered {}). It will be sent later.", reply.status))))
}

/// The type the service stores the attachment as, from its extension.
pub fn content_type(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "txt" | "log" => "text/plain",
        _ => "application/octet-stream",
    }
}

/// The real transport: HTTPS to `ENDPOINT`.
pub struct Https {
    pub endpoint: String,
}

impl Https {
    fn agent(&self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(120)))
            // A slow field connection may take hours over a large video;
            // the ceiling only stops a send that never ends.
            .timeout_send_body(Some(Duration::from_secs(6 * 60 * 60)))
            // HTTPS only, except a service on this computer (tests, self-hosting checks).
            .https_only(!self.endpoint.starts_with("http://127.0.0.1:"))
            .http_status_as_error(false)
            .build()
            .into()
    }

    fn reply(r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Reply, String> {
        let mut r = r.map_err(|e| format!("Could not reach the feedback service ({e}). It will be sent when you are online."))?;
        let status = r.status().as_u16();
        let body = r.body_mut().read_to_string().unwrap_or_default();
        Ok(Reply { status, body })
    }
}

impl Transport for Https {
    fn post_report(&self, id: &str, report: &str) -> Result<Reply, String> {
        let url = format!("{}/v1/reports/{id}", self.endpoint);
        Self::reply(self.agent().post(&url).header("Content-Type", "application/json").send(report))
    }

    fn put_attachment(&self, id: &str, token: &str, file: &Path, content_type: &str) -> Result<Reply, String> {
        let url = format!("{}/v1/reports/{id}/attachment", self.endpoint);
        let f = fs::File::open(file).map_err(|e| e.to_string())?;
        Self::reply(self.agent().put(&url).header("Authorization", &format!("Bearer {token}")).header("Content-Type", content_type).send(f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        posts: RefCell<Vec<String>>,
        puts: RefCell<Vec<(String, u64)>>,
        offline: bool,
        answer: Option<(u16, &'static str)>,
    }

    impl Transport for Fake {
        fn post_report(&self, id: &str, report: &str) -> Result<Reply, String> {
            if self.offline {
                return Err("offline".into());
            }
            self.posts.borrow_mut().push(format!("{id}:{report}"));
            let (status, body) = self.answer.unwrap_or((201, r#"{"upload":{"token":"tok"}}"#));
            // The real service always names the report it answers about.
            let body = match serde_json::from_str::<Value>(body) {
                Ok(Value::Object(mut m)) => {
                    m.entry("id").or_insert(json!(id));
                    Value::Object(m).to_string()
                }
                _ => body.to_string(),
            };
            Ok(Reply { status, body })
        }
        fn put_attachment(&self, id: &str, token: &str, file: &Path, _: &str) -> Result<Reply, String> {
            assert_eq!(token, "tok");
            self.puts.borrow_mut().push((id.into(), fs::metadata(file).unwrap().len()));
            Ok(Reply { status: 201, body: "{}".into() })
        }
    }

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("wugs-feedback-{}", ulid()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_report_with_a_video_is_sent_and_leaves_the_outbox() {
        let app = tmp();
        let video = app.join("clip.mp4");
        fs::write(&video, vec![1u8; 2048]).unwrap();
        let id = enqueue(&app, "  The timeline froze  ", "gsi@example.edu", Some(details(json!({"kit": "tongues"}))), Some(&video)).unwrap();
        // The person's copy can go; the outbox kept its own.
        fs::remove_file(&video).unwrap();
        let net = Fake::default();
        let out = send_waiting(&app, &net, Some(&id));
        assert_eq!(out, Outcome { sent: true, waiting: 0, problem: None, configured: true });
        let posted = net.posts.borrow()[0].clone();
        assert!(posted.contains("\"message\": \"The timeline froze\""));
        assert!(posted.contains("\"kit\": \"tongues\""));
        assert!(posted.contains(&format!("\"wugs\": \"{}\"", env!("CARGO_PKG_VERSION"))));
        assert_eq!(net.puts.borrow()[0], (id, 2048));
    }

    #[test]
    fn offline_reports_wait_and_go_later_without_being_posted_twice() {
        let app = tmp();
        let id = enqueue(&app, "hello", "", None, None).unwrap();
        let out = send_waiting(&app, &Fake { offline: true, ..Fake::default() }, Some(&id));
        assert!(!out.sent);
        assert_eq!(out.waiting, 1);
        assert!(out.problem.unwrap().contains("offline"));
        let net = Fake::default();
        assert!(send_waiting(&app, &net, None).waiting == 0);
        assert_eq!(net.posts.borrow().len(), 1);
    }

    #[test]
    fn an_upload_cut_off_is_tried_again_with_a_fresh_token() {
        let app = tmp();
        let video = app.join("clip.webm");
        fs::write(&video, b"abc").unwrap();
        let id = enqueue(&app, "x", "", None, Some(&video)).unwrap();
        struct UploadFails(Fake);
        impl Transport for UploadFails {
            fn post_report(&self, id: &str, r: &str) -> Result<Reply, String> {
                self.0.post_report(id, r)
            }
            fn put_attachment(&self, _: &str, _: &str, _: &Path, _: &str) -> Result<Reply, String> {
                Err("cut off".into())
            }
        }
        let first = UploadFails(Fake::default());
        assert!(!send_waiting(&app, &first, Some(&id)).sent);
        let net = Fake::default();
        assert!(send_waiting(&app, &net, Some(&id)).sent);
        assert_eq!(net.puts.borrow().len(), 1);
    }

    #[test]
    fn a_file_the_service_already_has_is_not_uploaded_again() {
        let app = tmp();
        let video = app.join("clip.webm");
        fs::write(&video, b"abc").unwrap();
        let id = enqueue(&app, "x", "", None, Some(&video)).unwrap();
        // No upload token in the answer: the service has the file.
        let net = Fake { answer: Some((200, r#"{"received":"now"}"#)), ..Fake::default() };
        assert!(send_waiting(&app, &net, Some(&id)).sent);
        assert!(net.puts.borrow().is_empty());
    }

    #[test]
    fn a_stuck_report_does_not_hold_up_the_ones_behind_it() {
        let app = tmp();
        let stuck = enqueue(&app, "stuck", "", None, None).unwrap();
        let next = enqueue(&app, "next", "", None, None).unwrap();
        struct StuckFirst(Fake, String);
        impl Transport for StuckFirst {
            fn post_report(&self, id: &str, r: &str) -> Result<Reply, String> {
                if id == self.1 {
                    return Ok(Reply { status: 503, body: r#"{"error":"could not be told yet"}"#.into() });
                }
                self.0.post_report(id, r)
            }
            fn put_attachment(&self, a: &str, b: &str, c: &Path, d: &str) -> Result<Reply, String> {
                self.0.put_attachment(a, b, c, d)
            }
        }
        let out = send_waiting(&app, &StuckFirst(Fake::default(), stuck.clone()), Some(&next));
        assert!(out.sent);
        assert_eq!(out.problem, None, "the stuck report's trouble is not this one's");
        assert_eq!(waiting(&app), vec![stuck]);
    }

    #[test]
    fn a_refused_report_is_set_aside_with_the_reason() {
        let app = tmp();
        let id = enqueue(&app, "x", "", None, None).unwrap();
        let net = Fake { answer: Some((400, r#"{"error":"The report has no message.","refused":true}"#)), ..Fake::default() };
        let out = send_waiting(&app, &net, Some(&id));
        assert!(!out.sent);
        assert_eq!(out.waiting, 0);
        assert_eq!(out.problem.as_deref(), Some("The report has no message."));
        assert!(app.join("feedback/refused").join(&id).join("report.json").is_file());
    }

    #[test]
    fn only_the_service_itself_can_refuse_a_report() {
        let app = tmp();
        let id = enqueue(&app, "x", "", None, None).unwrap();
        // A proxy or a Cloudflare page answers 403 with HTML: try later.
        let out = send_waiting(&app, &Fake { answer: Some((403, "<html>Just a moment...</html>")), ..Fake::default() }, Some(&id));
        assert_eq!(out.waiting, 1);
        assert!(out.problem.unwrap().contains("later"));
        let out = send_waiting(&app, &Fake { answer: Some((404, "")), ..Fake::default() }, Some(&id));
        assert_eq!(out.waiting, 1);
        // The service's own catch-all 404 is not a refusal of the report either.
        let out = send_waiting(&app, &Fake { answer: Some((404, r#"{"error":"Not found."}"#)), ..Fake::default() }, Some(&id));
        assert_eq!(out.waiting, 1);
    }

    #[test]
    fn email_addresses_are_checked_as_the_service_checks_them() {
        assert!(looks_like_email("maria@uni.edu"));
        assert!(!looks_like_email("a@b@c"));
        assert!(!looks_like_email("x<y>@z"));
        assert!(!looks_like_email("maria at uni"));
        assert!(!looks_like_email("@uni.edu"));
    }

    #[test]
    fn a_busy_service_keeps_the_report_for_later() {
        let app = tmp();
        let id = enqueue(&app, "x", "", None, None).unwrap();
        let out = send_waiting(&app, &Fake { answer: Some((429, r#"{"error":"Too many"}"#)), ..Fake::default() }, Some(&id));
        assert_eq!(out.waiting, 1);
        assert_eq!(out.problem.as_deref(), Some("Too many"));
    }

    /// The real sender against the real service code, run locally:
    /// `node services/feedback/dev/serve-local.mjs` first (it listens on 8799).
    #[test]
    #[ignore = "needs the feedback service running on 127.0.0.1:8799"]
    fn the_real_sender_talks_to_the_service() {
        let app = tmp();
        let video = app.join("Screen Recording.mp4");
        fs::write(&video, vec![9u8; 300_000]).unwrap();
        let id = enqueue(&app, "It froze", "gsi@example.edu", Some(details(json!({}))), Some(&video)).unwrap();
        let out = send_waiting(&app, &Https { endpoint: "http://127.0.0.1:8799".into() }, Some(&id));
        assert_eq!(out, Outcome { sent: true, waiting: 0, problem: None, configured: true });
    }

    #[test]
    fn reports_are_checked_before_they_are_kept() {
        let app = tmp();
        assert!(enqueue(&app, "  ", "", None, None).is_err());
        assert!(enqueue(&app, "x", "not an address", None, None).is_err());
        let big = app.join("big.mp4");
        let f = fs::File::create(&big).unwrap();
        f.set_len(MAX_ATTACHMENT_BYTES + 1).unwrap();
        assert!(enqueue(&app, "x", "", None, Some(&big)).unwrap_err().contains("95 MB"));
        assert!(waiting(&app).is_empty());
    }
}
