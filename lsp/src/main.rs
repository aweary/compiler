use log::info;
use lsp_server::Connection;
use lsp_types::ServerCapabilities;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Sync + Send>>;

fn server_capabilities() -> serde_json::Value {
    // use lsp_types::{
    //     HoverProviderCapability,
    //     SelectionRangeProviderCapability,
    //     TextDocumentSyncCapability,
    //     CompletionCapability,
    //     SignatureHelpCapability,
    //     TypeDefinitionProviderCapability,
    //     ImplementationProviderCapability,
    //     CodeActionProviderCapability,
    //     CodeLensOptions,
    //     DocumentOnTypeFormattingOptions,
    //     RenameProviderCapability,
    //     DocumentLinkOptions,
    //     ColorProviderCapability,
    //     FoldingRangeProviderCapability,
    //     ExecuteCommandOptions,
    //     WorkspaceCapability,
    //     SemanticHighlightingServerCapability,
    //     CallHierarchyServerCapability
    // };
    let capabilities = ServerCapabilities {
        text_document_sync: None,
        selection_range_provider: None,
        hover_provider: None,
        completion_provider: None,
        signature_help_provider: None,
        definition_provider: None,
        type_definition_provider: None,
        implementation_provider: None,
        references_provider: None,
        document_highlight_provider: None,
        document_symbol_provider: None,
        workspace_symbol_provider: None,
        code_action_provider: None,
        code_lens_provider: None,
        document_formatting_provider: None,
        document_range_formatting_provider: None,
        document_on_type_formatting_provider: None,
        rename_provider: None,
        document_link_provider: None,
        color_provider: None,
        folding_range_provider: None,
        declaration_provider: None,
        execute_command_provider: None,
        workspace: None,
        experimental: None,
        semantic_highlighting: None,
        call_hierarchy_provider: None,
        semantic_tokens_provider: None,
    };
    serde_json::to_value(capabilities).unwrap()
}

fn main() -> Result<()> {
    flexi_logger::Logger::with_str("info").start().unwrap();
    info!("Starting LSP server");
    let (connnection, io_threads) = Connection::stdio();
    let server_capabilities = server_capabilities();
    let initialization_params = connnection.initialize(server_capabilities)?;
    main_loop(&connnection, initialization_params)?;
    io_threads.join()?;
    Ok(())
}

fn main_loop(_connection: &Connection, _params: serde_json::Value) -> Result<()> {
    info!("Starting LSP server loop");
    Ok(())
    // ...
}
