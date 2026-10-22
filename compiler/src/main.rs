use std::io::Write;
use std::process::exit;

use zorcc::backend::{BuildOptions, OptLevel, OverflowMode};

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Version,
    /// L2e：`--help` 任意位置短路；固定文本、rc=0、stdout。
    Help,
    /// 缺源文件：保留 0.1.2 在册 usage 句原样，不加前缀
    Usage,
    /// 无 `-o`：0.1.x 检查模式（零产物）
    Compile(String, bool),
    /// 有 `-o`：0.2.0 构建模式（显式产出原生可执行文件）
    Build {
        src: String,
        out: String,
        opt: OptLevel,
        debug: bool,
        overflow: OverflowMode,
        diag_json: bool,
    },
}

/// L2e 固定帮助文本（PRD §3.2 逐字）：15 行／706 B，LF、无尾随空格、末行 LF。
/// 不含版本号、`-Zoverflow`、`-Ofast`／`-O4`／`print`、`-h`。
const HELP: &str = "\
usage: zorcc [flags] <file.zor>
       zorcc [flags] <file.zor> -o <output>

flags:
  -o <output>       build a native executable (atomic publish)
  -O0               no optimization; checked overflow
  -O1               basic optimization; checked overflow
  -O2               standard optimization; wrapping overflow (default)
  -O3               aggressive optimization; wrapping overflow
  -Os               optimize for size; wrapping overflow
  -Og               debug-friendly optimization; checked overflow
  -g                emit debug information
  --diag=json       compile diagnostics as NDJSON on stderr
  --version, -V     print version and exit
  --help            print this help and exit
";

/// L2e 白名单：六档 `-O*`、`-g` 与内部 `-Zoverflow=*`。
/// `-O*` 在构建模式实映射到 C 编译器同名旗标；`-Zoverflow=*` 是内部／CI 位，
/// 仅接受 check／wrap，不进 help／用户文档。这些是 ci/run_ci.sh 的在册注入位。
fn is_whitelisted_flag(f: &str) -> bool {
    matches!(
        f,
        "-O0"
            | "-O1"
            | "-O2"
            | "-O3"
            | "-Os"
            | "-Og"
            | "-Zoverflow=check"
            | "-Zoverflow=wrap"
            | "-g"
    )
}

/// Ok(Cli) 继续；Err(指名句)＝CLI 面误用，出口码固定 rc=2（C-2 硬约束：不得占 rc=1）。
fn parse_cli(args: &[String]) -> Result<Cli, String> {
    // 0.1.2 在册短路语义原样保留：-V/--version 在 argv 任意位置先于文件处理命中
    if args.iter().any(|a| a == "--version" || a == "-V") {
        return Ok(Cli::Version);
    }
    // L2e：--help 任意位置短路，且仅长旗标；-h 仍按未知选项 rc=2。
    if args.iter().any(|a| a == "--help") {
        return Ok(Cli::Help);
    }
    let mut positional: Vec<&str> = Vec::new();
    let mut out: Option<String> = None;
    let mut opt = OptLevel::O2;
    let mut debug = false;
    let mut overflow_explicit: Option<OverflowMode> = None;
    let mut diag_json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--diag=json" {
            diag_json = true;
            continue;
        }
        if a == "--diag" || a.starts_with("--diag=") {
            return Err(format!(
                "未知选项 '{a}'：'--diag' 仅接受 'json'（NDJSON 写 stderr）"
            ));
        }
        if a == "-o" {
            if out.is_some() {
                return Err("'-o' 重复：只允许一次，避免产物路径歧义".to_string());
            }
            match it.next() {
                Some(v) if v.is_empty() => {
                    return Err("'-o' 的值为空：必须显式给出产物路径".to_string());
                }
                Some(v) => out = Some(v.clone()),
                None => {
                    return Err(
                        "'-o' 缺值：用法为 zorcc <file.zor> -o <output>（不提供隐式产物路径）"
                            .to_string(),
                    );
                }
            }
            continue;
        }
        if a == "--output" || a.starts_with("--output=") {
            return Err(format!(
                "'{a}' 不是 0.2.0 契约：只接受 '-o'（--output 不作别名）"
            ));
        }
        if !a.starts_with('-') {
            positional.push(a.as_str());
            continue;
        }
        if is_whitelisted_flag(a) {
            match a.as_str() {
                "-O0" => opt = OptLevel::O0,
                "-O1" => opt = OptLevel::O1,
                "-O2" => opt = OptLevel::O2,
                "-O3" => opt = OptLevel::O3,
                "-Os" => opt = OptLevel::Os,
                "-Og" => opt = OptLevel::Og,
                "-g" => debug = true,
                "-Zoverflow=check" => overflow_explicit = Some(OverflowMode::Check),
                "-Zoverflow=wrap" => overflow_explicit = Some(OverflowMode::Wrap),
                _ => {}
            }
            continue;
        }
        if a.starts_with("-Zoverflow") {
            return Err(format!(
                "未知选项 '{a}'：'-Zoverflow' 仅接受 'check' 或 'wrap'（内部／CI 位，不对外）"
            ));
        }
        return Err(format!("未知选项 '{a}'：不在本窗白名单内，不予静默接受"));
    }
    let overflow = overflow_explicit.unwrap_or_else(|| OverflowMode::from_opt(opt));
    match (positional.len(), out) {
        (0, _) => Ok(Cli::Usage),
        (1, None) => Ok(Cli::Compile(positional[0].to_string(), diag_json)),
        (1, Some(o)) => Ok(Cli::Build {
            src: positional[0].to_string(),
            out: o,
            opt,
            debug,
            overflow,
            diag_json,
        }),
        (n, _) => Err(format!(
            "只接受一个源文件，收到 {n} 枚：{}（第二枚起显式拒判，静默丢弃会漏检未检查的源文件）",
            positional.join(" ")
        )),
    }
}

fn read_source(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zorcc: cannot read '{}': {}", path, e);
            exit(2);
        }
    }
}

fn render_diags(path: &str, diags: &[zorcc::diag::Diag], json: bool) {
    let err = std::io::stderr();
    let mut w = err.lock();
    for d in diags {
        if json {
            let _ = writeln!(w, "{}", d.render_json(path));
        } else {
            let _ = writeln!(w, "{}", d.render(path));
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_cli(&args) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("zorcc: CLI: {msg}");
            exit(2);
        }
    };
    match cli {
        Cli::Version => {
            println!("zorcc {}", env!("CARGO_PKG_VERSION"));
            exit(0);
        }
        Cli::Help => {
            let mut out = std::io::stdout();
            let _ = out.write_all(HELP.as_bytes());
            let _ = out.flush();
            exit(0);
        }
        Cli::Usage => {
            eprintln!("usage: zorcc [flags] <file.zor>");
            exit(2);
        }
        Cli::Compile(path, diag_json) => {
            let src = read_source(&path);
            let diags = zorcc::compile_to_diags(&src);
            if diags.is_empty() {
                exit(0);
            }
            render_diags(&path, &diags, diag_json);
            exit(1);
        }
        Cli::Build {
            src: path,
            out,
            opt,
            debug,
            overflow,
            diag_json,
        } => {
            let src = read_source(&path);
            let c = zorcc::compile_typed(&src);
            if !c.diags.is_empty() {
                render_diags(&path, &c.diags, diag_json);
                exit(1);
            }
            let source = std::path::Path::new(&path);
            let output = std::path::Path::new(&out);
            match zorcc::backend::build(
                &c.prog,
                &c.types,
                source,
                output,
                BuildOptions { opt, debug, overflow },
            ) {
                Ok(()) => exit(0),
                Err(e) => {
                    eprintln!("{}", e.render(source));
                    exit(e.rc());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_cli, Cli, OptLevel, OverflowMode, HELP};

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn build(src: &str, out: &str, opt: OptLevel, debug: bool, overflow: OverflowMode) -> Cli {
        Cli::Build {
            src: src.to_string(),
            out: out.to_string(),
            opt,
            debug,
            overflow,
            diag_json: false,
        }
    }

    /// L2e：六档 `-O*`、`-g` 与内部 `-Zoverflow=*` 在检查模式逐枚接受并忽略；
    /// `--version`／`-V` 短路先于文件处理。
    #[test]
    fn c1_whitelisted_flags_still_accepted_and_rc_path_unchanged() {
        for f in [
            "-O0",
            "-O1",
            "-O2",
            "-O3",
            "-Os",
            "-Og",
            "-Zoverflow=check",
            "-Zoverflow=wrap",
            "-g",
        ] {
            assert_eq!(
                parse_cli(&v(&[f, "a.zor"])),
                Ok(Cli::Compile("a.zor".to_string(), false)),
                "在册注入位 {f} 不得被拒判（拒了 CI 七段当场全红）"
            );
        }
        assert!(matches!(parse_cli(&v(&["--version"])), Ok(Cli::Version)));
        assert!(matches!(
            parse_cli(&v(&["-O2", "-V", "a.zor"])),
            Ok(Cli::Version)
        ));
    }

    /// 形①（D-020-9 允许更新的 2 条在册测试之一）：D-020-1 A 后 `-o` 正形进入
    /// 构建模式（移入新增 form1a）；`--output`／`--output=`／`-o` 缺值仍指名拒判。
    /// 旧断言：三种位置的 `-o` + `--output` + `--output=` + `-o` 缺值全部 Err；
    /// 新断言：仅保留后者，`-o` 正形不再属于本测试的拒判面。
    #[test]
    fn form1_output_flag_rejected_at_any_position() {
        let e = parse_cli(&v(&["--output", "out.bin", "a.zor"])).unwrap_err();
        assert!(e.contains("--output") && e.contains("只接受 '-o'"), "{e}");

        let e = parse_cli(&v(&["a.zor", "--output=out.bin"])).unwrap_err();
        assert!(e.contains("--output="), "{e}");

        let e = parse_cli(&v(&["a.zor", "-o"])).unwrap_err();
        assert!(e.contains("-o") && e.contains("缺值"), "{e}");
    }

    /// D-020-9 新增：`-o` 任意位置进入构建模式；`-O0`／`-g` 实映射
    #[test]
    fn form1a_output_flag_selects_build_mode() {
        for argv in [
            v(&["-o", "out.bin", "a.zor"]),
            v(&["a.zor", "-o", "out.bin"]),
            v(&["-O2", "-o", "out.bin", "a.zor"]),
            v(&["-O2", "a.zor", "-o", "out.bin"]),
        ] {
            assert_eq!(
                parse_cli(&argv),
                Ok(build("a.zor", "out.bin", OptLevel::O2, false, OverflowMode::Wrap)),
                "{argv:?}"
            );
        }
        assert_eq!(
            parse_cli(&v(&["-O0", "a.zor", "-o", "out.bin"])),
            Ok(build("a.zor", "out.bin", OptLevel::O0, false, OverflowMode::Check))
        );
        for (f, opt, ov) in [
            ("-O1", OptLevel::O1, OverflowMode::Check),
            ("-O3", OptLevel::O3, OverflowMode::Wrap),
            ("-Os", OptLevel::Os, OverflowMode::Wrap),
            ("-Og", OptLevel::Og, OverflowMode::Check),
        ] {
            assert_eq!(
                parse_cli(&v(&[f, "a.zor", "-o", "out.bin"])),
                Ok(build("a.zor", "out.bin", opt, false, ov)),
                "{f}"
            );
        }
        assert_eq!(
            parse_cli(&v(&["-g", "a.zor", "-o", "out.bin"])),
            Ok(build("a.zor", "out.bin", OptLevel::O2, true, OverflowMode::Wrap))
        );
    }

    /// D-020-9 新增：`-o` 空值与重复指名拒判（rc=2 路径）
    #[test]
    fn form1b_output_flag_misuse_is_named_rejection() {
        let e = parse_cli(&v(&["-o", "", "a.zor"])).unwrap_err();
        assert!(e.contains("为空"), "{e}");

        let e = parse_cli(&v(&["-o", "a", "-o", "b", "x.zor"])).unwrap_err();
        assert!(e.contains("重复"), "{e}");
    }

    /// 形②：未列入 L2e 白名单的任意 flag 一律拒判；非法 `-O*` 逐 token 拒判。
    #[test]
    fn form2_unknown_flag_rejected() {
        for f in [
            "--emit-wasm=nonsense",
            "-O4",
            "-O9",
            "-Ofast",
            "-O2x",
            "-O",
            "-Oz",
            "-O10",
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
        assert!(matches!(parse_cli(&v(&["-o", "out.bin"])), Ok(Cli::Usage)));
    }

    /// L2b：非法 `-Zoverflow` 值一律 rc=2，且保留“未知选项＋仅接受”指名句。
    #[test]
    fn zoverflow_invalid_values_are_named_rc2_path() {
        for f in [
            "-Zoverflow",
            "-Zoverflow=",
            "-Zoverflow=CHECK",
            "-Zoverflow=Wrap",
            "-Zoverflow=check=wrap",
            "-Zoverflow=bogus",
        ] {
            let e = parse_cli(&v(&["a.zor", "-o", "out", f])).unwrap_err();
            assert!(e.contains("未知选项") && e.contains(f), "{f} → {e}");
            assert!(e.contains("-Zoverflow") && e.contains("check") && e.contains("wrap"), "{f} → {e}");
        }
        // 非法值先出现时也不被后值覆盖
        let e = parse_cli(&v(&["-Zoverflow=bogus", "-Zoverflow=check", "a.zor"])).unwrap_err();
        assert!(e.contains("-Zoverflow"), "{e}");
    }

    /// L2b：合法重复 last-wins；显式覆盖与顺序无关；-O0/-O2 重复 last-wins。
    #[test]
    fn zoverflow_precedence_and_duplicate_semantics() {
        assert_eq!(
            parse_cli(&v(&["-Zoverflow=check", "-Zoverflow=wrap", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O2, false, OverflowMode::Wrap))
        );
        assert_eq!(
            parse_cli(&v(&["-Zoverflow=wrap", "-Zoverflow=check", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O2, false, OverflowMode::Check))
        );
        assert_eq!(
            parse_cli(&v(&["-Zoverflow=check", "-O2", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O2, false, OverflowMode::Check))
        );
        assert_eq!(
            parse_cli(&v(&["-O0", "-Zoverflow=wrap", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O0, false, OverflowMode::Wrap))
        );
        assert_eq!(
            parse_cli(&v(&["-O0", "-O2", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O2, false, OverflowMode::Wrap))
        );
        assert_eq!(
            parse_cli(&v(&["-O2", "-O0", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O0, false, OverflowMode::Check))
        );
        assert_eq!(
            parse_cli(&v(&["-O1", "-O3", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O3, false, OverflowMode::Wrap))
        );
        assert_eq!(
            parse_cli(&v(&["-O3", "-O1", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O1, false, OverflowMode::Check))
        );
        assert_eq!(
            parse_cli(&v(&["-Og", "-O2", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O2, false, OverflowMode::Wrap))
        );
        assert_eq!(
            parse_cli(&v(&["-O2", "-Og", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::Og, false, OverflowMode::Check))
        );
        assert_eq!(
            parse_cli(&v(&["-O1", "-Zoverflow=wrap", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O1, false, OverflowMode::Wrap))
        );
        assert_eq!(
            parse_cli(&v(&["-Og", "-Zoverflow=wrap", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::Og, false, OverflowMode::Wrap))
        );
        assert_eq!(
            parse_cli(&v(&["-O3", "-Zoverflow=check", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::O3, false, OverflowMode::Check))
        );
        assert_eq!(
            parse_cli(&v(&["-Os", "-Zoverflow=check", "a.zor", "-o", "out"])),
            Ok(build("a.zor", "out", OptLevel::Os, false, OverflowMode::Check))
        );
    }

    /// L2e：白名单扩为六档；每档缺省 overflow 按产品矩阵；非法后缀仍 rc=2。
    #[test]
    fn l2e_opt_whitelist_expands_to_six() {
        for (f, opt, ov) in [
            ("-O0", OptLevel::O0, OverflowMode::Check),
            ("-O1", OptLevel::O1, OverflowMode::Check),
            ("-O2", OptLevel::O2, OverflowMode::Wrap),
            ("-O3", OptLevel::O3, OverflowMode::Wrap),
            ("-Os", OptLevel::Os, OverflowMode::Wrap),
            ("-Og", OptLevel::Og, OverflowMode::Check),
        ] {
            assert_eq!(
                parse_cli(&v(&[f, "a.zor", "-o", "out"])),
                Ok(build("a.zor", "out", opt, false, ov)),
                "{f}"
            );
        }
        for f in ["-O4", "-O9", "-Ofast", "-O2x", "-O", "-Oz", "-O10"] {
            let e = parse_cli(&v(&[f, "-Zoverflow=check", "a.zor", "-o", "out"])).unwrap_err();
            assert!(e.contains("未知选项") && e.contains(f), "{f} → {e}");
        }
    }

    /// 版本短路在一切校验之前；L2b 不改 0.1.2 在册语义。
    #[test]
    fn version_short_circuit_precedes_zoverflow_validation() {
        assert!(matches!(
            parse_cli(&v(&["-V", "-Zoverflow=bogus"])),
            Ok(Cli::Version)
        ));
    }

    /// L2e：`--help` 任意位置短路，且优先于非法 flag／`-o` 缺值／不存在的源。
    #[test]
    fn help_short_circuits_anywhere_and_precedes_other_parse() {
        for argv in [
            v(&["--help"]),
            v(&["--help", "a.zor"]),
            v(&["a.zor", "--help"]),
            v(&["-o", "--help", "a.zor"]),
            v(&["--help", "--bogus"]),
            v(&["--help", "-o"]),
            v(&["--help", "missing.zor"]),
            v(&["--diag=json", "--help"]),
        ] {
            assert_eq!(parse_cli(&argv), Ok(Cli::Help), "{argv:?}");
        }
    }

    /// L2e：`--version`／`-V` 优先于 `--help`（0.1.2 短路契约不变）。
    #[test]
    fn version_precedes_help() {
        assert_eq!(parse_cli(&v(&["--version", "--help"])), Ok(Cli::Version));
        assert_eq!(parse_cli(&v(&["--help", "-V"])), Ok(Cli::Version));
    }

    /// L2e：`--help` 仅长旗标；`-h`／拼写变体仍 rc=2 指名拒判。
    #[test]
    fn help_is_long_flag_only() {
        for f in ["-h", "-help", "--help=1", "--Help"] {
            let e = parse_cli(&v(&[f, "a.zor"])).unwrap_err();
            assert!(e.contains("未知选项") && e.contains(f), "{f} → {e}");
        }
    }

    /// L2e：固定帮助文本的字节级约束（15 行／706 B；无 CR／内部位／`-h`）。
    #[test]
    fn help_text_is_fixed_contract_bytes() {
        assert_eq!(HELP.len(), 706);
        assert_eq!(HELP.lines().count(), 15);
        assert!(HELP.ends_with('\n'));
        assert!(!HELP.contains('\r'));
        assert!(!HELP.contains("-Zoverflow"));
        assert!(!HELP.contains("\n  -h "));
        assert!(HELP.contains("  -O0               no optimization; checked overflow\n"));
        assert!(HELP.contains("  -O1               basic optimization; checked overflow\n"));
        assert!(HELP.contains(
            "  -O2               standard optimization; wrapping overflow (default)\n"
        ));
        assert!(HELP.contains("  -O3               aggressive optimization; wrapping overflow\n"));
        assert!(HELP.contains("  -Os               optimize for size; wrapping overflow\n"));
        assert!(HELP.contains("  -Og               debug-friendly optimization; checked overflow\n"));
        assert!(HELP.contains("  --diag=json       compile diagnostics as NDJSON on stderr\n"));
        assert!(HELP.contains("  --version, -V     print version and exit\n"));
        assert!(HELP.contains("  --help            print this help and exit\n"));
    }

    /// C-2：CLI 面诊断不占语言诊断码位（红线 2），也不写成 E1xxx/E2xxx 串。
    /// D-020-9 允许更新：`-o` 正形已改走构建路径，坏例替换为缺值／空值／重复。
    #[test]
    fn c2_cli_diagnostics_allocate_no_language_codes() {
        for bad in [
            v(&["a.zor", "-o"]),
            v(&["-o", "", "a.zor"]),
            v(&["-o", "a", "-o", "b", "x.zor"]),
            v(&["a.zor", "--bogus"]),
            v(&["a.zor", "b.zor"]),
        ] {
            let e = parse_cli(&bad).unwrap_err();
            assert!(
                !e.contains("E1") && !e.contains("E2") && !e.contains("E3"),
                "CLI 句不得携语言诊断码位: {e}"
            );
        }
    }

    /// 单文件正形仍走 Compile（回归：不改变门禁调用形）
    #[test]
    fn single_source_still_compiles_through() {
        assert_eq!(
            parse_cli(&v(&[
                "-Zoverflow=check",
                "tests/specs/positive/r1-01-prim-declare.zor"
            ])),
            Ok(Cli::Compile(
                "tests/specs/positive/r1-01-prim-declare.zor".to_string(),
                false
            ))
        );
    }

    /// L2c：`--diag=json` 进入编译／构建路径；其余 `--diag` 值仍 rc=2 文本。
    #[test]
    fn diag_json_is_accepted_and_other_values_rejected() {
        assert_eq!(
            parse_cli(&v(&["--diag=json", "a.zor"])),
            Ok(Cli::Compile("a.zor".to_string(), true))
        );
        assert_eq!(
            parse_cli(&v(&["a.zor", "-o", "out", "--diag=json"])),
            Ok(Cli::Build {
                src: "a.zor".to_string(),
                out: "out".to_string(),
                opt: OptLevel::O2,
                debug: false,
                overflow: OverflowMode::Wrap,
                diag_json: true,
            })
        );
        for f in ["--diag", "--diag=text", "--diag=", "--diag=jsonl"] {
            let e = parse_cli(&v(&["a.zor", f])).unwrap_err();
            assert!(e.contains("未知选项") && e.contains("--diag"), "{f} → {e}");
        }
    }

    /// L2c：版本短路优先于 `--diag=json`／未知 flag／不存在源。
    #[test]
    fn version_wins_over_diag_json_and_bad_flags() {
        assert!(matches!(
            parse_cli(&v(&["--version", "--diag=json", "missing.zor", "--bogus"])),
            Ok(Cli::Version)
        ));
        assert!(matches!(
            parse_cli(&v(&["-V", "--diag=text"])),
            Ok(Cli::Version)
        ));
    }
}
