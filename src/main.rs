use std::{collections::HashMap, fs::OpenOptions, io::Write, path::PathBuf};

use clap::Parser;
use eyre::{eyre, Result, WrapErr as _};
use lsp_server::Message;
use lsp_types::{
    notification::{self, Notification as _},
    request,
};
use serde_json::{json, Value};
use verbosity::Verbosity;

use crate::{
    client::Client,
    handler::{handle_notification, handle_request, handle_response, Move},
    lsp_extra::MessageExt as _,
    script::Scripts,
    server::Server,
};

mod client;
mod codec;
mod event;
mod handler;
mod lsp_extra;
mod script;
mod server;
mod verbosity;

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
        fmt.with_writer(|| std::io::stderr().lock()).init();
    }
}

fn modify_config(opts: &mut Value, mut rust_projects: Vec<Value>) {
    if opts.is_null() {
        *opts = json!({});
    }
    if let Some(opts) = opts.as_object_mut() {
        if rust_projects.is_empty() {
            // Push a dummy project to prevent rust-analyzer from complaining missing projects.
            rust_projects.push(json!({
                "crates": []
            }))
        }
        opts.insert("linkedProjects".to_owned(), Value::Array(rust_projects));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing_subscriber(&args);

    tracing::debug!(?args);

    let (event_sender, mut event_receiver) = event::new_event_bus();

    let client = Client::stdio(event_sender.clone());
    let server = Server::spawn(event_sender.clone(), args.rust_analyzer)
        .wrap_err("failed to spawn server")?;

    let mut scripts = Scripts::new(event_sender.clone(), args.rust_script)?;
    let mut requests_from_server = HashMap::new();
    let mut no_need_reload_version = event_sender.current_version();
    while let Some(event) = event_receiver.recv().await {
        // Need async non-move closure https://github.com/rust-lang/rust/issues/62290
        match event {
            event::Event::ClientToServer(mut message) => {
                tracing::debug!(?message, "Message from client");
                match &mut message {
                    Message::Request(ref mut request) => {
                        handle_request::<request::Initialize, _>(
                            request,
                            |Move(mut params)| async {
                                let opts = params
                                    .initialization_options
                                    .get_or_insert_with(|| json!({}));
                                no_need_reload_version = event_sender.start_reload();
                                modify_config(opts, scripts.projects().await);
                                params
                            },
                        )
                        .await;
                        handle_request::<lsp_extra::ReloadWorkspace, _>(request, |params| async {
                            scripts.queue_refresh_all().await;
                            params.moved()
                        })
                        .await;
                        // TODO: Other ones
                    }
                    Message::Response(ref mut response) => {
                        let request = requests_from_server
                            .remove(&response.id)
                            .ok_or(eyre!("invalid id received from client"))?;

                        handle_response::<request::WorkspaceConfiguration, _>(
                            &request,
                            response,
                            |Move(params), Move(mut result)| async {
                                for (i, item) in params.items.into_iter().enumerate() {
                                    // NOTE: Semantically we should probably handle scope_uri but
                                    // rust-analyzer doesn't specify them currenlty.
                                    if Some("rust-analyzer") == item.section.as_deref() {
                                        if let Some(value) = result.get_mut(i) {
                                            no_need_reload_version = event_sender.start_reload();
                                            modify_config(value, scripts.projects().await)
                                        }
                                    }
                                }
                                result
                            },
                        )
                        .await;
                    }
                    Message::Notification(ref mut notification) => {
                        handle_notification::<notification::DidOpenTextDocument, _>(
                            notification,
                            |Move(mut params)| async {
                                if &params.text_document.language_id == "rust-script" {
                                    scripts.register(params.text_document.uri.clone()).await;
                                    params.text_document.language_id = "rust".to_owned();
                                }
                                params
                            },
                        )
                        .await;
                        handle_notification::<notification::DidCloseTextDocument, _>(
                            notification,
                            |Move(params)| async {
                                scripts.deregister_if_registered(&params.text_document.uri);
                                params
                            },
                        )
                        .await;
                        // TODO: Only if checkOnSave is enabled?
                        handle_notification::<notification::DidSaveTextDocument, _>(
                            notification,
                            |Move(params)| async {
                                scripts.queue_refresh(&params.text_document.uri).await;
                                params
                            },
                        )
                        .await;
                    }
                }
                let need_exit = message.is_exit();
                server.sender.send(message).wrap_err("server stopped")?;
                if need_exit {
                    break;
                }
            }
            event::Event::ServerToClient(mut message) => {
                tracing::debug!(?message, "Message from server");
                match &mut message {
                    Message::Request(ref mut request) => {
                        requests_from_server.insert(request.id.clone(), request.clone());
                    }
                    Message::Response(_response) => {}
                    Message::Notification(_notification) => {}
                }
                client.sender.send(message).wrap_err("client stopped")?;
            }
            event::Event::ServerLog(line) => {
                tokio::task::spawn_blocking(move || {
                    writeln!(std::io::stderr().lock(), "{line}").unwrap()
                })
                .await
                .unwrap();
            }
            event::Event::NeedReload(dirty_version) => {
                if dirty_version < no_need_reload_version {
                    continue;
                }
                let config = lsp_types::DidChangeConfigurationParams {
                    settings: Default::default(),
                };
                let message = Message::Notification(lsp_server::Notification::new(
                    notification::DidChangeConfiguration::METHOD.to_owned(),
                    config,
                ));
                server.sender.send(message).wrap_err("server stopped")?;
                no_need_reload_version = dirty_version;
            }
        }
    }
    tracing::debug!("No more events, quitting...");
    Ok(())
}
