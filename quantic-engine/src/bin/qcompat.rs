use std::{collections::BTreeMap, process};

use quantic_engine::{
    effective_base_url, html, js_runtime::DomMutation, layout::DisplayItemKind, BoaRuntime,
    BrowserSession, Engine, FontSpec, FontSystem, JsRuntime, PrivacyDecision, RequestContext,
    ResourceClass, ResourceKind, ResourcePlan, ENGINE_VERSION,
};
use serde_json::json;

fn main() {
    let mut checks = BTreeMap::new();
    checks.insert(
        "non_chromium_identity",
        !ENGINE_VERSION.is_empty() && !ENGINE_VERSION.to_ascii_lowercase().contains("chrom"),
    );

    let engine = Engine::new().with_viewport_width(320);

    let base_document = html::parse(
        r#"<base href="https://cdn.example/assets/"><img id="hero" src="hero.webp">"#,
    );
    let document_url = url::Url::parse("https://app.example/path/page.html").unwrap();
    let effective_base = effective_base_url(&base_document, &document_url);
    let base_plan = ResourcePlan::discover(&base_document, &effective_base);
    checks.insert(
        "document_url_base_resolution",
        effective_base.as_str() == "https://cdn.example/assets/"
            && base_plan
                .candidates
                .iter()
                .any(|candidate| candidate.url.as_str() == "https://cdn.example/assets/hero.webp"),
    );

    let font_scans_before = FontSystem::system_scan_count();
    let _cached_fonts_one = FontSystem::system();
    let _cached_fonts_two = FontSystem::system();
    let font_scans_after = FontSystem::system_scan_count();
    checks.insert(
        "system_font_scan_cached",
        font_scans_before == font_scans_after && font_scans_after <= 1,
    );

    let cluster_fonts = FontSystem::system();
    let cluster_spec = FontSpec::new(&["sans-serif".into()], 400, false, 20);
    let cluster_text = "e\u{301}";
    let cluster_metrics = cluster_fonts.measure_text(cluster_text, &cluster_spec);
    checks.insert(
        "grapheme_font_fallback",
        cluster_fonts.face_count() > 0
            && cluster_fonts.fallback_run_count(cluster_text, &cluster_spec) >= 1
            && cluster_metrics.width > 0
            && cluster_fonts
                .rasterize_text(cluster_text, &cluster_spec)
                .is_some_and(|raster| raster.width > 0),
    );

    let privacy = engine.evaluate_request(&RequestContext {
        top_level_host: "app.example".into(),
        request_host: "tracker.invalid".into(),
        kind: ResourceKind::Script,
    });
    checks.insert(
        "third_party_privacy_gate",
        matches!(privacy, PrivacyDecision::Block { .. }),
    );

    let font_privacy = engine.evaluate_request(&RequestContext {
        top_level_host: "app.example".into(),
        request_host: "fonts.tracker.invalid".into(),
        kind: ResourceKind::Font,
    });
    checks.insert(
        "third_party_font_gate",
        matches!(font_privacy, PrivacyDecision::Block { .. }),
    );
    checks.insert("system_font_database", engine.font_face_count() > 0);

    let font_output =
        engine.load_html("<style>p{font-family:sans-serif;font-size:24px}</style><p>iiii WWWW</p>");
    let font_widths = font_output
        .display_list
        .iter()
        .filter(|item| item.kind == DisplayItemKind::Text)
        .filter_map(|item| match item.text.as_str() {
            "iiii" | "WWWW" => Some((item.text.as_str(), item.width)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let proportional_font_metrics = font_widths.len() == 2
        && font_widths[0].1 > 0
        && font_widths[1].1 > 0
        && font_widths[0].1 != font_widths[1].1;
    checks.insert(
        "shaped_proportional_font_metrics",
        proportional_font_metrics,
    );

    let webfont_page = engine.load_interactive_html_with_base(
        "<style>
           @font-face{font-family:'Blocked';src:url('https://fonts.tracker.invalid/font.ttf')}
           p{font-family:'Blocked',sans-serif}
         </style>
         <p>Quantic</p>",
        "https://app.example/start",
    );
    let webfont_host_bridge = webfont_page
        .as_ref()
        .ok()
        .map(|page| page.snapshot())
        .is_some_and(|snapshot| {
            snapshot.blocked_resources.iter().any(|resource| {
                resource.contains("fonts.tracker.invalid")
                    && resource.contains("third-party-denied-by-default")
            })
        });
    checks.insert("webfont_host_privacy_bridge", webfont_host_bridge);

    let websocket_privacy = engine.evaluate_request(&RequestContext {
        top_level_host: "app.example".into(),
        request_host: "tracker.invalid".into(),
        kind: ResourceKind::WebSocket,
    });
    checks.insert(
        "third_party_websocket_gate",
        matches!(websocket_privacy, PrivacyDecision::Block { .. }),
    );

    let websocket_page = engine.load_interactive_html_with_base(
        "<p id='ws'>pending</p>
         <script>
           const socket = new WebSocket('wss://tracker.invalid/events');
           socket.onerror = () => {
             document.getElementById('ws').textContent = 'blocked';
           };
         </script>",
        "https://app.example/start",
    );
    let websocket_bridge_ok = websocket_page
        .as_ref()
        .ok()
        .and_then(|page| page.document().find_by_id("ws").map(|id| (page, id)))
        .is_some_and(|(page, id)| {
            page.document().text_content(id) == "blocked"
                && page
                    .blocked_resources()
                    .iter()
                    .any(|resource| resource.contains("third-party-denied-by-default"))
        });
    checks.insert("websocket_host_bridge", websocket_bridge_ok);

    let interactive = engine.load_interactive_html_with_base(
        "<p id='out'>x</p>
         <script>
           document.cookie='mode=private; Path=/';
           const params=new URLSearchParams('a=1');
           params.set('a','2');
           history.pushState({},'', '/inside');
           const bytes=new TextEncoder().encode('hé');
           document.getElementById('out').textContent =
             [document.cookie, params.get('a'), bytes.length].join('|');
         </script>",
        "https://app.example/start",
    );
    let interactive_ok = interactive
        .as_ref()
        .ok()
        .and_then(|page| page.document().find_by_id("out").map(|id| (page, id)))
        .is_some_and(|(page, id)| {
            page.document().text_content(id) == "mode=private|2|3"
                && page.document_cookie() == "mode=private"
                && page.base_url().map(|url| url.path()) == Some("/inside")
        });
    checks.insert("interactive_web_apis", interactive_ok);

    let live_params = engine.load_interactive_html(
        "<p id='live'>pending</p>
         <script>
           const url=new URL('https://example.test/?a=1&b=2&c=3');
           const params=url.searchParams;
           const seen=[];
           for(const [key,value] of params){
             if(key==='a') params.delete('b');
             seen.push(key+':'+value);
           }
           url.search='?x=4';
           document.getElementById('live').textContent=
             seen.join(',')+'|'+params.get('x')+'|'+(params===url.searchParams);
         </script>",
    );
    checks.insert(
        "live_urlsearchparams",
        live_params
            .as_ref()
            .ok()
            .and_then(|page| {
                let id = page.document().find_by_id("live")?;
                Some(page.document().text_content(id) == "a:1,c:3|4|true")
            })
            .unwrap_or(false),
    );

    let output = engine.load_html(
        "<style>
           #row{display:flex;gap:10px;padding:6px;border:2px solid red}
           #a,#b{width:70px}
         </style>
         <div id='row'><div id='a'>A</div><div id='b'>B</div></div>",
    );
    let flex_ok = output
        .document
        .find_by_id("a")
        .zip(output.document.find_by_id("b"))
        .and_then(|(a, b)| {
            let a_box = output
                .display_list
                .iter()
                .find(|item| item.kind == DisplayItemKind::Box && item.node_id == a)?;
            let b_box = output
                .display_list
                .iter()
                .find(|item| item.kind == DisplayItemKind::Box && item.node_id == b)?;
            Some(a_box.y == b_box.y && b_box.x >= a_box.x + a_box.width + 10)
        })
        .unwrap_or(false);
    checks.insert("flexbox_layout", flex_ok);

    let grid_output = engine.load_html(
        "<style>
           #grid{display:grid;grid-template-columns:repeat(3,1fr);gap:8px}
         </style>
         <div id='grid'><div id='ga'>A</div><div id='gb'>B</div><div id='gc'>C</div></div>",
    );
    let grid_ok = grid_output
        .document
        .find_by_id("ga")
        .zip(grid_output.document.find_by_id("gb"))
        .zip(grid_output.document.find_by_id("gc"))
        .and_then(|((a, b), c)| {
            let box_for = |node_id| {
                grid_output
                    .display_list
                    .iter()
                    .find(|item| item.kind == DisplayItemKind::Box && item.node_id == node_id)
            };
            let a_box = box_for(a)?;
            let b_box = box_for(b)?;
            let c_box = box_for(c)?;
            Some(a_box.y == b_box.y && b_box.y == c_box.y && a_box.x < b_box.x && b_box.x < c_box.x)
        })
        .unwrap_or(false);
    checks.insert("grid_layout", grid_ok);

    let variables = engine.load_html(
        "<style>
           :root{--accent:#2255aa;--space:14px}
           #vars{color:var(--accent);padding:var(--space)}
         </style>
         <html><body><div id='vars'>variables</div></body></html>",
    );
    let css_variables_ok = variables
        .document
        .find_by_id("vars")
        .map(|id| {
            variables.styles[id].color == [34, 85, 170, 255]
                && variables.styles[id].padding_left == 14
                && variables.styles[id].padding_top == 14
        })
        .unwrap_or(false);
    checks.insert("css_custom_properties", css_variables_ok);

    let mut browser = BrowserSession::new(engine.clone()).with_viewport_height(160);
    let browser_ok = browser
        .open_html(
            "<main><div style='height:320px'>deterministic tall viewport fixture</div></main>",
        )
        .is_ok()
        && browser.current_url() == Some("about:blank")
        && browser.history().len() == 1
        && browser.snapshot().is_some()
        && browser
            .rasterize_viewport()
            .is_some_and(|image| image.height() == 160)
        && browser.scroll_to(u32::MAX) > 0;
    checks.insert("browser_session_viewport", browser_ok);

    let cache_policy_ok = quantic_engine::is_cacheable_kind(ResourceKind::Style)
        && quantic_engine::is_cacheable_kind(ResourceKind::Script)
        && quantic_engine::is_cacheable_kind(ResourceKind::Image)
        && !quantic_engine::is_cacheable_kind(ResourceKind::Document)
        && engine.resource_cache_entries() == 0;
    checks.insert("resource_cache_policy", cache_policy_ok);

    let selector_page = engine.load_interactive_html(
        "<style>form > input[type=checkbox]:checked:first-child{color:red}</style>
         <form><input id='one' type='checkbox' checked><input id='two' type='checkbox'></form>
         <p id='hit'>none</p>
         <script>
           const hit=document.querySelector('form > input[type=checkbox]:checked:first-child');
           document.getElementById('hit').textContent=hit ? hit.id : 'none';
         </script>",
    );
    let selector_ok = selector_page
        .as_ref()
        .ok()
        .and_then(|page| {
            let hit = page.document().find_by_id("hit")?;
            let one = page.document().find_by_id("one")?;
            let snapshot = page.snapshot();
            Some(
                page.document().text_content(hit) == "one"
                    && snapshot.styles[one].color == [255, 0, 0, 255],
            )
        })
        .unwrap_or(false);
    checks.insert("css_dom_selector_parity", selector_ok);

    let advanced_selector_page = engine.load_interactive_html(
        "<style>
           h2 + p[data-role^=lead] ~ p.note[lang|=fr]:nth-child(4){color:#1234}
         </style>
         <section>
           <h2>T</h2><p id='lead' data-role='leader'>A</p><span>x</span>
           <p id='target' class='note hot' lang='fr-CA'>B</p>
         </section>
         <p id='selector-result'>none</p>
         <script>
           const hit=document.querySelector(
             'h2 + p[data-role^=lead] ~ p.note[lang|=fr]:nth-child(4)'
           );
           document.getElementById('selector-result').textContent=hit ? hit.id : 'none';
         </script>",
    );
    let advanced_selector_ok = advanced_selector_page
        .as_ref()
        .ok()
        .and_then(|page| {
            let target = page.document().find_by_id("target")?;
            let result = page.document().find_by_id("selector-result")?;
            let snapshot = page.snapshot();
            Some(
                page.document().text_content(result) == "target"
                    && snapshot.styles[target].color == [17, 34, 51, 68],
            )
        })
        .unwrap_or(false);
    checks.insert("advanced_css_dom_selectors", advanced_selector_ok);

    let media_engine = Engine::new().with_viewport_width(480);
    let media_output = media_engine.load_html(
        "<style>
           #media{color:red}
           @media (max-width:500px){#media{color:#00ff00}}
           @media (min-width:700px){#media{color:blue}}
         </style><div id='media'>responsive</div>",
    );
    let media_css_ok = media_output
        .document
        .find_by_id("media")
        .is_some_and(|id| media_output.styles[id].color == [0, 255, 0, 255]);
    let media_js = media_engine.load_interactive_html(
        "<p id='media-js'>x</p><script>
           document.getElementById('media-js').textContent =
             [innerWidth,matchMedia('(max-width:500px)').matches,
              matchMedia('(min-width:700px)').matches].join('|');
         </script>",
    );
    let media_js_ok = media_js
        .as_ref()
        .ok()
        .and_then(|page| {
            let id = page.document().find_by_id("media-js")?;
            Some(page.document().text_content(id) == "480|true|false")
        })
        .unwrap_or(false);
    checks.insert("css_js_media_viewport_parity", media_css_ok && media_js_ok);

    let responsive_document = html::parse(
        r#"<picture>
             <source srcset="/hero.webp 1x, /hero@2x.webp 2x">
             <img id="hero" src="/fallback.jpg">
           </picture>
           <img id="duplicate-a" src="/same.png">
           <img id="duplicate-b" src="/same.png">"#,
    );
    let responsive_base = url::Url::parse("https://app.example/").unwrap();
    let responsive_plan = ResourcePlan::discover(&responsive_document, &responsive_base);
    let responsive_images = responsive_plan
        .candidates
        .iter()
        .filter(|candidate| candidate.class == ResourceClass::Image)
        .collect::<Vec<_>>();
    checks.insert(
        "responsive_image_planning",
        responsive_images.len() == 3
            && responsive_images[0].url.as_str() == "https://app.example/hero.webp"
            && responsive_images[1].node_id != responsive_images[2].node_id,
    );

    let mut history_browser = BrowserSession::new(Engine::new());
    let history_ok = history_browser
        .open_html_with_base("<main>history</main>", "https://app.example/start")
        .and_then(|_| {
            history_browser.execute_script(
                "history.pushState({},'', '/next');history.replaceState({},'', '/final');",
            )
        })
        .is_ok()
        && history_browser.history().len() == 2
        && history_browser.current_url() == Some("https://app.example/final");
    checks.insert("javascript_browser_history_sync", history_ok);

    let module_document = html::parse("<p id='module-result'>pending</p>");
    let mut module_runtime = BoaRuntime::new(&module_document, &BTreeMap::new()).unwrap();
    let mut module_dependencies = BTreeMap::new();
    module_dependencies.insert(
        "quantic/dep.js".to_string(),
        "export const value = 42;".to_string(),
    );
    let module_graph_ok = module_runtime
        .eval_module_graph(
            "import { value } from './dep.js';
             document.getElementById('module-result').textContent = String(value);",
            "quantic/main.js",
            &module_dependencies,
        )
        .is_ok()
        && module_runtime
            .drain_effects()
            .ok()
            .is_some_and(|effects| {
                effects.mutations.iter().any(|mutation| {
                    matches!(
                        mutation,
                        DomMutation::SetText { value, .. } if value == "42"
                    )
                })
            });
    checks.insert("ecmascript_relative_module_graph", module_graph_ok);

    let replace_navigation_ok = engine
        .load_interactive_html_with_base(
            "<main>replace</main>",
            "https://app.example/start",
        )
        .ok()
        .and_then(|mut page| {
            page.execute_script("location.replace('/final');").ok()?;
            page.take_navigation()
        })
        .is_some_and(|request| {
            request.url == "https://app.example/final"
                && request.method == "GET"
                && request.replace
        });
    checks.insert(
        "javascript_replace_navigation_semantics",
        replace_navigation_ok,
    );

    let fetch_document = html::parse("<p id='fetch'>x</p>");
    let mut fetch_runtime = BoaRuntime::new(&fetch_document, &BTreeMap::new()).unwrap();
    let fetch_surface_ok = fetch_runtime
        .eval_script(
            "const headers=new Headers({'X-Quantic':'yes'});
             const request=new Request('/api',{method:'POST',headers,body:'hello'});
             fetch(request);",
        )
        .is_ok()
        && fetch_runtime
            .drain_effects()
            .ok()
            .and_then(|effects| effects.fetches.into_iter().next())
            .is_some_and(|request| {
                request.method == "POST"
                    && request.body.as_deref() == Some("hello")
                    && request.headers.get("x-quantic").map(String::as_str) == Some("yes")
            });
    checks.insert("modern_fetch_surface", fetch_surface_ok);

    let constrained = engine.load_html(
        "<style>
           #box{width:500px;min-width:80px;max-width:120px;
                height:200px;max-height:60px;font-size:150%;line-height:1.5;
                background:rgba(10,20,30,.5)}
         </style><div id='box'>box</div>",
    );
    let sizing_ok = constrained
        .document
        .find_by_id("box")
        .and_then(|id| {
            let style = &constrained.styles[id];
            let item = constrained
                .display_list
                .iter()
                .find(|item| item.kind == DisplayItemKind::Box && item.node_id == id)?;
            Some(
                item.width == 120
                    && item.height == 60
                    && style.font_size == 24
                    && style.line_height() == 36
                    && style.background == Some([10, 20, 30, 128]),
            )
        })
        .unwrap_or(false);
    checks.insert("modern_css_sizing_typography_colors", sizing_ok);

    let malformed_html = html::parse(
        "<ul><li id='one'>One<li id='two'>Two</ul></ghost><p id='p'>A<div id='d'>B</div>",
    );
    let html_recovery_ok = malformed_html
        .find_by_id("one")
        .zip(malformed_html.find_by_id("two"))
        .zip(malformed_html.find_by_id("p"))
        .zip(malformed_html.find_by_id("d"))
        .is_some_and(|(((one, two), paragraph), div)| {
            malformed_html.nodes[one].parent == malformed_html.nodes[two].parent
                && malformed_html.nodes[div].parent != Some(paragraph)
                && malformed_html.text_content(two) == "Two"
        });
    checks.insert("html_implicit_end_tag_recovery", html_recovery_ok);

    let passed = checks.values().filter(|value| **value).count();
    let total = checks.len();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "engine": "Gekko / Quantic Engine",
            "version": ENGINE_VERSION,
            "milestone": "Gekko 0.8.5 quality gate",
            "passed": passed,
            "total": total,
            "score_percent": passed * 100 / total.max(1),
            "checks": checks
        }))
        .expect("compatibility scorecard JSON")
    );

    if passed != total {
        process::exit(1);
    }
}
