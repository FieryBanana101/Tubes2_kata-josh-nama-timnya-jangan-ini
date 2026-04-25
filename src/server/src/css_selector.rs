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
    name: String,
    args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeFilter {
    pub namespace: Namespace,
    pub tag: Option<String>,
    pub ids: Option<Vec<String>>,
    pub classes: Option<Vec<String>>,
    pub attributes: Option<Vec<AttributeFilter>>,
    pub pseudos: Option<Vec<PseudoFilter>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorUnit {
    pub prev_combinator: Option<Combinator>,
    pub filter: NodeFilter,
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
    /*
       Function to construct a new CssSelectorParser,
       By default css selector is absolute unless it is needed to be relative
    */
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

    /*
       Log parsing related-error message to be shown when irrecoverable error happens,
       if error is recoverable (when there is alternative valid syntax) then error message is kept hidden.
    */
    #[inline(always)]
    fn log_parser_error(&mut self, message: &str) -> () {
        self.error_message.push_str(&format!(
            "\n   ==> {} (caused by '{}' at position {})",
            message, self.css_lexer.current_text, self.css_lexer.current_start_pos
        ));
    }

    /*
       Function acquire the next token from the CSS tokenizer
    */
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

    /*
        Skip all whitespaces starting from the next token,
        if there is whitespace the parser will be located at the first non-whitespace token after the current token,
        if there is no whitespace nothing happens (parser state guaranteed to not change, this is achieved by "peeking" first).
    */
    #[inline]
    fn skip_whitespaces(&mut self) -> Result<(), ()> {
        while self.css_lexer.current_token.kind() == CssTokenType::Whitespace {
            self.next_token()?;
        }

        Ok(())
    }

    /*
        Sometimes we must consume all whitespace but stop at the last whitespace token
        and not consume the first non-whitespace character (because that is the assumption made by some parser function),
        this is a very dirty way but kinda ok I guess
    */
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

    /*
        The specification says,
            <complex-selector> = <complex-selector-unit> [ <combinator>? <complex-selector-unit> ]*
            <complex-selector-unit> = [ <compound-selector>? <pseudo-compound-selector>* ]!

        So we will go <complex-selector-unit> at a time, optionally with a combinator,
        user of struct CssSelectorParser can call .advance() within a loop until the method .is_eof() returns true.

        <complex-selector-unit> = [ <compound-selector>? <pseudo-compound-selector>* ]!
    */
    pub fn advance(&mut self) -> Result<(SelectorUnit, bool), String> {
        /*
            If we are at the beginning, skip leading whitespace and try to get a combinator, if failed then rewind
            This is done to account for relative selector list which comes from inside a pseudo-element.
        */
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
        }
        /*
           If we are not at the beginning parsing a combinator is mandatory, raise parser error when faield.
        */
        else {
            combinator = Some(
                self.parse_combinator()
                    .map_err(|_| self.error_message.clone())?,
            );
        }

        let mut valid_selector_unit = false;

        let mut filter = NodeFilter {
            namespace: Namespace::Default,
            tag: None,
            ids: None,
            classes: None,
            attributes: None,
            pseudos: None,
        };

        /* Consider parsing the compound selector first, if failed then rewind */
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

        /*
           Consider parsing the pseudo-compund selector, can be arbitary (possibly zero)
           but at leaset one compound selector or pseudo-compound selector must be parsed successfully,
           otherwise parsing error will be raised for this selector unit.
        */
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

        /* Validate current selector unit */
        if !valid_selector_unit {
            self.log_parser_error(
                "Expected at least one of <compound-selector> or <pseudo-compound-selector> while parsing selector unit, but found none"
            );

            return Err(self.error_message.clone());
        }

        /* Skip whitespace at the top-level to not confuse the lower level parsing function */
        self.skip_to_just_before_non_whitespace()
            .map_err(|_| self.error_message.clone())?;

        /* Check EOF every time we are done with a selector unit */
        let mut peeker = self.clone();
        peeker
            .next_token()
            .map_err(|_| self.error_message.clone())?;

        Ok((
            SelectorUnit {
                prev_combinator: combinator,
                filter: filter,
            },
            peeker.css_lexer.current_token.kind() == CssTokenType::Eof,
        ))
    }

    /*
        <combinator> = '>' | '+' | '~'
        We will also consider the whitespaces only case
    */
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

    // <compound-selector> = [ <type-selector>? <subclass-selector>* ]!
    fn parse_compound_selector(&mut self) -> Result<NodeFilter, ()> {
        let mut filter = NodeFilter {
            namespace: Namespace::Default,
            tag: None,
            ids: None,
            classes: None,
            attributes: None,
            pseudos: None,
        };

        let mut valid_compound_selector = false;

        /* Optionally parse the <type-selector>, rewind if not found */
        let saved = self.css_lexer.clone();
        let parse_result = self.parse_type_selector();

        if let Ok((ns, tag)) = parse_result {
            filter.namespace = ns;
            filter.tag = Some(tag);
            valid_compound_selector = true;
        } else {
            self.css_lexer = saved;
        }

        /* Then parse all possible <subclass-selector> */
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

    /*
       <pseudo-compound-selector> =  <pseudo-element-selector> <pseudo-class-selector>*

       This function will return a PseudoFilter describing a <pseudo-compound-selector>,
       or an error message in String if parsing failed
    */
    fn parse_pseudo_compound_selector(&mut self) -> Result<NodeFilter, ()> {
        let mut filter = NodeFilter {
            namespace: Namespace::Default,
            tag: None,
            ids: None,
            classes: None,
            attributes: None,
            pseudos: Some(Vec::new()),
        };

        /* First match the mandatory pseudo-element */
        let parse_result = self.parse_pseudo_element_selector();
        match parse_result {
            Ok(pseudo_element) => {
                filter.pseudos.as_mut().unwrap().push(pseudo_element);
            }

            Err(_) => {
                return Err(self.log_parser_error(
                    "Expected a <pseudo-element-selector> while parsing <pseudo-compound-selector>",
                ))
            }
        }

        /* Then match the optional arbitrary (possibly zero) pseudo-class */
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

    /*
        <type-selector> = <wq-name> | <ns-prefix>? '*'
        This function will return (namespace, identifier) describing a <type-selector>
        where namespace describes <ns-prefix> and identifier describese <wq-name>

        or error message in String if parsing failed
    */
    fn parse_type_selector(&mut self) -> Result<(Namespace, String), ()> {
        // Try parsing a <wq-name> first, rewind if failed
        let saved = self.css_lexer.clone();
        let parse_result = self.parse_wq_name();
        if let Ok((ns, name)) = parse_result {
            return Ok((ns, name));
        } else {
            self.css_lexer = saved;
        }

        // Now try parsing an optional <ns-prefix> with a '*' token
        let saved = self.css_lexer.clone();
        let parse_result = self.parse_ns_prefix();
        if let Err(_) = parse_result {
            self.css_lexer = saved;
        }

        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::Delim
            || self.css_lexer.current_text != "*"
        {
            return Err(self.log_parser_error(
                "Expected an identifier, or namespace prefix with '*' token while parsing type selector"
            ));
        }

        Ok((
            parse_result.unwrap_or(Namespace::Default),
            self.css_lexer.current_text.to_string(),
        ))
    }

    /*
        <subclass-selector> = <id-selector> | <class-selector> | <attribute-selector> | <pseudo-class-selector>

        <id-selector> = <hash-token>
        <class-selector> = '.' <ident-token>

        This function will return a NodeFilter describing a <subclass-selector>
        or error message in String if parsing failed
    */
    fn parse_subclass_selector(&mut self) -> Result<NodeFilter, ()> {
        /*
            We will try complex production first,
            First parsing an <attribute-selector>
        */
        let saved = self.css_lexer.clone();
        let parse_result = self.parse_attribute_selector();
        if let Ok(attribute_filter) = parse_result {
            return Ok(NodeFilter {
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

        // Now try parsing a <pseudo-class-selector>
        let saved = self.css_lexer.clone();
        let parse_result = self.parse_pseudo_class_selector();
        if let Ok(pseudo_filter) = parse_result {
            return Ok(NodeFilter {
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

        /*
            If both <attribute-selector> and <pseudo-class-selector> parsing failed, then we try the simpler production
            Now try to get either <id-selector> or <class-selector>, if both fail then we return an Err()
        */
        self.next_token()?;
        match self.css_lexer.current_token.kind() {

            CssTokenType::Hash => {
                return Ok(NodeFilter {
                        namespace: Namespace::None, tag: None, classes: None, attributes: None, pseudos: None,
                        ids: Some(vec![self.css_lexer.current_text[1..].to_string()])
                    });
            },

            CssTokenType::Delim if self.css_lexer.current_text == "." => {

                self.next_token()?;
                if self.css_lexer.current_token.kind() == CssTokenType::Ident {
                    return Ok(NodeFilter { 
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

    /*
        <attribute-selector> = '[' <wq-name> ']' | '[' <wq-name> <attr-matcher> [ <string-token> | <ident-token> ] <attr-modifier>? ']'
        <attr-matcher> = [ '~' | '|' | '^' | '$' | '*' ]? '='
        <attr-modifier> = i | s

        This function will return an AttributeFilter describing an <attribute-selector>
        or error message in String if parsing failed
    */
    fn parse_attribute_selector(&mut self) -> Result<AttributeFilter, ()> {
        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::LeftSquare {
            return Err(self.log_parser_error("Expected '[' while parsing attribute selector"));
        }

        // Parse the <wq-name>
        self.skip_to_just_before_non_whitespace()?;
        let (ns, ident) = self.parse_wq_name()?;

        // Check if the <attribute-selector> ends early (has ']' token right now)
        self.next_token()?;
        self.skip_whitespaces()?;
        if self.css_lexer.current_token.kind() == CssTokenType::RightSquare {
            return Ok(AttributeFilter {
                namespace: ns,
                name: ident,
                operator: None,
                value: None,
                modifier: None,
            });
        }

        // If not ends early, continue parsing the <attr-matcher>
        let operator;
        if self.css_lexer.current_token.kind() != CssTokenType::Delim {
            return Err(self.log_parser_error(
                "Expected attribute matcher operator while parsing attribute selector",
            ));
        }

        match self.css_lexer.current_text {
            "~" | "|" | "^" | "$" | "*" | "=" => operator = self.css_lexer.current_text,
            _ => {
                return Err(self.log_parser_error(
                    "Expected attribute matcher operator while parsing attribute selector",
                ))
            }
        }

        if operator != "=" {
            self.next_token()?;
            if self.css_lexer.current_token.kind() != CssTokenType::Delim
                || self.css_lexer.current_text != "="
            {
                return Err(
                    self.log_parser_error("Expected operator '=' while parsing attribute selector")
                );
            }
        }

        // Parse the <string-token> or <ident-token>, make sure found at least one of it
        let value;

        self.next_token()?;
        self.skip_whitespaces()?;
        match self.css_lexer.current_token.kind() {
            CssTokenType::Ident | CssTokenType::String => value = self.css_lexer.current_text,
            _ => {
                return Err(self.log_parser_error(
                    "Expected string or identifier while parsing attribute selector",
                ))
            }
        }

        // Parse the last ']' token while possibly accounting for optional <attr-modifier> and leading whitespace
        let modifier;

        self.next_token()?;
        self.skip_whitespaces()?;

        match self.css_lexer.current_token.kind() {
            CssTokenType::RightSquare => {
                return Ok(AttributeFilter {
                    namespace: ns,
                    name: ident,
                    operator: Some(operator.to_string()),
                    value: Some(value.to_string()),
                    modifier: None,
                });
            }

            CssTokenType::Ident
                if self.css_lexer.current_text == "i" || self.css_lexer.current_text == "s" =>
            {
                modifier = self.css_lexer.current_text.chars().next();
            }

            _ => {
                return Err(self
                    .log_parser_error("Expected 'i', 's' or ']' while parsing attribute selector"))
            }
        }

        // If we are here, that means a <attr-modifier> has been found and we just need to parse the last ']' token with possibly leading whitespace
        self.next_token()?;
        self.skip_whitespaces()?;

        match self.css_lexer.current_token.kind() {
            CssTokenType::RightSquare => {
                return Ok(AttributeFilter {
                    namespace: ns,
                    name: ident,
                    operator: Some(operator.to_string()),
                    value: Some(value.to_string()),
                    modifier: modifier,
                });
            }

            _ => return Err(self.log_parser_error("Expected ']' while parsing attribute selector")),
        }
    }

    /*
        <pseudo-class-selector> = : <ident-token> | : <function-token> <any-value> )

    */
    fn parse_pseudo_class_selector(&mut self) -> Result<PseudoFilter, ()> {
        /* Try parsing the first ':' token */
        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::Colon {
            return Err(self.log_parser_error("Expected ':' while parsing pseudo-class"));
        }

        /* Then try to parse and <ident-token> or <function-token> while also parsing the argument carefully */
        self.next_token()?;
        match self.css_lexer.current_token.kind() {
            CssTokenType::Ident => {
                return Ok(PseudoFilter {
                    name: self.css_lexer.current_text.to_string(),
                    args: None,
                });
            }

            CssTokenType::Function => {
                let name = self.css_lexer.current_text;
                let mut args = String::from("");
                let mut rparen_needed = 1;

                loop {
                    self.next_token()?;

                    match self.css_lexer.current_token.kind() {
                        CssTokenType::LeftParen | CssTokenType::Function => rparen_needed += 1,
                        CssTokenType::RightParen => {
                            rparen_needed -= 1;
                            if rparen_needed == 0 {
                                break;
                            }
                        }
                        _ => (),
                    }

                    args.push_str(self.css_lexer.current_text);
                }

                let len = name.len();
                return Ok(PseudoFilter {
                    name: name[0..len - 1].to_string(),
                    args: Some(args),
                }); // Truncate last char to remove trailing '(' in function token
            }

            _ => {
                return Err(self.log_parser_error(
                    "Expected identifier or function while parsing pseudo-class",
                ))
            }
        }
    }

    /*
       <pseudo-element-selector> = : <pseudo-class-selector> | <legacy-pseudo-element-selector>
       <legacy-pseudo-element-selector> =  : [before | after | first-line | first-letter]
    */
    fn parse_pseudo_element_selector(&mut self) -> Result<PseudoFilter, ()> {
        /* Try parsing the first ':' token */
        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::Colon {
            return Err(self.log_parser_error("Expected ':' while parsing pseudo-element"));
        }

        /* Then try matching the legacy element first */
        let saved = self.css_lexer.clone();
        let mut legacy_name = "";

        self.next_token()?;
        match self.css_lexer.current_token.kind() {
            CssTokenType::Ident if self.css_lexer.current_text == "before" => {
                legacy_name = self.css_lexer.current_text
            }
            CssTokenType::Ident if self.css_lexer.current_text == "after" => {
                legacy_name = self.css_lexer.current_text
            }
            CssTokenType::Ident if self.css_lexer.current_text == "first-line" => {
                legacy_name = self.css_lexer.current_text
            }
            CssTokenType::Ident if self.css_lexer.current_text == "first-letter" => {
                legacy_name = self.css_lexer.current_text
            }
            _ => self.css_lexer = saved,
        }
        if legacy_name != "" {
            return Ok(PseudoFilter {
                name: legacy_name.to_string(),
                args: None,
            });
        }

        /* Lastly, try the general pseuo-class selector */
        if let Ok(pseudo_class) = self.parse_pseudo_class_selector() {
            Ok(PseudoFilter {
                name: pseudo_class.name,
                args: pseudo_class.args,
            })
        } else {
            Err(self.log_parser_error(
                "Expected pseudo-class or legacy pseudo element while parsing pseudo-element",
            ))
        }
    }

    /*
        <wq-name> = <ns-prefix>? <ident-token>

        This function will return (identifier) describing a <wq-name>
        or error message in String if parsing failed
    */
    fn parse_wq_name(&mut self) -> Result<(Namespace, String), ()> {
        /* Try parsing the optional ns-prefix */
        let saved = self.css_lexer.clone();
        let ns_parsed = self.parse_ns_prefix();

        let ns;
        if let Ok(result) = ns_parsed {
            ns = result;
        } else {
            self.css_lexer = saved;
            ns = Namespace::Default;
        }

        /* Get the mandatory identifier token */
        self.next_token()?;
        match self.css_lexer.current_token.kind() {
            CssTokenType::Ident => return Ok((ns, self.css_lexer.current_text.to_string())),
            _ => return Err(self.log_parser_error("Expected identifier or '*' symbol")),
        };
    }

    /*
        <ns-prefix> = [ <ident-token> | '*' ]? '|'

        This function will return (namespace) describing a <ns-prefix>
        or error message in String if parsing failed
    */
    fn parse_ns_prefix(&mut self) -> Result<Namespace, ()> {
        let ns;

        /* Get possibly the identifier token or the '*' delimiter token (both are optional) */
        let saved = self.css_lexer.clone();
        self.next_token()?;

        match self.css_lexer.current_token.kind() {
            CssTokenType::Ident => ns = Namespace::Named(self.css_lexer.current_text.to_string()),
            CssTokenType::Delim if self.css_lexer.current_text == "*" => ns = Namespace::Any,
            _ => {
                self.css_lexer = saved;
                ns = Namespace::None;
            }
        };

        /* Then we ensure that we have the '|' delimiter token */
        self.next_token()?;
        if self.css_lexer.current_token.kind() != CssTokenType::Delim
            || self.css_lexer.current_text != "|"
        {
            return Err(self.log_parser_error("Expected '|' symbol"));
        }

        Ok(ns)
    }
}

/*
    Function to unit test our css selector parser, test result are manually checked for now,
    Also see this function for reference on how to use the CSS parser.

    CURRENT TEST STATUS: PASSED ALL
*/
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_absolute_css_parser() {
        static ABSOLUTE_CSS_PARSER_TESTS: &[&str; 21] = &[
            r#".btn-primary"#,
            r#"#main-header"#,
            r#"input[type="checkbox"]"#,
            r#"nav a.active"#,
            r#"ul > li"#,
            r#"h2 + p"#,
            r#"li:first-child"#,
            r#"button:hover:disabled"#,
            r#"svg|rect[*|href^="https" i]"#,
            r#"|div[prefix|attr="value" s]"#,
            r#"*|*[ns|hidden]"#,
            r#"div:not(.class1#id1[attr]):hover"#,
            r#"article:has(> h2 + p):is(:nth-child(2n+1), [data-priority="high"])"#,
            r#"section:where(header, footer) > .content:not(:empty)"#,
            r#"::part(header-btn):focus-visible::before"#,
            r#"li:nth-last-of-type(3n-1):no-fallback"#,
            r#"   div  .class  #id > [attr] + * ~ b    "#,
            r#"html > body  main[role="main"]  nav  ul  li:first-child > a[href^="/"]"#,
            r#"*[a|b=c][d|e=f]#id:pseudo"#,
            r#"[attr=" value with spaces "][empty=""]"#,
            r#"  *|svg  [  ns1|attr1  =  "val1"  ]  [  *|attr2  ^=  "val2"  i  ]  :nth-child(  3n  -  1  of  .class  :not(  #id  )  )  [  |attr3  *=  "val3"  ]  [attr4~="val4"  s]  abcd[  ns2|attr5  |=  "val5"  ].class#id.class2  >  |div  +  ::after:hover:where(  [title^="foo"  s],  :dir(ltr)  ) "#,
        ];

        for (idx, testcase) in ABSOLUTE_CSS_PARSER_TESTS.iter().enumerate() {
            println!("\n### Test Case {} ###", idx + 1);
            dbg!(testcase);

            /* General way to initialize the CSS parser */
            let mut parser = CssSelectorParser::new(testcase, false);

            println!("\n### Absolute Parsing Result ###\n");

            /* General usage of the CSS parser */
            loop {
                let (filter, is_eof) = parser.advance().unwrap();
                dbg!(filter);

                if is_eof {
                    break;
                }
            }
        }
        println!("\n");
    }

    #[test]
    fn test_relative_css_parser() {
        static RELATIVE_CSS_PARSER_TESTS: &[&str; 21] = &[
            r#"    ~    .btn-primary"#,
            r#"    +.btn-secondary"#,
            r#"~    .btn-secondary"#,
            r#">#main-header"#,
            r#"nav a.active"#,
            r#"+ ul > li"#,
            r#"~h2 + p"#,
            r#"li:first-child"#,
            r#"button:hover:disabled"#,
            r#" ~ svg|rect[*|href^="https" i]"#,
            r#"+|div[prefix|attr="value" s]"#,
            r#"~*|*[ns|hidden]"#,
            r#"div:not(.class1#id1[attr]):hover"#,
            r#">article:has(> h2 + p):is(:nth-child(2n+1), [data-priority="high"])"#,
            r#"section:where(header, footer) > .content:not(:empty)"#,
            r#"~ ::part(header-btn):focus-visible::before"#,
            r#" +::part(header):nth-last-of-type(3n-1):no-fallback"#,
            r#"html > body  main[role="main"]  nav  ul  li:first-child > a[href^="/"]"#,
            r#"~ *[a|b=c][d|e=f]#id:pseudo"#,
            r#">[attr=" value with spaces "][empty=""]"#,
            r#" +*|svg  [  ns1|attr1  =  "val1"  ]  [  *|attr2  ^=  "val2"  i  ]  :nth-child(  3n  -  1  of  .class  :not(  #id  )  )  [  |attr3  *=  "val3"  ]  [attr4~="val4"  s]  abcd[  ns2|attr5  |=  "val5"  ].class#id.class2  >  |div  +  ::after:hover:where(  [title^="foo"  s],  :dir(ltr)  ) "#,
        ];

        for (idx, testcase) in RELATIVE_CSS_PARSER_TESTS.iter().enumerate() {
            println!("\n### Test Case {} ###", idx + 1);
            dbg!(testcase);

            /* General way to initialize the CSS parser */
            let mut relative_parser = CssSelectorParser::new(testcase, true);

            println!("\n### Relative Parsing Result ###\n");

            /* General usage of the CSS parser */
            loop {
                let (filter, is_eof) = relative_parser.advance().unwrap();
                dbg!(filter);

                if is_eof {
                    break;
                }
            }
        }
        println!("\n");
    }

    #[test]
    #[should_panic]
    fn test_css_parser_invalid() {
        let testcase = "[|]";

        /* General way to initialize the CSS parser */
        let mut parser = CssSelectorParser::new(testcase, false);

        println!("\n### Parsing Invalid Selector Result ###\n");

        /* General usage of the CSS parser */
        loop {
            let parse_result = parser.advance();

            if let Err(e) = &parse_result {
                println!("{}", e);
                panic!("Panicked as expected.");
            }

            if let Ok((filter, is_eof)) = parse_result {
                dbg!(filter);
                if is_eof {
                    break;
                }
            }
        }
    }
}
