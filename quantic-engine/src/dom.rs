#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Document,
    Element {
        tag: String,
        attributes: Vec<Attribute>,
    },
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub nodes: Vec<Node>,
    pub root: usize,
}

impl Document {
    pub fn new() -> Self {
        Self {
            nodes: vec![Node {
                kind: NodeKind::Document,
                parent: None,
                children: Vec::new(),
            }],
            root: 0,
        }
    }

    pub fn append(&mut self, parent: usize, kind: NodeKind) -> usize {
        let id = self.create_detached(kind);
        self.attach(parent, id);
        id
    }

    pub fn create_detached(&mut self, kind: NodeKind) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Node {
            kind,
            parent: None,
            children: Vec::new(),
        });
        id
    }

    pub fn create_detached_expected(
        &mut self,
        expected_id: usize,
        kind: NodeKind,
    ) -> Result<usize, &'static str> {
        if expected_id != self.nodes.len() {
            return Err("dynamic DOM node id is out of sequence");
        }
        Ok(self.create_detached(kind))
    }

    pub fn attach(&mut self, parent: usize, child: usize) -> bool {
        if parent >= self.nodes.len() || child >= self.nodes.len() || parent == child {
            return false;
        }
        self.detach(child);
        self.nodes[child].parent = Some(parent);
        if !self.nodes[parent].children.contains(&child) {
            self.nodes[parent].children.push(child);
        }
        true
    }

    pub fn insert_before(
        &mut self,
        parent: usize,
        child: usize,
        before: Option<usize>,
    ) -> bool {
        if parent >= self.nodes.len() || child >= self.nodes.len() || parent == child {
            return false;
        }
        if let Some(before) = before {
            if before >= self.nodes.len() || self.nodes[before].parent != Some(parent) {
                return false;
            }
        }

        self.detach(child);
        self.nodes[child].parent = Some(parent);
        let children = &mut self.nodes[parent].children;
        if let Some(before) = before {
            let Some(index) = children.iter().position(|id| *id == before) else {
                return false;
            };
            children.insert(index, child);
        } else {
            children.push(child);
        }
        true
    }

    pub fn detach(&mut self, child: usize) -> bool {
        let Some(node) = self.nodes.get(child) else {
            return false;
        };
        let parent = node.parent;
        if let Some(parent) = parent {
            if let Some(parent_node) = self.nodes.get_mut(parent) {
                parent_node.children.retain(|id| *id != child);
            }
        }
        if let Some(node) = self.nodes.get_mut(child) {
            node.parent = None;
        }
        true
    }

    pub fn node(&self, id: usize) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn element_tag(&self, id: usize) -> Option<&str> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Element { tag, .. } => Some(tag),
            _ => None,
        }
    }

    pub fn attributes(&self, id: usize) -> Option<&[Attribute]> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Element { attributes, .. } => Some(attributes),
            _ => None,
        }
    }

    pub fn attribute(&self, id: usize, name: &str) -> Option<&str> {
        self.attributes(id)?
            .iter()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
            .map(|attribute| attribute.value.as_str())
    }

    pub fn set_attribute(&mut self, id: usize, name: &str, value: impl Into<String>) -> bool {
        let Some(Node {
            kind: NodeKind::Element { attributes, .. },
            ..
        }) = self.nodes.get_mut(id)
        else {
            return false;
        };
        if let Some(attribute) = attributes
            .iter_mut()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
        {
            attribute.value = value.into();
        } else {
            attributes.push(Attribute {
                name: name.to_ascii_lowercase(),
                value: value.into(),
            });
        }
        true
    }

    pub fn remove_attribute(&mut self, id: usize, name: &str) -> bool {
        let Some(Node {
            kind: NodeKind::Element { attributes, .. },
            ..
        }) = self.nodes.get_mut(id)
        else {
            return false;
        };
        let before = attributes.len();
        attributes.retain(|attribute| !attribute.name.eq_ignore_ascii_case(name));
        before != attributes.len()
    }

    pub fn set_style_property(&mut self, id: usize, name: &str, value: &str) -> bool {
        if self.element_tag(id).is_none() {
            return false;
        }
        let existing = self.attribute(id, "style").unwrap_or_default();
        let mut declarations = existing
            .split(';')
            .filter_map(|entry| entry.split_once(':'))
            .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
            .filter(|(key, _)| !key.is_empty())
            .collect::<Vec<_>>();

        let property = name.trim().to_ascii_lowercase();
        if let Some((_, current)) = declarations.iter_mut().find(|(key, _)| *key == property) {
            *current = value.to_string();
        } else {
            declarations.push((property, value.to_string()));
        }

        let inline = declarations
            .into_iter()
            .map(|(key, value)| format!("{key}:{value}"))
            .collect::<Vec<_>>()
            .join(";");
        self.set_attribute(id, "style", inline)
    }

    pub fn has_class(&self, id: usize, class: &str) -> bool {
        self.attribute(id, "class")
            .map(|classes| classes.split_ascii_whitespace().any(|item| item == class))
            .unwrap_or(false)
    }

    pub fn find_by_id(&self, value: &str) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .find_map(|(id, _)| (self.attribute(id, "id") == Some(value)).then_some(id))
    }

    pub fn ancestors(&self, id: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut current = self.nodes.get(id).and_then(|node| node.parent);
        while let Some(parent) = current {
            out.push(parent);
            current = self.nodes.get(parent).and_then(|node| node.parent);
        }
        out
    }

    pub fn descendants(&self, id: usize) -> Vec<usize> {
        let mut out = Vec::new();
        self.collect_descendants(id, &mut out);
        out
    }

    fn collect_descendants(&self, id: usize, out: &mut Vec<usize>) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        for child in &node.children {
            out.push(*child);
            self.collect_descendants(*child, out);
        }
    }

    pub fn set_text_content(&mut self, id: usize, value: impl Into<String>) -> bool {
        if id >= self.nodes.len() {
            return false;
        }
        let old_children = self.nodes[id].children.clone();
        for child in old_children {
            self.detach(child);
        }
        let value = value.into();
        if !value.is_empty() {
            self.append(id, NodeKind::Text(value));
        }
        true
    }

    pub fn set_text_node(&mut self, id: usize, value: impl Into<String>) -> bool {
        let Some(node) = self.nodes.get_mut(id) else {
            return false;
        };
        let NodeKind::Text(text) = &mut node.kind else {
            return false;
        };
        *text = value.into();
        true
    }

    pub fn text_content(&self, id: usize) -> String {
        let mut out = String::new();
        self.collect_text(id, &mut out);
        out
    }

    fn collect_text(&self, id: usize, out: &mut String) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        if let NodeKind::Text(text) = &node.kind {
            out.push_str(text);
        }
        for child in &node.children {
            self.collect_text(*child, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_before_preserves_dom_order() {
        let mut document = Document::new();
        let parent = document.append(
            document.root,
            NodeKind::Element {
                tag: "div".to_string(),
                attributes: Vec::new(),
            },
        );
        let one = document.append(parent, NodeKind::Text("one".to_string()));
        let three = document.append(parent, NodeKind::Text("three".to_string()));
        let two = document.create_detached(NodeKind::Text("two".to_string()));

        assert!(document.insert_before(parent, two, Some(three)));
        assert_eq!(document.nodes[parent].children, vec![one, two, three]);
        assert_eq!(document.text_content(parent), "onetwothree");
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
