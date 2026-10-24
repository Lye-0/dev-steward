// A standalone test executable, compiled only into the test's temporary directory.
// It emulates read-only manager queries without installing or changing a manager.
use std::{env, path::PathBuf};
fn escaped(path: PathBuf) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let root = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["list-remote"] => println!(
            "v22.2.0
v22.3.0 (LTS)
v24.1.0"
        ),
        ["install", "--list"] => println!(
            "  3.13.7
  3.13.8
  3.14.1"
        ),
        ["list", "--online", "--format=json", "3.14"] => {
            println!(r#"{{"versions":[{{"tag":"3.14","sort-version":"3.14.3"}}]}}"#)
        }
        ["search", "--json", "python"] => {
            println!(r#"{{"python":[{{"version":"3.12.8"}},{{"version":"3.12.9"}}]}}"#)
        }
        [
            "install",
            "--prefix",
            _,
            "--freeze-installed",
            "python=3.12.9",
            "--dry-run",
            "--json",
        ] => println!(
            r#"{{"success":true,"actions":{{"LINK":[{{"name":"python","version":"3.12.9"}}]}}}}"#
        ),
        [
            "list",
            "--id",
            "jdx.mise",
            "--exact",
            "--disable-interactivity",
        ] => println!("Name ID Version Source\nMise jdx.mise 2026.10.12 winget"),
        [
            "list",
            "--id",
            id,
            "--exact",
            "--source",
            "winget",
            "--disable-interactivity",
        ] => {
            if *id == "Example.App" {
                if root.join("manager-current").exists() {
                    println!("Name ID Version\nApp Example.App 1.1.0");
                } else {
                    println!(
                        "Name ID Version Available Source\nApp Example.App 1.0.0 1.1.0 winget"
                    );
                }
            } else if *id == "jdx.mise" {
                println!("Name ID Version Source\nMise jdx.mise 2026.10.12 winget");
            } else {
                std::process::exit(9);
            }
        }
        ["upgrade", "--disable-interactivity"] => println!(
            "Name ID Version Available Source
-----------------------------------------
Example App  Example.App  1.0.0  1.1.0  winget"
        ),
        [
            "show",
            "--id",
            "Example.App",
            "--exact",
            "--source",
            "winget",
            "--version",
            "1.1.0",
            "--disable-interactivity",
        ] => println!(
            "Example.App 1.1.0\n{}",
            "Release notes and installer metadata. ".repeat(100)
        ),
        ["--list-paths"] => {
            eprintln!(
                " -3.10-64 * {}",
                root.join("Legacy Python/python.exe").display()
            );
        }
        ["list", "--format=json"] => println!(
            r#"{{"versions":[{{"id":"cpython-314","tag":"3.14","sort-version":"3.14.2","executable":"{}"}}]}}"#,
            escaped(root.join("Managed Python/python.exe"))
        ),
        ["versions", "--bare"] => println!("3.13.7"),
        ["prefix", "3.13.7"] => println!("{}", root.join("pyenv versions/3.13.7").display()),
        ["env", "list", "--json"] => {
            println!(r#"{{"envs":["{}"]}}"#, escaped(root.join("Conda Env")))
        }
        ["list", "--prefix", _, "--json"] => println!(
            r#"[{{"name":"python","version":"3.12.8","channel":"https://token@private.invalid"}}]"#
        ),
        ["list"] => println!("* v22.2.0 default"),
        ["exec", "--using=22.2.0", "node", "-p", "process.execPath"] => {
            println!("{}", root.join("fnm Node/node.exe").display())
        }
        ["root"] => println!("Current Root: {}", root.join("nvm root").display()),
        _ => {
            eprintln!("Unexpected command");
            std::process::exit(9);
        }
    }
}
