use std::{collections::HashMap, fs::OpenOptions, path::PathBuf};

use clap::Parser;
use context::Context;
use eyre::{eyre, Result, WrapErr as _};
use lsp_server::Message;
use verbosity::Verbosity;

use crate::{client::Client, server::Server};

mod client;
mod context;
mod event;
mod handler;
mod script;
mod server;
mod verbosity;

// TODO: Support --port (TCP socket) and --pipe (socket file)

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The rust-script script file path.
    script: PathBuf,

    /// The rust-script executable path.
    #[arg(long, default_value = "rust-script")]
    rust_script: PathBuf,

    /// The rust-analyzer executable path.
    #[arg(long, default_value = "rust-analyzer")]
    rust_analyzer: PathBuf,

    /// The commandline arguments to give to rust-analyzer.
    #[arg(last = true)]
    rust_analyzer_args: Vec<String>,

    /// The file to use as the log output instead of stderr.
    #[arg(short('o'), long)]
    log_file: Option<PathBuf>,

    #[command(flatten)]
    verbosity: Verbosity<verbosity::WarnLevel>,
}

fn init_tracing_subscriber(args: &Args) {
    let fmt = tracing_subscriber::fmt().with_max_level(args.verbosity.level_filter());
    if let Some(ref file) = args.log_file {
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(file)
            .unwrap();
        fmt.with_writer(file).init();
    } else {
        fmt.with_writer(std::io::stderr).init();
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing_subscriber(&args);

    tracing::debug!(?args);

    let script =
        script::Script::new(args.rust_script, args.script).wrap_err("failed to scriptize")?;
    script.regenerate().wrap_err("failed to generate package")?;

    let (event_sender, event_receiver) = event::new_event_bus();

    let client = Client::stdio(event_sender.clone());
    let server = Server::spawn(
        event_sender,
        script.package_dir(),
        args.rust_analyzer,
        args.rust_analyzer_args,
    )
    .wrap_err("failed to spawn server")?;

    let mut requests_from_client = HashMap::new();
    let mut requests_from_server = HashMap::new();

    for event in event_receiver.into_iter() {
        let mut context = Context::new(&script);
        match event {
            event::Event::ClientToServer(mut message) => {
                tracing::debug!(?message, "Message from client");
                match &mut message {
                    Message::Request(ref mut request) => {
                        use lsp_types::request::*;

                        use crate::handler::handle_client_to_server_request as handle;
                        requests_from_client.insert(request.id.clone(), request.clone());

                        handle::<Initialize>(request, &mut context);
                        handle::<WillSaveWaitUntil>(request, &mut context);
                        handle::<GotoDeclaration>(request, &mut context);
                        handle::<GotoDefinition>(request, &mut context);
                        handle::<GotoTypeDefinition>(request, &mut context);
                        handle::<GotoImplementation>(request, &mut context);
                        handle::<References>(request, &mut context);
                        handle::<CallHierarchyPrepare>(request, &mut context);
                        handle::<CallHierarchyIncomingCalls>(request, &mut context);
                        handle::<CallHierarchyOutgoingCalls>(request, &mut context);
                        handle::<TypeHierarchyPrepare>(request, &mut context);
                        handle::<TypeHierarchySupertypes>(request, &mut context);
                        handle::<TypeHierarchySubtypes>(request, &mut context);
                        handle::<DocumentHighlightRequest>(request, &mut context);
                        handle::<DocumentLinkRequest>(request, &mut context);
                        handle::<DocumentLinkResolve>(request, &mut context);
                        handle::<HoverRequest>(request, &mut context);
                        handle::<CodeLensRequest>(request, &mut context);
                        handle::<CodeLensResolve>(request, &mut context);
                        handle::<FoldingRangeRequest>(request, &mut context);
                        handle::<SelectionRangeRequest>(request, &mut context);
                        handle::<DocumentSymbolRequest>(request, &mut context);
                        handle::<SemanticTokensFullRequest>(request, &mut context);
                        handle::<SemanticTokensFullDeltaRequest>(request, &mut context);
                        handle::<SemanticTokensRangeRequest>(request, &mut context);
                        handle::<InlayHintRequest>(request, &mut context);
                        handle::<InlayHintResolveRequest>(request, &mut context);
                        handle::<InlineValueRequest>(request, &mut context);
                        handle::<MonikerRequest>(request, &mut context);
                        handle::<Completion>(request, &mut context);
                        handle::<ResolveCompletionItem>(request, &mut context);
                        handle::<SignatureHelpRequest>(request, &mut context);
                        handle::<CodeActionRequest>(request, &mut context);
                        handle::<CodeActionResolveRequest>(request, &mut context);
                        handle::<DocumentColor>(request, &mut context);
                        handle::<ColorPresentationRequest>(request, &mut context);
                        handle::<Formatting>(request, &mut context);
                        handle::<RangeFormatting>(request, &mut context);
                        handle::<OnTypeFormatting>(request, &mut context);
                        handle::<Rename>(request, &mut context);
                        handle::<PrepareRenameRequest>(request, &mut context);
                        handle::<LinkedEditingRange>(request, &mut context);
                        handle::<WorkspaceSymbolRequest>(request, &mut context);
                        handle::<WorkspaceSymbolResolve>(request, &mut context);
                        handle::<WillCreateFiles>(request, &mut context);
                        handle::<WillRenameFiles>(request, &mut context);
                        handle::<WillDeleteFiles>(request, &mut context);
                        handle::<ExecuteCommand>(request, &mut context);
                    }
                    Message::Response(ref mut response) => {
                        use lsp_types::request::*;

                        use crate::handler::handle_client_to_server_response as handle;
                        let request = requests_from_server
                            .remove(&response.id)
                            .ok_or(eyre!("Invalid id received from client"))?;

                        handle::<CodeLensRefresh>(&request, response, &mut context);
                        handle::<SemanticTokensRefresh>(&request, response, &mut context);
                        handle::<InlayHintRefreshRequest>(&request, response, &mut context);
                        handle::<InlineValueRefreshRequest>(&request, response, &mut context);
                        handle::<WorkspaceConfiguration>(&request, response, &mut context);
                        handle::<WorkspaceFoldersRequest>(&request, response, &mut context);
                        handle::<ApplyWorkspaceEdit>(&request, response, &mut context);
                        handle::<ShowMessageRequest>(&request, response, &mut context);
                        handle::<ShowDocument>(&request, response, &mut context);
                        handle::<WorkDoneProgressCreate>(&request, response, &mut context);
                    }
                    Message::Notification(ref mut notification) => {
                        use lsp_types::notification::*;

                        use crate::handler::handle_client_to_server_notification as handle;

                        handle::<SetTrace>(notification, &mut context);
                        handle::<DidOpenTextDocument>(notification, &mut context);
                        handle::<DidChangeTextDocument>(notification, &mut context);
                        handle::<WillSaveTextDocument>(notification, &mut context);
                        handle::<DidSaveTextDocument>(notification, &mut context);
                        handle::<DidCloseTextDocument>(notification, &mut context);
                        handle::<DidChangeConfiguration>(notification, &mut context);
                        handle::<DidChangeWorkspaceFolders>(notification, &mut context);
                        handle::<DidCreateFiles>(notification, &mut context);
                        handle::<DidRenameFiles>(notification, &mut context);
                        handle::<DidDeleteFiles>(notification, &mut context);
                        handle::<DidChangeWatchedFiles>(notification, &mut context);
                    }
                }
                server.sender.send(message).wrap_err("Server stopped")?;
            }
            event::Event::ServerToClient(mut message) => {
                tracing::debug!(?message, "Message from server");
                match &mut message {
                    Message::Request(ref mut request) => {
                        use lsp_types::request::*;

                        use crate::handler::handle_server_to_client_request as handle;
                        requests_from_server.insert(request.id.clone(), request.clone());

                        handle::<CodeLensRefresh>(request, &mut context);
                        handle::<SemanticTokensRefresh>(request, &mut context);
                        handle::<InlayHintRefreshRequest>(request, &mut context);
                        handle::<InlineValueRefreshRequest>(request, &mut context);
                        handle::<WorkspaceConfiguration>(request, &mut context);
                        handle::<WorkspaceFoldersRequest>(request, &mut context);
                        handle::<ApplyWorkspaceEdit>(request, &mut context);
                        handle::<ShowMessageRequest>(request, &mut context);
                        handle::<ShowDocument>(request, &mut context);
                        handle::<WorkDoneProgressCreate>(request, &mut context);
                    }
                    Message::Response(ref mut response) => {
                        use lsp_types::request::*;

                        use crate::handler::handle_server_to_client_response as handle;
                        let request = requests_from_client
                            .remove(&response.id)
                            .ok_or(eyre!("Invalid id received from server"))?;

                        handle::<Initialize>(&request, response, &mut context);
                        handle::<WillSaveWaitUntil>(&request, response, &mut context);
                        handle::<GotoDeclaration>(&request, response, &mut context);
                        handle::<GotoDefinition>(&request, response, &mut context);
                        handle::<GotoTypeDefinition>(&request, response, &mut context);
                        handle::<GotoImplementation>(&request, response, &mut context);
                        handle::<References>(&request, response, &mut context);
                        handle::<CallHierarchyPrepare>(&request, response, &mut context);
                        handle::<CallHierarchyIncomingCalls>(&request, response, &mut context);
                        handle::<CallHierarchyOutgoingCalls>(&request, response, &mut context);
                        handle::<TypeHierarchyPrepare>(&request, response, &mut context);
                        handle::<TypeHierarchySupertypes>(&request, response, &mut context);
                        handle::<TypeHierarchySubtypes>(&request, response, &mut context);
                        handle::<DocumentHighlightRequest>(&request, response, &mut context);
                        handle::<DocumentLinkRequest>(&request, response, &mut context);
                        handle::<DocumentLinkResolve>(&request, response, &mut context);
                        handle::<HoverRequest>(&request, response, &mut context);
                        handle::<CodeLensRequest>(&request, response, &mut context);
                        handle::<CodeLensResolve>(&request, response, &mut context);
                        handle::<FoldingRangeRequest>(&request, response, &mut context);
                        handle::<SelectionRangeRequest>(&request, response, &mut context);
                        handle::<DocumentSymbolRequest>(&request, response, &mut context);
                        handle::<SemanticTokensFullRequest>(&request, response, &mut context);
                        handle::<SemanticTokensFullDeltaRequest>(&request, response, &mut context);
                        handle::<SemanticTokensRangeRequest>(&request, response, &mut context);
                        handle::<InlayHintRequest>(&request, response, &mut context);
                        handle::<InlayHintResolveRequest>(&request, response, &mut context);
                        handle::<InlineValueRequest>(&request, response, &mut context);
                        handle::<MonikerRequest>(&request, response, &mut context);
                        handle::<Completion>(&request, response, &mut context);
                        handle::<ResolveCompletionItem>(&request, response, &mut context);
                        handle::<SignatureHelpRequest>(&request, response, &mut context);
                        handle::<CodeActionRequest>(&request, response, &mut context);
                        handle::<CodeActionResolveRequest>(&request, response, &mut context);
                        handle::<DocumentColor>(&request, response, &mut context);
                        handle::<ColorPresentationRequest>(&request, response, &mut context);
                        handle::<Formatting>(&request, response, &mut context);
                        handle::<RangeFormatting>(&request, response, &mut context);
                        handle::<OnTypeFormatting>(&request, response, &mut context);
                        handle::<Rename>(&request, response, &mut context);
                        handle::<PrepareRenameRequest>(&request, response, &mut context);
                        handle::<LinkedEditingRange>(&request, response, &mut context);
                        handle::<WorkspaceSymbolRequest>(&request, response, &mut context);
                        handle::<WorkspaceSymbolResolve>(&request, response, &mut context);
                        handle::<WillCreateFiles>(&request, response, &mut context);
                        handle::<WillRenameFiles>(&request, response, &mut context);
                        handle::<WillDeleteFiles>(&request, response, &mut context);
                        handle::<ExecuteCommand>(&request, response, &mut context);
                    }
                    Message::Notification(notification) => {
                        use lsp_types::notification::*;

                        use crate::handler::handle_server_to_client_notification as handle;

                        handle::<PublishDiagnostics>(notification, &mut context);
                        handle::<ShowMessage>(notification, &mut context);
                        handle::<LogMessage>(notification, &mut context);
                        handle::<WorkDoneProgressCancel>(notification, &mut context);
                        handle::<TelemetryEvent>(notification, &mut context);
                    }
                }
                client.sender.send(message).wrap_err("Client stopped")?;
            }
            event::Event::ServerLog(line) => {
                tracing::info!(line);
            }
        }
    }
    Ok(())
}
