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
        4
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
