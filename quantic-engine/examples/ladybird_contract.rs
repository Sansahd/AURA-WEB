use quantic_engine::dom::NodeKind;
use quantic_engine::html::parse;

fn main() {
    let doc = parse("<main id='root'><h1>Hello</h1><p>Quantic &amp; Glide</p></main>");
    let root = doc.find_by_id("root").expect("root element");
    assert_eq!(doc.element_tag(root), Some("main"));
    assert!(doc
        .nodes
        .iter()
        .any(|node| matches!(&node.kind, NodeKind::Text(text) if text.contains("Quantic & Glide"))));

    let doc = parse(
        r#"<script id="s">if (a < b) { root.innerHTML = "<strong>ok</strong>"; }</script><p>after</p>"#,
    );
    let script = doc.find_by_id("s").expect("script element");
    let script_text = doc.text_content(script);
    assert!(script_text.contains("a < b"));
    assert!(script_text.contains("<strong>ok</strong>"));

    let doc = parse(r#"<textarea id="t">&amp;&lt;b&gt;</textarea>"#);
    let textarea = doc.find_by_id("t").expect("textarea element");
    assert_eq!(doc.text_content(textarea), "&<b>");

    println!("LADYBIRD_HTML_CONTRACT_PASSED");
}
