//! 系统 C 编译器调用、临时目录与原子落盘。
//!
//! 安全纪律：`Command` 参数数组、不经 shell；临时件在目标同目录的独占临时目录内；
//! 成功后 rename 原子发布；任何失败清理临时目录且不改动既有目标。

use super::super::OptLevel;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn compile(
    c_source: &str,
    output: &Path,
    opt: OptLevel,
    debug: bool,
) -> Result<(), String> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(format!(
            "输出目录不存在：'{}'（不创建父目录）",
            parent.display()
        ));
    }
    if output.is_dir() {
        return Err(format!("输出路径是目录：'{}'", output.display()));
    }
    let tmpdir = create_tmpdir(parent)
        .map_err(|e| format!("无法在输出目录创建临时件：{}", e))?;
    let result = compile_in(&tmpdir, c_source, output, opt, debug);
    let _ = fs::remove_dir_all(&tmpdir);
    result
}

fn compile_in(
    tmpdir: &Path,
    c_source: &str,
    output: &Path,
    opt: OptLevel,
    debug: bool,
) -> Result<(), String> {
    let c_path = tmpdir.join("out.c");
    let bin_path = tmpdir.join("out.bin");
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&c_path)
            .map_err(|e| format!("无法写临时 C 源：{}", e))?;
        f.write_all(c_source.as_bytes())
            .map_err(|e| format!("无法写临时 C 源：{}", e))?;
        f.flush().map_err(|e| format!("无法写临时 C 源：{}", e))?;
    }

    let cc = cc_program();
    let mut cmd = Command::new(&cc);
    cmd.arg("-std=c11");
    cmd.arg(match opt {
        OptLevel::O0 => "-O0",
        OptLevel::O2 => "-O2",
    });
    if debug {
        cmd.arg("-g");
    }
    cmd.arg("-o").arg(&bin_path).arg(&c_path);

    let out = cmd
        .output()
        .map_err(|e| format!("无法执行 C 编译器 '{}'：{}", cc, e))?;
    if !out.status.success() {
        let code = out
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        let stderr = String::from_utf8_lossy(&out.stderr);
        let mut msg = format!("C 编译器 '{}' 失败（exit={}）", cc, code);
        let trimmed = stderr.trim();
        if !trimmed.is_empty() {
            msg.push_str("：");
            msg.push_str(&truncate(trimmed, 4000));
        }
        return Err(msg);
    }
    if !bin_path.is_file() {
        return Err(format!("C 编译器 '{}' 未产出可执行文件", cc));
    }
    fs::rename(&bin_path, output)
        .map_err(|e| format!("无法发布产物 '{}'：{}", output.display(), e))?;
    Ok(())
}

fn cc_program() -> String {
    match std::env::var("CC") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => "cc".to_string(),
    }
}

fn create_tmpdir(parent: &Path) -> std::io::Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    for _ in 0..64 {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = parent.join(format!(
            ".zorcc-tmp-{}-{}-{}",
            std::process::id(),
            nanos,
            seq
        ));
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "临时目录名冲突",
    ))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}
