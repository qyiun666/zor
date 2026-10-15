use std::io::Write;
use std::process::exit;

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Version,
    /// 缺源文件：保留 0.1.2 在册 usage 句原样，不加前缀
    Usage,
    Compile(String),
}

/// C-1 白名单（冻结件 §〇-3／§三 C-1 逐字照录）：仍为接受并忽略、rc 不变。
/// 语义锚：这些是 ci/run_ci.sh:30-31 的在册注入位与 ASAN 判据句要求，非本窗拒判面。
fn is_whitelisted_flag(f: &str) -> bool {
    matches!(
        f,
        "-O0" | "-O2" | "-Zoverflow=check" | "-Zoverflow=wrap" | "-g"
    )
}

/// Ok(Cli) 继续；Err(指名句)＝CLI 面误用，出口码固定 rc=2（C-2 硬约束：不得占 rc=1）。
fn parse_cli(args: &[String]) -> Result<Cli, String> {
    // 0.1.2 在册短路语义原样保留：-V/--version 在 argv 任意位置先于文件处理命中
    if args.iter().any(|a| a == "--version" || a == "-V") {
        return Ok(Cli::Version);
    }
    let mut positional: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if !a.starts_with('-') {
            positional.push(a.as_str());
            continue;
        }
        if a == "-o" || a == "--output" {
            // 连同其值一并拒判，避免出参路径被后续当源文件（①形的错指根因）
            let _ = it.next();
            return Err(format!(
                "'{a}' 未实现：0.1.x 无代码生成路径，不产出任何文件（显式拒判，非接受并忽略）"
            ));
        }
        if a.starts_with("--output=") {
            return Err(
                "'--output=' 未实现：0.1.x 无代码生成路径，不产出任何文件（显式拒判，非接受并忽略）"
                    .to_string(),
            );
        }
        if is_whitelisted_flag(a) {
            continue;
        }
        return Err(format!("未知选项 '{a}'：不在本窗白名单内，不予静默接受"));
    }
    match positional.len() {
        0 => Ok(Cli::Usage),
        1 => Ok(Cli::Compile(positional[0].to_string())),
        n => Err(format!(
            "只接受一个源文件，收到 {n} 枚：{}（第二枚起显式拒判，静默丢弃会漏检未检查的源文件）",
            positional.join(" ")
        )),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = match parse_cli(&args) {
        Ok(Cli::Version) => {
            println!("zorcc {}", env!("CARGO_PKG_VERSION"));
            exit(0);
        }
        Ok(Cli::Usage) => {
            eprintln!("usage: zorcc [flags] <file.zor>");
            exit(2);
        }
        Ok(Cli::Compile(p)) => p,
        Err(msg) => {
            eprintln!("zorcc: CLI: {msg}");
            exit(2);
        }
    };
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zorcc: cannot read '{}': {}", path, e);
            exit(2);
        }
    };
    let diags = zorcc::compile_to_diags(&src);
    if diags.is_empty() {
        exit(0);
    }
    let err = std::io::stderr();
    let mut w = err.lock();
    for d in &diags {
        let _ = writeln!(w, "{}", d.render(&path));
    }
    exit(1);
}

#[cfg(test)]
mod tests {
    use super::{parse_cli, Cli};

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// C-1：白名单逐枚仍接受并忽略，且 --version/-V 短路先于文件处理
    #[test]
    fn c1_whitelisted_flags_still_accepted_and_rc_path_unchanged() {
        for f in ["-O0", "-O2", "-Zoverflow=check", "-Zoverflow=wrap", "-g"] {
            assert_eq!(
                parse_cli(&v(&[f, "a.zor"])),
                Ok(Cli::Compile("a.zor".to_string())),
                "在册注入位 {f} 不得被拒判（拒了 CI 七段当场全红）"
            );
        }
        assert!(matches!(parse_cli(&v(&["--version"])), Ok(Cli::Version)));
        assert!(matches!(parse_cli(&v(&["-O2", "-V", "a.zor"])), Ok(Cli::Version)));
    }

    /// 形①：`-o <path>` 任何位置一律指名拒判
    #[test]
    fn form1_output_flag_rejected_at_any_position() {
        for argv in [
            v(&["-o", "out.bin", "a.zor"]),
            v(&["a.zor", "-o", "out.bin"]),
            v(&["-O2", "-o", "out.bin", "a.zor"]),
        ] {
            let e = parse_cli(&argv).unwrap_err();
            assert!(e.contains("-o") && e.contains("未实现"), "{e}");
        }
        assert!(parse_cli(&v(&["--output", "out.bin", "a.zor"]))
            .unwrap_err()
            .contains("--output"));
        assert!(parse_cli(&v(&["a.zor", "--output=out.bin"]))
            .unwrap_err()
            .contains("--output="));
        // -o 缺值也不放行
        assert!(parse_cli(&v(&["a.zor", "-o"])).unwrap_err().contains("-o"));
    }

    /// 形②：未列入 C-1 白名单的任意 flag 一律拒判
    #[test]
    fn form2_unknown_flag_rejected() {
        for f in [
            "--emit-wasm=nonsense",
            "-O1",
            "-Os",
            "-g=1",
            "-Zoverflow=bogus",
            "-Z",
            "-Q",
            "--verbose",
        ] {
            let e = parse_cli(&v(&["a.zor", f])).unwrap_err();
            assert!(e.contains("未知选项") && e.contains(f), "{f} → {e}");
        }
    }

    /// 形③：≥2 枚位置参数显式拒判，禁静默丢弃
    #[test]
    fn form3_extra_positional_source_rejected_not_dropped() {
        let e = parse_cli(&v(&["b1.zor", "b2.zor"])).unwrap_err();
        assert!(e.contains("只接受一个源文件") && e.contains("2 枚"), "{e}");
        assert!(e.contains("b2.zor"), "拒判句须点名被丢弃的源文件: {e}");
        assert!(parse_cli(&v(&["a.zor", "b.zor", "-O2", "c.zor"]))
            .unwrap_err()
            .contains("3 枚"));
    }

    /// 形④：缺源文件走在册 usage 句（原样、不加前缀）
    #[test]
    fn form4_missing_source_keeps_in_record_usage_line() {
        assert!(matches!(parse_cli(&v(&[])), Ok(Cli::Usage)));
        assert!(matches!(parse_cli(&v(&["-O2"])), Ok(Cli::Usage)));
        assert!(matches!(parse_cli(&v(&["-Zoverflow=check"])), Ok(Cli::Usage)));
    }

    /// C-2：CLI 面诊断不占语言诊断码位（红线 2），也不写成 E1xxx/E2xxx 串
    #[test]
    fn c2_cli_diagnostics_allocate_no_language_codes() {
        for bad in [
            v(&["-o", "out.bin", "a.zor"]),
            v(&["a.zor", "--bogus"]),
            v(&["a.zor", "b.zor"]),
        ] {
            let e = parse_cli(&bad).unwrap_err();
            assert!(!e.contains("E1") && !e.contains("E2") && !e.contains("E3"), "CLI 句不得携语言诊断码位: {e}");
        }
    }

    /// 单文件正形仍走 Compile（回归：不改变门禁调用形）
    #[test]
    fn single_source_still_compiles_through() {
        assert_eq!(
            parse_cli(&v(&["-Zoverflow=check", "tests/specs/positive/r1-01-prim-declare.zor"])),
            Ok(Cli::Compile(
                "tests/specs/positive/r1-01-prim-declare.zor".to_string()
            ))
        );
    }
}
