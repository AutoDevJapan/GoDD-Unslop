//! Private, explicitly selected recipient context. Never transmitted or enumerated by MCP.
use crate::Error;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const APPLICATION_ID: i64 = 0x4755534c;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub id: String,
    pub label: String,
    pub confirmed: bool,
    pub source: String,
    pub context: Value,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}

impl Relationship {
    fn validate(&self) -> Result<(), Error> {
        if !valid_id(&self.id)
            || self.label.trim().is_empty()
            || self.label.chars().count() > 100
            || self.source.trim().is_empty()
            || self.source.chars().count() > 200
        {
            return Err(Error(
                "相手のid・表示名・確認元を確認してください。idは英数字・ハイフン・下線で80文字までです。",
            ));
        }
        crate::rewrite::validate_context(Some(&self.context))?;
        if serde_json::to_vec(self)
            .map_err(|_| Error("関係性を保存用に変換できません。"))?
            .len()
            > 16 * 1024
        {
            return Err(Error("相手1人の設定は16KiBまでです。"));
        }
        Ok(())
    }
}

pub struct Store(Connection);
fn check_path(path: &Path) -> Result<(), Error> {
    // A home-directory dotfile repository must not make all app data unusable.
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .and_then(|p| p.canonicalize().ok());
    for item in path.ancestors() {
        let resolved = match item.canonicalize() {
            Ok(resolved) => resolved,
            Err(_) => match std::fs::symlink_metadata(item) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                _ => {
                    return Err(Error(
                        "DBの保存先を確認できません。リンクやアクセス権を確認してください。",
                    ));
                }
            },
        };
        if resolved
            .ancestors()
            .any(|ancestor| Some(ancestor) != home.as_deref() && ancestor.join(".git").exists())
        {
            return Err(Error("関係性DBはGitリポジトリの外に保存してください。"));
        }
    }
    Ok(())
}
fn initialize(conn: &Connection, create: bool) -> Result<(), Error> {
    let id: i64 = conn
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(|_| Error("関係性DBの種類を確認できません。"))?;
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| Error("関係性DBの版を確認できません。"))?;
    if id == 0 && version == 0 && create {
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| Error("関係性DBを確認できません。"))?;
        if tables != 0 {
            return Err(Error(
                "別の用途のDBには書き込みません。関係性専用の保存先を指定してください。",
            ));
        }
        conn.execute_batch("CREATE TABLE relationships (id TEXT PRIMARY KEY NOT NULL, record TEXT NOT NULL); PRAGMA application_id=1196774220; PRAGMA user_version=1;")
            .map_err(|_|Error("関係性DBを初期化できません。"))?;
    } else if id != APPLICATION_ID || version != 1 {
        return Err(Error("GoDD Unslopの対応する関係性DBではありません。"));
    }
    Ok(())
}
impl Store {
    /// `create` is reserved for explicit writes. Reads never create a database.
    pub fn open(path: &Path, create: bool) -> Result<Self, Error> {
        check_path(path)?;
        if create && let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            let existed = parent.exists();
            std::fs::create_dir_all(parent).map_err(|_| Error("関係性DBの保存先を作れません。"))?;
            #[cfg(unix)]
            if !existed {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                    .map_err(|_| Error("関係性フォルダーの権限を設定できません。"))?;
            }
            #[cfg(not(unix))]
            let _ = existed;
        }
        check_path(path)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | if create {
                OpenFlags::SQLITE_OPEN_CREATE
            } else {
                OpenFlags::empty()
            };
        let mut conn = Connection::open_with_flags(path, flags)
            .map_err(|_| Error("関係性DBを開けません。保存先とファイルを確認してください。"))?;
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(|_| Error("関係性DBを設定できません。"))?;
        if create {
            let transaction = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| Error("関係性DBの初期化を開始できません。"))?;
            initialize(&transaction, true)?;
            transaction
                .commit()
                .map_err(|_| Error("関係性DBの初期化を保存できません。"))?;
        } else {
            initialize(&conn, false)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| Error("関係性DBの権限を設定できません。"))?;
        }
        conn.pragma_update(None, "secure_delete", true)
            .map_err(|_| Error("関係性DBの削除設定を確認できません。"))?;
        Ok(Self(conn))
    }
    pub fn put(&self, relationship: &Relationship) -> Result<(), Error> {
        relationship.validate()?;
        let record =
            serde_json::to_string(relationship).map_err(|_| Error("関係性を変換できません。"))?;
        self.0.execute("INSERT INTO relationships (id, record) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record", params![relationship.id, record])
            .map_err(|_|Error("関係性を保存できません。ほかの操作が終わってから再実行してください。"))?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Option<Relationship>, Error> {
        if !valid_id(id) {
            return Err(Error("相手のidを確認してください。"));
        }
        let record: Option<String> = self
            .0
            .query_row(
                "SELECT record FROM relationships WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| Error("関係性を読み込めません。"))?;
        record
            .map(|record| {
                let relationship: Relationship = serde_json::from_str(&record)
                    .map_err(|_| Error("保存された関係性の形式が不正です。"))?;
                relationship.validate()?;
                if relationship.id != id {
                    return Err(Error("保存された相手のidが一致しません。"));
                }
                Ok(relationship)
            })
            .transpose()
    }
    pub fn list(&self) -> Result<Vec<Value>, Error> {
        let mut statement = self
            .0
            .prepare("SELECT id FROM relationships ORDER BY id")
            .map_err(|_| Error("関係性一覧を読み込めません。"))?;
        let ids: Vec<String> = statement
            .query_map([], |r| r.get(0))
            .map_err(|_| Error("関係性一覧を読み込めません。"))?
            .collect::<Result<_, _>>()
            .map_err(|_| Error("関係性一覧を読み込めません。"))?;
        ids.iter()
            .map(|id| {
                let r = self
                    .get(id)?
                    .ok_or(Error("関係性一覧が更新されています。再実行してください。"))?;
                Ok(serde_json::json!({"id":r.id,"label":r.label,"confirmed":r.confirmed}))
            })
            .collect()
    }
    pub fn delete(&self, id: &str) -> Result<bool, Error> {
        if !valid_id(id) {
            return Err(Error("相手のidを確認してください。"));
        }
        Ok(self
            .0
            .execute("DELETE FROM relationships WHERE id=?1", [id])
            .map_err(|_| Error("関係性を削除できません。"))?
            != 0)
    }
}

/// Explicit invocation only: inject just one confirmed recipient into one draft.
pub fn apply_recipient(store: &Store, id: &str, request: &Value) -> Result<Value, Error> {
    let relationship = store
        .get(id)?
        .ok_or(Error("指定した相手は登録されていません。"))?;
    if !relationship.confirmed {
        return Err(Error(
            "この相手の設定は未確認です。内容を確認してconfirmedをtrueにして保存してください。",
        ));
    }
    let mut input = request
        .as_object()
        .cloned()
        .ok_or(Error("入力にはJSONオブジェクトを指定してください。"))?;
    let mut context = relationship
        .context
        .as_object()
        .cloned()
        .ok_or(Error("保存された背景の形式が不正です。"))?;
    if let Some(overrides) = input.get("context") {
        let overrides = overrides
            .as_object()
            .ok_or(Error("contextにはJSONオブジェクトを指定してください。"))?;
        for (key, value) in overrides {
            if let (Some(base), Some(fields)) = (
                context.get_mut(key).and_then(Value::as_object_mut),
                value.as_object(),
            ) {
                for (field, value) in fields {
                    base.insert(field.clone(), value.clone());
                }
            } else {
                context.insert(key.clone(), value.clone());
            }
        }
    }
    input.insert("context".into(), Value::Object(context));
    input.insert("action".into(), Value::String("rewrite".into()));
    let input = Value::Object(input);
    crate::rewrite::prepare(&input)?;
    Ok(input)
}

pub fn default_path() -> Result<PathBuf, Error> {
    if let Some(path) = std::env::var_os("GODD_UNSLOP_DB") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(Error(
                "GODD_UNSLOP_DBには非公開フォルダーの絶対パスを指定してください。",
            ));
        }
        return Ok(path);
    }
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
    } else {
        std::env::var_os("XDG_DATA_HOME").or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share").into_os_string())
        })
    };
    base.map(|p| PathBuf::from(p).join("godd-unslop/relationships.sqlite3"))
        .ok_or(Error(
            "関係性DBの保存先を決められません。GODD_UNSLOP_DBを指定してください。",
        ))
}
