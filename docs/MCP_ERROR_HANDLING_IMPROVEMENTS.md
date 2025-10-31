# MCP Error Handling Improvements

## Problem Investigation
The issue was that MCP server errors were failing silently, making it difficult to diagnose why tool calls weren't working when MCP servers encountered problems.

## Root Causes Identified
1. **Silent failures in MCP message handling loop**: Connection errors were logged but didn't clearly indicate tool unavailability
2. **Vague error messages**: MCP tool execution errors didn't provide enough context about the underlying issue
3. **Missing timeouts**: No timeout for MCP server responses, could hang indefinitely
4. **No connection health checks**: Tool calls proceeded without verifying the MCP server was still running
5. **Inadequate error logging during startup**: MCP connection failures during initialization weren't prominent enough

## Improvements Made

### 1. Enhanced MCP Connection Error Logging
**File**: `src/mcp.rs` (line 275-277)
- Before: Only logged the error
- After: Added clear message about tool unavailability
```rust
error!("MCP server {} connection broken - tools may be unavailable", name);
```

### 2. Better MCP Tool Execution Error Messages
**File**: `src/agent.rs` (line 348-355)
- Before: Generic "MCP tool call failed" message
- After: Specific error mentioning server name and potential unavailability
```rust
error!("MCP server '{}' may have encountered an error or is unavailable", server_name);
let error_content = format!("MCP tool call failed: {}. Check if MCP server '{}' is running and responsive.", e, server_name);
```

### 3. Added Response Timeouts
**File**: `src/mcp.rs` (line 485-490)
- Added 30-second timeout for MCP server responses
- Clear error message when timeout occurs
```rust
let response = tokio::time::timeout(
    std::time::Duration::from_secs(30),
    rx
).await
    .map_err(|_| anyhow::anyhow!("MCP server '{}' timed out after 30 seconds", self.name))?;
```

### 4. Connection Health Checks
**File**: `src/mcp.rs` (line 445-458)
- Verify process is still running before making tool calls
- Clear error when process has terminated
```rust
if let Some(ref mut process) = self.process {
    match process.try_wait() {
        Ok(Some(_)) => {
            return Err(anyhow::anyhow!("MCP server '{}' process has terminated", self.name));
        }
        // ...
    }
}
```

### 5. Enhanced Logging During Tool Calls
**File**: `src/mcp.rs` (line 460, 474, 478)
- Added debug logging for tool call start and success
- Added error logging when tool calls fail
```rust
debug!("Calling MCP tool '{}' on server '{}'", name, self.name);
error!("MCP tool '{}' failed on server '{}': {:?}", name, self.name, error);
debug!("MCP tool '{}' completed successfully on server '{}'", name, self.name);
```

### 6. Better Startup Error Handling
**File**: `src/main.rs` (line 651-652, 657-658)
- Enhanced error messages during MCP server connection failures
- Clear indication when MCP tools might not be available
```rust
error!("Some MCP servers may not be available - tool calls to those servers will fail");
error!("MCP tools were not loaded - tool calls to MCP servers will fail");
```

### 7. Improved Tool Refresh Error Handling
**File**: `src/agent.rs` (line 231-232)
- Added prominent error logging when MCP tool refresh fails
```rust
error!("MCP tools may not be available - some tool calls might fail");
```

## Testing
The improvements were tested by:
1. Running the agent with debug logging to verify MCP server connections
2. Confirming that MCP server startup errors are now more visible
3. Verifying that tool execution errors provide clearer diagnostic information

## Benefits
1. **Faster debugging**: Users can now quickly identify MCP server issues
2. **Better user experience**: Clear error messages guide users to fix configuration issues
3. **Improved reliability**: Timeouts prevent hanging on unresponsive MCP servers
4. **Enhanced monitoring**: Connection health checks catch issues early
5. **Comprehensive logging**: All failure points now have appropriate error messages

## Example Error Messages

### Before
```
[WARN] Failed to refresh MCP tools: connection error
[ERROR] Error executing MCP tool 'mcp_server_tool': connection refused
```

### After
```
[WARN] Failed to refresh MCP tools: connection error
[ERROR] MCP tools may not be available - some tool calls might fail
[ERROR] Error executing MCP tool 'mcp_yolink_getDevice': connection refused
[ERROR] MCP server 'yolink' may have encountered an error or is unavailable
[ERROR] MCP tool call failed: connection refused. Check if MCP server 'yolink' is running and responsive.
```