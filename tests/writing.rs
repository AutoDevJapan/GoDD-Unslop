use godd_unslop::{MAX_TEXT_BYTES, analyze, compare, dispatch, plan};
use serde_json::json;

#[test]
fn feedback_has_original_unicode_location() {
    let text = "猫の写真を保存。\n既定の保存先です。";
    let result = analyze(text).unwrap();
    let hit = &result.findings[0];
    assert_eq!(hit.rule, "reader.ambiguous_term");
    assert_eq!(hit.start.line, 2);
    assert_eq!(hit.start.column, 1);
    assert_eq!(&text[hit.start.byte..hit.end.byte], "既定");
    assert_eq!(hit.start.character, 9);
}

#[test]
fn source_literals_are_not_reviewed_as_prose() {
    for text in [
        "```text\n既定 ローカル\n```",
        "~~~\nフォールバック\n~~~",
        "`既定`",
        "[保存先](https://example.test/既定)",
        "<https://example.test/ローカル>",
        "> 既定では見送り。",
        "「既定の保存先」について尋ねた。",
        "---\ntitle: 既定\n---\n保存しました。",
    ] {
        assert!(analyze(text).unwrap().findings.is_empty(), "{text}");
    }
}

#[test]
fn ordinary_contrast_and_concrete_work_remain_untouched() {
    for text in [
        "料金は500円です。担当者が明日までに確認します。",
        "速度より正確さを優先します。",
        "資料を作るのではなく、届いた資料を読みます。",
        "猫は庭を走っています。",
        "画面の幅は800ピクセルです。",
    ] {
        assert!(analyze(text).unwrap().findings.is_empty());
        assert_eq!(plan(text, "説明文").unwrap()["original"], text);
    }
}

#[test]
fn experience_rules_give_review_candidates() {
    for (text, expected) in [
        (
            "背景として、昨日の作業の流れを説明します。修正しました。",
            "reader.result_location",
        ),
        ("Q17です。", "reader.identifier_meaning"),
        (
            "通知送信処理実行完了確認結果保存処理を実施します。",
            "reader.noun_relationship",
        ),
        ("結果をご報告いたします。", "reader.action_visibility"),
        (
            "状態: 完了。\n確認テストはまだ実行していません。",
            "reader.status_alignment",
        ),
    ] {
        assert!(
            analyze(text)
                .unwrap()
                .findings
                .iter()
                .any(|f| f.rule == expected),
            "{expected}"
        );
    }
}

#[test]
fn parser_feature_counts_have_explicit_units() {
    let result = analyze("猫がいます。\n犬もいます。").unwrap();
    assert_eq!(result.metrics.visible_characters, 12);
    assert_eq!(result.metrics.sentence_lengths, vec![5, 5]);
    assert_eq!(result.metrics.kanji_count, 2);
    assert_eq!(result.metrics.mean_sentence_length, 5.0);
    let blocks = analyze("# 見出し\n\n- **記録**\n- 次の記録").unwrap();
    assert_eq!(blocks.metrics.headings, 1);
    assert_eq!(blocks.metrics.lists, 1);
    assert_eq!(blocks.metrics.strong_spans, 1);
}

#[test]
fn comparisons_never_certify_semantic_equivalence() {
    let result = compare("失敗した場合だけ、3回再送してください。", "5回再送します。").unwrap();
    assert_eq!(result["numeric_fragments_changed"], true);
    assert_eq!(result["listed_marker_counts_changed"], true);
    assert_eq!(result["semantic_equality"], "not_established");
    assert_eq!(result["changed_region"]["original_start"]["byte"], 0);
    let equal = compare("保存しました。", "保存しました。").unwrap();
    assert_eq!(equal["changed"], false);
    assert_eq!(equal["semantic_equality"], "not_established");
    let changed = compare("犬は3匹です。", "犬は5匹です。").unwrap();
    assert_eq!(changed["changed_region"]["original_start"]["column"], 3);
    assert_eq!(changed["changed_region"]["original_end"]["column"], 4);
    assert_eq!(compare("abc猫", "abc").unwrap()["changed"], true);
    assert_eq!(compare("", "猫").unwrap()["changed"], true);
}

#[test]
fn changed_exact_values_and_code_require_review() {
    for (before, after) in [
        ("`store(3)`", "`store(4)`"),
        ("https://example.test/a", "https://example.test/b"),
        ("担当者が承認する。", "承認者が担当する。"),
    ] {
        assert_eq!(compare(before, after).unwrap()["changed"], true);
        assert_eq!(
            compare(before, after).unwrap()["semantic_equality"],
            "not_established"
        );
    }
}

#[test]
fn limits_and_unknown_fields_are_rejected_without_echoing_input() {
    assert!(analyze(&"x".repeat(MAX_TEXT_BYTES + 1)).is_err());
    assert!(dispatch(&json!({"action":"analyze","text":"猫","path":"secret"})).is_err());
    assert!(dispatch(&json!({"action":"features","text":5})).is_err());
    assert!(dispatch(&json!({"action":"plan","text":"猫","context":false})).is_err());
    assert!(
        dispatch(&json!({"action":"unknown-sensitive-string"}))
            .unwrap_err()
            .to_string()
            .contains("未対応")
    );
    assert_eq!(
        dispatch(&json!({"action":"features"})).unwrap()["authorship_detection"],
        false
    );
    assert_eq!(
        plan("不明です。", "追加情報はありません。").unwrap()["performs_rewrite"],
        false
    );
}

#[test]
fn finding_volume_is_bounded_and_declared() {
    let report = analyze(&"既定です。 ".repeat(300)).unwrap();
    assert_eq!(report.findings.len(), 128);
    assert!(report.findings_limited);
}

#[test]
fn a_url_does_not_silence_following_japanese() {
    let text = "詳細はhttps://x.test/aを見てください。既定の設定です。";
    let hit = analyze(text)
        .unwrap()
        .findings
        .into_iter()
        .find(|f| f.rule == "reader.ambiguous_term")
        .unwrap();
    assert_eq!(&text[hit.start.byte..hit.end.byte], "既定");
    assert!(
        analyze("<https://x.test/既定>")
            .unwrap()
            .findings
            .is_empty()
    );
    assert!(
        analyze("[案内](https://x.test/既定)")
            .unwrap()
            .findings
            .is_empty()
    );
}

#[test]
fn numeric_order_units_kanji_signs_and_bounds_are_candidates() {
    for (before, after) in [
        ("3回失敗したら5回再送。", "5回失敗したら3回再送。"),
        ("3日待ちます。", "3時間待ちます。"),
        ("三回です。", "五回です。"),
        ("3回以上です。", "3回以下です。"),
        ("金額は-500円。", "金額は500円。"),
    ] {
        let result = compare(before, after).unwrap();
        assert_eq!(
            result["numeric_fragments_changed"], true,
            "{before} -> {after}"
        );
        assert_eq!(result["semantic_equality"], "not_established");
    }
    let not_covered = compare("3kgです。", "3gです。").unwrap();
    assert_eq!(not_covered["changed"], true);
    assert_eq!(not_covered["numeric_fragments_changed"], false);
    assert!(
        not_covered["comparison_scope"]
            .as_str()
            .unwrap()
            .contains("網羅しません")
    );
}

#[test]
fn a_missing_quote_does_not_hide_other_paragraphs() {
    let report = analyze("「途中の文\n\n既定の保存先です。\n\n引用の終わり。」").unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == "reader.ambiguous_term")
    );
}

#[test]
fn status_check_is_prioritized_and_understands_heading_and_wide_colon() {
    let text = format!(
        "# 状態：完了\n\n{}\n\nテストは未完了です。",
        "既定です。 ".repeat(200)
    );
    let result = analyze(&text).unwrap();
    assert!(result.findings_limited);
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.rule == "reader.status_alignment")
    );
}

#[test]
fn protected_islands_do_not_hide_unrelated_work_in_the_same_paragraph() {
    for text in [
        "状態: 完了\n\nテストは未完了です。詳細は https://ci.example/run/1 を参照。",
        "状態: 完了\n\n「確認」は未完了です。",
        "状態: 完了\n\n「既定の確認」はまだ実行していません。",
    ] {
        let report = analyze(text).unwrap();
        let hit = report
            .findings
            .iter()
            .find(|f| f.rule == "reader.status_alignment")
            .unwrap();
        assert!(["未完了", "まだ実行していません"].contains(&&text[hit.start.byte..hit.end.byte]));
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.rule == "reader.ambiguous_term")
        );
    }
    let lead = analyze("背景として「資料」を説明します。修正は完了しました。").unwrap();
    assert!(
        lead.findings
            .iter()
            .any(|f| f.rule == "reader.result_location")
    );
}
