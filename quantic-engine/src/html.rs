use crate::dom::{Attribute, Document, NodeKind};

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
const RAW_TEXT_TAGS: &[&str] = &["script", "style"];
const RCDATA_TAGS: &[&str] = &["textarea", "title"];

pub fn parse(input: &str) -> Document {
    let mut document = Document::new();
    let mut stack = vec![document.root];
    let bytes = input.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        if input[cursor..].starts_with("<!--") {
            if let Some(comment_end) = input[cursor + 4..].find("-->") {
                cursor = cursor + 4 + comment_end + 3;
            } else {
                cursor = input.len();
            }
            continue;
        }

        if bytes[cursor] == b'<' {
            if let Some(end) = find_tag_end(input, cursor) {
                let raw = input[cursor + 1..end].trim();

                if let Some(stripped) = raw.strip_prefix('/') {
                    let closing = tag_name(stripped);
                    if let Some(position) = stack.iter().rposition(|id| {
                        matches!(
                            &document.nodes[*id].kind,
                            NodeKind::Element { tag, .. } if tag == &closing
                        )
                    }) {
                        if position > 0 {
                            stack.truncate(position);
                        }
                    }
                } else if !raw.starts_with('!') && !raw.starts_with('?') {
                    let name = tag_name(raw);
                    if !name.is_empty() {
                        auto_close_before_start(&document, &mut stack, &name);
                        let parent = *stack.last().unwrap();
                        let attributes = parse_attributes(raw, &name);
                        let id = document.append(
                            parent,
                            NodeKind::Element {
                                tag: name.clone(),
                                attributes,
                            },
                        );
                        let self_closing = raw.ends_with('/') || VOID_TAGS.contains(&name.as_str());

                        if !self_closing
                            && (RAW_TEXT_TAGS.contains(&name.as_str())
                                || RCDATA_TAGS.contains(&name.as_str()))
                        {
                            let content_start = end + 1;
                            if let Some((close_start, close_end)) =
                                find_raw_closing(input, content_start, &name)
                            {
                                let value = &input[content_start..close_start];
                                if !value.is_empty() {
                                    let text = if RCDATA_TAGS.contains(&name.as_str()) {
                                        decode_entities(value)
                                    } else {
                                        value.to_string()
                                    };
                                    document.append(id, NodeKind::Text(text));
                                }
                                cursor = close_end;
                                continue;
                            }

                            let value = &input[content_start..];
                            if !value.is_empty() {
                                let text = if RCDATA_TAGS.contains(&name.as_str()) {
                                    decode_entities(value)
                                } else {
                                    value.to_string()
                                };
                                document.append(id, NodeKind::Text(text));
                            }
                            cursor = input.len();
                            continue;
                        }

                        if !self_closing {
                            stack.push(id);
                        }
                    }
                }

                cursor = end + 1;
                continue;
            }
        }

        let next = input[cursor..]
            .find('<')
            .map(|offset| cursor + offset)
            .unwrap_or(input.len());
        if next > cursor {
            let parent = *stack.last().unwrap();
            let text = decode_entities(&input[cursor..next]);
            if !text.is_empty() {
                document.append(parent, NodeKind::Text(text));
            }
        }
        cursor = next.max(cursor + 1);
    }

    document
}

fn auto_close_before_start(document: &Document, stack: &mut Vec<usize>, incoming: &str) {
    let should_close_paragraph = matches!(
        incoming,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "div"
            | "dl"
            | "fieldset"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "main"
            | "menu"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "ul"
    );

    let targets: &[&str] = match incoming {
        "li" => &["li"],
        "dt" | "dd" => &["dt", "dd"],
        "tr" => &["tr"],
        "td" | "th" => &["td", "th"],
        "option" => &["option"],
        "optgroup" => &["option", "optgroup"],
        _ if should_close_paragraph => &["p"],
        _ => &[],
    };

    if targets.is_empty() {
        return;
    }

    if let Some(position) = stack.iter().rposition(|id| {
        document
            .element_tag(*id)
            .is_some_and(|tag| targets.contains(&tag))
    }) {
        if position > 0 {
            stack.truncate(position);
        }
    }
}

fn find_tag_end(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut quote = None;
    let mut cursor = start.saturating_add(1);

    while cursor < bytes.len() {
        match (quote, bytes[cursor]) {
            (Some(expected), current) if current == expected => quote = None,
            (None, b'"' | b'\'') => quote = Some(bytes[cursor]),
            (None, b'>') => return Some(cursor),
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn find_raw_closing(input: &str, start: usize, tag: &str) -> Option<(usize, usize)> {
    let remainder = &input[start..];
    let needle = format!("</{tag}");
    let lower = remainder.to_ascii_lowercase();
    let mut search_from = 0usize;

    while let Some(relative) = lower[search_from..].find(&needle) {
        let close_start = start + search_from + relative;
        let after_name = close_start + needle.len();
        let boundary = input.as_bytes().get(after_name).copied();
        if boundary.is_none_or(|byte| byte.is_ascii_whitespace() || byte == b'>') {
            let close_end = find_tag_end(input, close_start)?;
            return Some((close_start, close_end + 1));
        }
        search_from += relative + needle.len();
    }
    None
}

fn tag_name(raw: &str) -> String {
    raw.trim_start()
        .split(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn parse_attributes(raw: &str, tag: &str) -> Vec<Attribute> {
    let mut rest = raw.trim_start();
    if rest.len() >= tag.len() {
        rest = &rest[tag.len()..];
    }

    let bytes = rest.as_bytes();
    let mut cursor = 0usize;
    let mut attributes = Vec::new();

    while cursor < bytes.len() {
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b'/')
        {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }

        let name_start = cursor;
        while cursor < bytes.len()
            && !bytes[cursor].is_ascii_whitespace()
            && bytes[cursor] != b'='
            && bytes[cursor] != b'/'
        {
            cursor += 1;
        }
        if cursor == name_start {
            cursor += 1;
            continue;
        }

        let name = rest[name_start..cursor].to_ascii_lowercase();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let mut value = String::new();
        if cursor < bytes.len() && bytes[cursor] == b'=' {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor < bytes.len() && (bytes[cursor] == b'"' || bytes[cursor] == b'\'') {
                let quote = bytes[cursor];
                cursor += 1;
                let value_start = cursor;
                while cursor < bytes.len() && bytes[cursor] != quote {
                    cursor += 1;
                }
                value = decode_entities(&rest[value_start..cursor]);
                if cursor < bytes.len() {
                    cursor += 1;
                }
            } else {
                let value_start = cursor;
                while cursor < bytes.len()
                    && !bytes[cursor].is_ascii_whitespace()
                    && bytes[cursor] != b'/'
                {
                    cursor += 1;
                }
                value = decode_entities(&rest[value_start..cursor]);
            }
        }

        attributes.push(Attribute { name, value });
    }

    attributes
}

fn decode_entities(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut cursor = 0usize;

    while cursor < value.len() {
        let remainder = &value[cursor..];
        if !remainder.starts_with('&') {
            let next = remainder.find('&').unwrap_or(remainder.len());
            out.push_str(&remainder[..next]);
            cursor += next;
            continue;
        }

        let Some(relative_end) = remainder.find(';') else {
            out.push('&');
            cursor += 1;
            continue;
        };
        let entity = &remainder[1..relative_end];
        let decoded = match entity {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        };

        if let Some(character) = decoded {
            out.push(character);
        } else {
            out.push_str(&remainder[..=relative_end]);
        }
        cursor += relative_end + 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::NodeKind;

    #[test]
    fn implicit_end_tags_and_stray_closers_do_not_corrupt_tree() {
        let doc = parse(
            "<ul><li id='one'>One<li id='two'>Two</ul></ghost><p id='p'>A<div id='d'>B</div>",
        );
        let one = doc.find_by_id("one").unwrap();
        let two = doc.find_by_id("two").unwrap();
        let p = doc.find_by_id("p").unwrap();
        let div = doc.find_by_id("d").unwrap();

        assert_eq!(doc.nodes[one].parent, doc.nodes[two].parent);
        assert_ne!(doc.nodes[div].parent, Some(p));
        assert_eq!(doc.text_content(two), "Two");
    }

    #[test]
    fn dom_preserves_source_whitespace_for_css_to_resolve() {
        let doc = parse("<p id='p'>A   B\n C</p><pre id='pre'>A   B\n C</pre>");
        assert_eq!(doc.text_content(doc.find_by_id("p").unwrap()), "A   B\n C");
        assert_eq!(doc.text_content(doc.find_by_id("pre").unwrap()), "A   B\n C");
    }

    #[test]
    fn parses_nested_html_and_text() {
        let doc = parse("<main><h1>Hello</h1><p>Quantic &amp; Glide</p></main>");
        assert!(doc
            .nodes
            .iter()
            .any(|n| matches!(&n.kind, NodeKind::Element { tag, .. } if tag == "main")));
        assert!(doc
            .nodes
            .iter()
            .any(|n| matches!(&n.kind, NodeKind::Text(t) if t.contains("Quantic & Glide"))));
    }

    #[test]
    fn parses_id_class_and_href() {
        let doc = parse("<a id='go' class=\"cta primary\" href=\"/next\">Go</a>");
        let link = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "a"))
            .unwrap();
        assert_eq!(doc.attribute(link, "id"), Some("go"));
        assert!(doc.has_class(link, "primary"));
        assert_eq!(doc.attribute(link, "href"), Some("/next"));
    }

    #[test]
    fn ignores_comments() {
        let doc = parse("<p>A</p><!-- secret --><p>B</p>");
        assert!(!doc
            .nodes
            .iter()
            .any(|n| matches!(&n.kind, NodeKind::Text(t) if t.contains("secret"))));
    }

    #[test]
    fn raw_script_text_keeps_markup_like_javascript_intact() {
        let source = r#"<script>if (a < b) { root.innerHTML = "<strong>ok</strong>"; }</script><p>after</p>"#;
        let doc = parse(source);
        let script = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "script"))
            .unwrap();

        assert!(doc.text_content(script).contains("a < b"));
        assert!(doc.text_content(script).contains("<strong>ok</strong>"));
        assert!(doc
            .nodes
            .iter()
            .any(|node| matches!(&node.kind, NodeKind::Text(text) if text == "after")));
    }

    #[test]
    fn style_and_rcdata_have_distinct_entity_rules() {
        let doc = parse(
            r#"<style>.x::before{content:"&amp;<b>"}</style><textarea>&amp;&lt;b&gt;</textarea>"#,
        );
        let style = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "style"))
            .unwrap();
        let textarea = doc
            .nodes
            .iter()
            .position(
                |node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "textarea"),
            )
            .unwrap();

        assert!(doc.text_content(style).contains("&amp;<b>"));
        assert_eq!(doc.text_content(textarea), "&<b>");
    }

    #[test]
    fn tag_end_ignores_greater_than_inside_quoted_attributes() {
        let doc = parse(r#"<div data-expression="a > b">ok</div>"#);
        let div = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "div"))
            .unwrap();
        assert_eq!(doc.attribute(div, "data-expression"), Some("a > b"));
    }

    #[test]
    fn decodes_numeric_entities() {
        let doc = parse("<p>&#65;&#x42;</p>");
        assert!(doc
            .nodes
            .iter()
            .any(|node| matches!(&node.kind, NodeKind::Text(text) if text == "AB")));
    }
}
