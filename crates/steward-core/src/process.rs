use crate::models::Environment;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
pub static CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone, serde::Serialize)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

pub fn run_raw(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    timeout: u64,
) -> Result<Output, String> {
    run_raw_input(program, args, cwd, timeout, None)
}
fn run_raw_input(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    timeout: u64,
    input: Option<&[u8]>,
) -> Result<Output, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
        .env("NO_COLOR", "1")
        .env("CI", "1")
        .env("GIT_TERMINAL_PROMPT", "0");
    hidden(&mut command);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| format!("{program}: {e}"))?;
    if let Some(bytes) = input {
        use std::io::Write;
        let mut stdin = child.stdin.take().ok_or("stdin unavailable")?;
        let bytes = bytes.to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let stdout = child.stdout.take().ok_or("stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("stderr unavailable")?;
    // Always drain pipes, but bound retained output. Avoid deadlocks on verbose package managers.
    fn drain(mut reader: impl Read) -> Vec<u8> {
        let mut result = Vec::new();
        let mut buf = [0; 8192];
        while let Ok(count) = reader.read(&mut buf) {
            if count == 0 {
                break;
            }
            let remaining = 2_000_000usize.saturating_sub(result.len());
            result.extend_from_slice(&buf[..count.min(remaining)]);
        }
        result
    }
    let out = thread::spawn(move || drain(stdout));
    let err = thread::spawn(move || drain(stderr));
    let start = Instant::now();
    let code = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status.code().unwrap_or(-1);
        }
        if start.elapsed() > Duration::from_secs(timeout)
            || CANCEL.load(std::sync::atomic::Ordering::Relaxed)
        {
            #[cfg(windows)]
            {
                let mut killer = Command::new("taskkill.exe");
                killer.args(["/PID", &child.id().to_string(), "/T", "/F"]);
                hidden(&mut killer);
                let _ = killer.output();
            }
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-TERM", "--", &format!("-{}", child.id())])
                    .output();
            }
            let _ = child.kill();
            let _ = child.wait();
            // Do not join pipe readers: descendants on Unix may still hold descriptors.
            return Err(format!(
                "{program}: {timeout}秒でタイムアウト。子プロセスの残存を確認してから再実行してください。"
            ));
        }
        thread::sleep(Duration::from_millis(40));
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    if stdout.len() >= 2_000_000 || stderr.len() >= 2_000_000 {
        return Err(format!(
            "{program}: 出力が2MBの上限に達したため結果を採用しません。処理が完了している場合もあるため、状態を確認してください。"
        ));
    }
    Ok(Output {
        code,
        stdout: decode(&stdout),
        stderr: decode(&stderr),
    })
}

pub fn decode(bytes: &[u8]) -> String {
    let utf16 = bytes.starts_with(&[0xff, 0xfe])
        || bytes
            .as_chunks::<2>()
            .0
            .iter()
            .take(60)
            .filter(|c| c[1] == 0)
            .count()
            > 2;
    if utf16 {
        let bytes = bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes);
        String::from_utf16_lossy(
            &bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(bytes)
            .trim_start_matches('\u{feff}')
            .to_string()
    }
}

pub fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

pub fn run(
    env: &Environment,
    program: &str,
    args: &[String],
    cwd: &str,
    timeout: u64,
) -> Result<Output, String> {
    if env.kind == "wsl" {
        let mut all = strings(&["--distribution", &env.name, "--cd", cwd, "--exec", program]);
        all.extend_from_slice(args);
        run_raw("wsl.exe", &all, None, timeout)
    } else {
        run_raw(program, args, Some(cwd), timeout)
    }
}

pub fn checked(
    env: &Environment,
    program: &str,
    args: &[&str],
    cwd: &str,
) -> Result<String, String> {
    let out = run(env, program, &strings(args), cwd, 40)?;
    if out.code != 0 {
        return Err(format!("{program} ({}): {}", out.code, out.stderr.trim()));
    }
    Ok(out.stdout.trim().to_string())
}

pub fn resolve(env: &Environment, name: &str) -> Option<String> {
    // Only fixed manager identifiers ever enter this shell fragment.
    if ![
        "mise",
        "uv",
        "rustup",
        "cargo",
        "git",
        "gh",
        "node",
        "pnpm",
        "npm",
        "winget",
        "dpkg-query",
        "brew",
    ]
    .contains(&name)
    {
        return None;
    }
    if env.kind == "wsl" {
        let script =
            format!("export PATH=\"$HOME/.local/bin:$HOME/.cargo/bin:$PATH\"; command -v {name}");
        return checked(env, "/bin/sh", &["-lc", &script], &env.home)
            .ok()
            .filter(|s| s.starts_with('/'));
    }
    let paths = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&paths) {
        #[cfg(windows)]
        let suffixes = [".exe", ""];
        #[cfg(not(windows))]
        let suffixes = [""];
        for suffix in suffixes {
            let path = dir.join(format!("{name}{suffix}"));
            if path.is_file() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    None
}

pub fn local_home() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}
pub fn read(env: &Environment, path: &str) -> Result<String, String> {
    if env.kind == "wsl" {
        checked(env, "/bin/cat", &["--", path], &env.home)
    } else {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.len() > 8_000_000 {
            return Err("8MBを超えるファイルは静的調査の対象外です".into());
        }
        std::fs::read_to_string(path)
            .map(|s| s.trim_start_matches('\u{feff}').to_string())
            .map_err(|e| e.to_string())
    }
}
pub fn join(base: &str, name: &str, env: &Environment) -> String {
    if env.kind == "wsl" {
        format!("{}/{}", base.trim_end_matches('/'), name)
    } else {
        Path::new(base).join(name).to_string_lossy().to_string()
    }
}

pub fn write_atomic(env: &Environment, path: &str, text: &str) -> Result<(), String> {
    let temporary = format!("{path}.steward-{}", crate::models::id());
    if env.kind == "wsl" {
        let args = strings(&[
            "--distribution",
            &env.name,
            "--cd",
            &env.home,
            "--exec",
            "/usr/bin/tee",
            "--",
            &temporary,
        ]);
        let out = run_raw_input("wsl.exe", &args, None, 20, Some(text.as_bytes()))?;
        if out.code != 0 {
            return Err(out.stderr);
        }
        checked(env, "/bin/mv", &["--", &temporary, path], &env.home)?;
    } else {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn wsl_utf16_names_are_preserved() {
        let input = "Ubuntu 日本語\r\n";
        let data = input
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(super::decode(&data), input);
    }
}
