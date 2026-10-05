use url::Url;

use crate::dom::{Document, NodeKind};

pub fn effective_base_url(document: &Document, document_url: &Url) -> Url {
    for (node_id, node) in document.nodes.iter().enumerate() {
        let NodeKind::Element { tag, .. } = &node.kind else {
            continue;
        };
        if tag != "base" {
            continue;
        }
        let Some(href) = document.attribute(node_id, "href") else {
            continue;
        };
        if href.trim().is_empty() {
            continue;
        }
        if let Ok(url) = document_url.join(href.trim()) {
            return url;
        }
    }
    document_url.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    #[test]
    fn first_valid_base_href_wins() {
        let document = html::parse(
            r#"<html><head>
                 <base href="/assets/">
                 <base href="https://ignored.example/">
               </head></html>"#,
        );
        let page = Url::parse("https://app.example/a/page.html").unwrap();

        assert_eq!(
            effective_base_url(&document, &page).as_str(),
            "https://app.example/assets/"
        );
    }

    #[test]
    fn invalid_or_empty_base_falls_back_to_document_url() {
        let document = html::parse(r#"<base href=""><main>x</main>"#);
        let page = Url::parse("https://app.example/page").unwrap();
        assert_eq!(effective_base_url(&document, &page), page);
    }

    #[test]
    fn base_href_may_change_resolution_origin_without_changing_document_origin() {
        let document = html::parse(r#"<base href="https://cdn.example/static/">"#);
        let page = Url::parse("https://app.example/page").unwrap();
        let base = effective_base_url(&document, &page);

        assert_eq!(base.as_str(), "https://cdn.example/static/");
        assert_eq!(page.origin().ascii_serialization(), "https://app.example");
    }
}
