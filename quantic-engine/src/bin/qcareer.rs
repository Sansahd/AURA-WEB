use std::{
    collections::{BTreeSet, HashSet},
    env, fs,
    path::Path,
    process,
};

use quantic_engine::{AutomationBridge, AutomationElement, AutomationForm};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct Identity {
    #[serde(rename = "fullName")]
    full_name: String,
    email: String,
    phone: String,
    location: String,
}

#[derive(Debug, Deserialize)]
struct Profile {
    identity: Identity,
    #[serde(default)]
    headline: String,
    #[serde(rename = "targetRoles", default)]
    target_roles: Vec<String>,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    experience: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SearchConfig {
    sources: Vec<SearchSource>,
}

#[derive(Debug, Deserialize)]
struct SearchSource {
    name: String,
    url: String,
    #[serde(rename = "linkContains", default)]
    link_contains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct OfferScore {
    #[serde(default)]
    score: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    company: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    contract: String,
    #[serde(default)]
    salary: String,
    #[serde(default)]
    reasons: Vec<String>,
    #[serde(default)]
    blockers: Vec<String>,
    #[serde(rename = "coverLetter", default)]
    cover_letter: String,
    #[serde(rename = "fitSummary", default)]
    fit_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApplicationRecord {
    source: String,
    url: String,
    title: String,
    company: String,
    score: i64,
    status: String,
    reason: String,
    cover_letter: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CareerState {
    applications: Vec<ApplicationRecord>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("AURA Career / Quantic Engine: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        return Err(
            "usage: qcareer <profile.json> <searches.json> [state.json]".to_string(),
        );
    }

    let profile: Profile = load_json(&args[0])?;
    let searches: SearchConfig = load_json(&args[1])?;
    let state_path = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "career-state.json".to_string());
    let mut state: CareerState = if Path::new(&state_path).exists() {
        load_json(&state_path).unwrap_or_default()
    } else {
        CareerState::default()
    };

    let min_score = env::var("MIN_SCORE")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(72);
    let max_offers = env::var("MAX_APPLICATIONS_PER_RUN")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);
    let auto_submit = env::var("AUTO_SUBMIT")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let cv_path = env::var("CV_PATH").ok();

    println!("AURA Career + Quantic Engine Q0.5");
    println!("Profile: {}", profile.identity.full_name);
    println!("Mode: {}", if auto_submit { "AUTO_SUBMIT" } else { "PREPARE_ONLY" });

    let mut bridge = AutomationBridge::new();
    let mut handled = 0usize;
    let mut seen = HashSet::new();

    for source in &searches.sources {
        if handled >= max_offers {
            break;
        }

        println!("SOURCE {}", source.name);
        if let Err(error) = bridge.open(&source.url) {
            eprintln!("SOURCE ERROR {}: {error}", source.name);
            continue;
        }

        let mut links = BTreeSet::new();
        for (url, _) in bridge.resolved_links() {
            if source.link_contains.iter().any(|needle| url.contains(needle.as_str())) {
                links.insert(url);
            }
        }

        for url in links {
            if handled >= max_offers {
                break;
            }
            if !seen.insert(url.clone()) {
                continue;
            }
            if state.applications.iter().any(|item| {
                item.url == url && matches!(item.status.as_str(), "submitted" | "prepared" | "needs_review")
            }) {
                continue;
            }

            println!("OFFER {url}");
            if let Err(error) = bridge.open(&url) {
                eprintln!("OFFER ERROR {url}: {error}");
                continue;
            }

            let body = bridge.page_text();
            if body.trim().is_empty() {
                record(
                    &mut state,
                    &state_path,
                    ApplicationRecord {
                        source: source.name.clone(),
                        url: url.clone(),
                        title: String::new(),
                        company: String::new(),
                        score: 0,
                        status: "needs_review".to_string(),
                        reason: "empty_page".to_string(),
                        cover_letter: String::new(),
                    },
                )?;
                continue;
            }

            let score = score_offer(&profile, &body, &url)?;
            println!("SCORE {} {}", score.score, score.title);

            if score.score < min_score || !score.blockers.is_empty() {
                record(
                    &mut state,
                    &state_path,
                    ApplicationRecord {
                        source: source.name.clone(),
                        url: url.clone(),
                        title: score.title.clone(),
                        company: score.company.clone(),
                        score: score.score,
                        status: "skipped".to_string(),
                        reason: if score.score < min_score {
                            "score_below_threshold".to_string()
                        } else {
                            format!("blockers: {}", score.blockers.join(" | "))
                        },
                        cover_letter: score.cover_letter.clone(),
                    },
                )?;
                continue;
            }

            let (status, reason) = prepare_application(
                &mut bridge,
                &profile,
                &score,
                auto_submit,
                cv_path.as_deref(),
            );
            println!("RESULT {status} {reason}");

            record(
                &mut state,
                &state_path,
                ApplicationRecord {
                    source: source.name.clone(),
                    url: url.clone(),
                    title: score.title.clone(),
                    company: score.company.clone(),
                    score: score.score,
                    status,
                    reason,
                    cover_letter: score.cover_letter.clone(),
                },
            )?;
            handled += 1;
        }
    }

    println!("DONE handled={handled}");
    Ok(())
}

fn prepare_application(
    bridge: &mut AutomationBridge,
    profile: &Profile,
    score: &OfferScore,
    auto_submit: bool,
    cv_path: Option<&str>,
) -> (String, String) {
    let _ = bridge.execute_script(
        r#"(() => {
            let n = 0;
            document.querySelectorAll('input,textarea,select,button,form,a').forEach((el) => {
                if (!el.id) el.id = 'aura-career-' + (++n);
            });
        })();"#,
    );

    let body = bridge.page_text().to_ascii_lowercase();
    let elements = bridge.elements();

    if elements.iter().any(|element| {
        element.tag.eq_ignore_ascii_case("input")
            && element.input_type.eq_ignore_ascii_case("password")
    }) {
        return ("needs_review".to_string(), "login_required".to_string());
    }

    let sensitive = [
        "salaire actuel",
        "prétentions salariales",
        "salary expectation",
        "handicap",
        "état de santé",
        "religion",
        "origine ethnique",
        "casier judiciaire",
        "work authorization",
        "sponsorship",
        "autorisé à travailler",
        "déclaration sur l'honneur",
    ];
    if sensitive.iter().any(|needle| body.contains(*needle)) {
        return (
            "needs_review".to_string(),
            "sensitive_or_unknown_question".to_string(),
        );
    }

    fill_known_fields(bridge, profile, score, &elements);

    let forms = bridge.forms();
    let Some(form) = choose_form(&forms) else {
        return ("prepared".to_string(), "no_safe_form_found".to_string());
    };
    if form.id.is_empty() {
        return ("prepared".to_string(), "form_without_id".to_string());
    }

    if !auto_submit {
        return (
            "prepared".to_string(),
            if form.has_file {
                "ready_with_native_cv_upload".to_string()
            } else {
                "auto_submit_disabled".to_string()
            },
        );
    }

    if form.has_file {
        let Some(cv_path) = cv_path else {
            return ("needs_review".to_string(), "cv_path_missing".to_string());
        };
        if !Path::new(cv_path).is_file() {
            return (
                "needs_review".to_string(),
                format!("cv_file_not_found: {cv_path}"),
            );
        }
        let Some(file_field) = form.fields.iter().find(|field| {
            field.input_type.eq_ignore_ascii_case("file") && !field.name.is_empty()
        }) else {
            return (
                "needs_review".to_string(),
                "file_input_without_name".to_string(),
            );
        };

        return match bridge.submit_form_with_file(
            &form.id,
            &file_field.name,
            cv_path,
            "application/pdf",
        ) {
            Ok(true) => ("submitted".to_string(), "multipart_cv_submitted".to_string()),
            Ok(false) => (
                "needs_review".to_string(),
                "multipart_submission_prevented".to_string(),
            ),
            Err(error) => (
                "needs_review".to_string(),
                format!("multipart_submit_error: {error}"),
            ),
        };
    }

    match bridge.submit_form(&form.id) {
        Ok(true) => ("submitted".to_string(), "form_submitted".to_string()),
        Ok(false) => (
            "needs_review".to_string(),
            "form_submission_prevented".to_string(),
        ),
        Err(error) => (
            "needs_review".to_string(),
            format!("submit_error: {error}"),
        ),
    }
}

fn choose_form(forms: &[AutomationForm]) -> Option<&AutomationForm> {
    forms
        .iter()
        .filter(|form| !form.fields.is_empty())
        .max_by_key(|form| {
            let file_bonus = if form.has_file { 1000usize } else { 0usize };
            file_bonus + form.fields.len()
        })
}

fn fill_known_fields(
    bridge: &mut AutomationBridge,
    profile: &Profile,
    score: &OfferScore,
    elements: &[AutomationElement],
) {
    let mut names = profile.identity.full_name.split_whitespace();
    let first = names.next().unwrap_or_default().to_string();
    let last = names.collect::<Vec<_>>().join(" ");

    for element in elements {
        if element.id.is_empty() {
            continue;
        }
        let tag = element.tag.to_ascii_lowercase();
        if tag != "input" && tag != "textarea" {
            continue;
        }

        let signature = format!(
            "{} {} {} {}",
            element.name, element.input_type, element.placeholder, element.id
        )
        .to_ascii_lowercase();

        let value = if element.input_type.eq_ignore_ascii_case("email")
            || signature.contains("email")
            || signature.contains("e-mail")
        {
            Some(profile.identity.email.as_str())
        } else if element.input_type.eq_ignore_ascii_case("tel")
            || signature.contains("phone")
            || signature.contains("telephone")
            || signature.contains("téléphone")
        {
            Some(profile.identity.phone.as_str())
        } else if signature.contains("first_name")
            || signature.contains("firstname")
            || signature.contains("first-name")
            || signature.contains("prénom")
            || signature.contains("prenom")
        {
            Some(first.as_str())
        } else if signature.contains("last_name")
            || signature.contains("lastname")
            || signature.contains("last-name")
            || signature.contains("nom de famille")
        {
            Some(last.as_str())
        } else if signature.contains("location")
            || signature.contains("city")
            || signature.contains("ville")
        {
            Some(profile.identity.location.as_str())
        } else if tag == "textarea"
            && (signature.contains("cover")
                || signature.contains("motivation")
                || signature.contains("lettre")
                || signature.contains("message"))
        {
            Some(score.cover_letter.as_str())
        } else {
            None
        };

        if let Some(value) = value {
            let _ = bridge.set_value(&element.id, value);
        }
    }
}

fn score_offer(profile: &Profile, offer: &str, url: &str) -> Result<OfferScore, String> {
    let ollama_url =
        env::var("OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:4b".to_string());

    let profile_summary = json!({
        "headline": &profile.headline,
        "targetRoles": &profile.target_roles,
        "skills": &profile.skills,
        "experience": &profile.experience,
        "location": &profile.identity.location,
    });

    let system = r#"Tu es AURA Career. Compare strictement l'offre au profil réel.
N'invente jamais diplôme, expérience, compétence, niveau de langue, salaire ou autorisation.
Retourne uniquement un JSON:
{"score":0,"title":"","company":"","location":"","contract":"","salary":"","reasons":[],"blockers":[],"coverLetter":"","fitSummary":""}
Une exigence obligatoire non démontrée doit apparaître dans blockers.
La lettre doit rester factuelle et concise."#;

    let payload = json!({
        "model": model,
        "stream": false,
        "format": "json",
        "messages": [
            {"role":"system","content":system},
            {"role":"user","content":json!({
                "profile": profile_summary,
                "url": url,
                "offer": offer.chars().take(18000).collect::<String>()
            }).to_string()}
        ],
        "options":{"temperature":0.1}
    });

    let endpoint = format!("{}/api/chat", ollama_url.trim_end_matches('/'));
    let response = ureq::post(&endpoint)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .map_err(|error| format!("Ollama unavailable: {error}"))?;

    let body = response
        .into_string()
        .map_err(|error| format!("Ollama response unreadable: {error}"))?;
    let root: Value =
        serde_json::from_str(&body).map_err(|error| format!("Ollama JSON invalid: {error}"))?;
    let content = root
        .get("message")
        .and_then(|value| value.get("content"))
        .and_then(Value::as_str)
        .ok_or_else(|| "Ollama response missing message.content".to_string())?;

    serde_json::from_str(content)
        .map_err(|error| format!("Ollama score JSON invalid: {error}; content={content}"))
}

fn load_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let raw = fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("invalid JSON in {path}: {error}"))
}

fn record(state: &mut CareerState, path: &str, record: ApplicationRecord) -> Result<(), String> {
    if let Some(existing) = state
        .applications
        .iter_mut()
        .find(|item| item.url == record.url)
    {
        *existing = record;
    } else {
        state.applications.push(record);
    }
    let raw = serde_json::to_string_pretty(state)
        .map_err(|error| format!("cannot serialize state: {error}"))?;
    fs::write(path, raw).map_err(|error| format!("cannot write {path}: {error}"))
}
