use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::tool::{ChatMessage, StreamEvent, ToolCall, ToolDef};
use crate::provider::ModelProvider;

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute_tool(&self, org_id: &str, tool_call: &ToolCall) -> Result<String, String>;
}

pub async fn ai_chat_impl(
    provider: &dyn ModelProvider,
    executor: &dyn ToolExecutor,
    org_id: &str,
    messages: &[ChatMessage],
    tools: &[ToolDef],
    sender: mpsc::UnboundedSender<StreamEvent>,
) -> Result<(), String> {
    let mut conversation: Vec<ChatMessage> = messages.to_vec();
    let max_tool_rounds: usize = 10;

    for _round in 0..max_tool_rounds {
        let (tx, mut rx) = mpsc::unbounded_channel::<StreamEvent>();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut final_content = String::new();

        let provider_sender = sender.clone();
        let provider = provider;
        let msgs = conversation.clone();

        provider.complete_stream(&msgs, tools, tx).await?;

        while let Some(event) = rx.recv().await {
            match &event {
                StreamEvent::Token { content } => {
                    final_content.push_str(content);
                    let _ = sender.send(event.clone());
                }
                StreamEvent::ToolCall { name, args } => {
                    tool_calls.push(ToolCall {
                        id: format!("call_{}", tool_calls.len()),
                        call_type: "function".into(),
                        function: crate::tool::FunctionCall {
                            name: name.clone(),
                            arguments: args.to_string(),
                        },
                    });
                    let _ = sender.send(event.clone());
                }
                _ => {}
            }
        }

        if tool_calls.is_empty() {
            let _ = sender.send(StreamEvent::Done {
                message: final_content,
            });
            return Ok(());
        }

        conversation.push(ChatMessage {
            role: "assistant".into(),
            content: Some(final_content.clone()),
            tool_calls: Some(tool_calls.clone()),
            tool_call_id: None,
        });

        for tc in &tool_calls {
            let result = executor.execute_tool(org_id, tc).await.unwrap_or_else(|e| format!("Error: {}", e));
            let _ = sender.send(StreamEvent::ToolResult {
                name: tc.function.name.clone(),
                result: result.clone(),
            });
            conversation.push(ChatMessage {
                role: "tool".into(),
                content: Some(result),
                tool_calls: None,
                tool_call_id: Some(tc.id.clone()),
            });
        }
    }

    let _ = sender.send(StreamEvent::Error {
        message: "Max tool call rounds reached".into(),
    });
    Err("Max tool call rounds reached".into())
}
