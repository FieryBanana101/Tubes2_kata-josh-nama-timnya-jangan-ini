use crate::css_selector::{AttributeFilter, CssSelectorParser, PseudoFilter, SelectorUnit};
use crate::html::{Element, Node};

/* Implemented a method to match CSS Selector Unit with a DOM Node */
impl SelectorUnit {
    pub fn match_node(&self, element: &Element, node_child_idx: usize, parent: &Element) -> bool {
        /* Match the tag */
        if let Some(ref selector_tag) = self.tag {
            if selector_tag != &element.tag && selector_tag != "*" {
                return false;
            }
        }

        /* Match the ID */
        if let Some(ref selector_ids) = self.ids {
            if let Some(element_id) = element.attributes.get("id") {
                for id in selector_ids {
                    if id != element_id {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }

        /* Match the classes */
        if let Some(ref selector_classes) = self.classes {
            if let Some(class_attr) = element.attributes.get("class") {
                let element_classes: Vec<&str> = class_attr.split_whitespace().collect();

                for class in selector_classes {
                    if !element_classes.contains(&class.as_str()) {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }

        /* Match other attributes */
        if let Some(ref filters) = self.attributes {
            for filter in filters {
                if !self.match_attribute_filter(element, filter) {
                    return false;
                }
            }
        }

        /* Match the pseudo-elements and pseudo-classes */
        if let Some(ref filters) = self.pseudos {
            for filter in filters {
                if !self.match_pseudo_filter(element, node_child_idx, parent, filter) {
                    return false;
                }
            }
        }

        true
    }

    fn match_attribute_filter(&self, element: &Element, filter: &AttributeFilter) -> bool {
        /* Get the attribute value from filter */
        let element_attr_value = match element.attributes.get(&filter.name) {
            Some(v) => v,
            None => return false,
        };

        /* Get the equality operator from filter */
        let op = match &filter.operator {
            Some(op_str) => op_str.as_str(),
            None => return true,
        };

        if let Some(filter_value) = &filter.value {
            /* Get the attribute value from DOM Node */
            let mut value = filter_value.clone();
            if value.starts_with('"') {
                value = value[1..value.len() - 1].to_string();
            }

            /* If we have 'i' (case-insensitivity) modifier then convert both attribute value to be lowercase */
            let (element_attr_value, value) = match filter.modifier {
                Some('i') => (element_attr_value.to_lowercase(), value.to_lowercase()),

                _ => (element_attr_value.to_string(), value),
            };

            match op {
                "=" => return element_attr_value == value,
                "~" => {
                    return element_attr_value
                        .to_string()
                        .split_whitespace()
                        .any(|word| word == value)
                }
                "|" => {
                    return element_attr_value == value
                        || element_attr_value.starts_with(&format!("{}-", value))
                }
                "^" => return element_attr_value.starts_with(&value),
                "$" => return element_attr_value.ends_with(&value),
                "*" => return element_attr_value.contains(&value),
                _ => return false,
            }
        };

        return false;
    }

    /*
        Supported pseudo-classes,

        :any-link
        :optional
        :required
        :read-write
        :read-only

        :empty

        :first-child
        :first-of-type

        :only-child
        :only-of-type

        :last-child
        :last-of-type

        No pseudo-element is supported due to a pseudo-element needing a state tracker in DOM node (out of scope of this project)
    */
    fn match_pseudo_filter(
        &self,
        element: &Element,
        node_child_idx: usize,
        parent: &Element,
        filter: &PseudoFilter,
    ) -> bool {
        match filter.name.to_ascii_lowercase().as_str() {
            "empty" => return element.children.is_empty(),
            "first-child" => return node_child_idx == 0,
            "last-child" => return node_child_idx == parent.children.len() - 1,

            "first-of-type" => {
                let first_idx = parent
                    .children
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, node)| match node {
                        Node::Element(e) => Some((idx, e)),
                        _ => None,
                    })
                    .find(|(_, e)| e.tag == element.tag)
                    .map(|(idx, _)| idx);

                return Some(node_child_idx) == first_idx;
            }

            "last-of-type" => {
                let last_idx = parent
                    .children
                    .iter()
                    .enumerate()
                    .rev()
                    .filter_map(|(idx, node)| match node {
                        Node::Element(e) => Some((idx, e)),
                        _ => None,
                    })
                    .find(|(_, e)| e.tag == element.tag)
                    .map(|(idx, _)| idx);

                return Some(node_child_idx) == last_idx;
            }

            "only-child" => {
                let element_count = parent
                    .children
                    .iter()
                    .filter(|node| {
                        if let Node::Element(_) = node {
                            true
                        } else {
                            false
                        }
                    })
                    .count();
                return element_count == 1;
            }

            "only-of-type" => {
                let count = parent
                    .children
                    .iter()
                    .filter(|node| {
                        if let Node::Element(e) = node {
                            e.tag == element.tag
                        } else {
                            false
                        }
                    })
                    .count();

                return count == 1;
            }

            "any-link" => match element.tag.as_str() {
                "a" | "area" => return element.attributes.contains_key("href"),
                _ => return false,
            },

            "required" => match element.tag.as_str() {
                "input" | "textarea" | "select" => {
                    return element.attributes.contains_key("required")
                }
                _ => return false,
            },

            "optional" => match element.tag.as_str() {
                "input" | "textarea" | "select" => {
                    return !element.attributes.contains_key("required")
                }
                _ => return false,
            },

            "read-write" => {
                if matches!(element.tag.as_str(), "input" | "textarea") {
                    return !element.attributes.contains_key("readonly")
                        && !element.attributes.contains_key("disabled");
                }

                match element
                    .attributes
                    .get("contenteditable")
                    .map(|value| value.as_str())
                {
                    Some("true") | Some("") => return true,
                    _ => return false,
                }
            }

            "read-only" => {
                if matches!(element.tag.as_str(), "input" | "textarea") {
                    return element.attributes.contains_key("readonly")
                        || element.attributes.contains_key("disabled");
                }

                match element
                    .attributes
                    .get("contenteditable")
                    .map(|value| value.as_str())
                {
                    Some("true") | Some("") => return false,
                    _ => return true,
                }
            }

            _ => return true, /* By default we will consider the filter match if no implementation is present (flexible) */
        };
    }
}
