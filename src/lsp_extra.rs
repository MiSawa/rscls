use lsp_types::{notification::Notification, request::Request, TextDocumentIdentifier};
use serde::{Deserialize, Serialize};

pub enum ReloadWorkspace {}
impl Request for ReloadWorkspace {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "rust-analyzer/reloadWorkspace";
}

pub enum RebuildProcMacros {}
impl Request for RebuildProcMacros {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "rust-analyzer/rebuildProcMacros";
}

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
