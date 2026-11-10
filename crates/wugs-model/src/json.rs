//! Minimal strict JSON: parse + serialize. No external dependencies.
//! Numbers are f64 (op-log numbers are millis/counters, all < 2^53).

use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(BTreeMap<String, Value>),
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(s.to_string())
    }
    pub fn obj(pairs: Vec<(&str, Value)>) -> Value {
        Value::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_obj(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Obj(m) => Some(m),
            _ => None,
        }
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_obj().and_then(|m| m.get(key))
    }
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    pub fn to_json(&self) -> String {
        let mut s = String::new();
        write_value(self, &mut s);
        s
    }
}

fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Num(n) => {
            if !n.is_finite() {
                out.push_str("null");
            } else if n.fract() == 0.0 && n.abs() < 9.0e15 {
                let _ = write!(out, "{}", *n as i64);
            } else {
                let _ = write!(out, "{}", n);
            }
        }
        Value::Str(s) => write_str(s, out),
        Value::Arr(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(x, out);
            }
            out.push(']');
        }
        Value::Obj(m) => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_str(k, out);
                out.push(':');
                write_value(x, out);
            }
            out.push('}');
        }
    }
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[derive(Debug)]
pub struct JsonError {
    pub msg: String,
    pub at: usize,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "json error at byte {}: {}", self.at, self.msg)
    }
}

pub fn parse(s: &str) -> Result<Value, JsonError> {
    let mut p = Parser {
        b: s.as_bytes(),
        i: 0,
        depth: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(p.err("trailing data"));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    depth: u32,
}

const MAX_DEPTH: u32 = 128;

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> JsonError {
        JsonError {
            msg: msg.to_string(),
            at: self.i,
        }
    }
    fn ws(&mut self) {
        while self.i < self.b.len() {
            match self.b[self.i] {
                b' ' | b'\t' | b'\n' | b'\r' => self.i += 1,
                _ => break,
            }
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> Result<(), JsonError> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err("unexpected character"))
        }
    }

    fn value(&mut self) -> Result<Value, JsonError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        let v = match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            _ => Err(self.err("expected a value")),
        };
        self.depth -= 1;
        v
    }

    fn literal(&mut self, word: &str, v: Value) -> Result<Value, JsonError> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err("bad literal"))
        }
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.i;
        while let Some(c) = self.peek() {
            match c {
                b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9' => self.i += 1,
                _ => break,
            }
        }
        let text = std::str::from_utf8(&self.b[start..self.i]).unwrap();
        text.parse::<f64>()
            .map(Value::Num)
            .map_err(|_| self.err("bad number"))
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.eat(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek().ok_or_else(|| self.err("unterminated string"))?;
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek().ok_or_else(|| self.err("bad escape"))?;
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                // surrogate pair
                                if self.peek() == Some(b'\\') {
                                    self.i += 1;
                                    self.eat(b'u')?;
                                    let lo = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&lo) {
                                        return Err(self.err("bad low surrogate"));
                                    }
                                    0x10000 + (((hi - 0xD800) as u32) << 10) + (lo - 0xDC00) as u32
                                } else {
                                    return Err(self.err("lone surrogate"));
                                }
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(self.err("lone surrogate"));
                            } else {
                                hi as u32
                            };
                            let ch = char::from_u32(cp).ok_or_else(|| self.err("bad codepoint"))?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(self.err("bad escape")),
                    }
                }
                c if c < 0x20 => return Err(self.err("control char in string")),
                c => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("invalid utf-8"))
    }

    fn hex4(&mut self) -> Result<u16, JsonError> {
        if self.i + 4 > self.b.len() {
            return Err(self.err("short \\u escape"));
        }
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4])
            .map_err(|_| self.err("bad \\u escape"))?;
        let v = u16::from_str_radix(s, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(v)
    }

    fn object(&mut self) -> Result<Value, JsonError> {
        self.eat(b'{')?;
        let mut m = BTreeMap::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Obj(m));
        }
        loop {
            self.ws();
            let k = self.string()?;
            self.ws();
            self.eat(b':')?;
            self.ws();
            let v = self.value()?;
            m.insert(k, v);
            self.ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(self.err("expected , or }")),
            }
        }
        Ok(Value::Obj(m))
    }

    fn array(&mut self) -> Result<Value, JsonError> {
        self.eat(b'[')?;
        let mut a = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Arr(a));
        }
        loop {
            self.ws();
            a.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(self.err("expected , or ]")),
            }
        }
        Ok(Value::Arr(a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let src = r#"{"a":[1,2.5,-3],"b":"köpek \"x\" \\ \n ✓","c":null,"d":true,"e":{"f":false}}"#;
        let v = parse(src).unwrap();
        let out = v.to_json();
        let v2 = parse(&out).unwrap();
        assert_eq!(v, v2);
        assert_eq!(v.get_str("b").unwrap(), "köpek \"x\" \\ \n ✓");
    }

    #[test]
    fn unicode_escapes() {
        let v = parse(r#""ça va 😀""#).unwrap();
        assert_eq!(v.as_str().unwrap(), "ça va 😀");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("{").is_err());
        assert!(parse(r#"{"a":}"#).is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("tru").is_err());
        assert!(parse(r#""\ud800x""#).is_err());
        assert!(parse("1 2").is_err());
    }

    #[test]
    fn integers_stay_integers_in_output() {
        let v = parse("{\"n\":42}").unwrap();
        assert_eq!(v.to_json(), "{\"n\":42}");
    }
}
