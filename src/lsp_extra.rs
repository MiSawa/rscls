use lsp_types::{notification::Notification, request::Request, TextDocumentIdentifier};
use serde::{Deserialize, Serialize};

pub trait MessageExt {
    fn is_exit(&self) -> bool;
}
impl MessageExt for lsp_server::Message {
    fn is_exit(&self) -> bool {
        use lsp_types::notification::{Exit, Notification as _};
        matches!(self, lsp_server::Message::Notification(notification) if notification.method == Exit::METHOD)
    }
}

#[derive(Debug)]
pub enum ReloadWorkspace {}
impl Request for ReloadWorkspace {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "rust-analyzer/reloadWorkspace";
}

#[derive(Debug)]
pub enum RebuildProcMacros {}
impl Request for RebuildProcMacros {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "rust-analyzer/rebuildProcMacros";
}

#[derive(Debug)]
pub enum RunFlyCheck {}
impl Notification for RunFlyCheck {
    type Params = RunFlyCheckParams;
    const METHOD: &'static str = "rust-analyzer/runFlyCheck";
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunFlyCheckParams {
    pub text_document: Option<TextDocumentIdentifier>,
}
