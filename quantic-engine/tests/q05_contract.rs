use std::collections::BTreeMap;

use image::{Rgba, RgbaImage};
use quantic_engine::{
    compositor::ViewportCompositor,
    html,
    js_runtime::DomMutation,
    resources::{ResourceClass, ResourcePlan},
    BoaRuntime, BrowserSession, Engine, JsRuntime,
};
use url::Url;

#[test]
fn q05_exposes_a_browser_session_not_just_a_document_renderer() {
    let session = BrowserSession::new(Engine::new()).with_viewport_height(720);
    assert!(session.current_page().is_none());
    assert!(session.history().is_empty());
    assert_eq!(session.scroll_y(), 0);
}

#[test]
fn q05_builds_a_resource_plan_before_transport() {
    let document = html::parse(
        r#"<html><head>
        <link rel="stylesheet" href="app.css">
        <script defer src="app.js"></script>
        </head><body><img src="hero.webp"></body></html>"#,
    );
    let base = Url::parse("https://quantic.test/site/").unwrap();
    let plan = ResourcePlan::discover(&document, &base);

    assert_eq!(plan.candidates.len(), 3);
    assert_eq!(plan.candidates[0].class, ResourceClass::Stylesheet);
    assert!(plan.candidates[0].render_blocking);
    assert!(plan.candidates.iter().any(|candidate| {
        candidate.class == ResourceClass::Image
            && candidate.url.as_str() == "https://quantic.test/site/hero.webp"
    }));
}

#[test]
fn q05_compositor_produces_a_scrolled_viewport() {
    let mut page = RgbaImage::from_pixel(3, 6, Rgba([0, 0, 0, 255]));
    page.put_pixel(1, 5, Rgba([255, 255, 255, 255]));

    let compositor = ViewportCompositor::new(3, 2);
    let viewport = compositor.compose(&page, u32::MAX);

    assert_eq!(viewport.dimensions(), (3, 2));
    assert_eq!(viewport.get_pixel(1, 1), &Rgba([255, 255, 255, 255]));
}

#[test]
fn q05_grid_places_children_in_columns() {
    let output = Engine::new().with_viewport_width(600).load_html_with_css(
        r#"<div id="grid"><div id="a">A</div><div id="b">B</div><div id="c">C</div></div>"#,
        "#grid{display:grid;grid-template-columns:repeat(3,1fr);gap:10px}",
    );
    let a = output.document.find_by_id("a").unwrap();
    let b = output.document.find_by_id("b").unwrap();
    let c = output.document.find_by_id("c").unwrap();

    let box_for = |node_id| {
        output
            .display_list
            .iter()
            .find(|item| {
                item.kind == quantic_engine::layout::DisplayItemKind::Box && item.node_id == node_id
            })
            .unwrap()
    };
    let a_box = box_for(a);
    let b_box = box_for(b);
    let c_box = box_for(c);

    assert_eq!(a_box.y, b_box.y);
    assert_eq!(b_box.y, c_box.y);
    assert!(a_box.x < b_box.x && b_box.x < c_box.x);
}


#[test]
fn q05_executes_a_relative_ecmascript_module_graph() {
    let document = html::parse("<p id='out'>pending</p>");
    let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
    let mut dependencies = BTreeMap::new();
    dependencies.insert(
        "quantic/dep.js".to_string(),
        "export const answer = 42;".to_string(),
    );

    runtime
        .eval_module_graph(
            "import { answer } from './dep.js';
             document.getElementById('out').textContent = String(answer);",
            "quantic/main.js",
            &dependencies,
        )
        .unwrap();

    let effects = runtime.drain_effects().unwrap();
    assert!(effects.mutations.iter().any(|mutation| matches!(
        mutation,
        DomMutation::SetText { value, .. } if value == "42"
    )));
}

#[test]
fn q05_preserves_location_replace_navigation_semantics() {
    let engine = Engine::new();
    let mut page = engine
        .load_interactive_html_with_base(
            "<main>replace navigation</main>",
            "https://app.example/start",
        )
        .unwrap();

    page.execute_script("location.replace('/final');").unwrap();
    let request = page.take_navigation().unwrap();

    assert_eq!(request.url, "https://app.example/final");
    assert_eq!(request.method, "GET");
    assert!(request.replace);
}
