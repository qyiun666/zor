use std::io::Write;
use std::process::exit;

// 0.1.2 P3：--version/-V 独立版本串（版本源=Cargo.toml CARGO_PKG_VERSION）
fn wants_version(args: &[String]) -> bool {
    args.iter().any(|a| a == "--version" || a == "-V")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if wants_version(&args) {
        println!("zorcc {}", env!("CARGO_PKG_VERSION"));
        exit(0);
    }
    let mut file: Option<String> = None;
    // 0.1.x：flag（-O0/-O2/-Zoverflow=check|wrap 等，CI 双模式注入）接受并忽略，仅取第一个位置参数为源文件
    for a in args {
        if a.starts_with('-') {
            continue;
        }
        if file.is_none() {
            file = Some(a);
        }
    }
    let Some(path) = file else {
        eprintln!("usage: zorcc [flags] <file.zor>");
        exit(2);
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
    use super::wants_version;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn version_flag_short_circuits_before_file_handling() {
        assert!(wants_version(&v(&["--version"])));
        assert!(wants_version(&v(&["-O2", "-V", "a.zor"])));
        assert!(!wants_version(&v(&["-O2", "-Zoverflow=check", "a.zor"])));
        assert!(!wants_version(&v(&[])));
    }
}
