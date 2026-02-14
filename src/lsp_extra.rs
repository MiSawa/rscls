use lsp_types::request::Request;

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
