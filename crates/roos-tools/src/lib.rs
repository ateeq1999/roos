// roos-tools — standard tool belt (feature-flagged).

#[cfg(feature = "tools-fs")]
pub mod fs;
#[cfg(feature = "tools-fs")]
pub use fs::{ListDirectoryTool, ReadFileTool, WriteFileTool};

#[cfg(feature = "tools-shell")]
pub mod shell;
#[cfg(feature = "tools-shell")]
pub use shell::ExecuteShellTool;

#[cfg(feature = "tools-http")]
pub mod http;
#[cfg(feature = "tools-http")]
pub use http::{HttpGetTool, HttpPostTool};

#[cfg(feature = "tools-web")]
pub mod web;
#[cfg(feature = "tools-web")]
pub use web::SearchWebTool;
