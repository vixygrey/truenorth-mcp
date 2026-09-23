//! Shared, non-truncating MCP tool-result budget enforcement.

use rmcp::ErrorData;
use rmcp::model::{CallToolResult, ContentBlock};

use crate::engine::features::TokenCaps;
use crate::engine::tier::estimate_tokens_from_chars;

/// Build a successful tool result only when its text payload fits the configured budget.
pub fn success(content: Vec<ContentBlock>, caps: TokenCaps) -> Result<CallToolResult, ErrorData> {
    let chars: usize = content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.chars().count()),
            _ => None,
        })
        .sum();
    let tokens = estimate_tokens_from_chars(chars);
    if tokens > caps.tool_payload_tokens {
        return Err(ErrorData::invalid_params(
            format!(
                "tool response estimates {tokens} tokens, exceeding the configured {}-token cap; request a lean tier, reduce the input, or raise token_caps.tool_payload_tokens",
                caps.tool_payload_tokens
            ),
            None,
        ));
    }
    Ok(CallToolResult::success(content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_counts_all_text_blocks_and_preserves_under_cap_content() {
        let caps = TokenCaps {
            skill_lean_tokens: 1200,
            tool_payload_tokens: 2,
        };
        let content = vec![ContentBlock::text("éééé"), ContentBlock::text("test")];
        let result = success(content, caps).expect("eight characters fit two estimated tokens");
        assert_eq!(result.content.len(), 2);
        assert!(matches!(&result.content[0], ContentBlock::Text(text) if text.text == "éééé"));
        assert!(matches!(&result.content[1], ContentBlock::Text(text) if text.text == "test"));

        let error = success(
            vec![ContentBlock::text("12345"), ContentBlock::text("1234")],
            caps,
        )
        .expect_err("nine characters exceed two estimated tokens");
        assert!(error.message.contains("3 tokens"));
        assert!(error.message.contains("2-token cap"));
    }
}
