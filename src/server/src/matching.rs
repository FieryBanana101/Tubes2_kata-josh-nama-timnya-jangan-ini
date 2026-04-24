use crate::tokenizer::{Element};
use crate::css_selector::{SelectorUnit, AttributeFilter};


/* Implemented a method to match CSS Selector Unit with a DOM Node */
impl SelectorUnit {
    pub fn match_node(&self, element: &Element) -> bool {

        /* Match the tag */
        if let Some(ref selector_tag) = self.tag {
            if selector_tag != &element.tag {
                return false;
            }
        }
        
        /* Match the ID */
        if let Some(ref selector_ids) = self.ids {
            if let Some(element_id) = element.attributes.get("id"){

                for id in selector_ids {
                    if id != element_id {
                        return false;
                    }
                }

            } 
            else {
                return false;
            }
        }

        /* Match the classes */
        if let Some(ref selector_classes) = self.classes {
            if let Some(class_attr) = element.attributes.get("class"){

                let element_classes: Vec<&str> = class_attr.split_whitespace().collect();
                
                for class in selector_classes {
                    if !element_classes.contains(&class.as_str()) {
                        return false;
                    }
                }

            } 
            else {
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
            if value.starts_with('"'){ value = value[1..value.len()-1].to_string(); }


            /* If we have 'i' (case-insensitivity) modifier then convert both attribute value to be lowercase */
            let (element_attr_value, value) = match filter.modifier {
                Some('i') => {
                    (element_attr_value.to_lowercase(), value.to_lowercase())
                },

                _ => {
                    (element_attr_value.to_string(), value)
                }
            };


            match op {
                "="  => return element_attr_value == value,
                "~" => return element_attr_value.to_string().split_whitespace().any(|word| word == value),
                "|" => return element_attr_value == value || element_attr_value.starts_with(&format!("{}-", value)),
                "^" => return element_attr_value.starts_with(&value),
                "$" => return element_attr_value.ends_with(&value),
                "*" => return element_attr_value.contains(&value),
                _   => return false,
            }
        };

        return false;

    }
}


/* TODO

:empty
:any-link
:optional
:required
:read-write
:read-only

:first-child
:first-of-type

:only-child
:only-of-type

:nth-child()
:nth-last-child()
:nth-of-type()
:nth-last-of-type()

:last-child
:last-of-type

:not()
:where()
:is()
:has()

*/