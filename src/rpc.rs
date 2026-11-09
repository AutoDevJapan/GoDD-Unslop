use serde_json::{Value, json};

pub fn tools() -> Value {
    let descriptors = [
        (
            "unslop_rewrite",
            "明示入力をChatGPTサブスク経由のCodex CLIで自然な日本語へ書き換え、原文と比較を返す",
            json!({"text":{"type":"string"},"profile":{"type":"object"},"preserve":{"type":"array","items":{"type":"string"}}}),
            vec!["text"],
        ),
        (
            "unslop_analyze",
            "読み手が理解しにくい表現の候補を調べる",
            json!({"text":{"type":"string"}}),
            vec!["text"],
        ),
        (
            "unslop_compare",
            "原文と改稿を比較し、数量や意味を確認するための差分を返す",
            json!({"original":{"type":"string"},"rewrite":{"type":"string"}}),
            vec!["original", "rewrite"],
        ),
        (
            "unslop_plan",
            "文章の用途と元の内容を保つための推敲手順を返す",
            json!({"text":{"type":"string"},"context":{"type":"string"}}),
            vec!["text"],
        ),
        (
            "unslop_features",
            "対応するルールの一覧と、入力した文章の特徴を返す",
            json!({"text":{"type":"string"}}),
            vec![],
        ),
        (
            "unslop_style_learn",
            "明示した修正例から改行と空行の選択を学び、保存可能な文体設定を返す",
            json!({"examples":{"type":"array","items":{"type":"object","properties":{"before":{"type":"string"},"after":{"type":"string"}},"required":["before","after"],"additionalProperties":false}},"profile":{"type":"object"}}),
            vec!["examples"],
        ),
        (
            "unslop_style_review",
            "学習済み文体設定と文章の改行位置・間隔を照合する",
            json!({"text":{"type":"string"},"profile":{"type":"object"}}),
            vec!["text", "profile"],
        ),
    ];
    json!({"tools":descriptors.into_iter().map(|(name,description,properties,required)|json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":name == "unslop_rewrite"}})).collect::<Vec<_>>()})
}

#[cfg(feature = "mcp")]
mod transport {
    use super::*;
    use rmcp::{
        ErrorData, RoleServer, ServerHandler, ServiceExt, model::*, service::RequestContext,
    };

    #[derive(Clone)]
    struct WritingServer;
    impl ServerHandler for WritingServer {
        fn get_info(&self) -> ServerConfig {
            ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
                .with_instructions(
                    "指定された文章や修正例を分析し、確認する箇所や推敲の手順を返します。\n作者の判定は行いません。unslop_rewriteは明示入力を書き換えます。意味の一致は保証しません。",
                )
        }
        async fn list_tools(
            &self,
            _request: Option<PaginatedRequestParams>,
            context: RequestContext<RoleServer>,
        ) -> Result<ListToolsResult, ErrorData> {
            let _ = context;
            let items: Vec<Tool> = serde_json::from_value(tools()["tools"].clone())
                .map_err(|_| ErrorData::internal_error("tool schema", None))?;
            Ok(ListToolsResult::with_all_items(items))
        }
        async fn call_tool(
            &self,
            request: CallToolRequestParams,
            context: RequestContext<RoleServer>,
        ) -> Result<CallToolResponse, ErrorData> {
            let action = match request.name.as_ref() {
                "unslop_rewrite" => "rewrite",
                "unslop_analyze" => "analyze",
                "unslop_compare" => "compare",
                "unslop_plan" => "plan",
                "unslop_features" => "features",
                "unslop_style_learn" => "style_learn",
                "unslop_style_review" => "style_review",
                _ => return Err(ErrorData::invalid_params("Unknown tool", None)),
            };
            let mut arguments = request.arguments.unwrap_or_default();
            if arguments.contains_key("action") {
                return Err(ErrorData::invalid_params("Unexpected action", None));
            }
            arguments.insert("action".into(), json!(action));
            let input = Value::Object(arguments);
            let result = if action == "rewrite" {
                let cancel = crate::rewrite::Cancellation::default();
                struct CancelOnDrop(crate::rewrite::Cancellation);
                impl Drop for CancelOnDrop {
                    fn drop(&mut self) {
                        self.0.cancel();
                    }
                }
                let guard = CancelOnDrop(cancel.clone());
                let mut task = tokio::task::spawn_blocking(move || {
                    crate::rewrite::run_cancelled(&input, cancel)
                });
                let result = tokio::select! {
                    result = &mut task => result,
                    _ = context.ct.cancelled() => {
                        guard.0.cancel();
                        task.await
                    }
                };
                drop(guard);
                result
                    .map_err(|_| ErrorData::internal_error("AI処理の実行に失敗しました。", None))?
            } else {
                crate::dispatch(&input)
            };
            Ok(match result {
                Ok(value) => CallToolResult::success(vec![ContentBlock::text(value.to_string())]),
                Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
            }
            .into())
        }
    }
    pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
        let running = WritingServer.serve(rmcp::transport::stdio()).await?;
        running.waiting().await?;
        Ok(())
    }
}
#[cfg(feature = "mcp")]
pub use transport::serve;
