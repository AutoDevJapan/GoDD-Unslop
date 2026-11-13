#![cfg(feature = "mcp")]
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

fn cli(args: &[&str], text: &[u8]) -> std::process::Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_godd-unslop"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process.stdin.take().unwrap().write_all(text).unwrap();
    process.wait_with_output().unwrap()
}

#[test]
fn rewrite_sends_explicit_model_effort_and_speed_to_the_trusted_process() {
    let (root, fixture) = codex_fixture();
    let _cleanup = Cleanup(root.clone());
    let result = rewrite_with_fixture(&fixture, &root, "none");
    assert!(
        result.status.success(),
        "contract field: {}; {}",
        std::fs::read_to_string(root.join("missing-contract"))
            .unwrap_or_else(|_| "none reported".into()),
        String::from_utf8_lossy(&result.stderr)
    );
    let result: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(result["status"], "rewritten");
    assert_eq!(result["rewritten_text"], "保存しました。");
}

struct Cleanup(std::path::PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn codex_fixture() -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "unslop-model-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let fixture = root.join(if cfg!(windows) {
        "codex-contract.exe"
    } else {
        "codex-contract"
    });
    // Cargo test needs a Rust toolchain; RUSTC also supports a custom compiler path.
    let mut compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    compiler
        .args(["--edition=2024", "--crate-name", "codex_contract"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/codex_contract.rs"
        ))
        .arg("-o")
        .arg(&fixture);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        compiler.creation_flags(0x08000000);
    }
    let compiled = compiler
        .output()
        .expect("the contract fixture requires rustc on PATH or RUSTC");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    (root, fixture)
}

fn rewrite_with_fixture(
    fixture: &std::path::Path,
    root: &std::path::Path,
    mode: &str,
) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_godd-unslop"))
        .arg("rewrite")
        .env("GODD_UNSLOP_CODEX", fixture)
        .env("UNSLOP_TEST_WARNING_MODE", mode)
        .env("RUST_LOG", "off")
        .env(
            "UNSLOP_TEST_CONTRACT_FAILURE",
            root.join("missing-contract"),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"text":"\u4fdd\u5b58\u3057\u307e\u3057\u305f\u3002"}"#)
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn rewrite_cli_surfaces_only_known_runtime_warnings_without_changing_legacy_output() {
    let (root, fixture) = codex_fixture();
    let _cleanup = Cleanup(root.clone());
    #[derive(serde::Deserialize)]
    struct LegacyResult {
        status: String,
        rewritten_text: String,
        comparison: Value,
    }
    for (mode, ids) in [
        ("json", vec!["skills_context_budget"]),
        (
            "stderr",
            vec!["skills_scan_limit", "project_instructions_truncated"],
        ),
        (
            "mixed",
            vec![
                "skills_context_budget",
                "skills_scan_limit",
                "project_instructions_truncated",
            ],
        ),
        ("none", vec![]),
        ("unknown", vec![]),
        ("text", vec![]),
        ("spanned-stderr", vec!["project_instructions_truncated"]),
        ("filtered-stderr", vec!["project_instructions_truncated"]),
    ] {
        let output = rewrite_with_fixture(&fixture, &root, mode);
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        let warnings = result["runtime_warnings"]
            .as_array()
            .expect("runtime_warnings must reach the CLI result");
        assert_eq!(
            warnings
                .iter()
                .map(|v| v["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ids,
            "{mode}"
        );
        for warning in warnings {
            assert_eq!(warning.as_object().unwrap().len(), 2);
            assert!(warning["message"].as_str().unwrap().contains('。'));
        }
        assert!(
            !result["runtime_warnings"]
                .to_string()
                .contains("synthetic-secret")
        );
        assert!(!result["runtime_warnings"].to_string().contains("file:///"));
        let legacy: LegacyResult = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(legacy.status, "rewritten");
        assert!(legacy.rewritten_text.starts_with("保存しました。"));
        assert!(legacy.comparison.is_object());
    }
    for mode in ["fail", "empty"] {
        let output = rewrite_with_fixture(&fixture, &root, mode);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Exceeded skills"));
    }
}

#[test]
fn rewrite_runtime_warnings_reach_mcp_and_recipient_cli() {
    let (root, fixture) = codex_fixture();
    let _cleanup = Cleanup(root.clone());
    let db = root.join("synthetic-relationships.sqlite3");
    let relationship = serde_json::from_value(json!({
        "id":"synthetic-peer", "label":"架空の同僚", "confirmed":true,
        "source":"テスト用の設定", "context":{"channel":"chat"}
    }))
    .unwrap();
    godd_unslop::relationships::Store::open(&db, true)
        .unwrap()
        .put(&relationship)
        .unwrap();
    let mut recipient = Command::new(env!("CARGO_BIN_EXE_godd-unslop"))
        .args(["rewrite-for", "synthetic-peer"])
        .env("GODD_UNSLOP_DB", &db)
        .env("GODD_UNSLOP_CODEX", &fixture)
        .env("UNSLOP_TEST_WARNING_MODE", "mixed")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    recipient
        .stdin
        .take()
        .unwrap()
        .write_all("{\"text\":\"保存しました。\"}".as_bytes())
        .unwrap();
    let output = recipient.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let recipient_value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        recipient_value["runtime_warnings"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(recipient_value["rewritten_text"], "保存しました。");

    let mut server = Command::new(env!("CARGO_BIN_EXE_godd-unslop"))
        .arg("serve")
        .env("GODD_UNSLOP_CODEX", &fixture)
        .env("UNSLOP_TEST_WARNING_MODE", "mixed")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = server.stdin.take().unwrap();
    let mut output = BufReader::new(server.stdout.take().unwrap());
    let send = |input: &mut std::process::ChildStdin, v: Value| {
        writeln!(input, "{v}").unwrap();
        input.flush().unwrap();
    };
    let receive = |output: &mut BufReader<std::process::ChildStdout>| {
        let mut line = String::new();
        assert!(output.read_line(&mut line).unwrap() > 0);
        serde_json::from_str::<Value>(&line).unwrap()
    };
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"synthetic-runtime-warning-test","version":"1"}}}),
    );
    assert!(receive(&mut output)["result"]["capabilities"].is_object());
    send(
        &mut input,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"unslop_rewrite","arguments":{"text":"保存しました。"}}}),
    );
    let response = receive(&mut output);
    assert_ne!(response["result"]["isError"], true, "{response}");
    let mcp_value: Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        mcp_value["runtime_warnings"],
        recipient_value["runtime_warnings"]
    );
    assert_eq!(mcp_value["rewritten_text"], "保存しました。");
    drop(input);
    assert!(server.wait().unwrap().success());
}

#[test]
fn cli_uses_real_exit_codes_and_rejects_non_utf8() {
    assert_eq!(
        cli(&["analyze", "--strict"], "確認しました。".as_bytes())
            .status
            .code(),
        Some(0)
    );
    assert_eq!(
        cli(&["analyze", "--strict"], "既定です。".as_bytes())
            .status
            .code(),
        Some(1)
    );
    assert_eq!(cli(&["analyze"], &[0xff]).status.code(), Some(2));
    assert_eq!(cli(&["features", "--catalog"], &[]).status.code(), Some(0));
    assert_eq!(cli(&["analyze", "--catalog"], &[]).status.code(), Some(2));
    assert_eq!(
        cli(&["analyze", "missing-test-input"], &[]).status.code(),
        Some(2)
    );
    let parsed: Value =
        serde_json::from_slice(&cli(&["features"], "資料を保存しました。".as_bytes()).stdout)
            .unwrap();
    assert!(parsed["metrics"]["visible_characters"].as_u64().unwrap() > 0);
}

#[test]
fn mcp_handshake_and_every_tool_use_the_shipped_process() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_godd-unslop"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let send = |input: &mut std::process::ChildStdin, v: Value| {
        writeln!(input, "{v}").unwrap();
        input.flush().unwrap();
    };
    let receive = |output: &mut BufReader<std::process::ChildStdout>| {
        let mut line = String::new();
        assert!(output.read_line(&mut line).unwrap() > 0);
        serde_json::from_str::<Value>(&line).unwrap()
    };
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"1"}}}),
    );
    assert!(receive(&mut output)["result"]["capabilities"]["tools"].is_object());
    send(
        &mut input,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    );
    assert_eq!(
        receive(&mut output)["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    for (name, arguments) in [
        ("unslop_analyze", json!({"text":"既定です。"})),
        (
            "unslop_compare",
            json!({"original":"3回です。","rewrite":"5回です。"}),
        ),
        (
            "unslop_plan",
            json!({"text":"修正しました。","context":"作業の報告"}),
        ),
        ("unslop_features", json!({"text":"保存しました。"})),
        (
            "unslop_style_learn",
            json!({"examples":[{"before":"保存。確認。","after":"保存。\n確認。"}]}),
        ),
        (
            "unslop_style_review",
            json!({"text":"保存。確認。","profile":{"version":1,"rules":[]}}),
        ),
    ] {
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
        );
        let response = receive(&mut output);
        assert_ne!(response["result"]["isError"], true, "{response}");
        assert!(
            serde_json::from_str::<Value>(
                response["result"]["content"][0]["text"].as_str().unwrap()
            )
            .is_ok()
        );
    }
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"unslop_analyze","arguments":{"text":5}}}),
    );
    assert_eq!(receive(&mut output)["result"]["isError"], true);
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"unknown","arguments":{}}}),
    );
    assert!(receive(&mut output)["error"].is_object());
    drop(input);
    assert!(child.wait().unwrap().success());
}

#[test]
fn cli_retains_corrections_between_explicit_calls() {
    let input = json!({"examples":[{"before":"結果を保存しました。次に確認します。","after":"結果を保存しました。\n\n次に確認します。"}]});
    let learned = cli(&["style-learn"], input.to_string().as_bytes());
    assert_eq!(learned.status.code(), Some(0));
    let output: Value = serde_json::from_slice(&learned.stdout).unwrap();
    let request =
        json!({"profile":output["profile"],"text":"結果を保存しました。次に確認します。"});
    let checked = cli(
        &["style-review", "--strict"],
        request.to_string().as_bytes(),
    );
    assert_eq!(checked.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert!(!report["findings"].as_array().unwrap().is_empty());
    assert_eq!(cli(&["style-learn"], b"[]").status.code(), Some(2));
}

#[test]
fn json_envelope_keeps_profile_separate_from_text_limit() {
    let learned = cli(
        &["style-learn"],
        json!({"examples":[{"before":"甲。乙。","after":"甲。\n乙。"}]})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(learned.status.code(), Some(0));
    let profile = serde_json::from_slice::<Value>(&learned.stdout).unwrap()["profile"].clone();
    let text = "あ".repeat(43000);
    let request = json!({"text":text,"profile":profile})
        .to_string()
        .replace('あ', "\\u3042");
    assert!(request.len() > godd_unslop::MAX_TEXT_BYTES);
    let reviewed = cli(&["style-review"], request.as_bytes());
    assert_eq!(
        reviewed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&reviewed.stderr)
    );
}
