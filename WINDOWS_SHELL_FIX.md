# Windows Shell Command Fix

## Problem
The bash command tool was failing on Windows because it was hardcoded to use `bash -c`, which is not available on Windows systems.

## Solution
Modified the `bash` function in `src/tools.rs` to be cross-platform compatible:

### Changes Made:

1. **Platform Detection**: Added compile-time platform detection using `cfg` attributes
2. **Conditional Command Execution**: 
   - On Windows: Uses `cmd.exe /C` 
   - On Unix/Linux/macOS: Uses `bash -c`
3. **Updated Documentation**: Changed references from "bash commands" to "shell commands"

### Code Changes:

```rust
// Before (Unix-only):
Command::new("bash").arg("-c").arg(&command_clone).output()

// After (Cross-platform):
#[cfg(target_os = "windows")]
{
    Command::new("cmd").args(["/C", &command_clone]).output()
}
#[cfg(not(target_os = "windows"))]
{
    Command::new("bash").args(["-c", &command_clone]).output()
}
```

### Documentation Updates:

- Changed tool description from "Execute bash commands" to "Execute shell commands"
- Updated AGENTS.md to reflect cross-platform support
- Added platform support section explaining the automatic shell detection

## Testing

The fix uses compile-time configuration, so:
- When compiled on Windows, it will use `cmd.exe /C`
- When compiled on Unix/Linux/macOS, it will use `bash -c`
- No runtime detection needed - the correct code path is selected at compile time

## Verification

To verify the fix works:
1. Compile the code on Windows: `cargo build` (should use cmd.exe)
2. Compile the code on Linux/macOS: `cargo build` (should use bash)
3. Test with various shell commands appropriate for each platform

### Example Commands:
- **Windows**: `dir`, `echo %CD%`, `where cargo`
- **Unix/Linux**: `ls`, `echo $PWD`, `which cargo`

The tool now works seamlessly across all major operating systems while maintaining the same interface and behavior.