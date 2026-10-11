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
}
