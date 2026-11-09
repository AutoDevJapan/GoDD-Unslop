use godd_unslop::relationships::{Relationship, Store, apply_recipient};
use serde_json::json;

fn location(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "unslop-relationships-{}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path.join("relationships.sqlite3")
}
fn example() -> Relationship {
    serde_json::from_value(json!({"id":"colleague-1","label":"架空の同僚",
        "confirmed":true,"source":"本人が入力したサンプル",
        "context":{"audience":"同じ部署の同僚","recipient":{"relation":"peer","closeness":"close"},
        "tone":{"formality":1,"warmth":3,"directness":2,"emoji":"light"},"channel":"chat"}}))
    .unwrap()
}
#[test]
fn saves_reopens_overrides_one_field_and_deletes() {
    let path = location("roundtrip");
    let store = Store::open(&path, true).unwrap();
    store.put(&example()).unwrap();
    drop(store);
    let store = Store::open(&path, false).unwrap();
    assert_eq!(
        store.get("colleague-1").unwrap().unwrap().label,
        "架空の同僚"
    );
    let input = apply_recipient(&store, "colleague-1", &json!({"text":"明日までに確認してください。","context":{"tone":{"formality":3},"channel":"official"}})).unwrap();
    assert_eq!(input["context"]["tone"]["formality"], 3);
    assert_eq!(input["context"]["tone"]["warmth"], 3);
    assert_eq!(input["context"]["recipient"]["relation"], "peer");
    assert_eq!(input["context"]["channel"], "official");
    assert_eq!(store.list().unwrap().len(), 1);
    assert!(store.delete("colleague-1").unwrap());
    assert!(store.get("colleague-1").unwrap().is_none());
    assert!(!store.delete("colleague-1").unwrap());
    drop(store);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn unconfirmed_and_invalid_profiles_cannot_be_used() {
    let path = location("validation");
    let store = Store::open(&path, true).unwrap();
    let mut profile = example();
    profile.confirmed = false;
    store.put(&profile).unwrap();
    assert!(apply_recipient(&store, &profile.id, &json!({"text":"依頼します。"})).is_err());
    profile.context["tone"]["warmth"] = json!(10);
    assert!(store.put(&profile).is_err());
    assert_eq!(
        store.get("colleague-1").unwrap().unwrap().context["tone"]["warmth"],
        3
    );
    profile = example();
    profile.id = "../private".into();
    assert!(store.put(&profile).is_err());
    assert!(store.get("unknown").unwrap().is_none());
    drop(store);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn concurrent_initialization_keeps_both_records() {
    let path = location("concurrent");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|index| {
            let barrier = barrier.clone();
            let path = path.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let store = Store::open(&path, true).unwrap();
                let mut record = example();
                record.id = format!("sample-{index}");
                store.put(&record).unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let store = Store::open(&path, false).unwrap();
    assert_eq!(store.list().unwrap().len(), 2);
    drop(store);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn refuses_database_inside_repository() {
    let path = location("repo");
    std::fs::create_dir(path.parent().unwrap().join(".git")).unwrap();
    assert!(Store::open(&path, true).is_err());
    assert!(!path.exists());
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn selected_request_excludes_labels_and_other_recipients() {
    let path = location("selection");
    let store = Store::open(&path, true).unwrap();
    store.put(&example()).unwrap();
    let mut other = example();
    other.id = "other".into();
    other.context["audience"] = json!("他の相手だけの情報");
    store.put(&other).unwrap();
    let request =
        apply_recipient(&store, "colleague-1", &json!({"text":"確認してください。"})).unwrap();
    let serialized = request.to_string();
    assert!(!serialized.contains("架空の同僚"));
    assert!(!serialized.contains("本人が入力したサンプル"));
    assert!(!serialized.contains("他の相手だけの情報"));
    drop(store);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[cfg(feature = "mcp")]
#[test]
fn cli_crud_and_unconfirmed_rewrite_without_ai() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let path = location("cli");
    let run = |args: &[&str], text: &str| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_godd-unslop"));
        command
            .args(args)
            .env("GODD_UNSLOP_DB", &path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let mut record = example();
    record.confirmed = false;
    assert!(
        run(
            &["relationship-put"],
            &serde_json::to_string(&record).unwrap()
        )
        .status
        .success()
    );
    let got = run(&["relationship-get", "colleague-1"], "");
    assert!(got.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&got.stdout).unwrap()["relationship"]["confirmed"],
        false
    );
    let rejected = run(
        &["rewrite-for", "colleague-1"],
        r#"{"text":"明日までに確認してください。"}"#,
    );
    assert_eq!(rejected.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("未確認"));
    record.confirmed = true;
    assert!(
        run(
            &["relationship-put"],
            &serde_json::to_string(&record).unwrap()
        )
        .status
        .success()
    );
    let preview = run(
        &["context-for", "colleague-1"],
        r#"{"text":"確認してください。","context":{"tone":{"formality":4}}}"#,
    );
    assert!(preview.status.success());
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["sends_to_ai"], false);
    assert_eq!(preview["context"]["tone"]["formality"], 4);
    assert_eq!(preview["context"]["tone"]["warmth"], 3);
    assert!(
        run(&["relationship-delete", "colleague-1"], "")
            .status
            .success()
    );
    let list = run(&["relationship-list"], "");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&list.stdout).unwrap()["relationships"],
        json!([])
    );
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[cfg(unix)]
#[test]
fn file_permissions_and_symlink_target_are_checked() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let path = location("permissions").join("nested").join("data.sqlite3");
    let store = Store::open(&path, true).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    drop(store);
    let repo = path.parent().unwrap().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let link = path.parent().unwrap().join("linked.sqlite3");
    symlink(repo.join("missing.sqlite3"), &link).unwrap();
    assert!(Store::open(&link, true).is_err());
    assert!(!repo.join("missing.sqlite3").exists());
    std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
}
#[test]
fn missing_database_read_does_not_create_it_and_foreign_database_is_untouched() {
    let path = location("identity");
    assert!(Store::open(&path, false).is_err());
    assert!(!path.exists());
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("CREATE TABLE other_data (value TEXT)", [])
        .unwrap();
    conn.execute("INSERT INTO other_data VALUES ('keep')", [])
        .unwrap();
    drop(conn);
    assert!(Store::open(&path, true).is_err());
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row("SELECT value FROM other_data", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "keep"
    );
    drop(conn);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
