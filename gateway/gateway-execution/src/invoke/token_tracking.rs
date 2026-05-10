//! # Token Tracking
//!
//! Update token counts and emit token usage events.

use gateway_events::GatewayEvent;

use super::stream_context::StreamContext;

/// Update token counts and emit token usage event.
pub fn handle_token_update(ctx: &StreamContext, tokens_in: u64, tokens_out: u64) {
    // Update execution token counts — via batch writer if available, else direct
    if let Some(writer) = &ctx.batch_writer {
        writer.token_update(&ctx.execution_id, tokens_in, tokens_out);
    } else if let Err(e) =
        ctx.state_service
            .update_execution_tokens(&ctx.execution_id, tokens_in, tokens_out)
    {
        tracing::warn!("Failed to update execution tokens: {}", e);
    }

    // Emit token usage event for real-time UI updates
    let event_bus = ctx.event_bus.clone();
    let sess_id = ctx.session_id.clone();
    let exec_id = ctx.execution_id.clone();
    let conv_id = ctx.conversation_id.clone();
    tokio::spawn(async move {
        event_bus
            .publish(GatewayEvent::TokenUsage {
                session_id: sess_id,
                execution_id: exec_id,
                tokens_in,
                tokens_out,
                conversation_id: Some(conv_id),
            })
            .await;
    });
}
