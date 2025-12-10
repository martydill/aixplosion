# Glob Tool Documentation

## Overview

The `glob` tool is a read-only utility that allows finding files and directories using glob patterns. It provides powerful pattern matching capabilities similar to shell globbing, making it easy to locate files based on their names, paths, and extensions.

## Features

- **Read-only**: Safe to use in any context without requiring permissions
- **Pattern matching**: Supports standard glob patterns including wildcards and recursive searches
- **File information**: Shows file sizes and distinguishes between files and directories
- **Base path support**: Can search from any directory, not just the current one
- **Cross-platform**: Works on Windows, macOS, and Linux

## Usage

### Basic Syntax

```json
{
  "pattern": "*.rs",
  "base_path": "src"
}
```

### Pattern Examples

#### Simple Wildcards
- `*.rs` - All Rust files in the current directory
- `test_*` - All files starting with "test_"
- `*.json` - All JSON files

#### Directory Patterns
- `src/**/*.rs` - All Rust files in src directory and subdirectories
- `docs/*` - All files in docs directory
- `**/*.md` - All Markdown files in current directory and subdirectories

#### Complex Patterns
- `src/**/mod.rs` - All mod.rs files in src directory tree
- `tests/**/*_test.rs` - All test files in tests directory
- `target/**/*.exe` - All executable files in target directory

#### Character Classes
- `src/[abc]*.rs` - Files starting with a, b, or c in src
- `config/*.toml` - All TOML files in config directory

### Parameters

- **pattern** (required): Glob pattern to match files
- **base_path** (optional): Base directory to search from (default: current directory)

## Return Format

The tool returns a formatted list showing:
- 📁 for directories with size information
- 📄 for files with their size in bytes
- Total count of items found
- Error messages if the pattern is invalid

## Examples

### Find all Rust source files
```json
{
  "pattern": "**/*.rs"
}
```

### Find configuration files
```json
{
  "pattern": "*.toml",
  "base_path": "."
}
```

### Find all test files
```json
{
  "pattern": "**/*test*.rs",
  "base_path": "src"
}
```

### Find all documentation
```json
{
  "pattern": "**/*.md"
}
```

## Security Considerations

The glob tool is read-only and:
- Cannot modify or delete files
- Cannot access files outside the allowed search directories
- Requires no special permissions
- Safe to use in plan mode and restricted contexts

## Integration

The glob tool is automatically available as a default tool when creating new agents and is included in the list of read-only tools that can be used in plan mode.