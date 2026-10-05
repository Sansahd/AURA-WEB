use quantic_engine::{layout::DisplayItemKind, Engine};

#[test]
fn q03_javascript_event_timer_dom_and_renderer_work_end_to_end() {
    let engine = Engine::new().with_viewport_width(420);
    let mut page = engine
        .load_interactive_html(
            "<main>
               <button id='go'>Run</button>
               <p id='status'>Idle</p>
               <script>
                 document.getElementById('go').addEventListener('click', () => {
                   setTimeout(() => {
                     const status = document.getElementById('status');
                     status.textContent = 'Running independently';
                     status.style.color = '#008000';
                     localStorage.setItem('lastState', 'running');
                   }, 1);
                 });
               </script>
             </main>",
        )
        .unwrap();

    page.click("go").unwrap();

    let status = page.document().find_by_id("status").unwrap();
    assert_eq!(
        page.document().text_content(status),
        "Running independently"
    );
    assert_eq!(
        page.local_storage()
            .unwrap()
            .get("lastState")
            .map(String::as_str),
        Some("running")
    );

    let snapshot = page.snapshot();
    let running = snapshot
        .display_list
        .iter()
        .find(|item| item.kind == DisplayItemKind::Text && item.text == "Running")
        .unwrap();
    assert_eq!(running.color, [0, 128, 0, 255]);

    let raster = engine.rasterize(&snapshot);
    assert_eq!(raster.width(), 420);
    assert!(raster.pixels().any(|pixel| pixel.0 != [255, 255, 255, 255]));
}

#[test]
fn q03_dynamic_dom_is_visible_in_the_next_layout_pass() {
    let engine = Engine::new().with_viewport_width(360);
    let page = engine
        .load_interactive_html(
            "<body><section id='root'></section>
             <script>
               const card = document.createElement('div');
               card.id = 'created';
               card.style.backgroundColor = '#eeeeee';
               card.append('Dynamic Quantic DOM');
               document.getElementById('root').appendChild(card);
             </script></body>",
        )
        .unwrap();

    let created = page.document().find_by_id("created").unwrap();
    assert_eq!(page.document().text_content(created), "Dynamic Quantic DOM");

    let snapshot = page.snapshot();
    assert!(snapshot
        .display_list
        .iter()
        .any(|item| item.kind == DisplayItemKind::Text && item.text == "Dynamic"));
}

#[test]
fn q03_local_storage_survives_new_page_runtime_on_same_engine_partition() {
    let engine = Engine::new();

    engine
        .load_interactive_html("<script>localStorage.setItem('quanticKey', 'persisted');</script>")
        .unwrap();

    let second = engine
        .load_interactive_html(
            "<p id='out'>empty</p>
             <script>
               document.getElementById('out').textContent =
                 localStorage.getItem('quanticKey') || 'missing';
             </script>",
        )
        .unwrap();

    let out = second.document().find_by_id("out").unwrap();
    assert_eq!(second.document().text_content(out), "persisted");
}

#[test]
fn q03_form_host_input_and_submit_event_share_the_same_dom_state() {
    let engine = Engine::new();
    let mut page = engine
        .load_interactive_html(
            "<form id='contact' method='post' action='/send'>
               <input id='name' name='name' value='Ada'>
               <input name='ignored' type='checkbox'>
             </form>
             <script>
               document.getElementById('contact').addEventListener('submit', event => {
                 event.preventDefault();
               });
             </script>",
        )
        .unwrap();

    page.set_value("name", "Grace").unwrap();
    let submission = page.submit_form("contact").unwrap();

    assert_eq!(submission.method, "POST");
    assert!(submission.prevented);
    assert_eq!(submission.fields.len(), 1);
    assert_eq!(submission.fields[0].name, "name");
    assert_eq!(submission.fields[0].value, "Grace");
}
