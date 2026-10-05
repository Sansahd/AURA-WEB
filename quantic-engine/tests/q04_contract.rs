use quantic_engine::{layout::DisplayItemKind, Engine};

#[test]
fn q04_partitioned_cookie_history_and_location_work_across_page_runtimes() {
    let engine = Engine::new();

    let mut first = engine
        .load_interactive_html_with_base(
            "<p id='out'>start</p>
             <script>
               document.cookie = 'theme=dark; Path=/';
               history.pushState({}, '', '/account?tab=privacy');
               location.assign('/next');
               document.getElementById('out').textContent = document.cookie;
             </script>",
            "https://app.example/start",
        )
        .unwrap();

    assert_eq!(first.document_cookie(), "theme=dark");
    assert_eq!(
        first.base_url().map(|url| url.as_str()),
        Some("https://app.example/account?tab=privacy")
    );
    assert_eq!(
        first.take_navigation().map(|request| request.url),
        Some("https://app.example/next".to_string())
    );

    let second = engine
        .load_interactive_html_with_base(
            "<p id='cookie'>missing</p>
             <script>document.getElementById('cookie').textContent = document.cookie;</script>",
            "https://app.example/another",
        )
        .unwrap();
    let cookie = second.document().find_by_id("cookie").unwrap();
    assert_eq!(second.document().text_content(cookie), "theme=dark");

    let foreign = engine
        .load_interactive_html_with_base(
            "<p id='cookie'>empty</p>
             <script>document.getElementById('cookie').textContent = document.cookie || 'empty';</script>",
            "https://other.example/",
        )
        .unwrap();
    let cookie = foreign.document().find_by_id("cookie").unwrap();
    assert_eq!(foreign.document().text_content(cookie), "empty");
}

#[test]
fn q04_cross_origin_push_state_is_rejected_by_the_host() {
    let engine = Engine::new();
    let page = engine
        .load_interactive_html_with_base(
            "<script>history.pushState({}, '', 'https://tracker.invalid/escape');</script>",
            "https://app.example/start",
        )
        .unwrap();

    assert_eq!(
        page.base_url().map(|url| url.as_str()),
        Some("https://app.example/start")
    );
    assert!(page
        .script_errors()
        .iter()
        .any(|error| error.contains("history cross-origin URL rejected")));
}

#[test]
fn q04_flex_padding_border_and_rasterizer_work_together() {
    let engine = Engine::new().with_viewport_width(320);
    let output = engine.load_html(
        "<style>
           #row{display:flex;gap:12px;padding:8px;border:2px solid red;background:#eeeeee}
           #a,#b{width:80px;height:32px}
         </style>
         <div id='row'><div id='a'>A</div><div id='b'>B</div></div>",
    );

    let a = output.document.find_by_id("a").unwrap();
    let b = output.document.find_by_id("b").unwrap();
    let row = output.document.find_by_id("row").unwrap();

    let row_box = output
        .display_list
        .iter()
        .find(|item| item.kind == DisplayItemKind::Box && item.node_id == row)
        .unwrap();
    let a_box = output
        .display_list
        .iter()
        .find(|item| item.kind == DisplayItemKind::Box && item.node_id == a)
        .unwrap();
    let b_box = output
        .display_list
        .iter()
        .find(|item| item.kind == DisplayItemKind::Box && item.node_id == b)
        .unwrap();

    assert_eq!(a_box.y, b_box.y);
    assert!(b_box.x >= a_box.x + a_box.width + 12);
    assert_eq!(row_box.border_width, 2);
    assert_eq!(row_box.border_color, Some([255, 0, 0, 255]));

    let raster = engine.rasterize(&output);
    assert_eq!(raster.get_pixel(row_box.x, row_box.y).0, [255, 0, 0, 255]);
}

#[test]
fn q04_css_and_dom_selectors_agree_on_attribute_child_and_pseudo_matching() {
    let engine = Engine::new();
    let page = engine
        .load_interactive_html(
            "<style>
               form > input[type=checkbox]:checked:first-child{color:red}
             </style>
             <form>
               <input id='one' type='checkbox' checked>
               <input id='two' type='checkbox'>
             </form>
             <p id='result'>no</p>
             <script>
               const hit = document.querySelector('form > input[type=checkbox]:checked:first-child');
               document.getElementById('result').textContent = hit ? hit.id : 'none';
             </script>",
        )
        .unwrap();

    let one = page.document().find_by_id("one").unwrap();
    let result = page.document().find_by_id("result").unwrap();
    assert_eq!(page.document().text_content(result), "one");

    let snapshot = page.snapshot();
    assert_eq!(snapshot.styles[one].color, [255, 0, 0, 255]);
}

#[test]
fn q04_text_encoding_and_traversal_are_available_to_page_javascript() {
    let engine = Engine::new();
    let page = engine
        .load_interactive_html(
            "<div><span id='a'>A</span><span id='b'>B</span></div>
             <p id='out'>x</p>
             <script>
               const encoded = new TextEncoder().encode('hé');
               const decoded = new TextDecoder().decode(encoded);
               const next = document.getElementById('a').nextElementSibling.id;
               document.getElementById('out').textContent =
                 decoded + ':' + encoded.length + ':' + next;
             </script>",
        )
        .unwrap();

    let out = page.document().find_by_id("out").unwrap();
    assert_eq!(page.document().text_content(out), "hé:3:b");
}
