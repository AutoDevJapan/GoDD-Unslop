use godd_unslop::style::{learn, review};
use serde_json::json;
#[test]
fn spacing_and_byte_locations() {
    let p=learn(&json!({"examples":[{"before":"甲。乙。","after":"甲。\n乙。"},{"before":"甲。\r\n乙。","after":"甲。\r\n\r\n乙。"}]})).unwrap()["profile"].clone();
    let r = review(&json!({"text":"前。\n\n甲。\r\n乙。","profile":p})).unwrap();
    assert_eq!(r["findings"].as_array().unwrap().len(), 1);
    assert_eq!(r["findings"][0]["start"]["byte"], 14);
    assert_eq!(r["findings"][0]["kind"], "layout_conflict");
}
#[test]
fn protected_and_tone() {
    let p=learn(&json!({"examples":[{"before":"甲、乙。","after":"甲。乙。"},{"before":"行く。","after":"行きます。"}]})).unwrap();
    assert_eq!(p["ignored_examples"], 1);
    let r=review(&json!({"text":"`甲、乙。`\n\n> 甲、乙。\n\n「甲、乙。」\n\n甲、乙。","profile":p["profile"]})).unwrap();
    assert_eq!(r["findings"].as_array().unwrap().len(), 1);
    assert_eq!(r["findings"][0]["kind"], "explicit_wording");
}
#[test]
fn limits_and_roundtrip() {
    assert!(learn(&json!({"examples":[{"before":"a".repeat(4097),"after":"b"}]})).is_err());
    assert!(review(&json!({"text":"a","profile":{"version":2,"rules":[]}})).is_err());
    let p = learn(&json!({"examples":[]})).unwrap()["profile"].clone();
    let encoded = serde_json::to_string(&p).unwrap();
    assert!(review(&json!({"text":"a","profile":serde_json::from_str::<serde_json::Value>(&encoded).unwrap()})).is_ok());
}
#[test]
fn local_boundary_and_conflict() {
    let p=learn(&json!({"examples":[{"before":"甲乙丙丁戊己庚辛","after":"甲乙丙丁\n戊己庚辛"},{"before":"甲乙丙丁\n戊己庚辛","after":"甲乙丙丁\n\n戊己庚辛"}]})).unwrap();
    let r = review(&json!({"text":"追加甲乙丙丁 戊己庚辛末尾","profile":p["profile"]})).unwrap();
    assert!(
        r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "layout_conflict" && f["choices"].as_array().unwrap().len() == 2)
    );
}
#[test]
fn imported_tone_is_rejected() {
    assert!(review(&json!({"text":"行く。","profile":{"version":1,"rules":[{"before":"行く。","after":"行きます。","kind":"explicit_wording"}]}})).is_err());
}

#[test]
fn normalized_line_endings_and_protected_layout() {
    let p=learn(&json!({"examples":[{"before":"甲。乙。","after":"甲。\r\n乙。"}]})).unwrap()["profile"].clone();
    assert_eq!(
        review(&json!({"text":"甲。\n乙。","profile":p})).unwrap()["findings"],
        json!([])
    );
    let text = "`甲。乙。`\n\n```\n甲。乙。\n```\n\n> 甲。乙。\n\n[甲。乙。](https://example.com)\n\n「甲。乙。」\n\n追加甲。乙。末尾";
    let r = review(&json!({"text":text,"profile":p})).unwrap();
    assert_eq!(r["findings"].as_array().unwrap().len(), 1);
    let byte = r["findings"][0]["start"]["byte"].as_u64().unwrap() as usize;
    assert_eq!(&text[byte..byte + 3], "乙");
    let p = learn(&json!({"examples":[{"before":"甲。\n乙。","after":"甲。\r\n乙。"}]})).unwrap();
    assert_eq!(
        review(&json!({"text":"甲。乙。","profile":p["profile"]})).unwrap()["findings"],
        json!([])
    );
}
#[test]
fn combined_findings_are_bounded() {
    let p=learn(&json!({"examples":[{"before":"甲。乙。","after":"甲。\n乙。"},{"before":"丙、丁。","after":"丙。丁。"}]})).unwrap();
    let r = review(
        &json!({"text":format!("{}丙、丁。", "甲。乙。\n\n".repeat(130)),"profile":p["profile"]}),
    )
    .unwrap();
    assert_eq!(r["findings"].as_array().unwrap().len(), 128);
    assert_eq!(r["findings_limited"], true);
}
#[test]
fn quote_markers_in_code_and_urls_do_not_hide_prose() {
    let p=learn(&json!({"examples":[{"before":"abcd,efgh","after":"abcd.efgh"},{"before":"abcd,efgh","after":"abcd;efgh"}]})).unwrap();
    let text = "`\"`\n\nhttps://example.test/abcd,efgh\n\n' abcd,efgh '\n\nDon't hide abcd,efgh";
    let r = review(&json!({"text":text,"profile":p["profile"]})).unwrap();
    assert_eq!(r["findings"].as_array().unwrap().len(), 1);
    assert_eq!(r["findings"][0]["kind"], "explicit_wording_conflict");
    assert!(r["findings"][0]["suggestion"].is_null());
}
#[test]
fn unknown_examples_and_serialized_profile_cap() {
    assert!(learn(&json!({"examples":[{"before":"甲乙","after":"甲 乙","extra":true}]})).is_err());
    let rules:Vec<_>=(0..20).map(|i|json!({"before":format!("{}{}乙", "甲".repeat(1200),i),"after":format!("{}{} 乙", "甲".repeat(1200),i),"kind":"layout"})).collect();
    assert!(review(&json!({"text":"甲乙","profile":{"version":1,"rules":rules}})).is_err());
}
