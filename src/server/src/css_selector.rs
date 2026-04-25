use actix_web::cookie::time::format_description::parse;
use css_lexer::{
    EmptyAtomSet, Kind as CssTokenType, Lexer as CssLexer, SourceOffset, Token as CssToken,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Combinator {
    Descendant,
    Child,
    DirectNextSibling,
    NextSibling,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Namespace {
    Default,
    Any,
    None,
    Named(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeFilter {
    pub namespace: Namespace,
    pub name: String,
    pub operator: Option<String>,
    pub value: Option<String>,
    pub modifier: Option<char>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PseudoFilter {
    pub name: String,
    pub args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorUnit {
    pub namespace: Namespace,
    pub tag: Option<String>,
    pub ids: Option<Vec<String>>,
    pub classes: Option<Vec<String>>,
    pub attributes: Option<Vec<AttributeFilter>>,
    pub pseudos: Option<Vec<PseudoFilter>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeFilter {
    pub prev_combinator: Option<Combinator>,
    pub selector: SelectorUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssSelectorStep {
    pub step_type: String,
    pub current_text: String,
    pub position: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssSelectorTraversal {
    pub steps: Vec<CssSelectorStep>,
}

#[derive(Clone)]
struct CssLexerWrapper<'a> {
    tokenizer: CssLexer<'a>,
    current_token: CssToken,
    current_text: &'a str,
    current_start_pos: usize,
}

#[derive(Clone)]
pub struct CssSelectorParser<'a> {
    css_lexer: CssLexerWrapper<'a>,
    is_relative_selector: bool,
    error_message: String,
    pub traversal: CssSelectorTraversal,
}

impl<'a> CssSelectorParser<'a> {
    #[inline(always)]
    pub fn new(input: &'a str, is_relative: bool) -> Self {
        CssSelectorParser {
            css_lexer: CssLexerWrapper {
                tokenizer: CssLexer::new(&EmptyAtomSet::ATOMS, input),
                current_token: CssToken::EOF,
                current_text: "",
                current_start_pos: 0,
            },
            is_relative_selector: is_relative,
            error_message: String::from(
                "CSS Parser error. Due to these sequence of error, syntax is considered invalid:",
            ),
            traversal: CssSelectorTraversal { steps: Vec::new() },
        }
    }

    pub fn parse_all(&mut self) -> Vec<NodeFilter> {
        let mut vec: Vec<NodeFilter> = Vec::new();
        loop {
            let (filter, is_eof) = self.advance().unwrap();
            vec.push(filter);
            if is_eof {
                break;
            }
        }
        vec
    }

    #[inline(always)]
    fn log_parser_error(&mut self, message: &str) -> () {
        self.error_message.push_str(&format!(
            "\n   ==> {} (caused by '{}' at position {})",
            message, self.css_lexer.current_text, self.css_lexer.current_start_pos
        ));
    }

    fn next_token(&mut self) -> Result<(), ()> {
        self.css_lexer.current_start_pos = self.css_lexer.tokenizer.offset().into();
        self.css_lexer.current_token = self.css_lexer.tokenizer.advance();
        self.css_lexer.current_text = self
            .css_lexer
            .current_token
            .with_cursor(SourceOffset(self.css_lexer.current_start_pos as u32))
            .str_slice(self.css_lexer.tokenizer.source());

        self.traversal.steps.push(CssSelectorStep {
            step_type: format!("{:?}", self.css_lexer.current_token.kind()),
            current_text: self.css_lexer.current_text.to_string(),
            position: self.css_lexer.current_start_pos,
        });

        if self.css_lexer.current_token.is_bad() {
            return Err(self.log_parser_error("Encountered bad token"));
        }

        Ok(())
    }

    #[inline]
    fn skip_whitespaces(&mut self) -> Result<(), ()> {
        while self.css_lexer.current_token.kind() == CssTokenType::Whitespace {
            self.next_token()?;
        }
        Ok(())
    }

    #[inline]
    fn skip_to_just_before_non_whitespace(&mut self) -> Result<(), ()> {
        let mut peeker = self.clone();
        peeker.next_token()?;

        while peeker.css_lexer.current_token.kind() == CssTokenType::Whitespace {
            self.next_token()?;
            peeker.next_token()?;
        }

        Ok(())
    }

    pub fn advance(&mut self) -> Result<(NodeFilter, bool), String> {
        let mut combinator = None;
        if self.css_lexer.current_token.kind() == CssTokenType::Eof {
            self.skip_to_just_before_non_whitespace()
                .map_err(|_| self.error_message.clone())?;

            if self.is_relative_selector {
                let saved = self.css_lexer.clone();
                let parse_result = self.parse_combinator();

                if let Ok(value) = parse_result {
                    combinator = Some(value);
                } else {
                    self.css_lexer = saved;
                }
            }
        } else {
            combinator = Some(
                self.parse_combinator()
                    .map_err(|_| self.error_message.clone())?,
            );
        }

        let mut valid_selector_unit = false;

        let mut filter = SelectorUnit {
            namespace: Namespace::Default,
            tag: None,
            ids: None,
            classes: None,
            attributes: None,
            pseudos: None,
        };

        let saved = self.css_lexer.clone();
        let parse_result = self.parse_compound_selector();
        if let Ok(compound) = parse_result {
            filter.namespace = compound.namespace;
            filter.tag = compound.tag;

            if let Some(mut ids) = compound.ids {
                filter.ids.get_or_insert_with(Vec::new).append(&mut ids);
            }

            if let Some(mut classes) = compound.classes {
                filter
                    .classes
                    .get_or_insert_with(Vec::new)
                    .append(&mut classes);
            }

            if let Some(mut attributes) = compound.attributes {
                filter
                    .attributes
                    .get_or_insert_with(Vec::new)
                    .append(&mut attributes);
            }

            if let Some(mut pseudos) = compound.pseudos {
                filter
                    .pseudos
                    .get_or_insert_with(Vec::new)
                    .append(&mut pseudos);
            }

            valid_selector_unit = true;
        } else {
            self.css_lexer = saved;
        }

        loop {
            let saved = self.css_lexer.clone();
            let parse_result = self.parse_pseudo_compound_selector();

            if let Ok(pseudo_compound) = parse_result {
                if let Some(mut pseudos) = pseudo_compound.pseudos {
                    filter
                        .pseudos
                        .get_or_insert_with(Vec::new)
                        .append(&mut pseudos);
                }

                valid_selector_unit = true;
            } else {
                self.css_lexer = saved;
                break;
            }
        }

        if !valid_selector_unit {
            self.log_parser_error(
                "Expected at least one of <compound-selector> or <pseudo-compound-selector> while parsing selector unit, but found none"
            );

            return Err(self.error_message.clone());
        }

        self.skip_to_just_before_non_whitespace()
            .map_err(|_| self.error_message.clone())?;

        let mut peeker = self.clone();
        peeker
            .next_token()
            .map_err(|_| self.error_message.clone())?;

        Ok((
            NodeFilter {
                prev_combinator: combinator,
                selector: filter,
            },
            peeker.css_lexer.current_token.kind() == CssTokenType::Eof,
        ))
    }

    fn parse_combinator(&mut self) -> Result<Combinator, ()> {
        let mut combinator: Combinator = Combinator::Descendant;

        let saved = self.css_lexer.clone();
        self.next_token()?;
        if self.css_lexer.current_token.kind() == CssTokenType::Delim {
            match self.css_lexer.current_text {
                ">" => {
                    combinator = Combinator::Child;
                    self.skip_to_just_before_non_whitespace()?;
                }
                "+" => {
                    combinator = Combinator::DirectNextSibling;
                    self.skip_to_just_before_non_whitespace()?;
                }
                "~" => {
                    combinator = Combinator::NextSibling;
                    self.skip_to_just_before_non_whitespace()?;
                }
                _ => self.css_lexer = saved,
            }
        } else {
            self.css_lexer = saved;
        }

        Ok(combinator)
    }

    fn parse_compound_selector(&mut self) -> Result<SelectorUnit, ()> {
        let mut filter = SelectorUnit {
            namespace: Namespace::Default,
            tag: None,
            ids: None,
            classes: None,
            attributes: None,
            pseudos: None,
        };

        let mut valid_compound_selector = false;

        let saved = self.css_lexer.clone();
        let parse_result = self.parse_type_selector();

        if let Ok((ns, tag)) = parse_result {
            filter.namespace = ns;
            filter.tag = Some(tag);
            valid_compound_selector = true;
        } else {
            self.css_lexer = saved;
        }

        loop {
            let saved = self.css_lexer.clone();

            match self.parse_subclass_selector() {
                Ok(result) => {
                    valid_compound_selector = true;

                    if let Some(mut ids) = result.ids {
                        filter.ids.get_or_insert_with(Vec::new).append(&mut ids);
                    }
                    if let Some(mut classes) = result.classes {
                        filter
                            .classes
                            .get_or_insert_with(Vec::new)
                            .append(&mut classes);
                    }
                    if let Some(mut attributes) = result.attributes {
                        filter
                            .attributes
                            .get_or_insert_with(Vec::new)
                            .append(&mut attributes);
                    }
                    if let Some(mut pseudos) = result.pseudos {
                        filter
                            .pseudos
                            .get_or_insert_with(Vec::new)
                            .append(&mut pseudos);
                    }
                }

                Err(_) => {
                    self.css_lexer = saved;
                    break;
                }
            };
        }

        if !valid_compound_selector {
            return Err(self.log_parser_error(
                "Expected at least one of <type-selector> or <subclass-selector> while parsing compound selector, but found none"
            ));
        }

        Ok(filter)
    }

    fn parse_type_selector(&mut self) -> Result<(Namespace, String), ()> {
        self.next_token()?;

        if self.css_lexer.current_token.kind() == CssTokenType::Ident {
            return Ok((Namespace::Default, self.css_lexer.current_text.to_string()));
        }

        if self.css_lexer.current_token.kind() == CssTokenType::Delim
            && self.css_lexer.current_text == "*"
        {
            return Ok((Namespace::Default, "*".to_string()));
        }

        Err(())
    }

    fn parse_subclass_selector(&mut self) -> Result<SelectorUnit, ()> {
        let saved = self.css_lexer.clone();
        let parse_result = self.parse_attribute_selector();
        if let Ok(attribute_filter) = parse_result {
            return Ok(SelectorUnit {
                namespace: Namespace::None,
                tag: None,
                classes: None,
                pseudos: None,
                ids: None,
                attributes: Some(vec![attribute_filter]),
            });
        } else {
            self.css_lexer = saved;
        }

        let saved = self.css_lexer.clone();
        let parse_result = self.parse_pseudo_class_selector();
        if let Ok(pseudo_filter) = parse_result {
            return Ok(SelectorUnit {
                namespace: Namespace::None,
                tag: None,
                classes: None,
                attributes: None,
                ids: None,
                pseudos: Some(vec![pseudo_filter]),
            });
        } else {
            self.css_lexer = saved;
        }

        self.next_token()?;
        match self.css_lexer.current_token.kind() {

            CssTokenType::Hash => {
                return Ok(SelectorUnit {
                        namespace: Namespace::None, tag: None, classes: None, attributes: None, pseudos: None,
                        ids: Some(vec![self.css_lexer.current_text[1..].to_string()])
                    });
            },

            CssTokenType::Delim if self.css_lexer.current_text == "." => {

                self.next_token()?;
                if self.css_lexer.current_token.kind() == CssTokenType::Ident {
                    return Ok(SelectorUnit { 
                            namespace: Namespace::None, tag: None, ids: None, attributes: None, pseudos: None,
                            classes: Some(vec![self.css_lexer.current_text.to_string()])
                        });
                }
                else {
                    return Err(self.log_parser_error("Expected identifier while parsing class selector"));
                }

            },

            _ => {
                return Err(self.log_parser_error(
            "Expected an <id-selector>, <class-selector>, <attribute-selector>, or <pseudo-class-selector> while parsing subclass selector"
                ))
            }

        }
    }

    fn parse_attribute_selector(&mut self) -> Result<AttributeFilter, ()> {
        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::LeftSquare {
            return Err(());
        }

        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::Ident {
            return Err(());
        }

        let attr_name = self.css_lexer.current_text.trim().to_string();
        self.next_token()?;

        let mut attr = AttributeFilter {
            namespace: Namespace::Default,
            name: attr_name,
            operator: None,
            value: None,
            modifier: None,
        };

        if self.css_lexer.current_token.kind() == CssTokenType::Delim
            && self.css_lexer.current_token.char() == Some('=')
        {
            attr.operator = Some("=".to_string());
            self.next_token()?;
            if self.css_lexer.current_token.kind() == CssTokenType::String {
                attr.value = Some(self.css_lexer.current_text.trim().to_string());
            }
        }

        if self.css_lexer.current_token.kind() == CssTokenType::Delim
            && self.css_lexer.current_token.char() == Some(']')
        {
            self.next_token()?;
        }

        Ok(attr)
    }

    fn parse_pseudo_class_selector(&mut self) -> Result<PseudoFilter, ()> {
        self.next_token()?;

        if self.css_lexer.current_token.kind() != CssTokenType::Delim
            || self.css_lexer.current_token.char() != Some(':')
        {
            return Err(());
        }

        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::Ident {
            return Err(());
        }

        let pseudo_name = self.css_lexer.current_text.trim().to_string();
        let mut pseudo = PseudoFilter {
            name: pseudo_name,
            args: None,
        };

        self.next_token()?;

        if self.css_lexer.current_token.kind() == CssTokenType::Delim
            && self.css_lexer.current_token.char() == Some('(')
        {
            self.next_token()?;
            if self.css_lexer.current_token.kind() == CssTokenType::Ident
                || self.css_lexer.current_token.kind() == CssTokenType::Number
            {
                pseudo.args = Some(self.css_lexer.current_text.trim().to_string());
                self.next_token()?;
            }
            if self.css_lexer.current_token.kind() == CssTokenType::Delim
                && self.css_lexer.current_token.char() == Some(')')
            {
                self.next_token()?;
            }
        }

        Ok(pseudo)
    }

    fn parse_pseudo_compound_selector(&mut self) -> Result<SelectorUnit, ()> {
        let saved = self.css_lexer.clone();
        let parse_result = self.parse_pseudo_element_selector();

        if parse_result.is_err() {
            self.css_lexer = saved;
            return Err(());
        }

        let pseudo_element = parse_result.unwrap();
        let mut filter = SelectorUnit {
            namespace: Namespace::Default,
            tag: None,
            ids: None,
            classes: None,
            attributes: None,
            pseudos: Some(vec![pseudo_element]),
        };

        loop {
            let saved = self.css_lexer.clone();
            let parse_result = self.parse_pseudo_class_selector();

            match parse_result {
                Ok(pseudo_class) => {
                    filter.pseudos.as_mut().unwrap().push(pseudo_class);
                }
                Err(_) => {
                    self.css_lexer = saved;
                    break;
                }
            }
        }

        Ok(filter)
    }

    fn parse_pseudo_element_selector(&mut self) -> Result<PseudoFilter, ()> {
        self.next_token()?;

        if self.css_lexer.current_token.kind() != CssTokenType::Delim
            || self.css_lexer.current_token.char() != Some(':')
        {
            return Err(());
        }

        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::Ident {
            return Err(());
        }

        let pseudo_name = self.css_lexer.current_text.trim().to_string();
        self.next_token()?;

        Ok(PseudoFilter {
            name: pseudo_name,
            args: None,
        })
    }
}
