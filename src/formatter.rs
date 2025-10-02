use std::io::{self, Write};
use console::Term;
use colored::*;
use anyhow::Result;
use regex::Regex;

pub struct CodeFormatter {
    term: Term,
    code_block_regex: Regex,
}

impl CodeFormatter {
    pub fn new() -> Result<Self> {
        let code_block_regex = Regex::new(r"```(\w*)\n([\s\S]*?)```")?;
        let term = Term::stdout();

        Ok(Self {
            term,
            code_block_regex,
        })
    }

    pub fn format_response(&self, response: &str) -> Result<String> {
        let formatted = self.format_text_with_code_blocks(response)?;
        Ok(formatted)
    }

    fn format_text_with_code_blocks(&self, text: &str) -> Result<String> {
        let mut result = String::new();
        let mut last_end = 0;

        for caps in self.code_block_regex.captures_iter(text) {
            let full_match = caps.get(0).unwrap();
            let lang = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let code = caps.get(2).unwrap().as_str();

            // Add text before the code block
            result.push_str(&text[last_end..full_match.start()]);

            // Format and add the code block
            let formatted_code = self.format_code_block(code, lang)?;
            result.push_str(&formatted_code);

            last_end = full_match.end();
        }

        // Add remaining text after the last code block
        result.push_str(&text[last_end..]);

        Ok(result)
    }

    fn format_code_block(&self, code: &str, lang: &str) -> Result<String> {
        let mut result = String::new();

        // Normalize language name
        let normalized_lang = self.normalize_language(lang);

        // Add header with language info
        let header = format!(
            "{}{} {} {}{}",
            "┌".bold().cyan(),
            " ".repeat(2),
            normalized_lang.to_uppercase().bold().white(),
            " ".repeat(2),
            "┐".bold().cyan()
        );
        result.push_str(&header);
        result.push('\n');

        // Add code content with line numbers and syntax highlighting
        let lines: Vec<&str> = code.lines().collect();
        let max_line_num = lines.len();
        let line_num_width = max_line_num.to_string().len();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let line_num_str = format!("{:>width$}", line_num, width = line_num_width);

            result.push_str(&format!("{} │ ",
                line_num_str.dimmed().cyan()
            ));

            // Apply basic syntax highlighting based on language
            let highlighted_line = self.highlight_line(line, &normalized_lang);
            result.push_str(&highlighted_line);
            result.push('\n');
        }

        // Add footer
        let footer_width = line_num_width + 4 + normalized_lang.len();
        let footer = format!(
            "{}{}{}",
            "└".bold().cyan(),
            "─".repeat(footer_width).cyan(),
            "┘".bold().cyan()
        );
        result.push_str(&footer);
        result.push('\n');

        Ok(result)
    }

    fn highlight_line(&self, line: &str, lang: &str) -> String {
        match lang {
            "rust" => self.highlight_rust(line),
            "python" => self.highlight_python(line),
            "javascript" | "js" | "jsx" => self.highlight_javascript(line),
            "typescript" | "ts" | "tsx" => self.highlight_typescript(line),
            "json" => self.highlight_json(line),
            "yaml" | "yml" => self.highlight_yaml(line),
            "html" => self.highlight_html(line),
            "css" => self.highlight_css(line),
            "bash" | "sh" => self.highlight_bash(line),
            "sql" => self.highlight_sql(line),
            "markdown" | "md" => self.highlight_markdown(line),
            "toml" => self.highlight_toml(line),
            "xml" => self.highlight_xml(line),
            "c" | "cpp" | "c++" => self.highlight_c_cpp(line),
            "java" => self.highlight_java(line),
            "go" => self.highlight_go(line),
            _ => line.normal().to_string(),
        }
    }

    // Basic syntax highlighting for various languages
    fn highlight_rust(&self, line: &str) -> String {
        let mut result = line.to_string();

        // Keywords
        let keywords = ["fn", "let", "mut", "const", "static", "if", "else", "match", "for", "while", "loop", "break", "continue", "return", "struct", "enum", "impl", "trait", "mod", "use", "pub", "crate", "super", "self", "Self", "where", "async", "await", "move", "ref", "unsafe", "extern"];
        for keyword in &keywords {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(keyword))).unwrap();
            result = regex.replace_all(&result, keyword.bold().blue().to_string()).to_string();
        }

        // Types
        let types = ["String", "str", "Vec", "Option", "Result", "Box", "Rc", "Arc", "Cell", "RefCell", "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64", "bool", "char"];
        for type_name in &types {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(type_name))).unwrap();
            result = regex.replace_all(&result, type_name.bold().cyan().to_string()).to_string();
        }

        // Strings
        let string_regex = Regex::new(r#""([^"\\]|\\.)*""#).unwrap();
        result = string_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("\"{}\"", &caps[0][1..caps[0].len()-1].green())
        }).to_string();

        // Comments
        if result.starts_with("//") {
            result = result.dimmed().to_string();
        } else if let Some(pos) = result.find("//") {
            let (before, after) = result.split_at(pos);
            result = format!("{}{}", before, after.dimmed());
        }

        result
    }

    fn highlight_python(&self, line: &str) -> String {
        let mut result = line.to_string();

        // Keywords
        let keywords = ["def", "class", "if", "elif", "else", "for", "while", "try", "except", "finally", "with", "as", "import", "from", "return", "yield", "lambda", "and", "or", "not", "in", "is", "None", "True", "False", "pass", "break", "continue", "global", "nonlocal", "async", "await"];
        for keyword in &keywords {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(keyword))).unwrap();
            result = regex.replace_all(&result, keyword.bold().blue().to_string()).to_string();
        }

        // Strings
        let string_regex = Regex::new(r#"'([^'\\]|\\.)*'|"""([^"\\]|\\.)*"""|"([^"\\]|\\.)*"|"""([^"\\]|\\.)*"""#).unwrap();
        result = string_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].green().to_string()
        }).to_string();

        // Comments
        if result.starts_with("#") {
            result = result.dimmed().to_string();
        } else if let Some(pos) = result.find("#") {
            let (before, after) = result.split_at(pos);
            result = format!("{}{}", before, after.dimmed());
        }

        result
    }

    fn highlight_javascript(&self, line: &str) -> String {
        let mut result = line.to_string();

        // Keywords
        let keywords = ["function", "const", "let", "var", "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return", "try", "catch", "finally", "throw", "new", "this", "typeof", "instanceof", "in", "of", "class", "extends", "super", "static", "async", "await", "import", "export", "from", "default"];
        for keyword in &keywords {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(keyword))).unwrap();
            result = regex.replace_all(&result, keyword.bold().blue().to_string()).to_string();
        }

        // Strings
        let string_regex = Regex::new(r#"'([^'\\]|\\.)*'|"(?:"([^"\\]|\\.)*")|`([^`\\]|\\.)*`"#).unwrap();
        result = string_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].green().to_string()
        }).to_string();

        // Comments
        if result.starts_with("//") {
            result = result.dimmed().to_string();
        } else if let Some(pos) = result.find("//") {
            let (before, after) = result.split_at(pos);
            result = format!("{}{}", before, after.dimmed());
        }

        result
    }

    fn highlight_typescript(&self, line: &str) -> String {
        // TypeScript is similar to JavaScript but with additional type keywords
        let mut result = self.highlight_javascript(line);

        // TypeScript specific keywords
        let ts_keywords = ["interface", "type", "enum", "namespace", "module", "declare", "abstract", "readonly", "private", "public", "protected", "implements", "keyof", "unknown", "never", "any"];
        for keyword in &ts_keywords {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(keyword))).unwrap();
            result = regex.replace_all(&result, keyword.bold().magenta().to_string()).to_string();
        }

        result
    }

    fn highlight_json(&self, line: &str) -> String {
        let mut result = line.to_string();

        // JSON keys (strings before colons)
        let key_regex = Regex::new(r#""([^"\\]|\\.)*"\s*:"#).unwrap();
        result = key_regex.replace_all(&result, |caps: &regex::Captures| {
            let key_part = &caps[0][..caps[0].len()-1];
            format!("{}:", key_part.bold().cyan())
        }).to_string();

        // JSON string values
        let string_regex = Regex::new(r#":\s*"([^"\\]|\\.)*""#).unwrap();
        result = string_regex.replace_all(&result, |caps: &regex::Captures| {
            format!(": {}", &caps[0][2..].green())
        }).to_string();

        // JSON numbers and booleans
        let value_regex = Regex::new(r":\s*(true|false|null|\d+\.?\d*)").unwrap();
        result = value_regex.replace_all(&result, |caps: &regex::Captures| {
            format!(": {}", &caps[0][2..].yellow())
        }).to_string();

        result
    }

    fn highlight_yaml(&self, line: &str) -> String {
        let mut result = line.to_string();

        // YAML keys (before colons)
        if let Some(colon_pos) = result.find(':') {
            let (key, rest) = result.split_at(colon_pos);
            result = format!("{}{}", key.bold().cyan(), rest);
        }

        // YAML string values
        let string_regex = Regex::new(r#":\s*["'][^"']*["']"#).unwrap();
        result = string_regex.replace_all(&result, |caps: &regex::Captures| {
            format!(": {}", &caps[0][2..].green())
        }).to_string();

        // YAML numbers and booleans
        let value_regex = Regex::new(r":\s*(true|false|null|\d+\.?\d*)").unwrap();
        result = value_regex.replace_all(&result, |caps: &regex::Captures| {
            format!(": {}", &caps[0][2..].yellow())
        }).to_string();

        result
    }

    fn highlight_html(&self, line: &str) -> String {
        let mut result = line.to_string();

        // HTML tags
        let tag_regex = Regex::new(r"</?[^>]+>").unwrap();
        result = tag_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].blue().to_string()
        }).to_string();

        // HTML attributes
        let attr_regex = Regex::new(r#"(\w+)=["'][^"']*["']"#).unwrap();
        result = attr_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("{}={}", caps[1].cyan(), &caps[0][caps[1].len()..].green())
        }).to_string();

        result
    }

    fn highlight_css(&self, line: &str) -> String {
        let mut result = line.to_string();

        // CSS selectors and properties
        let selector_regex = Regex::new(r"[.#]?[\w-]+\s*\{").unwrap();
        result = selector_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].bold().blue().to_string()
        }).to_string();

        // CSS properties
        let prop_regex = Regex::new(r"[\w-]+:").unwrap();
        result = prop_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].cyan().to_string()
        }).to_string();

        // CSS values
        let value_regex = Regex::new(r":\s*[^;]+;?").unwrap();
        result = value_regex.replace_all(&result, |caps: &regex::Captures| {
            format!(": {}", &caps[0][2..].green())
        }).to_string();

        result
    }

    fn highlight_bash(&self, line: &str) -> String {
        let mut result = line.to_string();

        // Bash commands
        let commands = ["if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "function", "return", "exit", "export", "local", "readonly", "declare", "typeset", "alias", "unalias", "cd", "pwd", "ls", "mkdir", "rmdir", "rm", "cp", "mv", "ln", "cat", "less", "more", "head", "tail", "grep", "sed", "awk", "sort", "uniq", "wc", "find", "locate", "which", "whereis", "man", "echo", "printf", "read", "trap", "wait", "jobs", "fg", "bg", "kill", "ps", "top", "df", "du", "free", "uname", "uptime", "date", "cal", "tar", "gzip", "gunzip", "zip", "unzip", "ssh", "scp", "rsync", "git", "make", "gcc", "g++", "python", "python3", "node", "npm", "yarn", "docker", "kubectl"];
        for command in &commands {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(command))).unwrap();
            result = regex.replace_all(&result, command.bold().green().to_string()).to_string();
        }

        // Strings
        let string_regex = Regex::new(r#"'([^'\\]|\\.)*'|"(?:[^"\\]|\\.)*""#).unwrap();
        result = string_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].yellow().to_string()
        }).to_string();

        // Comments
        if result.starts_with("#") {
            result = result.dimmed().to_string();
        }

        result
    }

    fn highlight_sql(&self, line: &str) -> String {
        let mut result = line.to_string();

        // SQL keywords
        let keywords = ["SELECT", "FROM", "WHERE", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "DROP", "TABLE", "INDEX", "DATABASE", "SCHEMA", "PRIMARY", "FOREIGN", "KEY", "REFERENCES", "JOIN", "INNER", "LEFT", "RIGHT", "FULL", "OUTER", "ON", "GROUP", "BY", "ORDER", "HAVING", "LIMIT", "OFFSET", "UNION", "ALL", "DISTINCT", "COUNT", "SUM", "AVG", "MIN", "MAX", "AND", "OR", "NOT", "IN", "EXISTS", "BETWEEN", "LIKE", "ILIKE", "NULL", "IS", "AS", "CASE", "WHEN", "THEN", "ELSE", "END", "IF", "COALESCE", "CAST", "CONVERT", "TRY_CAST", "TRY_CONVERT"];
        for keyword in &keywords {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(keyword))).unwrap();
            result = regex.replace_all(&result, keyword.bold().blue().to_string()).to_string();
        }

        // SQL identifiers (backticks, brackets, quotes)
        let ident_regex = Regex::new(r#"[`'"\[\]]([^`'"\[\]]*)[`'"\[\]]"#).unwrap();
        result = ident_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].cyan().to_string()
        }).to_string();

        result
    }

    fn highlight_markdown(&self, line: &str) -> String {
        let mut result = line.to_string();

        // Headers
        if result.starts_with('#') {
            let header_level = result.chars().take_while(|&c| c == '#').count();
            let remaining = &result[header_level..];
            result = format!("{}{}", "#".repeat(header_level).bold().red(), remaining.bold());
        }

        // Bold text
        let bold_regex = Regex::new(r"\*\*([^*]+)\*\*").unwrap();
        result = bold_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].bold().to_string()
        }).to_string();

        // Italic text
        let italic_regex = Regex::new(r"\*([^*]+)\*").unwrap();
        result = italic_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].italic().to_string()
        }).to_string();

        // Code inline
        let code_regex = Regex::new(r"`([^`]+)`").unwrap();
        result = code_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("`{}`", caps[1].black().on_white())
        }).to_string();

        // Links
        let link_regex = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
        result = link_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("[{}]({})", caps[1].blue().underline(), caps[2].dimmed())
        }).to_string();

        result
    }

    fn highlight_toml(&self, line: &str) -> String {
        let mut result = line.to_string();

        // TOML sections
        if result.starts_with('[') && result.ends_with(']') {
            result = result.bold().blue().to_string();
        }

        // TOML keys
        if let Some(eq_pos) = result.find('=') {
            let (key, rest) = result.split_at(eq_pos);
            result = format!("{}{}", key.cyan(), rest);
        }

        // TOML strings
        let string_regex = Regex::new(r#"="([^"\\]|\\.)*"|'([^'\\]|\\.)*'"#).unwrap();
        result = string_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[0].green().to_string()
        }).to_string();

        result
    }

    fn highlight_xml(&self, line: &str) -> String {
        self.highlight_html(line) // XML highlighting is similar to HTML
    }

    fn highlight_c_cpp(&self, line: &str) -> String {
        let mut result = line.to_string();

        // C/C++ keywords
        let keywords = ["int", "char", "float", "double", "void", "long", "short", "unsigned", "signed", "const", "static", "extern", "auto", "register", "volatile", "sizeof", "typedef", "struct", "union", "enum", "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return", "goto", "include", "define", "ifdef", "ifndef", "endif", "class", "public", "private", "protected", "virtual", "inline", "friend", "operator", "new", "delete", "this", "namespace", "using", "template", "typename"];
        for keyword in &keywords {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(keyword))).unwrap();
            result = regex.replace_all(&result, keyword.bold().blue().to_string()).to_string();
        }

        // Preprocessor directives
        if result.starts_with('#') {
            result = result.bold().magenta().to_string();
        }

        // Comments
        if result.starts_with("//") {
            result = result.dimmed().to_string();
        } else if result.starts_with("/*") || result.contains("*/") {
            result = result.dimmed().to_string();
        }

        result
    }

    fn highlight_java(&self, line: &str) -> String {
        let mut result = line.to_string();

        // Java keywords
        let keywords = ["public", "private", "protected", "static", "final", "abstract", "synchronized", "volatile", "transient", "native", "strictfp", "class", "interface", "extends", "implements", "import", "package", "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return", "throw", "throws", "try", "catch", "finally", "new", "this", "super", "null", "true", "false", "instanceof", "enum", "assert"];
        for keyword in &keywords {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(keyword))).unwrap();
            result = regex.replace_all(&result, keyword.bold().blue().to_string()).to_string();
        }

        // Annotations
        if result.starts_with('@') {
            result = result.bold().magenta().to_string();
        }

        result
    }

    fn highlight_go(&self, line: &str) -> String {
        let mut result = line.to_string();

        // Go keywords
        let keywords = ["break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "type", "var"];
        for keyword in &keywords {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(keyword))).unwrap();
            result = regex.replace_all(&result, keyword.bold().blue().to_string()).to_string();
        }

        // Go types
        let types = ["int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16", "uint32", "uint64", "float32", "float64", "complex64", "complex128", "bool", "string", "byte", "rune"];
        for type_name in &types {
            let regex = Regex::new(&format!(r"\b{}\b", regex::escape(type_name))).unwrap();
            result = regex.replace_all(&result, type_name.bold().cyan().to_string()).to_string();
        }

        result
    }

    fn normalize_language<'a>(&self, lang: &'a str) -> &'a str {
        match lang.to_lowercase().as_str() {
            "js" => "javascript",
            "ts" => "typescript",
            "jsx" => "javascript",
            "tsx" => "typescript",
            "py" => "python",
            "rb" => "ruby",
            "sh" | "bash" | "zsh" => "bash",
            "yml" => "yaml",
            "rs" => "rust",
            "c" => "c",
            "cpp" | "cxx" | "cc" => "cpp",
            "md" => "markdown",
            _ => lang,
        }
    }

    pub fn print_formatted(&self, text: &str) -> Result<()> {
        let formatted = self.format_response(text)?;
        print!("{}", formatted);
        io::stdout().flush()?;
        Ok(())
    }
}

pub fn create_code_formatter() -> Result<CodeFormatter> {
    CodeFormatter::new()
}