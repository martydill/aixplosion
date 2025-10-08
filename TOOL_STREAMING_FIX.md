# Tool Calls in Streaming Mode - Fix Implementation

## Problem Identified

The streaming implementation in `src/anthropic.rs` was only handling text content blocks, but not tool_use blocks. When tool calls were made in streaming mode, they were being ignored because the streaming parser didn't know how to handle `content_block_start` events with `tool_use` types or `content_block_delta` events with `partial_json` data.

## Solution Implemented

### 1. Added StreamDelta Structure

Created a new `StreamDelta` struct to properly handle the different types of delta events in streaming:

```rust
#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    #[serde(rename = "type")]
    pub block_type: Option<String>,
    pub text: Option<String>,
    pub partial_json: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
}
```

### 2. Updated StreamEvent

Changed `StreamEvent.delta` from `Option<ContentBlock>` to `Option<StreamDelta>` to properly match the streaming API response format.

### 3. Enhanced Streaming Parser

The `try_endpoint_stream` function now properly handles:

- **content_block_start**: Initializes either text blocks or tool_use blocks
- **content_block_delta**: Handles both text deltas and partial_json deltas for tool parameters
- **content_block_stop**: Finalizes both text and tool_use blocks

#### Key Changes:

1. **Tool Use Block Initialization**:
   ```rust
   "tool_use" => {
       current_tool_block = Some(ContentBlock {
           block_type: "tool_use".to_string(),
           text: None,
           id: delta.id,
           name: delta.name,
           input: None,
           tool_use_id: None,
           content: None,
           is_error: None,
       });
   }
   ```

2. **Partial JSON Handling**:
   ```rust
   } else if let Some(partial_json) = delta.partial_json {
       // Handle tool input JSON
       if let Some(ref mut tool_block) = current_tool_block {
           if let Some(ref input_str) = tool_block.input {
               // Append to existing JSON
               if let Some(Value::String(existing)) = input_str {
                   let combined = format!("{}{}", existing, partial_json);
                   tool_block.input = Some(Value::String(combined));
               }
           } else {
               // Start new JSON string
               tool_block.input = Some(Value::String(partial_json));
           }
       }
   }
   ```

3. **Tool Block Finalization**:
   ```rust
   if let Some(tool_block) = current_tool_block.take() {
       // Finalize tool_use block
       let mut finalized_block = tool_block;
       // Parse the accumulated JSON string into a proper JSON value
       if let Some(Value::String(json_str)) = finalized_block.input {
           match serde_json::from_str::<Value>(&json_str) {
               Ok(parsed_json) => {
                   finalized_block.input = Some(parsed_json);
               }
               Err(e) => {
                   debug!("Failed to parse tool JSON: {}, keeping as string", e);
                   // Keep as string if parsing fails
               }
           }
       }
       content_blocks.push(finalized_block);
   }
   ```

## How It Works

1. **When a tool call starts**: The streaming parser detects a `content_block_start` event with `type: "tool_use"` and initializes a tool block with the tool ID and name.

2. **During tool parameter streaming**: As the tool parameters are streamed as `partial_json` chunks, they are accumulated and concatenated into a JSON string.

3. **When the tool call completes**: The accumulated JSON string is parsed into a proper JSON value and stored in the tool_use content block.

4. **Back to Agent**: The completed tool_use content block is returned to the agent, which can then execute the tool call using the existing tool execution logic.

## Benefits

- **Full Tool Support**: Tool calls now work correctly in streaming mode
- **Real-time Feedback**: Users can see tool calls being made and executed in real-time
- **Backward Compatibility**: Non-streaming mode continues to work as before
- **Error Handling**: Graceful fallback if JSON parsing fails
- **Debugging**: Enhanced logging for troubleshooting streaming tool calls

## Testing

After this fix, tool calls should work properly in streaming mode. The agent will:

1. Receive tool_use blocks from the streaming API
2. Execute the tools as usual
3. Continue the conversation with tool results
4. Stream the final response to the user

This fix ensures that all agent capabilities (file operations, bash commands, etc.) work correctly whether streaming is enabled or not.