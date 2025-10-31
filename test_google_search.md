# Google Search Tool Test

This file contains test cases and examples for the Google search functionality.

## Usage Examples

### Basic Search
```bash
aixplosion "Search for Rust programming language"
```

### Search with Specific Number of Results
```bash
aixplosion "Find information about machine learning, return 5 results"
```

### Technical Searches
```bash
aixplosion "What are the best practices for REST API design?"
aixplosion "How to implement authentication in Node.js applications?"
```

### Current Events
```bash
aixplosion "Latest developments in artificial intelligence"
aixplosion "New features in Rust 2024"
```

## Expected Output Format

The Google search tool returns results in the following format:

```
Google Search Results for 'query':
============================================================

1. Result Title
   URL: https://example.com/url
   Description of the search result with relevant information...

2. Another Result Title
   URL: https://another-example.com/url
   Another description that provides context about the search result...

Found X results
```

## Implementation Details

The Google search tool uses:
- **headless_chrome**: For browser automation
- **scraper**: For HTML parsing and extraction
- **urlencoding**: For proper URL encoding of search queries

### Features
- Headless browser automation to avoid detection
- Multiple CSS selectors to handle Google's changing HTML structure
- Automatic URL cleaning and extraction from Google redirects
- Configurable number of results (1-20, default: 10)
- Error handling and fallback mechanisms

### Security Considerations
- Searches are performed in a sandboxed headless browser
- No persistent cookies or browsing history
- Automatic browser cleanup after each search
- Rate limiting through natural browser behavior

## Troubleshooting

### Common Issues

1. **Browser Launch Failure**
   - Ensure Chrome/Chromium is installed
   - Check system permissions for browser automation

2. **No Results Found**
   - Verify internet connectivity
   - Check if Google is accessible from your network
   - Try simpler search queries

3. **Parsing Errors**
   - Google may have changed their HTML structure
   - The tool includes multiple fallback selectors
   - Results may vary based on location and language settings

### Error Messages

- "Failed to create browser launch options": Chrome installation issue
- "Failed to navigate to search URL": Network connectivity problem
- "No results found": No matching search results or parsing failure
- "Google search failed": General error with detailed message

## Testing

To test the Google search functionality:

1. Simple search query
2. Search with special characters
3. Search with quotes for exact phrases
4. Search with different result counts
5. Error handling with invalid queries

## Performance

- Typical search time: 3-8 seconds
- Browser startup: ~1-2 seconds
- Page loading: ~2-5 seconds
- Parsing and extraction: ~0.5 seconds

The tool is optimized for reliability over speed, ensuring consistent results across different Google page layouts.