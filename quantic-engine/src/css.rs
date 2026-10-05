use crate::dom::{Document, NodeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    pub value: String,
    pub important: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeOperator {
    Exists,
    Equals,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeSelector {
    pub name: String,
    pub operator: AttributeOperator,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseudoClass {
    FirstChild,
    LastChild,
    FirstOfType,
    LastOfType,
    NthChild(u32),
    NthChildOdd,
    NthChildEven,
    Checked,
    Disabled,
    Root,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleSelector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<AttributeSelector>,
    pub pseudos: Vec<PseudoClass>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
    AdjacentSibling,
    GeneralSibling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub parts: Vec<SimpleSelector>,
    pub combinators: Vec<Combinator>,
    pub specificity: (u16, u16, u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
    pub order: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

pub fn parse_stylesheet(source: &str) -> Stylesheet {
    parse_stylesheet_for_viewport(source, 1024)
}

pub fn parse_stylesheet_for_viewport(source: &str, viewport_width: u32) -> Stylesheet {
    let source = strip_comments(source);
    let mut rules = Vec::new();
    let mut order = 0usize;
    parse_rule_list(&source, viewport_width, &mut order, &mut rules);
    Stylesheet { rules }
}

fn parse_rule_list(
    source: &str,
    viewport_width: u32,
    order: &mut usize,
    rules: &mut Vec<Rule>,
) {
    let mut cursor = 0usize;

    while cursor < source.len() {
        cursor += source[cursor..]
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(source.len() - cursor);
        if cursor >= source.len() {
            break;
        }

        let Some(open) = find_next_top_level(source, cursor, '{') else {
            break;
        };
        let prelude = source[cursor..open].trim();
        let Some(close) = find_matching_brace(source, open) else {
            break;
        };
        let body = &source[open + 1..close];

        if prelude.starts_with("@media") {
            if media_query_matches(prelude.trim_start_matches("@media").trim(), viewport_width) {
                parse_rule_list(body, viewport_width, order, rules);
            }
        } else if prelude.starts_with("@supports") || prelude.starts_with("@layer") {
            parse_rule_list(body, viewport_width, order, rules);
        } else if prelude.starts_with('@') {
            // @font-face is handled by the font subsystem. Keyframes and other
            // at-rules do not directly produce element style rules yet.
        } else {
            let selectors = split_top_level(prelude, ',')
                .into_iter()
                .filter_map(|selector| parse_selector(selector.trim()))
                .collect::<Vec<_>>();
            let declarations = parse_declarations(body);

            if !selectors.is_empty() && !declarations.is_empty() {
                rules.push(Rule {
                    selectors,
                    declarations,
                    order: *order,
                });
                *order = order.saturating_add(1);
            }
        }

        cursor = close + 1;
    }
}

pub fn parse_declarations(body: &str) -> Vec<Declaration> {
    split_top_level(body, ';')
        .into_iter()
        .filter_map(|entry| {
            let (name, value) = split_once_top_level(entry, ':')?;
            let raw_name = name.trim();
            let name = if raw_name.starts_with("--") {
                raw_name.to_string()
            } else {
                raw_name.to_ascii_lowercase()
            };
            let raw_value = value.trim();
            let lower = raw_value.to_ascii_lowercase();
            let important = lower.ends_with("!important");
            let value = if important {
                raw_value[..raw_value.len().saturating_sub("!important".len())]
                    .trim()
                    .to_string()
            } else {
                raw_value.to_string()
            };
            if name.is_empty() || value.is_empty() {
                None
            } else {
                Some(Declaration {
                    name,
                    value,
                    important,
                })
            }
        })
        .collect()
}

fn find_next_top_level(source: &str, start: usize, needle: char) -> Option<usize> {
    let mut quote = None;
    let mut escape = false;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;

    for (offset, character) in source[start..].char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if character == '\\' {
            escape = true;
            continue;
        }
        if let Some(expected) = quote {
            if character == expected {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if character == needle && paren_depth == 0 && bracket_depth == 0 => {
                return Some(start + offset)
            }
            _ => {}
        }
    }
    None
}

fn find_matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0u32;
    let mut quote = None;
    let mut escape = false;

    for (offset, character) in source[open..].char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if character == '\\' {
            escape = true;
            continue;
        }
        if let Some(expected) = quote {
            if character == expected {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'') {
            quote = Some(character);
            continue;
        }
        match character {
            '{' => depth = depth.saturating_add(1),
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(source: &str, separator: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut escape = false;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;

    for (index, character) in source.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if character == '\\' {
            escape = true;
            continue;
        }
        if let Some(expected) = quote {
            if character == expected {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if character == separator && paren_depth == 0 && bracket_depth == 0 => {
                out.push(&source[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&source[start..]);
    out
}

fn split_once_top_level(source: &str, separator: char) -> Option<(&str, &str)> {
    let mut quote = None;
    let mut escape = false;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;

    for (index, character) in source.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if character == '\\' {
            escape = true;
            continue;
        }
        if let Some(expected) = quote {
            if character == expected {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if character == separator && paren_depth == 0 && bracket_depth == 0 => {
                let next = index + character.len_utf8();
                return Some((&source[..index], &source[next..]));
            }
            _ => {}
        }
    }
    None
}

fn media_query_matches(query: &str, viewport_width: u32) -> bool {
    split_top_level(query, ',').into_iter().any(|branch| {
        let branch = branch.trim().to_ascii_lowercase();
        if branch.is_empty() {
            return false;
        }
        let negated = branch.starts_with("not ");
        let mut matches = true;

        for condition in branch.split("and").map(str::trim) {
            if matches!(condition, "" | "all" | "screen" | "only screen") {
                continue;
            }
            let condition = condition
                .trim_start_matches("only ")
                .trim()
                .trim_matches(|character| character == '(' || character == ')')
                .trim();
            if let Some(value) = condition.strip_prefix("min-width:") {
                matches &= parse_media_px(value).is_some_and(|min| viewport_width >= min);
            } else if let Some(value) = condition.strip_prefix("max-width:") {
                matches &= parse_media_px(value).is_some_and(|max| viewport_width <= max);
            } else if condition == "screen" || condition == "all" {
                matches &= true;
            } else {
                matches = false;
            }
        }

        if negated { !matches } else { matches }
    })
}

fn parse_media_px(value: &str) -> Option<u32> {
    value.trim().strip_suffix("px")?.trim().parse::<u32>().ok()
}

pub fn selector_matches(document: &Document, node_id: usize, selector: &Selector) -> bool {
    let Some(rightmost) = selector.parts.last() else {
        return false;
    };
    if !simple_selector_matches(document, node_id, rightmost) {
        return false;
    }

    let mut current = node_id;
    for index in (1..selector.parts.len()).rev() {
        let left = &selector.parts[index - 1];
        let combinator = selector
            .combinators
            .get(index - 1)
            .copied()
            .unwrap_or(Combinator::Descendant);

        match combinator {
            Combinator::Child => {
                let Some(parent) = document.nodes.get(current).and_then(|node| node.parent) else {
                    return false;
                };
                if !simple_selector_matches(document, parent, left) {
                    return false;
                }
                current = parent;
            }
            Combinator::Descendant => {
                let mut ancestor = document.nodes.get(current).and_then(|node| node.parent);
                let mut matched = None;
                while let Some(id) = ancestor {
                    if simple_selector_matches(document, id, left) {
                        matched = Some(id);
                        break;
                    }
                    ancestor = document.nodes.get(id).and_then(|node| node.parent);
                }
                let Some(id) = matched else {
                    return false;
                };
                current = id;
            }
            Combinator::AdjacentSibling => {
                let Some(sibling) = previous_element_siblings(document, current).last().copied()
                else {
                    return false;
                };
                if !simple_selector_matches(document, sibling, left) {
                    return false;
                }
                current = sibling;
            }
            Combinator::GeneralSibling => {
                let Some(sibling) = previous_element_siblings(document, current)
                    .into_iter()
                    .rev()
                    .find(|id| simple_selector_matches(document, *id, left))
                else {
                    return false;
                };
                current = sibling;
            }
        }
    }
    true
}

fn parse_selector(value: &str) -> Option<Selector> {
    let (tokens, combinators) = tokenize_selector(value)?;
    let parts = tokens
        .iter()
        .map(|token| parse_simple_selector(token))
        .collect::<Option<Vec<_>>>()?;
    if parts.is_empty() || combinators.len() + 1 != parts.len() {
        return None;
    }

    let mut ids = 0u16;
    let mut classes = 0u16;
    let mut tags = 0u16;
    for part in &parts {
        ids = ids.saturating_add(u16::from(part.id.is_some()));
        classes = classes
            .saturating_add(part.classes.len() as u16)
            .saturating_add(part.attributes.len() as u16)
            .saturating_add(part.pseudos.len() as u16);
        tags = tags.saturating_add(u16::from(part.tag.is_some()));
    }

    Some(Selector {
        parts,
        combinators,
        specificity: (ids, classes, tags),
    })
}

fn tokenize_selector(value: &str) -> Option<(Vec<String>, Vec<Combinator>)> {
    let mut tokens = Vec::new();
    let mut combinators = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0u32;
    let mut paren_depth = 0u32;
    let mut pending_space = false;

    for character in value.trim().chars() {
        match character {
            '[' => {
                bracket_depth = bracket_depth.saturating_add(1);
                current.push(character);
            }
            ']' => {
                if bracket_depth == 0 {
                    return None;
                }
                bracket_depth -= 1;
                current.push(character);
            }
            '(' => {
                paren_depth = paren_depth.saturating_add(1);
                current.push(character);
            }
            ')' => {
                if paren_depth == 0 {
                    return None;
                }
                paren_depth -= 1;
                current.push(character);
            }
            '>' | '+' | '~' if bracket_depth == 0 && paren_depth == 0 => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                    current.clear();
                }
                if tokens.is_empty() {
                    return None;
                }
                let combinator = match character {
                    '>' => Combinator::Child,
                    '+' => Combinator::AdjacentSibling,
                    '~' => Combinator::GeneralSibling,
                    _ => unreachable!(),
                };
                if combinators.len() < tokens.len() {
                    combinators.push(combinator);
                } else if let Some(last) = combinators.last_mut() {
                    *last = combinator;
                }
                pending_space = false;
            }
            character
                if character.is_ascii_whitespace()
                    && bracket_depth == 0
                    && paren_depth == 0 =>
            {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                    current.clear();
                    pending_space = true;
                }
            }
            _ => {
                if pending_space && !tokens.is_empty() && combinators.len() < tokens.len() {
                    combinators.push(Combinator::Descendant);
                }
                pending_space = false;
                current.push(character);
            }
        }
    }

    if bracket_depth != 0 || paren_depth != 0 {
        return None;
    }
    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }

    while combinators.len() >= tokens.len() && !combinators.is_empty() {
        combinators.pop();
    }

    Some((tokens, combinators))
}

fn parse_simple_selector(value: &str) -> Option<SimpleSelector> {
    let value = value.trim();
    if value.is_empty() || value == "*" {
        return Some(SimpleSelector {
            tag: None,
            id: None,
            classes: Vec::new(),
            attributes: Vec::new(),
            pseudos: Vec::new(),
        });
    }

    let mut tag = None;
    let mut id = None;
    let mut classes = Vec::new();
    let mut attributes = Vec::new();
    let mut pseudos = Vec::new();
    let bytes = value.as_bytes();
    let mut cursor = 0usize;

    if !matches!(bytes.first(), Some(b'.' | b'#' | b'[' | b':')) {
        let start = cursor;
        while cursor < bytes.len() && !matches!(bytes[cursor], b'.' | b'#' | b'[' | b':') {
            cursor += 1;
        }
        let candidate = value[start..cursor].trim();
        if !candidate.is_empty() && candidate != "*" {
            tag = Some(candidate.to_ascii_lowercase());
        }
    }

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'#' | b'.' => {
                let marker = bytes[cursor];
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && !matches!(bytes[cursor], b'.' | b'#' | b'[' | b':') {
                    cursor += 1;
                }
                let token = value[start..cursor].trim();
                if token.is_empty() {
                    return None;
                }
                if marker == b'#' {
                    id = Some(token.to_string());
                } else {
                    classes.push(token.to_string());
                }
            }
            b'[' => {
                let start = cursor + 1;
                let close = value[start..].find(']')? + start;
                let body = value[start..close].trim();
                if body.is_empty() {
                    return None;
                }
                let (name, operator, expected) = parse_attribute_selector(body)?;
                if name.is_empty() {
                    return None;
                }
                attributes.push(AttributeSelector {
                    name,
                    operator,
                    value: expected,
                });
                cursor = close + 1;
            }
            b':' => {
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && !matches!(bytes[cursor], b'.' | b'#' | b'[' | b':') {
                    cursor += 1;
                }
                let raw = value[start..cursor].trim().to_ascii_lowercase();
                let pseudo = parse_pseudo_class(&raw)?;
                pseudos.push(pseudo);
            }
            _ => return None,
        }
    }

    Some(SimpleSelector {
        tag,
        id,
        classes,
        attributes,
        pseudos,
    })
}

fn simple_selector_matches(document: &Document, node_id: usize, selector: &SimpleSelector) -> bool {
    let Some(node) = document.nodes.get(node_id) else {
        return false;
    };
    let NodeKind::Element { tag, .. } = &node.kind else {
        return false;
    };

    if let Some(expected) = &selector.tag {
        if tag != expected {
            return false;
        }
    }
    if let Some(expected) = &selector.id {
        if document.attribute(node_id, "id") != Some(expected.as_str()) {
            return false;
        }
    }
    if !selector
        .classes
        .iter()
        .all(|class| document.has_class(node_id, class))
    {
        return false;
    }
    if !selector
        .attributes
        .iter()
        .all(|attribute| attribute_matches(document, node_id, attribute))
    {
        return false;
    }
    selector
        .pseudos
        .iter()
        .all(|pseudo| pseudo_matches(document, node_id, *pseudo))
}

fn parse_attribute_selector(
    body: &str,
) -> Option<(String, AttributeOperator, Option<String>)> {
    for (needle, operator) in [
        ("~=", AttributeOperator::Includes),
        ("|=", AttributeOperator::DashMatch),
        ("^=", AttributeOperator::Prefix),
        ("$=", AttributeOperator::Suffix),
        ("*=", AttributeOperator::Substring),
        ("=", AttributeOperator::Equals),
    ] {
        if let Some((name, value)) = body.split_once(needle) {
            let name = name.trim().to_ascii_lowercase();
            let value = value
                .trim()
                .trim_matches(|character| character == '"' || character == '\'')
                .to_string();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            return Some((name, operator, Some(value)));
        }
    }

    let name = body.trim().to_ascii_lowercase();
    (!name.is_empty()).then_some((name, AttributeOperator::Exists, None))
}

fn parse_pseudo_class(raw: &str) -> Option<PseudoClass> {
    match raw {
        "first-child" => Some(PseudoClass::FirstChild),
        "last-child" => Some(PseudoClass::LastChild),
        "first-of-type" => Some(PseudoClass::FirstOfType),
        "last-of-type" => Some(PseudoClass::LastOfType),
        "nth-child(odd)" => Some(PseudoClass::NthChildOdd),
        "nth-child(even)" => Some(PseudoClass::NthChildEven),
        "checked" => Some(PseudoClass::Checked),
        "disabled" => Some(PseudoClass::Disabled),
        "root" => Some(PseudoClass::Root),
        "empty" => Some(PseudoClass::Empty),
        _ if raw.starts_with("nth-child(") && raw.ends_with(')') => {
            raw["nth-child(".len()..raw.len() - 1]
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|index| *index > 0)
                .map(PseudoClass::NthChild)
        }
        _ => None,
    }
}

fn attribute_matches(
    document: &Document,
    node_id: usize,
    selector: &AttributeSelector,
) -> bool {
    let actual = document.attribute(node_id, &selector.name);
    if selector.operator == AttributeOperator::Exists {
        return actual.is_some();
    }

    let (Some(actual), Some(expected)) = (actual, selector.value.as_deref()) else {
        return false;
    };
    match selector.operator {
        AttributeOperator::Exists => true,
        AttributeOperator::Equals => actual == expected,
        AttributeOperator::Includes => actual
            .split_ascii_whitespace()
            .any(|token| token == expected),
        AttributeOperator::DashMatch => {
            actual == expected || actual.starts_with(&format!("{expected}-"))
        }
        AttributeOperator::Prefix => actual.starts_with(expected),
        AttributeOperator::Suffix => actual.ends_with(expected),
        AttributeOperator::Substring => actual.contains(expected),
    }
}

fn element_children(document: &Document, parent: usize) -> Vec<usize> {
    document.nodes[parent]
        .children
        .iter()
        .copied()
        .filter(|child| matches!(document.nodes[*child].kind, NodeKind::Element { .. }))
        .collect()
}

fn previous_element_siblings(document: &Document, node_id: usize) -> Vec<usize> {
    let Some(parent) = document.nodes.get(node_id).and_then(|node| node.parent) else {
        return Vec::new();
    };
    element_children(document, parent)
        .into_iter()
        .take_while(|id| *id != node_id)
        .collect()
}

fn pseudo_matches(document: &Document, node_id: usize, pseudo: PseudoClass) -> bool {
    match pseudo {
        PseudoClass::Checked => document.attribute(node_id, "checked").is_some(),
        PseudoClass::Disabled => document.attribute(node_id, "disabled").is_some(),
        PseudoClass::Root => document.element_tag(node_id) == Some("html"),
        PseudoClass::Empty => document.nodes[node_id]
            .children
            .iter()
            .all(|child| match &document.nodes[*child].kind {
                NodeKind::Element { .. } => false,
                NodeKind::Text(text) => text.trim().is_empty(),
                NodeKind::Document => true,
            }),
        PseudoClass::FirstChild
        | PseudoClass::LastChild
        | PseudoClass::NthChild(_)
        | PseudoClass::NthChildOdd
        | PseudoClass::NthChildEven => {
            let Some(parent) = document.nodes[node_id].parent else {
                return false;
            };
            let children = element_children(document, parent);
            let Some(index) = children.iter().position(|id| *id == node_id) else {
                return false;
            };
            let position = index + 1;
            match pseudo {
                PseudoClass::FirstChild => position == 1,
                PseudoClass::LastChild => position == children.len(),
                PseudoClass::NthChild(expected) => position == expected as usize,
                PseudoClass::NthChildOdd => position % 2 == 1,
                PseudoClass::NthChildEven => position % 2 == 0,
                _ => false,
            }
        }
        PseudoClass::FirstOfType | PseudoClass::LastOfType => {
            let Some(parent) = document.nodes[node_id].parent else {
                return false;
            };
            let Some(tag) = document.element_tag(node_id) else {
                return false;
            };
            let same_type = element_children(document, parent)
                .into_iter()
                .filter(|id| document.element_tag(*id) == Some(tag))
                .collect::<Vec<_>>();
            match pseudo {
                PseudoClass::FirstOfType => same_type.first().copied() == Some(node_id),
                PseudoClass::LastOfType => same_type.last().copied() == Some(node_id),
                _ => false,
            }
        }
    }
}

fn strip_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    while let Some(start_offset) = source[cursor..].find("/*") {
        let start = cursor + start_offset;
        output.push_str(&source[cursor..start]);
        let Some(end_offset) = source[start + 2..].find("*/") else {
            return output;
        };
        cursor = start + 2 + end_offset + 2;
    }
    output.push_str(&source[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    #[test]
    fn media_queries_and_nested_blocks_follow_viewport() {
        let source = r#"
            .base { color: black; }
            @media screen and (min-width: 700px) {
                .wide { color: red; }
                @supports (display: grid) { .nested { display: grid; } }
            }
            @media (max-width: 500px) { .narrow { color: blue; } }
        "#;
        let wide = parse_stylesheet_for_viewport(source, 800);
        let narrow = parse_stylesheet_for_viewport(source, 400);

        assert_eq!(wide.rules.len(), 3);
        assert_eq!(narrow.rules.len(), 2);
    }

    #[test]
    fn declarations_keep_semicolons_and_colons_inside_functions_and_strings() {
        let declarations = parse_declarations(
            r#"background:url("data:image/svg+xml;a:b;c:d");content:"a;b:c";color:red!important"#,
        );
        assert_eq!(declarations.len(), 3);
        assert!(declarations[0].value.contains("data:image/svg+xml;a:b;c:d"));
        assert_eq!(declarations[1].value, "\"a;b:c\"");
        assert_eq!(declarations[2].value, "red");
    }

    #[test]
    fn important_is_preserved_by_declaration_parser() {
        let declarations = parse_declarations("color:red !IMPORTANT; padding:4px");
        assert!(declarations[0].important);
        assert_eq!(declarations[0].value, "red");
        assert!(!declarations[1].important);
    }

    #[test]
    fn custom_property_names_remain_case_sensitive() {
        let declarations = parse_declarations("--Accent:#fff;COLOR:red");
        assert_eq!(declarations[0].name, "--Accent");
        assert_eq!(declarations[1].name, "color");
    }

    #[test]
    fn parses_specificity_and_descendant_selector() {
        let sheet = parse_stylesheet("main .card#hero { color: #fff; margin-top: 8px; }");
        let selector = &sheet.rules[0].selectors[0];
        assert_eq!(selector.specificity, (1, 1, 1));
        assert_eq!(sheet.rules[0].declarations.len(), 2);

        let document = html::parse("<main><div class='card' id='hero'>x</div></main>");
        let id = document
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "div"))
            .unwrap();
        assert!(selector_matches(&document, id, selector));
    }

    #[test]
    fn matches_child_attribute_and_structural_pseudo_classes() {
        let sheet =
            parse_stylesheet("form > input[type=checkbox]:checked:first-child { color: red; }");
        assert_eq!(sheet.rules.len(), 1);
        let selector = &sheet.rules[0].selectors[0];
        assert_eq!(selector.specificity, (0, 3, 2));

        let document = html::parse(
            "<form><input id='one' type='checkbox' checked><input id='two' type='checkbox'></form>",
        );
        let one = document.find_by_id("one").unwrap();
        let two = document.find_by_id("two").unwrap();
        assert!(selector_matches(&document, one, selector));
        assert!(!selector_matches(&document, two, selector));
    }

    #[test]
    fn matches_sibling_combinators_and_attribute_operators() {
        let sheet = parse_stylesheet(
            "h2 + p[data-role^=lead] ~ p[class~=note][lang|=fr] { color: red; }",
        );
        let selector = &sheet.rules[0].selectors[0];
        let document = html::parse(
            "<section><h2>T</h2><p data-role='leader'>A</p><span>x</span><p id='target' class='note hot' lang='fr-CA'>B</p></section>",
        );
        let target = document.find_by_id("target").unwrap();
        assert!(selector_matches(&document, target, selector));
    }

    #[test]
    fn nth_child_and_type_pseudos_are_supported() {
        let sheet = parse_stylesheet(
            "section > p:nth-child(even):first-of-type { color: red; }",
        );
        let selector = &sheet.rules[0].selectors[0];
        let document = html::parse(
            "<section><span>A</span><p id='one'>1</p><p id='two'>2</p><p id='three'>3</p></section>",
        );
        let one = document.find_by_id("one").unwrap();
        let three = document.find_by_id("three").unwrap();
        assert!(selector_matches(&document, one, selector));
        assert!(!selector_matches(&document, three, selector));
    }

    #[test]
    fn root_and_empty_are_supported() {
        let sheet = parse_stylesheet("html:root > body > div:empty { background: red; }");
        let selector = &sheet.rules[0].selectors[0];
        let document = html::parse("<html><body><div id='empty'></div></body></html>");
        let empty = document.find_by_id("empty").unwrap();
        assert!(selector_matches(&document, empty, selector));
    }
}
