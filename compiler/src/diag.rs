/// 诊断输出格式 = T-4 黄金文件基线（改动须 PR+QA 评审）：
/// `zorcc: {E-code} {file}:{line}:{col}: {message}`
#[derive(Debug, Clone)]
pub struct Diag {
    pub code: String,
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

impl Diag {
    pub fn new(code: &str, line: usize, col: usize, msg: impl Into<String>) -> Self {
        Diag { code: code.to_string(), line, col, msg: msg.into() }
    }
    pub fn render(&self, file: &str) -> String {
        format!("zorcc: {} {}:{}:{}: {}", self.code, file, self.line, self.col, self.msg)
    }

    /// `--diag=json` 的 NDJSON 单行对象：六键固定序
    /// `code`／`severity`／`message`／`file`／`line`／`col`，无多余空白。
    pub fn render_json(&self, file: &str) -> String {
        format!(
            "{{\"code\":{},\"severity\":\"error\",\"message\":{},\"file\":{},\"line\":{},\"col\":{}}}",
            json_string(&self.code),
            json_string(&self.msg),
            json_string(file),
            self.line,
            self.col
        )
    }
}

/// JSON 字符串转义：引号／反斜杠／控制字符；非 ASCII UTF-8 原样输出。
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::Diag;

    #[test]
    fn text_render_matches_t4_baseline() {
        let d = Diag::new("E1101", 3, 5, "unknown identifier 'x'");
        assert_eq!(d.render("examples/a.zor"), "zorcc: E1101 examples/a.zor:3:5: unknown identifier 'x'");
    }

    #[test]
    fn json_render_has_fixed_key_order_and_schema() {
        let d = Diag::new("E1101", 3, 5, "unknown identifier 'x'");
        assert_eq!(
            d.render_json("examples/a.zor"),
            "{\"code\":\"E1101\",\"severity\":\"error\",\"message\":\"unknown identifier 'x'\",\"file\":\"examples/a.zor\",\"line\":3,\"col\":5}"
        );
    }

    #[test]
    fn json_render_escapes_quotes_backslashes_and_controls() {
        let d = Diag::new("E1101", 1, 1, "a\"b\\c\nd\te");
        assert_eq!(
            d.render_json("dir\\a\"b.zor"),
            "{\"code\":\"E1101\",\"severity\":\"error\",\"message\":\"a\\\"b\\\\c\\nd\\te\",\"file\":\"dir\\\\a\\\"b.zor\",\"line\":1,\"col\":1}"
        );
    }

    #[test]
    fn json_render_keeps_utf8_verbatim() {
        let d = Diag::new("E1101", 1, 2, "未知 '标识符'");
        let line = d.render_json("例子/你好.zor");
        assert!(line.contains("\"message\":\"未知 '标识符'\""), "{line}");
        assert!(line.contains("\"file\":\"例子/你好.zor\""), "{line}");
        assert!(!line.contains('\n'), "{line}");
    }
}
