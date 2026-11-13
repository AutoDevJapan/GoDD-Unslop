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
        ("color", pair("--color", "never")),
        ("warning_filter", env::var("RUST_LOG").as_deref() == Ok("error,opentelemetry_sdk=off,opentelemetry_otlp=off,codex_core::agents_md=warn")),
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
    let mode = env::var("UNSLOP_TEST_WARNING_MODE").unwrap_or_default();
    if matches!(mode.as_str(), "json" | "mixed" | "fail" | "empty") {
        for _ in 0..2 {
            println!(r#"{{"type":"item.completed","item":{{"type":"error","message":"Exceeded skills context budget. All skill descriptions were removed and 399 additional skills were not included in the model-visible skills list."}}}}"#);
        }
        println!(r#"{{"type":"item.completed","item":{{"type":"error","message":"Exceeded skills context budget. Some skills were not included in the model-visible skills list."}}}}"#);
    }
    if matches!(mode.as_str(), "stderr" | "mixed") {
        for _ in 0..2 {
            eprintln!("2026-01-01T00:00:00Z ERROR codex_skills_extension::loader::host: skills scan reached its traversal limit (root: file:///private/synthetic-path-secret)");
            eprintln!("2026-01-01T00:00:00Z WARN codex_core::agents_md: project doc exceeds remaining budget; truncating path=file:///private/synthetic-path-secret remaining_bytes=32768");
        }
    }
    if mode == "filtered-stderr" && env::var("RUST_LOG").is_ok_and(|s| s.contains("codex_core::agents_md=warn")) {
        eprintln!("2026-01-01T00:00:00Z WARN codex_core::agents_md: project doc exceeds remaining budget; truncating path=file:///private/synthetic-secret remaining_bytes=32768");
    }
    if mode == "spanned-stderr" {
        eprintln!("2026-01-01T00:00:00Z WARN agents_md.load{{max_total=32768}}: codex_core::agents_md: project doc exceeds remaining budget; truncating path=file:///private/synthetic-secret remaining_bytes=32768");
    }
    if mode == "unknown" {
        println!(r#"{{"type":"item.completed","item":{{"type":"error","message":"unknown synthetic-secret diagnostic"}}}}"#);
        println!(r#"{{"type":"item.started","item":{{"type":"error","message":"Exceeded skills context budget. Some skills were not included in the model-visible skills list."}}}}"#);
        eprintln!("user text: skills scan reached its traversal limit (root: synthetic-secret)");
        eprintln!("2026-01-01 WARN other_module: project doc exceeds remaining budget; truncating synthetic-secret");
    }
    if mode == "fail" { process::exit(7); }
    if mode != "empty" {
        if mode == "text" {
            println!(r#"{{"type":"item.completed","item":{{"type":"agent_message","text":"保存しました。Exceeded skills context budget. Some skills were not included in the model-visible skills list."}}}}"#);
        } else {
            println!(r#"{{"type":"item.completed","item":{{"type":"agent_message","text":"保存しました。"}}}}"#);
        }
    }
    if mode == "mixed" {
        println!(r#"{{"type":"item.completed","item":{{"type":"error","message":"Exceeded skills context budget. All skill descriptions were removed and 1 additional skill was not included in the model-visible skills list."}}}}"#);
        eprintln!("unknown synthetic-secret stderr");
    }
}
