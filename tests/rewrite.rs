use godd_unslop::rewrite::{finish, prepare};
use serde_json::json;
#[test]
fn rewrite_contract_and_meaning_warnings() {
    let request = json!({"text":"AutoDevJapanは3回失敗した場合だけ停止し、成功時は停止しない。","preserve":["AutoDevJapan"]});
    let prompt = prepare(&request).unwrap();
    assert!(prompt.contains("敬語"));
    assert!(prompt.contains("3回"));
    let result = finish(&request, "AutoDevJapanは5回失敗すると停止する。").unwrap();
    assert_eq!(result["original"], request["text"]);
    assert_eq!(result["comparison"]["numeric_fragments_changed"], true);
    assert_eq!(result["semantic_equality"], "not_established");
    assert!(!result["meaning_warnings"].as_array().unwrap().is_empty());
}
#[test]
fn reject_injection_fields_and_blank_model_output() {
    assert!(prepare(&json!({"text":"本文","executable":"evil"})).is_err());
    assert!(prepare(&json!({"text":"本文","profile":{"version":999,"rules":[]}})).is_err());
    assert!(finish(&json!({"text":"本文"}), " ").is_err());
}
#[test]
fn preserves_protected_regions_and_names() {
    let r =
        json!({"text":"名前はRe:X。`x = 3`\n\n[案内](https://example.test)","preserve":["Re:X"]});
    assert!(finish(&r, "名前は別名。`x = 5`\n\n[案内](https://other.test)").is_err());
}
#[test]
fn portable_skill_install_is_idempotent_and_preserves_existing() {
    let root = std::env::temp_dir().join(format!("unslop-skill-test-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let p = godd_unslop::skill::install(&root).unwrap();
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        godd_unslop::skill::CONTENT
    );
    assert_eq!(godd_unslop::skill::install(&root).unwrap(), p);
    std::fs::write(&p, "existing user's skill").unwrap();
    assert!(godd_unslop::skill::install(&root).is_err());
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        "existing user's skill"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn protected_code_whitespace_is_exact() {
    for (before, after) in [
        ("```rust\n    x = 3;\n```", "```rust\nx = 3;\n```"),
        ("`x = 3`", "`x  = 3`"),
        ("> first\n> second", "> first second"),
    ] {
        let request = json!({"text":before});
        assert!(finish(&request, before).is_ok());
        assert!(finish(&request, after).is_err(), "{before} -> {after}");
    }
}

#[test]
fn prompt_reuses_skill_layout_guidance_without_universal_rules() {
    let prompt = prepare(&json!({"text":"確認しました。次に進みます。"})).unwrap();
    let guidance = godd_unslop::skill::layout_guidance();
    assert!(
        prompt.contains(&guidance),
        "prompt must embed the Skill's layout section"
    );
    let skill = godd_unslop::skill::CONTENT.replace("\r\n", "\n");
    assert!(skill.contains(&guidance));
    // Ordinary line breaks and blank-line distance are separate concepts.
    for needle in [
        "改行は、同じ話題の中",
        "空行は、話題や段落が変わる距離",
        "原文に意図した改行や空行があれば尊重",
    ] {
        assert!(guidance.contains(needle), "{needle}");
    }
    // Real multi-line illustrations: blank line distance and ordinary newline both remain.
    assert!(guidance.contains("確認は終わり、結果に問題はありません。\n\n次の案件を説明します。\n担当者は火曜日までに資料を用意します。"));
    assert!(
        guidance
            .contains("料金は変わりません。\n\n解約は設定画面から行えます。\n\n不明な点があれば")
    );
    // Neither "always join" nor "break at every full stop" may be stated as a rule.
    for forbidden in [
        "句点ごとの機械的な分割や一律の空行追加はしない",
        "すべての句点で改行する",
        "全体を一文にまとめる",
        "必ず空行を入れる",
    ] {
        assert!(!prompt.contains(forbidden), "{forbidden}");
    }
}
#[test]
fn layout_warnings_distinguish_line_break_and_blank_line_distance() {
    let original = "確認は終わりました。\n結果に問題はありません。\n\n次の案件を説明します。\n担当者は火曜日までに資料を用意します。";
    let request = json!({"text":original});
    let layout = |rewritten: &str| {
        finish(&request, rewritten).unwrap()["layout_warnings"]
            .as_array()
            .unwrap()
            .len()
    };
    // Same line and paragraph structure after a wording change.
    assert_eq!(
        layout(
            "確認は終わり、結果に問題はありません。\n\n次の案件を説明します。\n担当者は火曜日までに資料を用意します。"
        ),
        0
    );
    // Ordinary newlines may be joined when the blank-line distance remains.
    assert_eq!(
        layout(
            "確認は終わり、結果に問題はありません。\n\n次の案件では、担当者が火曜日までに資料を用意します。"
        ),
        0
    );
    // The blank-line distance disappears.
    assert_eq!(
        layout(
            "確認は終わり、結果に問題はありません。\n次の案件を説明します。\n担当者は火曜日までに資料を用意します。"
        ),
        1
    );
    // Everything is compacted to one line.
    assert_eq!(
        layout(
            "確認は終わり、結果に問題はありません。次の案件では担当者が火曜日までに資料を用意します。"
        ),
        2
    );
    // A CRLF original is the same structure.
    let crlf = json!({"text":original.replace('\n', "\r\n")});
    assert!(
        finish(&crlf, original).unwrap()["layout_warnings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
/// Uses the operator's existing ChatGPT Codex login; set GODD_UNSLOP_CODEX if codex.exe is not on PATH.
#[test]
#[ignore = "calls the real Codex subscription login; run explicitly with --ignored"]
fn real_ai_keeps_paragraphs_and_does_not_compact_everything() {
    let cases = [
        "確認は終わりました。\n結果に問題はありません。\n\n次の案件を説明します。\n担当者は火曜日までに資料を用意します。",
        "料金は変わりません。\n\n解約は設定画面から行えます。\n\n不明な点は問い合わせ窓口へ連絡してください。",
        "保存先を変更しました。次回から新しいフォルダーへ保存します。既存のファイルは移動していません。",
    ];
    for text in cases {
        let result = godd_unslop::rewrite::run(&json!({"text":text})).unwrap();
        let out = result["rewritten_text"]
            .as_str()
            .unwrap()
            .replace("\r\n", "\n");
        eprintln!(
            "--- before\n{text}\n--- after\n{out}\n--- warnings {}",
            result["layout_warnings"]
        );
        let paragraphs = |s: &str| s.split("\n\n").filter(|p| !p.trim().is_empty()).count();
        assert!(
            paragraphs(&out) >= paragraphs(text),
            "paragraphs must not be merged: {out}"
        );
        if text.contains("\n\n") {
            assert!(out.contains("\n\n"), "blank-line distance lost: {out}");
        }
    }
}
