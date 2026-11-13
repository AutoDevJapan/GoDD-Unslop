//! Offline process fixture: checks the trusted Codex invocation without network access.
use std::{env, io::{self, Read}, process};

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["login", "status"] {
        println!("Logged in using ChatGPT");
        return;
    }
    let pair = |key: &str, value: &str| args.windows(2).any(|p| p[0] == key && p[1] == value);
    if args.first().map(String::as_str) != Some("exec")
        || !pair("--model", "gpt-6.1-sol")
        || !pair("-c", "model_reasoning_effort=\"max\"")
        || !pair("-c", "service_tier=\"fast\"")
        || !pair("-c", "forced_login_method=\"chatgpt\"")
        || !pair("-c", "model_provider=\"openai\"")
        || !pair("-c", "features.shell_tool=false")
        || !pair("-c", "web_search=\"disabled\"")
        || !pair("--sandbox", "read-only")
        || !args.iter().any(|a| a == "--ignore-user-config")
        || !args.iter().any(|a| a == "--ephemeral")
        || !args.iter().any(|a| a == "--json")
        || args.last().map(String::as_str) != Some("-")
    {
        eprintln!("rewrite invocation contract missing");
        process::exit(7);
    }
    let mut prompt = String::new();
    io::stdin().read_to_string(&mut prompt).unwrap();
    if !prompt.contains("保存しました。") { process::exit(8); }
    println!(r#"{{"type":"item.completed","item":{{"type":"agent_message","text":"保存しました。"}}}}"#);
}
