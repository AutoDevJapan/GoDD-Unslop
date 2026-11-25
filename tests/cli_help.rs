#![cfg(feature = "mcp")]

use std::process::Command;

#[test]
fn help_explains_exit_codes() {
    let output = Command::new(env!("CARGO_BIN_EXE_godd-unslop"))
        .arg("--help")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout.lines().skip_while(|line| *line != "終了コード");
    assert_eq!(lines.next(), Some("終了コード"));

    let expected = [
        "  0  正常に終了しました。",
        "  1  --strict を指定したときに、指摘または比較の差分がありました。",
        "  2  入力に誤りがあるか、実行に失敗しました。",
    ];
    for line in expected {
        assert_eq!(lines.next(), Some(line));
    }
}
