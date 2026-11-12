//! The Help sheet's questions (roadmap 67): sent to the help service
//! (`services/help/`), which answers from the user guide. Online. Nothing is
//! sent until the person asks, and a question is kept for the service's
//! "most asked" report only when they ticked the box (`share`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

/// Where the help service runs (`services/help/README.md`, Deploy). Empty
/// means no service: the panel then says it is not connected yet.
pub const ENDPOINT: &str = "https://wugs-help.judecutler88.workers.dev";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Answer {
    pub answer: String,
    /// The guide sections the answer drew on, as their heading paths.
    pub sources: Vec<String>,
}

/// The request body: the question, the turns before it, this build's version
/// (so the service reads the guide for it) and the person's choice to share.
pub fn request(question: &str, history: &[Turn], share: bool) -> Value {
    json!({ "question": question, "history": history, "version": env!("CARGO_PKG_VERSION"), "share": share })
}

/// What the service answered, in words the sheet can show as they are.
pub fn read_reply(status: u16, body: &str) -> Result<Answer, String> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if status != 200 {
        return Err(match v.get("error").and_then(Value::as_str) {
            Some(e) => e.to_string(),
            None => format!("The help service answered {status}. Try again in a moment."),
        });
    }
    let answer = v.get("answer").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if answer.is_empty() {
        return Err("The help service sent an empty answer. Try asking another way.".into());
    }
    let sources = v
        .get("sources")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.get("title").and_then(Value::as_str)).map(str::to_string).collect())
        .unwrap_or_default();
    Ok(Answer { answer, sources })
}

/// Ask over HTTPS. Blocking: call it off the async workers.
pub fn ask(endpoint: &str, question: &str, history: &[Turn], share: bool) -> Result<Answer, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(60)))
        .https_only(!endpoint.starts_with("http://127.0.0.1:"))
        .http_status_as_error(false)
        .build()
        .into();
    let mut r = agent
        .post(&format!("{endpoint}/v1/ask"))
        .header("Content-Type", "application/json")
        .send(request(question, history, share).to_string())
        .map_err(|e| format!("Could not reach the help service ({e}). Help needs the internet."))?;
    let status = r.status().as_u16();
    let body = r.body_mut().read_to_string().unwrap_or_default();
    read_reply(status, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_carries_the_version_and_the_choice_to_share() {
        let history = [Turn { role: "user".into(), content: "a".into() }, Turn { role: "assistant".into(), content: "b".into() }];
        let r = request("How?", &history, false);
        assert_eq!(r["question"], "How?");
        assert_eq!(r["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(r["share"], false);
        assert_eq!(r["history"][1]["role"], "assistant");
    }

    #[test]
    fn an_answer_comes_with_its_sources() {
        let a = read_reply(200, r#"{"answer":" Press S. ","sources":[{"title":"Segment the recording › Split, merge, and nudge","anchor":"x"}]}"#).unwrap();
        assert_eq!(a.answer, "Press S.");
        assert_eq!(a.sources, ["Segment the recording › Split, merge, and nudge"]);
    }

    #[test]
    fn the_services_own_words_are_shown_when_it_refuses() {
        assert_eq!(read_reply(429, r#"{"error":"Too many questions."}"#).unwrap_err(), "Too many questions.");
        assert!(read_reply(502, "<html>").unwrap_err().contains("502"));
        assert!(read_reply(200, r#"{"answer":"  "}"#).is_err());
    }

    #[test]
    #[ignore = "needs the help service running locally: cd services/help && npx wrangler@4 dev --port 8788"]
    fn the_real_service_answers() {
        let a = ask("http://127.0.0.1:8788", "How do I split a segment?", &[], false).unwrap();
        assert!(!a.answer.is_empty());
    }
}
