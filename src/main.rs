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
            "godd-unslop analyze|plan|features [FILE|-] [--json] [--strict]\ngodd-unslop compare ORIGINAL REWRITE [--json] [--strict]\ngodd-unslop style-learn|style-review [JSON_FILE|-]\ngodd-unslop dispatch [JSON_FILE|-]\ngodd-unslop serve\nファイルを省略すると、標準入力から読み込みます。\nfeaturesで機能の一覧だけを表示する場合は、--catalogを指定してください。\n\n終了コードは、成功時が0、--strictで候補がある場合が1、入力エラー時が2です。"
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
        "style-learn" | "style-review" => {
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
