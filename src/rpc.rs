use serde_json::{Value, json};

pub fn tools() -> Value {
    let descriptors = [
        (
            "unslop_analyze",
            "読み手が理解しにくい表現の候補を調べる",
            json!({"text":{"type":"string"}}),
            vec!["text"],
        ),
        (
            "unslop_compare",
            "原文と改稿の数量や意味を確認する差分",
            json!({"original":{"type":"string"},"rewrite":{"type":"string"}}),
            vec!["original", "rewrite"],
        ),
        (
            "unslop_plan",
            "用途と元の内容を保つ推敲手順",
            json!({"text":{"type":"string"},"context":{"type":"string"}}),
            vec!["text"],
        ),
        (
            "unslop_features",
            "対応ルールの一覧と入力文の特徴量",
            json!({"text":{"type":"string"}}),
            vec![],
        ),
    ];
    json!({"tools":descriptors.into_iter().map(|(name,description,properties,required)|json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false}})).collect::<Vec<_>>()})
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
                    "明示入力の文章だけを分析します。作者の判定や自動書き直しは行いません。",
                )
        }
        async fn list_tools(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListToolsResult, ErrorData> {
            let items: Vec<Tool> = serde_json::from_value(tools()["tools"].clone())
                .map_err(|_| ErrorData::internal_error("tool schema", None))?;
            Ok(ListToolsResult::with_all_items(items))
        }
        async fn call_tool(
            &self,
            request: CallToolRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> Result<CallToolResponse, ErrorData> {
            let action = match request.name.as_ref() {
                "unslop_analyze" => "analyze",
                "unslop_compare" => "compare",
                "unslop_plan" => "plan",
                "unslop_features" => "features",
                _ => return Err(ErrorData::invalid_params("Unknown tool", None)),
            };
            let mut arguments = request.arguments.unwrap_or_default();
            if arguments.contains_key("action") {
                return Err(ErrorData::invalid_params("Unexpected action", None));
            }
            arguments.insert("action".into(), json!(action));
            Ok(match crate::dispatch(&Value::Object(arguments)) {
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
