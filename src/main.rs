use std::{collections::HashMap, fs::OpenOptions, path::PathBuf};

use clap::Parser;
use eyre::{eyre, Result, WrapErr as _};
use lsp_server::Message;
use lsp_types::{notification, request};
use serde_json::Value;
use verbosity::Verbosity;

use crate::{
    client::Client,
    handler::{handle_notification, handle_request, handle_response},
    script::Scripts,
    server::Server,
};

mod client;
mod event;
mod handler;
mod rust_project;
mod script;
mod server;
mod verbosity;

// TODO: Support --port (TCP socket) and --pipe (socket file)

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The rust-script executable path.
    #[arg(long, default_value = "rust-script")]
    rust_script: PathBuf,

    /// The rust-analyzer executable path.
    #[arg(long, default_value = "rust-analyzer")]
    rust_analyzer: PathBuf,

    /// The file to use as the log output instead of stderr.
    #[arg(short('o'), long)]
    log_file: Option<PathBuf>,

    #[command(flatten)]
    verbosity: Verbosity<verbosity::WarnLevel>,
}

fn init_tracing_subscriber(args: &Args) {
    let fmt = tracing_subscriber::fmt()
        .with_max_level(args.verbosity.level_filter())
        .with_ansi(false);
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

fn modify_config(opts: &mut Value, rust_projects: Vec<Value>) {
    if opts.is_null() {
        *opts = serde_json::json!({});
    }
    if let Some(ra) = opts.as_object_mut() {
        if rust_projects.is_empty() {
            if let Some(files) = ra
                .entry("files")
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
            {
                if let Some(exclude_dirs) = files
                    .entry("excludeDirs")
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                {
                    exclude_dirs.push("./".into());
                }
            }
        }
        if let Some(check) = ra
            .entry("check")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
        {
            check.insert(
                "overrideCommand".to_owned(),
                serde_json::json!([
                    "cargo",
                    "check",
                    "--workspace",
                    "--message-format=json",
                    "--all-targets"
                ]),
            );
        }
        ra.insert("linkedProjects".to_owned(), Value::Array(rust_projects));
    }
}

// TODO: translate actions for diagnostics?
// fn rewrite_script_uri_to_project_uri(scripts: &Scripts, uri: &mut lsp_types::Url) {}
fn rewrite_project_uri_to_script_uri(scripts: &Scripts, uri: &mut lsp_types::Url) {
    if let Ok(file) = uri.to_file_path() {
        if let Some(new_file) = scripts.project_path_to_script_path(file) {
            if let Ok(new_uri) = lsp_types::Url::from_file_path(new_file) {
                *uri = new_uri;
            }
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing_subscriber(&args);

    tracing::debug!(?args);

    let (event_sender, event_receiver) = event::new_event_bus();

    let client = Client::stdio(event_sender.clone());
    let server = Server::spawn(event_sender.clone(), args.rust_analyzer)
        .wrap_err("failed to spawn server")?;

    let mut scripts = Scripts::new(event_sender.clone(), args.rust_script)?;
    let mut requests_from_server = HashMap::new();
    let mut current_version = event_sender.current_version();
    for event in event_receiver.into_iter() {
        match event {
            event::Event::ClientToServer(mut message) => {
                tracing::debug!(?message, "Message from client");
                match &mut message {
                    Message::Request(ref mut request) => {
                        handle_request::<request::Initialize>(request, |params| {
                            let opts = params
                                .initialization_options
                                .get_or_insert_with(|| serde_json::json!({}));
                            current_version = event_sender.start_reload();
                            modify_config(opts, scripts.projects())
                        });
                    }
                    Message::Response(ref mut response) => {
                        let request = requests_from_server
                            .remove(&response.id)
                            .ok_or(eyre!("invalid id received from client"))?;

                        handle_response::<request::WorkspaceConfiguration>(
                            &request,
                            response,
                            |params, result| {
                                for (i, item) in params.items.iter().enumerate() {
                                    // TODO: Handle scope_uri though rust-analyzer doesn't specify
                                    // them currenlty.
                                    if Some("rust-analyzer")
                                        == item.section.as_ref().map(|x| x.as_str())
                                    {
                                        if let Some(value) = result.get_mut(i) {
                                            current_version = event_sender.start_reload();
                                            modify_config(value, scripts.projects())
                                        }
                                    }
                                }
                            },
                        );
                    }
                    Message::Notification(ref mut notification) => {
                        handle_notification::<notification::DidOpenTextDocument>(
                            notification,
                            |params| {
                                if &params.text_document.language_id == "rust-script" {
                                    scripts.register(params.text_document.uri.clone());
                                    params.text_document.language_id = "rust".to_owned();
                                }
                            },
                        );
                        handle_notification::<notification::DidCloseTextDocument>(
                            notification,
                            |params| scripts.deregister_if_registered(&params.text_document.uri),
                        );
                        handle_notification::<notification::DidSaveTextDocument>(
                            notification,
                            |params| scripts.saved(&params.text_document.uri),
                        );
                    }
                }
                server.sender.send(message).wrap_err("server stopped")?;
            }
            event::Event::ServerToClient(mut message) => {
                tracing::debug!(?message, "Message from server");
                match &mut message {
                    Message::Request(ref mut request) => {
                        requests_from_server.insert(request.id.clone(), request.clone());
                        // handle_request::<request::RegisterCapability>(request, |params| {
                        //     for registration in params.registrations.iter_mut() {
                        //         if registration.method == notification::DidSaveTextDocument::METHOD
                        //         {
                        //             if let Some(value) = registration.register_options.as_mut() {
                        //                 if let Ok(mut options) = serde_json::from_value::<
                        //                     lsp_types::TextDocumentRegistrationOptions,
                        //                 >(
                        //                     value.clone()
                        //                 ) {
                        //                     if let Some(selectors) =
                        //                         options.document_selector.as_mut()
                        //                     {
                        //                         if selectors.iter().any(|selector| {
                        //                             selector.pattern.as_ref().map(|x| x.as_str())
                        //                                 == Some("**/*.rs")
                        //                         }) {
                        //                             selectors.push(lsp_types::DocumentFilter {
                        //                                 language: None,
                        //                                 scheme: None,
                        //                                 pattern: Some("**/*.ers".into()),
                        //                             });
                        //                         }
                        //                     }
                        //                     if let Ok(new_value) = serde_json::to_value(options) {
                        //                         *value = new_value;
                        //                     }
                        //                 }
                        //             }
                        //         }
                        //     }
                        // });
                    }
                    Message::Response(_response) => {}
                    Message::Notification(notification) => {
                        handle_notification::<notification::PublishDiagnostics>(
                            notification,
                            |params| {
                                rewrite_project_uri_to_script_uri(&scripts, &mut params.uri);
                                params.diagnostics.iter_mut().for_each(|diagnostic| {
                                    diagnostic.code_description.iter_mut().for_each(|desc| {
                                        rewrite_project_uri_to_script_uri(&scripts, &mut desc.href)
                                    });
                                    diagnostic
                                        .related_information
                                        .iter_mut()
                                        .flat_map(|v| v.iter_mut())
                                        .for_each(|info| {
                                            rewrite_project_uri_to_script_uri(
                                                &scripts,
                                                &mut info.location.uri,
                                            )
                                        });
                                });
                                eprintln!(
                                    "Rewritten diagnostics: {}",
                                    serde_json::to_string(&params).unwrap()
                                );
                            },
                        );
                    }
                }
                client.sender.send(message).wrap_err("client stopped")?;
            }
            event::Event::ServerLog(line) => {
                eprintln!("{line}");
            }
            event::Event::NeedReload(dirty_version) => {
                if dirty_version < current_version {
                    continue;
                }
                use lsp_types::notification::Notification;
                let config = lsp_types::DidChangeConfigurationParams {
                    settings: Default::default(),
                };
                let message = Message::Notification(lsp_server::Notification::new(
                    lsp_types::notification::DidChangeConfiguration::METHOD.to_owned(),
                    config,
                ));
                server.sender.send(message).wrap_err("server stopped")?;
            }
        }
    }
    Ok(())
}
