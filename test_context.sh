#!/bin/bash

# Test script for context files feature

echo "🧪 Testing Context Files Feature"
echo "================================"

# Build the project first
echo "📦 Building project..."
cargo build --quiet
if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi
echo "✅ Build successful!"

# Test 1: Help command shows context files info
echo ""
echo "📋 Test 1: Help command includes context files info"
./target/debug/aixplosion --help | grep -A 2 "Context files"
if [ $? -eq 0 ]; then
    echo "✅ Help shows context files info"
else
    echo "❌ Help missing context files info"
fi

# Test 2: Automatic AGENTS.md inclusion
echo ""
echo "📋 Test 2: Automatic AGENTS.md inclusion"
if [ -f "AGENTS.md" ]; then
    echo "✅ AGENTS.md exists - should be auto-included"
else
    echo "ℹ️  AGENTS.md not found - auto-inclusion not applicable"
fi

# Test 3: Explicit context file
echo ""
echo "📋 Test 3: Explicit context file"
echo "Testing with README.md as context..."
echo "What is this project about?" | ./target/debug/aixplosion -f README.md --non-interactive 2>/dev/null | head -5

# Test 4: Multiple context files
echo ""
echo "📋 Test 4: Multiple context files"
echo "Testing with multiple files..."
echo "Describe this Rust project" | ./target/debug/aixplosion -f README.md -f Cargo.toml --non-interactive 2>/dev/null | head -5

# Test 5: Error handling
echo ""
echo "📋 Test 5: Error handling for non-existent file"
echo "Testing error handling..."
echo "Test message" | ./target/debug/aixplosion -f nonexistent.md --non-interactive 2>&1 | grep -i "failed\|error" | head -3

echo ""
echo "🎉 Testing completed!"
echo "Run individual tests manually for detailed verification."