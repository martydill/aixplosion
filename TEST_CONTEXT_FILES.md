# Testing Context Files Feature

## Test Cases

### 1. Basic Functionality
```bash
# Test with explicit context file
ai-agent -f README.md -m "Summarize the project"

# Test automatic AGENTS.md inclusion
ai-agent -m "What agents are available?"
```

### 2. Multiple Files
```bash
# Test multiple context files
ai-agent -f README.md -f Cargo.toml -m "Describe this Rust project"
```

### 3. Error Handling
```bash
# Test with non-existent file
ai-agent -f nonexistent.md -m "Test error handling"

# Test with unreadable file (if possible)
ai-agent -f /root/protected.md -m "Test permission error"
```

### 4. Interactive Mode
```bash
# Test context files in interactive mode
ai-agent -f README.md
> "What is this project about?"
> "How do I build it?"
```

### 5. Non-interactive Mode
```bash
# Test with stdin
echo "Explain the project" | ai-agent -f README.md --non-interactive
```

## Expected Behavior

1. **Success Cases**:
   - Files are read and content is added as context
   - AI responses should reference the provided context
   - Multiple files should be processed in order
   - AGENTS.md should be auto-included when present

2. **Error Cases**:
   - Non-existent files should show error but not crash
   - Unreadable files should show appropriate error message
   - Agent should continue processing even with context file errors

3. **Performance**:
   - Large files should be handled efficiently
   - Context should not significantly impact response time
   - Memory usage should remain reasonable