#![cfg(feature = "mcp")]
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
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
    let result = rewrite_with_fixture(&fixture, &root, "capture-prompt");
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

    let received = std::fs::read_to_string(root.join("rewrite-prompt.txt")).unwrap();
    let input = json!({"action":"rewrite","text":"保存しました。"});
    assert_eq!(received, godd_unslop::rewrite::prepare(&input).unwrap());
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
    let receive = |output: &mut BufReader<std::process::ChildStdout>, expected_id: u64| {
        let mut line = String::new();
        assert!(output.read_line(&mut line).unwrap() > 0);
        let response = serde_json::from_str::<Value>(&line).unwrap();
        assert_eq!(response["id"], expected_id);
        response
    };
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"synthetic-runtime-warning-test","version":"1"}}}),
    );
    assert!(receive(&mut output, 1)["result"]["capabilities"].is_object());
    send(
        &mut input,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"unslop_rewrite","arguments":{"text":"保存しました。"}}}),
    );
    let response = receive(&mut output, 2);
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
fn cli_reports_its_package_version_with_empty_stdin() {
    for flag in ["--version", "-V"] {
        let output = cli(&[flag], &[]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            concat!("godd-unslop ", env!("CARGO_PKG_VERSION"), "\n")
        );
    }
    for args in [["--version", "serve"], ["-V", "--json"]] {
        let output = cli(&args, &[]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn cli_reports_version_before_stdin_eof() {
    struct VersionProcess {
        child: std::process::Child,
        stdin: Option<std::process::ChildStdin>,
        reaped: bool,
    }
    impl Drop for VersionProcess {
        fn drop(&mut self) {
            if !self.reaped {
                drop(self.stdin.take());
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }

    let root = std::env::temp_dir().join(format!(
        "unslop-version-open-stdin-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let _cleanup = Cleanup(root.clone());
    let mut cases = vec![
        (vec![std::ffi::OsString::from("--version")], true),
        (vec![std::ffi::OsString::from("-V")], true),
        (
            vec![std::ffi::OsString::from("--version"), "serve".into()],
            false,
        ),
        (vec![std::ffi::OsString::from("-V"), "--json".into()], false),
    ];
    for flag in ["--version", "-V"] {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            cases.push((
                vec![flag.into(), std::ffi::OsString::from_vec(vec![0xff])],
                false,
            ));
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            cases.push((
                vec![flag.into(), std::ffi::OsString::from_wide(&[0xd800])],
                false,
            ));
        }
    }
    for (index, (args, version_expected)) in cases.into_iter().enumerate() {
        let flag = format!("{args:?}");
        let stdout_path = root.join(format!("stdout-{index}"));
        let stderr_path = root.join(format!("stderr-{index}"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_godd-unslop"));
        command
            .args(&args)
            .env("TOKIO_WORKER_THREADS", "0")
            .stdin(Stdio::piped())
            .stdout(std::fs::File::create_new(&stdout_path).unwrap())
            .stderr(std::fs::File::create_new(&stderr_path).unwrap());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut process = VersionProcess {
            child: command.spawn().unwrap(),
            stdin: None,
            reaped: false,
        };
        process.stdin = process.child.stdin.take();
        assert!(process.stdin.is_some());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let status = loop {
            assert!(
                std::fs::metadata(&stdout_path).unwrap().len() <= 65_536
                    && std::fs::metadata(&stderr_path).unwrap().len() <= 65_536,
                "{flag}: version output exceeded its limit"
            );
            if std::time::Instant::now() >= deadline {
                let killed = process.child.kill();
                drop(process.stdin.take());
                let collected = process.child.wait();
                process.reaped = collected.is_ok();
                panic!("{flag}: version waited for stdin EOF; kill={killed:?}; wait={collected:?}");
            }
            if let Some(status) = process.child.try_wait().unwrap() {
                process.reaped = true;
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        // Keep the owned write end open until exit and both output checks finish.
        assert!(process.stdin.is_some());
        let mut stdout = Vec::new();
        std::fs::File::open(&stdout_path)
            .unwrap()
            .take(65_537)
            .read_to_end(&mut stdout)
            .unwrap();
        let mut stderr = Vec::new();
        std::fs::File::open(&stderr_path)
            .unwrap()
            .take(65_537)
            .read_to_end(&mut stderr)
            .unwrap();
        assert!(stdout.len() <= 65_536 && stderr.len() <= 65_536);
        if version_expected {
            assert_eq!(status.code(), Some(0));
            assert!(stderr.is_empty());
            assert_eq!(
                String::from_utf8(stdout).unwrap(),
                concat!("godd-unslop ", env!("CARGO_PKG_VERSION"), "\n")
            );
        } else {
            assert_eq!(status.code(), Some(2));
            assert!(stdout.is_empty());
            assert!(!stderr.is_empty());
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cli_dispatch_compare_preserves_json_output_and_failure_exit() {
    let request = json!({"action":"compare","original":"3回です。","rewrite":"5回です。"});
    let output = cli(&["dispatch", "--strict"], request.to_string().as_bytes());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["changed"], true);
    assert_eq!(result["numeric_fragments_changed"], true);
    assert_eq!(result["numeric_fragments_original"], json!(["3回"]));
    assert_eq!(result["numeric_fragments_rewrite"], json!(["5回"]));
    assert_eq!(result["semantic_equality"], "not_established");
    let invalid = json!({"action":"compare","original":3,"rewrite":"5回です。"});
    let output = cli(&["dispatch"], invalid.to_string().as_bytes());
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
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

fn guide_example_json(args: &[&str]) -> Value {
    let output = cli(args, &[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{args:?} wrote to stderr");
    serde_json::from_slice(&output.stdout).expect("guide command must return one JSON value")
}

#[test]
fn cli_analyzes_and_compares_the_distributed_guide_texts() {
    const ARTICLE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/guide-examples/article.txt"
    );
    const REVISED: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/guide-examples/revised.txt"
    );
    let article = std::fs::read_to_string(ARTICLE).unwrap();
    let revised = std::fs::read_to_string(REVISED).unwrap();

    let term = "既定";
    let start = article.find(term).expect("article.txt must contain 既定");
    let end = start + term.len();
    let analysis = guide_example_json(&["analyze", ARTICLE]);
    let findings = analysis["findings"].as_array().unwrap();
    let found = findings.iter().any(|finding| {
        finding["rule"] == "reader.ambiguous_term"
            && finding["start"]["byte"] == start
            && finding["end"]["byte"] == end
            && finding["start"]["character"] == article[..start].chars().count()
            && finding["end"]["character"] == article[..end].chars().count()
    });
    assert!(
        found,
        "analyze must flag 既定 at bytes {start}..{end}: {analysis}"
    );

    let revised_analysis = guide_example_json(&["analyze", REVISED]);
    assert!(
        revised_analysis["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|finding| finding["rule"] != "reader.ambiguous_term"),
        "revised.txt still has an ambiguous term"
    );

    let comparison = guide_example_json(&["compare", ARTICLE, REVISED]);
    assert_eq!(comparison["changed"], true);
    assert_eq!(comparison["semantic_equality"], "not_established");
    let changed_byte = |position: &str| {
        let byte = comparison["changed_region"][position]["byte"]
            .as_u64()
            .unwrap();
        usize::try_from(byte).unwrap()
    };
    for (text, start, end, term) in [
        (&article, "original_start", "original_end", term),
        (&revised, "rewrite_start", "rewrite_end", "経費"),
    ] {
        assert!(
            text.get(changed_byte(start)..changed_byte(end))
                .is_some_and(|span| span.contains(term)),
            "compare must include {term} in the changed region"
        );
    }
    assert_eq!(
        guide_example_json(&["compare", ARTICLE, ARTICLE])["changed"],
        false
    );
    assert_eq!(std::fs::read_to_string(ARTICLE).unwrap(), article);
    assert_eq!(std::fs::read_to_string(REVISED).unwrap(), revised);
}

#[test]
fn cli_learns_and_reviews_the_distributed_style_examples() {
    const LEARN: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/guide-examples/learn-request.json"
    );
    const REVIEW: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/guide-examples/style-review-request.json"
    );
    let request: Value = serde_json::from_slice(&std::fs::read(REVIEW).unwrap()).unwrap();
    let learned = guide_example_json(&["style-learn", LEARN]);
    assert_eq!(learned["ignored_examples"], 0);
    assert_eq!(learned["profile"], request["profile"]);

    let report = guide_example_json(&["style-review", REVIEW]);
    let findings = report["findings"].as_array().unwrap();
    for (spacing, whitespace, blank_lines) in
        [("line_break", "\n", 0), ("paragraph_distance", "\n\n", 1)]
    {
        let choices = json!([{
            "spacing": spacing,
            "whitespace": whitespace,
            "blank_lines": blank_lines,
        }]);
        let found = findings.iter().any(|finding| {
            finding["kind"] == "layout_boundary" && finding["choices"] == choices
        });
        assert!(found, "style-review must report {spacing}");
    }
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
    let receive = |output: &mut BufReader<std::process::ChildStdout>, expected_id: u64| {
        let mut line = String::new();
        assert!(output.read_line(&mut line).unwrap() > 0);
        let response = serde_json::from_str::<Value>(&line).unwrap();
        assert_eq!(response["id"], expected_id);
        response
    };
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"1"}}}),
    );
    let init = receive(&mut output, 1);
    assert!(init["result"]["capabilities"]["tools"].is_object());
    send(
        &mut input,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    );
    assert_eq!(
        receive(&mut output, 2)["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    for (index, (name, arguments)) in [
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
    ]
    .into_iter()
    .enumerate()
    {
        let request_id = 3 + index as u64;
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":request_id,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
        );
        let response = receive(&mut output, request_id);
        assert_ne!(response["result"]["isError"], true, "{response}");
        let result: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        if name == "unslop_compare" {
            assert_eq!(result["changed"], true);
            assert_eq!(result["numeric_fragments_changed"], true);
            assert_eq!(result["numeric_fragments_original"], json!(["3回"]));
            assert_eq!(result["numeric_fragments_rewrite"], json!(["5回"]));
            assert_eq!(result["semantic_equality"], "not_established");
            let request = json!({"action":"compare","original":"3回です。","rewrite":"5回です。"});
            let output = cli(&["dispatch"], request.to_string().as_bytes());
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            let standalone: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(result, standalone);
        }
    }
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"unslop_analyze","arguments":{"text":5}}}),
    );
    assert_eq!(receive(&mut output, 9)["result"]["isError"], true);
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"unknown","arguments":{}}}),
    );
    assert!(receive(&mut output, 10)["error"].is_object());
    drop(input);
    assert!(child.wait().unwrap().success());

    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    let server_info = &init["result"]["serverInfo"];
    assert_eq!(server_info["name"], env!("CARGO_PKG_NAME"), "{server_info}");
    assert_eq!(server_info["version"], env!("CARGO_PKG_VERSION"));
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
