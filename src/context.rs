use lsp_types::Url;

use crate::script::Script;

pub struct Context<'a> {
    script: &'a Script,
}

impl<'a> Context<'a> {
    pub fn new(script: &'a Script) -> Self {
        Self { script }
    }

    pub fn translate_client_uri_to_server_uri(&self, uri: &mut Url) {
        if uri.scheme() != "file" {
            return;
        }
        if Some(uri.path()) == self.script.script().to_str() {
            if let Some(new_path) = self.script.source_in_package().to_str() {
                uri.set_path(new_path);
            }
        }
    }
    pub fn translate_server_uri_to_client_uri(&self, uri: &mut Url) {
        if uri.scheme() != "file" {
            return;
        }
        if Some(uri.path()) == self.script.source_in_package().to_str() {
            if let Some(new_path) = self.script.script().to_str() {
                uri.set_path(new_path);
            }
        }
    }
    pub fn save(&self) {
        if let Err(e) = self.script.regenerate() {
            tracing::error!(?e, "failed to regenerate package");
        }
    }
}
