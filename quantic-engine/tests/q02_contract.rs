use quantic_engine::{css, layout::DisplayItemKind, network, style, Engine};
use url::Url;

#[test]
fn q02_css_cascade_layout_and_rasterizer_work_together() {
    let engine = Engine::new().with_viewport_width(360);
    let output = engine.load_html(
        "<style>
          body { color:#202020; }
          #hero { color:#ff3300; font-size:24px; margin:12px; }
          .card { background-color:#eeeeee; }
        </style>
        <body><section class='card'><h1 id='hero'>Quantic Engine</h1><p>Private by architecture.</p></section></body>",
    );

    let hero = output
        .display_list
        .iter()
        .find(|item| item.kind == DisplayItemKind::Text && item.text == "Quantic")
        .unwrap();
    assert_eq!(hero.color, [255, 51, 0, 255]);
    assert_eq!(hero.font_size, 24);
    assert!(output.content_height > 40);

    let image = engine.rasterize(&output);
    assert_eq!(image.width(), 360);
    assert!(image.pixels().any(|pixel| pixel.0 != [255, 255, 255, 255]));
}

#[test]
fn q02_selector_specificity_matches_supported_web_subset() {
    let document = quantic_engine::html::parse("<main id='app'><p class='note'>Hello</p></main>");
    let sheet = css::parse_stylesheet("p{color:blue}.note{color:red}#app .note{color:#00ff00}");
    let styles = style::compute_styles(&document, &sheet);
    let paragraph = document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(id, _)| (document.element_tag(id) == Some("p")).then_some(id))
        .unwrap();

    assert_eq!(styles[paragraph].color, [0, 255, 0, 255]);
}

#[test]
fn q02_url_resolution_keeps_same_origin_resource_explicit() {
    let base = Url::parse("https://example.com/docs/page.html").unwrap();
    let resolved = network::resolve_url(&base, "/assets/site.css").unwrap();
    assert_eq!(resolved.as_str(), "https://example.com/assets/site.css");
}
