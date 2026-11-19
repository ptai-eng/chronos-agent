//! Optional Hybrid Compiler LSP Integration (`LspSemanticAdapter` & `chronos lsp-hydrate`).
//!
//! Layers compiler-grade cross-module reference resolution on top of `AstSemanticAdapter`
//! while keeping the default `<15ms` zero-daemon experience intact.

pub mod adapter;
pub mod discovery;
pub mod transport;

pub use adapter::{
    byte_offset_to_utf16_position, path_to_file_uri, resolve_references_hybrid,
    uri_to_relative_path, utf16_position_to_byte_offset, LspHydrationResult, LspSemanticAdapter,
    DEFAULT_LSP_HYDRATE_TIMEOUT, DEFAULT_LSP_QUERY_TIMEOUT,
};
pub use discovery::{discover_lsp_command, LspCommandSpec, LspLanguage};
