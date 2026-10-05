use quantic_engine::{
    layout::DisplayItemKind, Engine, PrivacyDecision, RequestContext, ResourceKind, ENGINE_NAME,
};

#[test]
fn engine_is_not_chromium_shell() {
    assert_eq!(ENGINE_NAME, "Quantic Engine");

    let engine = Engine::new();
    let output = engine.load_html(
        "<!doctype html><html><body><h1>Quantic Glide</h1><p>Own engine core.</p></body></html>",
    );

    assert!(output.document.nodes.len() >= 5);
    let words = output
        .display_list
        .iter()
        .filter(|item| item.kind == DisplayItemKind::Text)
        .map(|item| item.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(words, vec!["Quantic", "Glide", "Own", "engine", "core."]);
}

#[test]
fn privacy_is_fail_closed_for_third_party_subresources() {
    let engine = Engine::new();
    let decision = engine.evaluate_request(&RequestContext {
        top_level_host: "quantic.test".into(),
        request_host: "analytics.example".into(),
        kind: ResourceKind::Script,
    });

    assert_eq!(
        decision,
        PrivacyDecision::Block {
            reason: "third-party-denied-by-default"
        }
    );
}
