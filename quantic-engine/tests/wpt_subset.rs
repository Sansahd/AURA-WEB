use quantic_engine::{
    css, html,
    layout::{self, DisplayItemKind},
    style,
};

#[test]
fn wpt_html_void_element_does_not_capture_following_content() {
    let document = html::parse("<div>A<br>B<img src='x.png'>C</div>");
    let text = document.text_content(document.root);
    assert!(text.contains('A'));
    assert!(text.contains('B'));
    assert!(text.contains('C'));
}

#[test]
fn wpt_css_specificity_id_beats_class_and_type() {
    let document = html::parse("<p id='target' class='note'>text</p>");
    let sheet = css::parse_stylesheet("p{color:blue}.note{color:red}#target{color:#00ff00}");
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
fn wpt_css_color_inherits_to_inline_descendant() {
    let document = html::parse("<div class='parent'><span>child</span></div>");
    let sheet = css::parse_stylesheet(".parent{color:#123456}");
    let styles = style::compute_styles(&document, &sheet);
    let span = document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(id, _)| (document.element_tag(id) == Some("span")).then_some(id))
        .unwrap();
    assert_eq!(styles[span].color, [0x12, 0x34, 0x56, 255]);
}

#[test]
fn wpt_block_flow_places_second_paragraph_after_first() {
    let document = html::parse("<p>First line</p><p>Second line</p>");
    let sheet = css::Stylesheet::default();
    let styles = style::compute_styles(&document, &sheet);
    let result = layout::build_display_list(&document, &styles, 320);

    let first_y = result
        .items
        .iter()
        .find(|item| item.kind == DisplayItemKind::Text && item.text == "First")
        .unwrap()
        .y;
    let second_y = result
        .items
        .iter()
        .find(|item| item.kind == DisplayItemKind::Text && item.text == "Second")
        .unwrap()
        .y;

    assert!(second_y > first_y);
}
