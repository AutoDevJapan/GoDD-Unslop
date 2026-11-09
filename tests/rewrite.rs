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
