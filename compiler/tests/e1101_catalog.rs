//! L2c CORE-03：E1101 逐层实发点编目守卫。
//!
//! 冻结口径：E1101 不扩面；编目按发点（字符串字面）计数，不按行号。
//! 本测试只守“发点数与分布未漂移”，不改五值／码表。

use std::path::Path;

#[test]
fn e1101_emission_sites_match_catalog() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected = [
        ("src/lexer.rs", 3usize),
        ("src/parser.rs", 19),
        ("src/check.rs", 7),
    ];
    let mut total = 0;
    for (rel, want) in expected {
        let text = std::fs::read_to_string(root.join(rel)).expect("read source");
        let got = text.matches("\"E1101\"").count();
        assert_eq!(got, want, "{rel}: E1101 发点数漂移（L2c 冻结不扩面）");
        total += got;
    }
    assert_eq!(total, 29, "E1101 发点总数漂移（编目=29）");
}
