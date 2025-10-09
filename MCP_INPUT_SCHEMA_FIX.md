# MCP Input Schema Fix

## Problem
When connecting to stdio MCP servers, users encountered the error: "missing field input_schema"

## Root Cause
The MCP protocol specification requires tools to have an `input_schema` field, but some MCP servers either:
1. Don't provide this field at all
2. Provide it in a different format than expected
3. Have malformed JSON in their tool definitions

## Solution Implemented

### 1. Made input_schema Optional with Default
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_input_schema")]
    pub input_schema: Value,
}

fn default_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "required": []
    })
}
```

### 2. Enhanced Error Handling in load_tools()
- Added detailed logging of raw tool responses
- Graceful fallback for malformed tools
- Better error messages with debugging information

### 3. Improved Notification Handling
- Applied the same robust parsing to tools received via notifications
- Consistent error handling across all tool loading scenarios

### 4. Fallback Tool Creation
When a tool can't be parsed properly, the system now:
1. Logs a detailed warning with the problematic data
2. Attempts to extract at least the tool name
3. Creates a minimal tool with a default schema
4. Continues processing instead of failing completely

## Benefits

### 1. Robustness
- No longer crashes on malformed tool definitions
- Gracefully handles missing fields
- Provides meaningful debugging information

### 2. Compatibility
- Works with a wider range of MCP servers
- Maintains compatibility with properly compliant servers
- Follows the principle of being tolerant in what you accept

### 3. Debugging
- Detailed logging helps identify problematic servers
- Clear error messages guide troubleshooting
- Raw response data available for analysis

## Default Schema
When `input_schema` is missing, a default schema is provided:
```json
{
    "type": "object",
    "properties": {},
    "required": []
}
```

This allows the tool to be used even without specific schema information, treating it as a tool that accepts no arguments.

## Error Messages
The fix provides much more informative error messages:
- Shows the index of the problematic tool
- Displays the raw tool data that failed to parse
- Indicates which field was missing or malformed
- Provides fallback tool creation status

## Testing Recommendations

### Test with Different MCP Servers
1. **Well-behaved servers**: Should work exactly as before
2. **Missing input_schema**: Should now work with default schema
3. **Malformed tools**: Should create fallback tools and continue
4. **Mixed tool responses**: Should handle valid and invalid tools in the same response

### Example Test Scenarios
```bash
# Connect to a server with missing input_schema
/mcp add test-server stdio some-mcp-server
/mcp connect test-server
/mcp tools  # Should show tools even if some had missing schemas

# Check logs for detailed parsing information
# Look for "Failed to parse tool" warnings
# Look for "Created fallback tool" messages
```

## Files Modified
- `src/mcp.rs`: Enhanced McpTool struct and tool loading logic

## Backward Compatibility
- Fully backward compatible with existing MCP servers
- No breaking changes to the API
- Existing functionality unchanged

## Future Considerations
- Consider adding a strict mode that rejects tools without proper schemas
- Add configuration to control fallback behavior
- Implement tool schema validation warnings