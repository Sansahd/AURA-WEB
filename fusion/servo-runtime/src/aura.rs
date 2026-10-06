use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_AURA_URL: &str = "http://127.0.0.1:8787";
const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_OLLAMA_MODEL: &str = "gemma3:12b";

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuraAction {
    Navigate { url: String },
    NewTab { url: String },
    Search { query: String },
    Back,
    Forward,
    Reload,
    Click { selector: String },
    Fill { selector: String, value: String },
}

#[derive(Clone, Debug)]
pub struct AuraReply {
    pub answer: String,
    pub engine: String,
    pub model: String,
    pub actions: Vec<AuraAction>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StructuredAnswer {
    #[serde(default)]
    reply: String,
    #[serde(default)]
    actions: Vec<AuraAction>,
}

fn configured_base_url(raw: &str, fallback: &str, allow_remote_https: bool) -> String {
    let Ok(url) = url::Url::parse(raw.trim().trim_end_matches('/')) else {
        return fallback.to_string();
    };
    let host = url.host_str().unwrap_or_default();
    let is_loopback = matches!(host, "127.0.0.1" | "localhost" | "::1");
    let accepted = (is_loopback && matches!(url.scheme(), "http" | "https"))
        || (allow_remote_https && url.scheme() == "https" && !host.is_empty());

    if accepted {
        url.origin().ascii_serialization()
    } else {
        fallback.to_string()
    }
}

fn aura_base_url() -> String {
    let raw = std::env::var("QUANTIC_AURA_URL").unwrap_or_else(|_| DEFAULT_AURA_URL.to_string());
    // AURA may be hosted remotely, but only when the operator explicitly configures
    // an HTTPS origin. Page content can never choose this destination.
    configured_base_url(&raw, DEFAULT_AURA_URL, true)
}

fn ollama_base_url() -> String {
    let raw = std::env::var("QUANTIC_OLLAMA_URL").unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string());
    // Direct Ollama remains loopback-only by design.
    configured_base_url(&raw, DEFAULT_OLLAMA_URL, false)
}

fn parse_answer(raw: &str) -> (String, Vec<AuraAction>) {
    let trimmed = raw.trim();
    if let Ok(structured) = serde_json::from_str::<StructuredAnswer>(trimmed) {
        let reply = if structured.reply.trim().is_empty() {
            "Action préparée.".to_string()
        } else {
            structured.reply.trim().to_string()
        };
        return (reply, structured.actions.into_iter().take(6).collect());
    }

    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                if let Ok(structured) =
                    serde_json::from_str::<StructuredAnswer>(&trimmed[start..=end])
                {
                    let reply = if structured.reply.trim().is_empty() {
                        "Action préparée.".to_string()
                    } else {
                        structured.reply.trim().to_string()
                    };
                    return (reply, structured.actions.into_iter().take(6).collect());
                }
            }
        }
    }

    (trimmed.to_string(), Vec::new())
}

fn system_instruction() -> &'static str {
    r#"Tu es AURA intégrée au navigateur Gekko. Tu reçois la demande utilisateur et un contexte de page limité.
Réponds de préférence en JSON strict:
{"reply":"réponse concise","actions":[...]}
Actions autorisées uniquement:
{"type":"navigate","url":"https://..."}
{"type":"new_tab","url":"https://..."}
{"type":"search","query":"..."}
{"type":"back"}
{"type":"forward"}
{"type":"reload"}
{"type":"click","selector":"sélecteur CSS simple"}
{"type":"fill","selector":"sélecteur CSS simple","value":"texte"}
N'invente jamais d'action destructrice. Maximum 6 actions. Si aucune action n'est nécessaire, actions=[].
Ne produis jamais de JavaScript arbitraire."#
}

fn request_aura(prompt: &str, page_context: &str) -> Result<AuraReply, String> {
    let base = aura_base_url();
    let full_prompt = format!(
        "DEMANDE UTILISATEUR:\n{}\n\nCONTEXTE PAGE GEKKO (lecture seule):\n{}",
        prompt.trim(),
        page_context.chars().take(14_000).collect::<String>()
    );
    let body = json!({
        "prompt": full_prompt,
        "system_instruction": system_instruction(),
        "task_role": "browser-agent",
        "max_tokens": 900,
        "distributed": false,
        "max_agents": 3,
        "source": "quantic-gekko"
    });

    let response = ureq::post(&format!("{base}/api/ai/generate"))
        .set("content-type", "application/json")
        .set("accept", "application/json")
        .set("user-agent", "Quantic-Gekko/AURA-2")
        .timeout(std::time::Duration::from_secs(75))
        .send_json(body)
        .map_err(|error| error.to_string())?;

    let value: Value = response.into_json().map_err(|error| error.to_string())?;
    let raw = value
        .get("answer")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if raw.is_empty() {
        return Err("AURA a renvoyé une réponse vide".into());
    }
    let (answer, actions) = parse_answer(raw);
    Ok(AuraReply {
        answer,
        engine: value
            .get("engine")
            .and_then(Value::as_str)
            .unwrap_or("aura-local")
            .to_string(),
        model: value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        actions,
        error: None,
    })
}

fn request_ollama(prompt: &str, page_context: &str) -> Result<AuraReply, String> {
    let base = ollama_base_url();
    let model = std::env::var("QUANTIC_OLLAMA_MODEL")
        .unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string());
    let full_prompt = format!(
        "DEMANDE UTILISATEUR:\n{}\n\nCONTEXTE PAGE GEKKO (lecture seule):\n{}",
        prompt.trim(),
        page_context.chars().take(14_000).collect::<String>()
    );

    let body = json!({
        "model": model,
        "prompt": full_prompt,
        "system": system_instruction(),
        "stream": false,
        "options": { "num_predict": 900 }
    });

    let response = ureq::post(&format!("{base}/api/generate"))
        .set("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(45))
        .send_json(body)
        .map_err(|error| error.to_string())?;

    let value: Value = response.into_json().map_err(|error| error.to_string())?;
    let raw = value
        .get("response")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if raw.is_empty() {
        return Err("Ollama a renvoyé une réponse vide".into());
    }
    let (answer, actions) = parse_answer(raw);
    Ok(AuraReply {
        answer,
        engine: "ollama-direct".into(),
        model,
        actions,
        error: None,
    })
}

pub fn generate(prompt: &str, page_context: &str) -> AuraReply {
    match request_aura(prompt, page_context) {
        Ok(reply) => reply,
        Err(aura_error) => match request_ollama(prompt, page_context) {
            Ok(mut reply) => {
                reply.error = Some(format!("AURA indisponible: {aura_error}"));
                reply
            }
            Err(ollama_error) => AuraReply {
                answer: "AURA et Ollama sont indisponibles localement.".into(),
                engine: "offline".into(),
                model: String::new(),
                actions: Vec::new(),
                error: Some(format!(
                    "AURA: {aura_error} · Ollama: {ollama_error}"
                )),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_browser_actions() {
        let (reply, actions) = parse_answer(
            r#"{"reply":"J'ouvre la page.","actions":[{"type":"navigate","url":"https://example.com"}]}"#,
        );
        assert_eq!(reply, "J'ouvre la page.");
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AuraAction::Navigate { .. }));
    }

    #[test]
    fn keeps_plain_answers_without_actions() {
        let (reply, actions) = parse_answer("Réponse simple");
        assert_eq!(reply, "Réponse simple");
        assert!(actions.is_empty());
    }

    #[test]
    fn aura_accepts_explicit_remote_https_origin() {
        assert_eq!(
            configured_base_url(
                "https://aura.example.org/private/path",
                DEFAULT_AURA_URL,
                true
            ),
            "https://aura.example.org"
        );
    }

    #[test]
    fn aura_rejects_remote_plain_http() {
        assert_eq!(
            configured_base_url("http://aura.example.org", DEFAULT_AURA_URL, true),
            DEFAULT_AURA_URL
        );
    }

    #[test]
    fn ollama_stays_loopback_only() {
        assert_eq!(
            configured_base_url("https://ollama.example.org", DEFAULT_OLLAMA_URL, false),
            DEFAULT_OLLAMA_URL
        );
        assert_eq!(
            configured_base_url("http://127.0.0.1:11434", DEFAULT_OLLAMA_URL, false),
            "http://127.0.0.1:11434"
        );
    }
}
