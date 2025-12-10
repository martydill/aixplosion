// Tools module - re-exports all tool functionality

pub mod types;
pub mod builtin;
pub mod mcp;
pub mod list_directory;
pub mod read_file;
pub mod search_in_files;
pub mod glob;
pub mod write_file;
pub mod edit_file;
pub mod delete_file;
pub mod create_directory;
pub mod bash;

// New display system modules
pub mod registry;
pub mod display;

// Re-export all public types and functions for backward compatibility
pub use types::*;
pub use builtin::*;
pub use mcp::*;

// Re-export new display system
pub use registry::*;
pub use display::{DisplayFactory, ToolDisplay};

// Re-export tool creation functions for security manager integration
pub use write_file::{write_file, create_write_file_tool};
pub use edit_file::{edit_file, create_edit_file_tool};
pub use delete_file::{delete_file, create_delete_file_tool};
pub use create_directory::{create_directory, create_create_directory_tool};
pub use bash::{bash, create_bash_tool};
pub use glob::{glob_files, create_glob_tool};