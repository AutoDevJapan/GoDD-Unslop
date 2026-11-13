//! Offline process fixture: checks the trusted Codex invocation without network access.
use std::{env, io::{self, Read}, process};

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["login", "status"] {
        println!("Logged in using ChatGPT");
        return;
    }
    let pair = |key: &str, value: &str| args.windows(2).any(|p| p[0] == key && p[1] == value);
    let checks = [
        ("exec", args.first().map(String::as_str) == Some("exec")),
        ("model", pair("--model", "gpt-6.1-sol") || pair("-m", "gpt-6.1-sol")),
        ("effort", pair("-c", "model_reasoning_effort=\"max\"")),
        ("tier", pair("-c", "service_tier=\"priority\"")),
        ("login", pair("-c", "forced_login_method=\"chatgpt\"")),
        ("provider", pair("-c", "model_provider=\"openai\"")),
        ("shell", pair("-c", "features.shell_tool=false")),
        ("web", pair("-c", "web_search=\"disabled\"")),
        ("sandbox", pair("--sandbox", "read-only")),
        ("config", args.iter().any(|a| a == "--ignore-user-config")),
        ("ephemeral", args.iter().any(|a| a == "--ephemeral")),
        ("json", args.iter().any(|a| a == "--json")),
        ("stdin", args.last().map(String::as_str) == Some("-")),
    ];
    if let Some((name, _)) = checks.iter().find(|(_, ok)| !ok) {
        if let Some(path) = env::var_os("UNSLOP_TEST_CONTRACT_FAILURE") {
            std::fs::write(path, name).unwrap();
        }
        eprintln!("rewrite invocation contract missing: {name}");
        process::exit(7);
    }
    let mut prompt = String::new();
    io::stdin().read_to_string(&mut prompt).unwrap();
    if !prompt.contains("保存しました。") { process::exit(8); }
    println!(r#"{{"type":"item.completed","item":{{"type":"agent_message","text":"保存しました。"}}}}"#);
}
