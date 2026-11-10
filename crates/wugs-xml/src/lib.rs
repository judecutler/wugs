//! Minimal strict XML DOM shared by the ELAN and FLEx interchange crates.
//! Not a general XML processor: no DTDs, no namespace resolution (`xmlns` is
//! an ordinary attribute), no whitespace normalization, so annotation text
//! survives byte for byte.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct XmlNode {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
    /// Concatenated character data directly inside this element (CDATA and
    /// entity references already resolved).
    pub text: String,
    /// Character data before the first child element. Together with each
    /// child's `tail` this keeps the order of mixed content such as LIFT's
    /// `<text>a<span>b</span>c</text>`.
    pub head: String,
    /// Character data after this element's end tag, up to the next sibling.
    pub tail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XmlError {
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "xml error at {}:{}: {}", self.line, self.col, self.msg)
    }
}

impl std::error::Error for XmlError {}

impl XmlNode {
    pub fn new(name: &str) -> XmlNode {
        XmlNode {
            name: name.to_string(),
            ..XmlNode::default()
        }
    }

    pub fn with_attr(mut self, key: &str, value: &str) -> XmlNode {
        self.attrs.push((key.to_string(), value.to_string()));
        self
    }

    pub fn with_child(mut self, child: XmlNode) -> XmlNode {
        self.children.push(child);
        self
    }

    pub fn with_text(mut self, text: &str) -> XmlNode {
        self.text = text.to_string();
        self
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.name == name)
    }

    /// All character data inside this element, descendants included, in
    /// document order.
    pub fn deep_text(&self) -> String {
        let mut out = self.head.clone();
        for c in &self.children {
            out.push_str(&c.deep_text());
            out.push_str(&c.tail);
        }
        out
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlNode> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// Serialize with an XML declaration and two-space indentation. Elements
    /// that have children never emit their own `text`, and mixed content
    /// (`head`, `tail`) is not written.
    pub fn to_xml(&self) -> String {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        self.write(&mut out, 0);
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        let pad = "  ".repeat(depth);
        out.push_str(&pad);
        out.push('<');
        out.push_str(&self.name);
        for (k, v) in &self.attrs {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            out.push_str(&escape(v, true));
            out.push('"');
        }
        if !self.children.is_empty() {
            out.push_str(">\n");
            for c in &self.children {
                c.write(out, depth + 1);
            }
            out.push_str(&pad);
        } else if self.text.is_empty() {
            out.push_str("/>\n");
            return;
        } else {
            out.push('>');
            out.push_str(&escape(&self.text, false));
        }
        out.push_str("</");
        out.push_str(&self.name);
        out.push_str(">\n");
    }
}

fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

/// Characters XML 1.0 cannot represent at all are replaced with U+FFFD; the
/// alternative is emitting a file no parser accepts.
pub fn escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            '\n' | '\t' if attr => out.push_str(if c == '\n' { "&#10;" } else { "&#9;" }),
            c if !is_xml_char(c) => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
    out
}

fn push_text(node: &mut XmlNode, s: &str) {
    node.text.push_str(s);
    match node.children.last_mut() {
        Some(last) => last.tail.push_str(s),
        None => node.head.push_str(s),
    }
}

pub fn parse(input: &str) -> Result<XmlNode, XmlError> {
    let mut p = Parser {
        src: input,
        pos: 0,
    };
    if p.rest().starts_with('\u{FEFF}') {
        p.pos += '\u{FEFF}'.len_utf8();
    }
    p.skip_misc()?;
    if p.rest().is_empty() {
        return Err(p.err("no root element"));
    }
    let root = p.element()?;
    p.skip_misc()?;
    if !p.rest().is_empty() {
        return Err(p.err("content after root element"));
    }
    Ok(root)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn err(&self, msg: impl Into<String>) -> XmlError {
        self.err_at(self.pos, msg)
    }

    fn err_at(&self, offset: usize, msg: impl Into<String>) -> XmlError {
        let before = &self.src[..offset];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        XmlError {
            line,
            col,
            msg: msg.into(),
        }
    }

    fn eat(&mut self, s: &str) -> bool {
        if self.rest().starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        let trimmed = self.rest().trim_start_matches([' ', '\t', '\r', '\n']);
        self.pos = self.src.len() - trimmed.len();
    }

    /// Skip whitespace, comments, processing instructions (incl. the XML
    /// declaration) outside the root element.
    fn skip_misc(&mut self) -> Result<(), XmlError> {
        loop {
            self.skip_ws();
            if self.rest().starts_with("<!--") {
                self.comment()?;
            } else if self.rest().starts_with("<?") {
                self.pi()?;
            } else if self.rest().starts_with("<!DOCTYPE") {
                return Err(self.err("DTDs are not supported"));
            } else {
                return Ok(());
            }
        }
    }

    fn comment(&mut self) -> Result<(), XmlError> {
        let start = self.pos;
        self.pos += 4;
        match self.rest().find("-->") {
            Some(i) => {
                self.pos += i + 3;
                Ok(())
            }
            None => Err(self.err_at(start, "unterminated comment")),
        }
    }

    fn pi(&mut self) -> Result<(), XmlError> {
        let start = self.pos;
        match self.rest().find("?>") {
            Some(i) => {
                self.pos += i + 2;
                Ok(())
            }
            None => Err(self.err_at(start, "unterminated processing instruction")),
        }
    }

    fn name(&mut self) -> Result<String, XmlError> {
        let rest = self.rest();
        let end = rest
            .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':')))
            .unwrap_or(rest.len());
        let first_ok = rest
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == ':');
        if end == 0 || !first_ok {
            return Err(self.err("expected a name"));
        }
        let n = rest[..end].to_string();
        self.pos += end;
        Ok(n)
    }

    fn element(&mut self) -> Result<XmlNode, XmlError> {
        let open_at = self.pos;
        if !self.eat("<") {
            return Err(self.err("expected '<'"));
        }
        let mut node = XmlNode::new(&self.name()?);
        loop {
            let had_ws = self.rest().starts_with([' ', '\t', '\r', '\n']);
            self.skip_ws();
            if self.eat("/>") {
                return Ok(node);
            }
            if self.eat(">") {
                break;
            }
            if self.rest().is_empty() {
                return Err(self.err_at(open_at, format!("unterminated start tag <{}>", node.name)));
            }
            if !had_ws {
                return Err(self.err("expected whitespace before attribute"));
            }
            let key_at = self.pos;
            let key = self.name()?;
            self.skip_ws();
            if !self.eat("=") {
                return Err(self.err("expected '=' after attribute name"));
            }
            self.skip_ws();
            let value = self.attr_value()?;
            if node.attr(&key).is_some() {
                return Err(self.err_at(key_at, format!("duplicate attribute {key}")));
            }
            node.attrs.push((key, value));
        }
        self.content(&mut node, open_at)?;
        Ok(node)
    }

    fn attr_value(&mut self) -> Result<String, XmlError> {
        let quote = match self.rest().chars().next() {
            Some(q @ ('"' | '\'')) => q,
            _ => return Err(self.err("attribute value must be quoted")),
        };
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.rest().chars().next() else {
                return Err(self.err("unterminated attribute value"));
            };
            match c {
                c if c == quote => {
                    self.pos += 1;
                    return Ok(out);
                }
                '<' => return Err(self.err("'<' in attribute value")),
                '&' => out.push(self.reference()?),
                c if !is_xml_char(c) => return Err(self.err("invalid character")),
                c => {
                    out.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
    }

    fn content(&mut self, node: &mut XmlNode, open_at: usize) -> Result<(), XmlError> {
        loop {
            let Some(c) = self.rest().chars().next() else {
                return Err(self.err_at(open_at, format!("unclosed element <{}>", node.name)));
            };
            if self.rest().starts_with("</") {
                self.pos += 2;
                let close_at = self.pos;
                let name = self.name()?;
                if name != node.name {
                    return Err(self.err_at(
                        close_at,
                        format!("mismatched close tag </{name}>, expected </{}>", node.name),
                    ));
                }
                self.skip_ws();
                if !self.eat(">") {
                    return Err(self.err("expected '>' in close tag"));
                }
                return Ok(());
            } else if self.rest().starts_with("<!--") {
                self.comment()?;
            } else if self.rest().starts_with("<![CDATA[") {
                let start = self.pos;
                self.pos += 9;
                let Some(i) = self.rest().find("]]>") else {
                    return Err(self.err_at(start, "unterminated CDATA section"));
                };
                let cdata = &self.rest()[..i];
                push_text(node, cdata);
                self.pos += i + 3;
            } else if self.rest().starts_with("<?") {
                self.pi()?;
            } else if c == '<' {
                node.children.push(self.element()?);
            } else if c == '&' {
                let r = self.reference()?;
                push_text(node, r.encode_utf8(&mut [0; 4]));
            } else if !is_xml_char(c) {
                return Err(self.err("invalid character"));
            } else {
                push_text(node, c.encode_utf8(&mut [0; 4]));
                self.pos += c.len_utf8();
            }
        }
    }

    fn reference(&mut self) -> Result<char, XmlError> {
        let start = self.pos;
        let Some(end) = self.rest().find(';') else {
            return Err(self.err("unterminated entity reference"));
        };
        let body = &self.rest()[1..end];
        let c = match body {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = if let Some(hex) = body.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = body.strip_prefix('#') {
                    dec.parse::<u32>().ok()
                } else {
                    None
                };
                match code.and_then(char::from_u32).filter(|c| is_xml_char(*c)) {
                    Some(c) => c,
                    None => return Err(self.err_at(start, format!("unknown entity &{body};"))),
                }
            }
        };
        self.pos += end + 1;
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_round_trips_through_writer_and_parser() {
        let nasty = "a & b < c > d \"q\" 'a' \r\n\ttab köpek\u{1F415}";
        let doc = XmlNode::new("r")
            .with_attr("k", nasty)
            .with_child(XmlNode::new("t").with_text(nasty));
        let back = parse(&doc.to_xml()).unwrap();
        assert_eq!(back.attr("k"), Some(nasty), "attribute survives write+parse");
        assert_eq!(back.child("t").unwrap().text, nasty, "text survives write+parse");
    }

    #[test]
    fn declaration_comments_cdata_and_quotes() {
        let doc = parse(
            "\u{FEFF}<?xml version=\"1.0\"?>\n<!-- hi --><a x='1' y=\"2\"><!-- c --><![CDATA[<raw> & ]]>tail<b/></a>\n<!-- end -->",
        )
        .unwrap();
        assert_eq!(doc.attr("x"), Some("1"), "single-quoted attribute");
        assert_eq!(doc.attr("y"), Some("2"), "double-quoted attribute");
        assert_eq!(doc.text, "<raw> & tail", "CDATA is literal text");
        assert_eq!(doc.children.len(), 1, "comment skipped, element kept");
    }

    #[test]
    fn mixed_content_keeps_document_order() {
        let doc = parse("<text>köp<span lang=\"tr\">ek</span>ler &amp; <b/>x</text>").unwrap();
        assert_eq!(doc.deep_text(), "köpekler & x", "span text stays in place");
        assert_eq!(doc.text, "köpler & x", "direct text keeps its old meaning");
    }

    #[test]
    fn named_and_numeric_entities() {
        let doc = parse("<a>&lt;&gt;&amp;&quot;&apos;&#246;&#xF6;&#x1F415;</a>").unwrap();
        assert_eq!(doc.text, "<>&\"'öö\u{1F415}", "all entity forms decode");
    }

    #[test]
    fn malformed_input_is_rejected_with_position() {
        let cases = [
            "",
            "<a>",
            "<a></b>",
            "<a x=1/>",
            "<a x=\"1\" x=\"2\"/>",
            "<a>&bogus;</a>",
            "<a>&#0;</a>",
            "<a/><b/>",
            "<a><!-- open</a>",
            "<a><![CDATA[ open</a>",
            "<!DOCTYPE a><a/>",
            "<a x=\"<\"/>",
            "<a\u{1}/>",
            "<a>\u{1}</a>",
        ];
        for bad in cases {
            assert!(parse(bad).is_err(), "must reject {bad:?}");
        }
        let e = parse("<a>\n  <b></c>\n</a>").unwrap_err();
        assert_eq!((e.line, e.col), (2, 8), "error points at the bad close tag: {e}");
    }
}
