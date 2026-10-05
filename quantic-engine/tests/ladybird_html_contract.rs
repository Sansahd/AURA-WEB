use quantic_engine::dom::NodeKind;
use quantic_engine::html::parse;

#[test]
fn ladybird_tokenizer_builds_quantic_dom() {
    let doc = parse("<main id='root'><h1>Hello</h1><p>Quantic &amp; Glide</p></main>");
    let root = doc.find_by_id("root").expect("root element");
    assert_eq!(doc.element_tag(root), Some("main"));
    assert!(doc
        .nodes
        .iter()
        .any(|node| matches!(&node.kind, NodeKind::Text(text) if text.contains("Quantic & Glide"))));
}

#[test]
fn ladybird_tokenizer_preserves_script_raw_text() {
    let doc = parse(
        r#"<script id="s">if (a < b) { root.innerHTML = "<strong>ok</strong>"; }</script><p>after</p>"#,
    );
    let script = doc.find_by_id("s").expect("script element");
    let text = doc.text_content(script);
    assert!(text.contains("a < b"));
    assert!(text.contains("<strong>ok</strong>"));
}

#[test]
fn ladybird_tokenizer_handles_rcdata_entities() {
    let doc = parse(r#"<textarea id="t">&amp;&lt;b&gt;</textarea>"#);
    let textarea = doc.find_by_id("t").expect("textarea element");
    assert_eq!(doc.text_content(textarea), "&<b>");
}
