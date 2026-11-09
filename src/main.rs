use serde_json::{Value, json};
use std::{
    io::{self, Read},
    process::ExitCode,
};

fn read(path: Option<&str>, limit: usize) -> Result<String, String> {
    let mut input: Box<dyn Read> = match path {
        None | Some("-") => Box::new(io::stdin()),
        Some(name) => Box::new(std::fs::File::open(name).map_err(|e| e.to_string())?),
    };
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("入力がサイズの上限を超えています。".into());
    }
    String::from_utf8(bytes).map_err(|_| "UTF-8の文章を指定してください。".into())
}
fn run(args: Vec<String>) -> Result<u8, String> {
    let command = args.first().map(String::as_str).unwrap_or("help");
    if matches!(command, "help" | "--help" | "-h") {
        println!(
            "godd-unslop analyze|plan|features [FILE|-] [--json] [--strict]\ngodd-unslop compare ORIGINAL REWRITE [--json] [--strict]\ngodd-unslop style-learn|style-review [JSON_FILE|-]\ngodd-unslop dispatch [JSON_FILE|-]\ngodd-unslop rewrite [JSON_FILE|-]\ngodd-unslop install-skill SKILLS_ROOT\ngodd-unslop relationship-put [JSON_FILE|-]\ngodd-unslop relationship-get|relationship-delete ID\ngodd-unslop relationship-list\ngodd-unslop rewrite-for|context-for ID [JSON_FILE|-]\ngodd-unslop serve\nファイルを省略すると、標準入力から読み込みます。\nfeaturesで機能の一覧だけを表示する場合は、--catalogを指定してください。\n\n終了コードは、成功時が0、--strictで候補がある場合が1、入力エラー時が2です。"
        );
        return Ok(0);
    }
    if command == "install-skill" {
        if args.len() != 2 {
            return Err("install-skillにはSkillのルートフォルダーを1つ指定してください。".into());
        }
        let path = godd_unslop::skill::install(std::path::Path::new(&args[1]))
            .map_err(|e| e.to_string())?;
        println!("{}", json!({"installed":path}));
        return Ok(0);
    }
    if matches!(
        command,
        "relationship-put"
            | "relationship-get"
            | "relationship-list"
            | "relationship-delete"
            | "rewrite-for"
            | "context-for"
    ) {
        use godd_unslop::relationships::{Relationship, Store, apply_recipient, default_path};
        let path = default_path().map_err(|e| e.to_string())?;
        let result = match command {
            "relationship-put" if args.len() <= 2 => {
                let record: Relationship =
                    serde_json::from_str(&read(args.get(1).map(String::as_str), 32 * 1024)?)
                        .map_err(|_| "相手の設定をJSONで指定してください。".to_string())?;
                let store = Store::open(&path, true).map_err(|e| e.to_string())?;
                store.put(&record).map_err(|e| e.to_string())?;
                json!({"saved":true,"id":record.id})
            }
            "relationship-get" if args.len() == 2 => {
                let store = Store::open(&path, false).map_err(|e| e.to_string())?;
                json!({"relationship":store.get(&args[1]).map_err(|e|e.to_string())?})
            }
            "relationship-list" if args.len() == 1 => {
                let store = Store::open(&path, false).map_err(|e| e.to_string())?;
                json!({"relationships":store.list().map_err(|e|e.to_string())?})
            }
            "relationship-delete" if args.len() == 2 => {
                let store = Store::open(&path, false).map_err(|e| e.to_string())?;
                json!({"deleted":store.delete(&args[1]).map_err(|e|e.to_string())?})
            }
            "rewrite-for" | "context-for" if args.len() == 2 || args.len() == 3 => {
                let request: Value =
                    serde_json::from_str(&read(args.get(2).map(String::as_str), 2 * 1024 * 1024)?)
                        .map_err(|_| "書き換える文章をJSONで指定してください。".to_string())?;
                let store = Store::open(&path, false).map_err(|e| e.to_string())?;
                let request =
                    apply_recipient(&store, &args[1], &request).map_err(|e| e.to_string())?;
                if command == "context-for" {
                    json!({"recipient_id":args[1],"context":request["context"],"sends_to_ai":false})
                } else {
                    godd_unslop::dispatch(&request).map_err(|e| e.to_string())?
                }
            }
            _ => {
                return Err(
                    "相手の登録・取得・削除は相手のidや入力JSONを指定してください。".into(),
                );
            }
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|_| "結果を出力できません。")?
        );
        return Ok(0);
    }
    let mut strict = false;
    let mut catalog = false;
    let mut paths = Vec::new();
    for arg in &args[1..] {
        match arg.as_str() {
            "--json" => {}
            "--strict" => strict = true,
            "--catalog" => catalog = true,
            s if s.starts_with('-') && s != "-" => return Err("未対応のオプションです。".into()),
            _ => paths.push(arg.as_str()),
        }
    }
    if paths.len() > if command == "compare" { 2 } else { 1 } {
        return Err("入力ファイルが多すぎます。".into());
    }
    if catalog && (command != "features" || !paths.is_empty()) {
        return Err("--catalogはfeatures専用です。".into());
    }
    let json_limit = 2 * 1024 * 1024;
    let action = match command {
        "analyze" | "plan" => {
            json!({"action":command,"text":read(paths.first().copied(), godd_unslop::MAX_TEXT_BYTES)?})
        }
        "features" if catalog => json!({"action":"features"}),
        "features" => {
            json!({"action":"features","text":read(paths.first().copied(), godd_unslop::MAX_TEXT_BYTES)?})
        }
        "compare" => {
            if paths.len() != 2 || paths.iter().filter(|p| **p == "-").count() > 1 {
                return Err("比較する2ファイルを指定してください。".into());
            }
            json!({"action":"compare","original":read(Some(paths[0]), godd_unslop::MAX_TEXT_BYTES)?,"rewrite":read(Some(paths[1]), godd_unslop::MAX_TEXT_BYTES)?})
        }
        "rewrite" | "style-learn" | "style-review" => {
            let mut value: Value = serde_json::from_str(&read(paths.first().copied(), json_limit)?)
                .map_err(|_| "JSON入力を確認してください。".to_string())?;
            value
                .as_object_mut()
                .ok_or("入力にはJSONオブジェクトを指定してください。")?
                .insert("action".into(), json!(command.replace('-', "_")));
            value
        }
        "dispatch" => serde_json::from_str::<Value>(&read(paths.first().copied(), json_limit)?)
            .map_err(|_| "JSON入力を確認してください。".to_string())?,
        _ => return Err("未対応のコマンドです。".into()),
    };
    let result = godd_unslop::dispatch(&action).map_err(|e| e.to_string())?;
    let flagged = result
        .get("findings")
        .and_then(Value::as_array)
        .is_some_and(|v| !v.is_empty())
        || result
            .pointer("/analysis/findings")
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty())
        || result.get("changed").and_then(Value::as_bool) == Some(true);
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
    );
    Ok(u8::from(strict && flagged))
}
#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "serve") {
        if args.len() != 1 {
            eprintln!("serveに引数は付けません。");
            return ExitCode::from(2);
        }
        return match godd_unslop::rpc::serve().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        };
    }
    match run(args) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
