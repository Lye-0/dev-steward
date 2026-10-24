// Read-only GitHub API fixture. Unexpected commands (including clone/write) fail.
use std::env;
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("api")
        || !args.windows(2).any(|a| a == ["--method", "GET"])
    {
        std::process::exit(9);
    }
    let endpoint = args
        .iter()
        .find(|a| a.starts_with("repos/") || *a == "user/repos")
        .map(String::as_str)
        .unwrap_or_default();
    let root = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let a = std::fs::read_to_string(root.join("fixed-head"))
        .unwrap_or_else(|_| "a".repeat(40))
        .trim()
        .to_string();
    let b = "b".repeat(40);
    let tree = "c".repeat(40);
    if endpoint == "user/repos" {
        println!(
            r#"[{{"full_name":"Example/monorepo","html_url":"https://github.example.invalid/Example/monorepo","archived":false,"default_branch":"main","description":"Read-only fixture"}},{{"full_name":"Example/denied","html_url":"https://github.example.invalid/Example/denied","archived":false,"default_branch":"main","description":"Failure fixture"}}]"#
        );
        return;
    }
    if endpoint.starts_with("repos/Example/denied") {
        eprintln!("access denied");
        std::process::exit(4);
    }
    match endpoint {
        "repos/Example/monorepo" => println!(
            r#"{{"full_name":"Example/monorepo","html_url":"https://github.example.invalid/Example/monorepo","default_branch":"main"}}"#
        ),
        "repos/Example/monorepo/commits" => println!(
            r#"[{{"sha":"{}","commit":{{"tree":{{"sha":"{}"}}}}}}]"#,
            if root.join("head-moved").exists() {
                b
            } else {
                a
            },
            tree
        ),
        endpoint if endpoint.contains("/git/trees/") => {
            let files = [
                ("package.json", '1'),
                ("pnpm-lock.yaml", '2'),
                ("mise.toml", '3'),
                ("apps/web/package.json", '4'),
                (".github/workflows/ci.yml", '5'),
                (".npmrc", '6'),
                ("apps/api/.python-version", '7'),
            ];
            println!(r#"{{"sha":"{}","truncated":false,"tree":[{}]}}"#,tree,files.iter().map(|(path,id)|format!(r#"{{"path":"{path}","sha":"{}","size":300,"type":"blob","mode":"100644"}}"#,id.to_string().repeat(40))).collect::<Vec<_>>().join(","));
        }
        endpoint if endpoint.contains("/git/blobs/") => {
            let key = endpoint.rsplit('/').next().unwrap().chars().next().unwrap();
            print!(
                "{}",
                match key {
                    '1' =>
                        r#"{"name":"root-fixture","packageManager":"pnpm@11.28.8","dependencies":{"kleur":"4.1.4"},"scripts":{"test":"DO NOT EXECUTE"}}"#,
                    '2' =>
                        "lockfileVersion: '9.0'\nimporters:\n  .:\n    dependencies:\n      kleur:\n        specifier: 4.1.4\n        version: 4.1.4\n  apps/web:\n    dependencies:\n      kleur:\n        specifier: ^4.1.4\n        version: 4.1.4\npackages:\n  kleur@4.1.4: {}\n",
                    '3' => "[tools]\nnode='22.0.0'\npnpm='11.28.8'\n",
                    '4' =>
                        r#"{"name":"web-fixture","dependencies":{"kleur":"^4.1.4"},"scripts":{"build":"DO NOT EXECUTE"}}"#,
                    '5' =>
                        "name: Verify\non: push\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v7\n",
                    '6' =>
                        "registry=https://registry.npmjs.org\n//registry.npmjs.org/:_authToken=never-persist-this-secret\n",
                    '7' => "3.12.15\n",
                    _ => std::process::exit(9),
                }
            );
        }
        _ => std::process::exit(9),
    }
}
