use crate::context::Context;

trait ToDo {}

pub trait HandleClientToServer {
    fn handle(&mut self, context: &mut Context);
}
pub trait HandleServerToClient {
    fn handle(&mut self, context: &mut Context);
}
trait HandleBiDirectional {
    fn handle_client_to_server(&mut self, context: &mut Context);
    fn handle_server_to_client(&mut self, context: &mut Context);
}

impl<T: HandleBiDirectional> HandleClientToServer for T {
    fn handle(&mut self, context: &mut Context) {
        HandleBiDirectional::handle_client_to_server(self, context);
    }
}

impl<T: HandleBiDirectional> HandleServerToClient for T {
    fn handle(&mut self, context: &mut Context) {
        HandleBiDirectional::handle_server_to_client(self, context)
    }
}

pub trait ClientToServerRequest
where
    Self: lsp_types::request::Request,
{
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context);
    fn handle_server_to_client_response(
        _params: &Self::Params,
        _result: &mut Self::Result,
        _context: &mut Context,
    ) {
    }
}
pub trait ServerToClientRequest
where
    Self: lsp_types::request::Request,
{
    fn handle_server_to_client_request(params: &mut Self::Params, context: &mut Context);
    fn handle_client_to_server_response(
        _params: &Self::Params,
        _result: &mut Self::Result,
        _context: &mut Context,
    ) {
    }
}
pub trait ClientToServerNotification
where
    Self: lsp_types::notification::Notification,
{
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context);
}
pub trait ServerToClientNotification
where
    Self: lsp_types::notification::Notification,
{
    fn handle_server_to_client_notification(params: &mut Self::Params, context: &mut Context);
}

pub fn handle_client_to_server_request<R>(request: &mut lsp_server::Request, context: &mut Context)
where
    R: ClientToServerRequest,
{
    handle_request::<R>(request, |params| {
        R::handle_client_to_server_request(params, context)
    });
}
pub fn handle_server_to_client_request<R>(request: &mut lsp_server::Request, context: &mut Context)
where
    R: ServerToClientRequest,
{
    handle_request::<R>(request, |params| {
        R::handle_server_to_client_request(params, context)
    });
}

pub fn handle_client_to_server_response<R>(
    request: &lsp_server::Request,
    response: &mut lsp_server::Response,
    context: &mut Context,
) where
    R: ServerToClientRequest,
{
    handle_response::<R>(request, response, |req, res| {
        R::handle_client_to_server_response(req, res, context)
    });
}
pub fn handle_server_to_client_response<R>(
    request: &lsp_server::Request,
    response: &mut lsp_server::Response,
    context: &mut Context,
) where
    R: ClientToServerRequest,
{
    handle_response::<R>(request, response, |req, res| {
        R::handle_server_to_client_response(req, res, context)
    });
}

pub fn handle_client_to_server_notification<N>(
    notification: &mut lsp_server::Notification,
    context: &mut Context,
) where
    N: ClientToServerNotification,
{
    handle_notification::<N>(notification, |params| {
        N::handle_client_to_server_notification(params, context)
    });
}
pub fn handle_server_to_client_notification<N>(
    notification: &mut lsp_server::Notification,
    context: &mut Context,
) where
    N: ServerToClientNotification,
{
    handle_notification::<N>(notification, |params| {
        N::handle_server_to_client_notification(params, context)
    });
}

fn handle_request<R: lsp_types::request::Request>(
    request: &mut lsp_server::Request,
    mut handler: impl FnMut(&mut R::Params),
) {
    if request.method != R::METHOD {
        return;
    }
    if let Ok((id, mut params)) = request.clone().extract::<R::Params>(R::METHOD) {
        handler(&mut params);
        *request = lsp_server::Request::new(id, std::mem::take(&mut request.method), params);
    }
}
fn handle_response<R: lsp_types::request::Request>(
    request: &lsp_server::Request,
    response: &mut lsp_server::Response,
    mut handler: impl FnMut(&R::Params, &mut R::Result),
) {
    if request.method != R::METHOD {
        return;
    }
    assert_eq!(request.id, response.id);
    if let Ok((id, request_params)) = request.clone().extract::<R::Params>(R::METHOD) {
        if let Some(value) = response.clone().result {
            if let Ok(mut result) = serde_json::from_value(value) {
                handler(&request_params, &mut result);
                *response = lsp_server::Response::new_ok(id, result);
            }
        }
    }
}
fn handle_notification<N: lsp_types::notification::Notification>(
    notification: &mut lsp_server::Notification,
    mut handler: impl FnMut(&mut N::Params),
) {
    if notification.method != N::METHOD {
        return;
    }
    if let Ok(mut params) = notification.clone().extract::<N::Params>(N::METHOD) {
        handler(&mut params);
        *notification =
            lsp_server::Notification::new(std::mem::take(&mut notification.method), params);
    }
}

impl HandleBiDirectional for lsp_types::Url {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        context.translate_client_uri_to_server_uri(self)
    }
    fn handle_server_to_client(&mut self, context: &mut Context) {
        context.translate_server_uri_to_client_uri(self)
    }
}
struct StringURI<'a>(&'a mut String);
impl HandleBiDirectional for StringURI<'_> {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        if let Ok(mut uri) = lsp_types::Url::parse(self.0) {
            uri.handle_client_to_server(context);
            *self.0 = uri.into()
        }
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        if let Ok(mut uri) = lsp_types::Url::parse(self.0) {
            uri.handle_server_to_client(context);
            *self.0 = uri.into()
        }
    }
}

impl HandleClientToServer for lsp_types::TextDocumentItem {
    fn handle(&mut self, context: &mut Context) {
        // TODO: See language id and detect rust-script or smth?
        self.uri.handle_client_to_server(context)
    }
}
impl HandleClientToServer for lsp_types::TextDocumentIdentifier {
    fn handle(&mut self, context: &mut Context) {
        self.uri.handle_client_to_server(context)
    }
}
impl HandleClientToServer for lsp_types::VersionedTextDocumentIdentifier {
    fn handle(&mut self, context: &mut Context) {
        self.uri.handle_client_to_server(context)
    }
}
impl HandleClientToServer for lsp_types::TextDocumentPositionParams {
    fn handle(&mut self, context: &mut Context) {
        self.text_document.handle(context)
    }
}
impl HandleBiDirectional for lsp_types::Location {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        self.uri.handle_client_to_server(context)
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        self.uri.handle_server_to_client(context)
    }
}
impl HandleServerToClient for lsp_types::LocationLink {
    fn handle(&mut self, context: &mut Context) {
        self.target_uri.handle_server_to_client(context)
    }
}
impl HandleBiDirectional for lsp_types::CallHierarchyItem {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        self.uri.handle_client_to_server(context)
    }
    fn handle_server_to_client(&mut self, context: &mut Context) {
        self.uri.handle_server_to_client(context)
    }
}
impl HandleServerToClient for lsp_types::CallHierarchyIncomingCall {
    fn handle(&mut self, context: &mut Context) {
        self.from.handle_server_to_client(context)
    }
}
impl HandleServerToClient for lsp_types::CallHierarchyOutgoingCall {
    fn handle(&mut self, context: &mut Context) {
        self.to.handle_server_to_client(context)
    }
}
impl HandleBiDirectional for lsp_types::TypeHierarchyItem {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        self.uri.handle_client_to_server(context)
    }
    fn handle_server_to_client(&mut self, context: &mut Context) {
        self.uri.handle_server_to_client(context)
    }
}
impl HandleBiDirectional for lsp_types::DocumentLink {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        if let Some(ref mut uri) = self.target {
            uri.handle_client_to_server(context)
        }
    }
    fn handle_server_to_client(&mut self, context: &mut Context) {
        if let Some(ref mut uri) = self.target {
            uri.handle_server_to_client(context)
        }
    }
}
impl HandleBiDirectional for lsp_types::InlayHint {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        if let lsp_types::InlayHintLabel::LabelParts(ref mut labels) = self.label {
            labels
                .iter_mut()
                .flat_map(|label| label.location.iter_mut())
                .for_each(|location| location.handle_client_to_server(context))
        }
    }
    fn handle_server_to_client(&mut self, context: &mut Context) {
        if let lsp_types::InlayHintLabel::LabelParts(ref mut labels) = self.label {
            labels
                .iter_mut()
                .flat_map(|label| label.location.iter_mut())
                .for_each(|location| location.handle_server_to_client(context))
        }
    }
}

impl HandleBiDirectional for lsp_types::Diagnostic {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        self.code_description
            .iter_mut()
            .for_each(|desc| desc.href.handle_client_to_server(context));
        self.related_information
            .iter_mut()
            .flat_map(|v| v.iter_mut())
            .for_each(|info| info.location.handle_client_to_server(context));
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        self.code_description
            .iter_mut()
            .for_each(|desc| desc.href.handle_server_to_client(context));
        self.related_information
            .iter_mut()
            .flat_map(|v| v.iter_mut())
            .for_each(|info| info.location.handle_server_to_client(context));
    }
}
impl HandleBiDirectional for lsp_types::CodeAction {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        // TODO: How Command and Data look like for rust-analyzer?
        self.diagnostics
            .iter_mut()
            .flat_map(|v| v.iter_mut())
            .for_each(|diagnostic| diagnostic.handle_client_to_server(context));
        self.edit
            .iter_mut()
            .for_each(|edit| edit.handle_client_to_server(context));
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        // TODO: How Command and Data look like for rust-analyzer?
        self.diagnostics
            .iter_mut()
            .flat_map(|v| v.iter_mut())
            .for_each(|diagnostic| diagnostic.handle_server_to_client(context));
        self.edit
            .iter_mut()
            .for_each(|edit| edit.handle_server_to_client(context));
    }
}
impl ToDo for lsp_types::Command {
    // TODO
}
impl HandleClientToServer for lsp_types::FileCreate {
    fn handle(&mut self, context: &mut Context) {
        StringURI(&mut self.uri).handle_client_to_server(context);
    }
}
impl HandleClientToServer for lsp_types::FileRename {
    fn handle(&mut self, context: &mut Context) {
        // TODO: The workspace itself may change due to this rename...
        StringURI(&mut self.old_uri).handle_client_to_server(context);
        StringURI(&mut self.new_uri).handle_client_to_server(context);
    }
}
impl HandleClientToServer for lsp_types::FileDelete {
    fn handle(&mut self, context: &mut Context) {
        // TODO: The workspace itself may change due to this delete...
        StringURI(&mut self.uri).handle_client_to_server(context);
    }
}
impl HandleClientToServer for lsp_types::FileEvent {
    fn handle(&mut self, context: &mut Context) {
        self.uri.handle_client_to_server(context)
    }
}

impl HandleBiDirectional for lsp_types::CreateFile {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        self.uri.handle_client_to_server(context)
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        self.uri.handle_server_to_client(context)
    }
}
impl HandleBiDirectional for lsp_types::RenameFile {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        // TODO: The workspace itself may change due to this rename...
        self.old_uri.handle_client_to_server(context);
        self.new_uri.handle_client_to_server(context);
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        // TODO: The workspace itself may change due to this rename...
        self.old_uri.handle_server_to_client(context);
        self.new_uri.handle_server_to_client(context);
    }
}
impl HandleBiDirectional for lsp_types::DeleteFile {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        self.uri.handle_client_to_server(context)
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        self.uri.handle_server_to_client(context)
    }
}
impl HandleBiDirectional for lsp_types::TextDocumentEdit {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        self.text_document.uri.handle_client_to_server(context);
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        self.text_document.uri.handle_server_to_client(context);
    }
}
impl HandleBiDirectional for lsp_types::DocumentChangeOperation {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        match self {
            lsp_types::DocumentChangeOperation::Op(op) => match op {
                lsp_types::ResourceOp::Create(create) => create.handle_client_to_server(context),
                lsp_types::ResourceOp::Rename(rename) => rename.handle_client_to_server(context),
                lsp_types::ResourceOp::Delete(delete) => delete.handle_client_to_server(context),
            },
            lsp_types::DocumentChangeOperation::Edit(edit) => edit.handle_client_to_server(context),
        }
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        match self {
            lsp_types::DocumentChangeOperation::Op(op) => match op {
                lsp_types::ResourceOp::Create(create) => create.handle_server_to_client(context),
                lsp_types::ResourceOp::Rename(rename) => rename.handle_server_to_client(context),
                lsp_types::ResourceOp::Delete(delete) => delete.handle_server_to_client(context),
            },
            lsp_types::DocumentChangeOperation::Edit(edit) => edit.handle_server_to_client(context),
        }
    }
}
impl HandleBiDirectional for lsp_types::WorkspaceEdit {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        if let Some(changes) = self.changes.take() {
            self.changes = Some(
                changes
                    .into_iter()
                    .map(|(mut k, v)| {
                        k.handle_client_to_server(context);
                        (k, v)
                    })
                    .collect(),
            );
        }
        self.document_changes
            .iter_mut()
            .for_each(|changes| match changes {
                lsp_types::DocumentChanges::Edits(edits) => edits
                    .iter_mut()
                    .for_each(|edit| edit.handle_client_to_server(context)),
                lsp_types::DocumentChanges::Operations(operations) => operations
                    .iter_mut()
                    .for_each(|operation| operation.handle_client_to_server(context)),
            });
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        if let Some(changes) = self.changes.take() {
            self.changes = Some(
                changes
                    .into_iter()
                    .map(|(mut k, v)| {
                        k.handle_server_to_client(context);
                        (k, v)
                    })
                    .collect(),
            );
        }
        self.document_changes
            .iter_mut()
            .for_each(|changes| match changes {
                lsp_types::DocumentChanges::Edits(edits) => edits
                    .iter_mut()
                    .for_each(|edit| edit.handle_server_to_client(context)),
                lsp_types::DocumentChanges::Operations(operations) => operations
                    .iter_mut()
                    .for_each(|operation| operation.handle_server_to_client(context)),
            });
    }
}
impl HandleBiDirectional for lsp_types::WorkspaceSymbol {
    fn handle_client_to_server(&mut self, context: &mut Context) {
        match self.location {
            lsp_types::OneOf::Left(ref mut location) => location.handle_client_to_server(context),
            lsp_types::OneOf::Right(ref mut workspace_location) => {
                workspace_location.uri.handle_client_to_server(context)
            }
        }
    }

    fn handle_server_to_client(&mut self, context: &mut Context) {
        match self.location {
            lsp_types::OneOf::Left(ref mut location) => location.handle_server_to_client(context),
            lsp_types::OneOf::Right(ref mut workspace_location) => {
                workspace_location.uri.handle_server_to_client(context)
            }
        }
    }
}
impl ToDo for lsp_types::PartialResultParams {
    // TODO: Register token with method type or smth?
    // and create something similar to lsp_types::notification::Progress?
    // Or... Does disabling all WorkDoneProgressOptions::workDoneProgress also disable partial
    // results?
}

impl ClientToServerRequest for lsp_types::request::Initialize {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Tweak client capabilities
        params
            .root_uri
            .iter_mut()
            .for_each(|uri| uri.handle_client_to_server(context));
        params
            .workspace_folders
            .iter_mut()
            .flat_map(|v| v.iter_mut())
            .for_each(|folder| folder.uri.handle_client_to_server(context));
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        _result: &mut Self::Result,
        _context: &mut Context,
    ) {
        // TODO: Tweak server capabilities
    }
}

impl ClientToServerNotification for lsp_types::notification::SetTrace {
    fn handle_client_to_server_notification(_params: &mut Self::Params, _context: &mut Context) {
        // TODO: Set it to tracing_subscriber
    }
}

impl ClientToServerNotification for lsp_types::notification::DidOpenTextDocument {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}

impl ClientToServerNotification for lsp_types::notification::DidChangeTextDocument {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}

impl ClientToServerNotification for lsp_types::notification::WillSaveTextDocument {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::WillSaveWaitUntil {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}

impl ClientToServerNotification for lsp_types::notification::DidSaveTextDocument {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        context.save();
        params.text_document.handle(context)
    }
}

impl ClientToServerNotification for lsp_types::notification::DidCloseTextDocument {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::GotoDeclaration {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document_position_params.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        if let Some(result) = result {
            match result {
                lsp_types::GotoDefinitionResponse::Scalar(ref mut location) => {
                    location.handle_server_to_client(context)
                }
                lsp_types::GotoDefinitionResponse::Array(locations) => locations
                    .iter_mut()
                    .for_each(|location| location.handle_server_to_client(context)),
                lsp_types::GotoDefinitionResponse::Link(links) => {
                    links.iter_mut().for_each(|link| link.handle(context))
                }
            }
        }
    }
}

impl ClientToServerRequest for lsp_types::request::GotoDefinition {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document_position_params.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        if let Some(result) = result {
            match result {
                lsp_types::GotoDefinitionResponse::Scalar(ref mut location) => {
                    location.handle_server_to_client(context)
                }
                lsp_types::GotoDefinitionResponse::Array(locations) => locations
                    .iter_mut()
                    .for_each(|location| location.handle_server_to_client(context)),
                lsp_types::GotoDefinitionResponse::Link(links) => {
                    links.iter_mut().for_each(|link| link.handle(context))
                }
            }
        }
    }
}

impl ClientToServerRequest for lsp_types::request::GotoTypeDefinition {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document_position_params.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        if let Some(result) = result {
            match result {
                lsp_types::GotoDefinitionResponse::Scalar(ref mut location) => {
                    location.handle_server_to_client(context)
                }
                lsp_types::GotoDefinitionResponse::Array(locations) => locations
                    .iter_mut()
                    .for_each(|location| location.handle_server_to_client(context)),
                lsp_types::GotoDefinitionResponse::Link(links) => {
                    links.iter_mut().for_each(|link| link.handle(context))
                }
            }
        }
    }
}

impl ClientToServerRequest for lsp_types::request::GotoImplementation {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document_position_params.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        if let Some(result) = result {
            match result {
                lsp_types::GotoDefinitionResponse::Scalar(ref mut location) => {
                    location.handle_server_to_client(context)
                }
                lsp_types::GotoDefinitionResponse::Array(locations) => locations
                    .iter_mut()
                    .for_each(|location| location.handle_server_to_client(context)),
                lsp_types::GotoDefinitionResponse::Link(links) => {
                    links.iter_mut().for_each(|link| link.handle(context))
                }
            }
        }
    }
}

impl ClientToServerRequest for lsp_types::request::References {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document_position.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|r| r.iter_mut())
            .for_each(|location| location.handle_server_to_client(context))
    }
}

impl ClientToServerRequest for lsp_types::request::CallHierarchyPrepare {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document_position_params.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|r| r.iter_mut())
            .for_each(|item| item.handle_server_to_client(context))
    }
}

impl ClientToServerRequest for lsp_types::request::CallHierarchyIncomingCalls {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.item.handle_client_to_server(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|r| r.iter_mut())
            .for_each(|call| call.handle(context))
    }
}

impl ClientToServerRequest for lsp_types::request::CallHierarchyOutgoingCalls {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.item.handle_client_to_server(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|r| r.iter_mut())
            .for_each(|call| call.handle(context))
    }
}

impl ClientToServerRequest for lsp_types::request::TypeHierarchyPrepare {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document_position_params.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|r| r.iter_mut())
            .for_each(|item| item.handle_server_to_client(context))
    }
}

impl ClientToServerRequest for lsp_types::request::TypeHierarchySupertypes {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.item.handle_client_to_server(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|r| r.iter_mut())
            .for_each(|item| item.handle_server_to_client(context));
    }
}

impl ClientToServerRequest for lsp_types::request::TypeHierarchySubtypes {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.item.handle_client_to_server(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|r| r.iter_mut())
            .for_each(|item| item.handle_server_to_client(context));
    }
}

impl ClientToServerRequest for lsp_types::request::DocumentHighlightRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document_position_params.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::DocumentLinkRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|r| r.iter_mut())
            .for_each(|link| link.handle_server_to_client(context));
    }
}

impl ClientToServerRequest for lsp_types::request::DocumentLinkResolve {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.handle_client_to_server(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result.handle_server_to_client(context)
    }
}

impl ClientToServerRequest for lsp_types::request::HoverRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document_position_params.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::CodeLensRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        _result: &mut Self::Result,
        _context: &mut Context,
    ) {
        // TODO: How Command and Data look like for rust-analyzer?
    }
}

impl ClientToServerRequest for lsp_types::request::CodeLensResolve {
    fn handle_client_to_server_request(_params: &mut Self::Params, _context: &mut Context) {
        // TODO: How Command and Data look like for rust-analyzer?
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        _result: &mut Self::Result,
        _context: &mut Context,
    ) {
        // TODO: How Command and Data look like for rust-analyzer?
    }
}

impl ServerToClientRequest for lsp_types::request::CodeLensRefresh {
    fn handle_server_to_client_request(_params: &mut Self::Params, _context: &mut Context) {}
}

impl ClientToServerRequest for lsp_types::request::FoldingRangeRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::SelectionRangeRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::DocumentSymbolRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        if let Some(lsp_types::DocumentSymbolResponse::Flat(symbol_informations)) = result {
            symbol_informations
                .iter_mut()
                .for_each(|symbol_information| {
                    symbol_information.location.handle_server_to_client(context)
                });
        }
    }
}

impl ClientToServerRequest for lsp_types::request::SemanticTokensFullRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::SemanticTokensFullDeltaRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::SemanticTokensRangeRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }
}

impl ServerToClientRequest for lsp_types::request::SemanticTokensRefresh {
    fn handle_server_to_client_request(_params: &mut Self::Params, _context: &mut Context) {}
}

impl ClientToServerRequest for lsp_types::request::InlayHintRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .flat_map(|v| v.iter_mut())
            .for_each(|hint| hint.handle_server_to_client(context))
    }
}

impl ClientToServerRequest for lsp_types::request::InlayHintResolveRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.handle_client_to_server(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result.handle_server_to_client(context)
    }
}

impl ServerToClientRequest for lsp_types::request::InlayHintRefreshRequest {
    fn handle_server_to_client_request(_params: &mut Self::Params, _context: &mut Context) {}
}

impl ClientToServerRequest for lsp_types::request::InlineValueRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}

impl ServerToClientRequest for lsp_types::request::InlineValueRefreshRequest {
    fn handle_server_to_client_request(_params: &mut Self::Params, _context: &mut Context) {}
}

impl ClientToServerRequest for lsp_types::request::MonikerRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document_position_params.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::Completion {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document_position.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        _result: &mut Self::Result,
        _context: &mut Context,
    ) {
        // TODO: How Command and Data look like for rust-analyzer?
    }
}

impl ClientToServerRequest for lsp_types::request::ResolveCompletionItem {
    fn handle_client_to_server_request(_params: &mut Self::Params, _context: &mut Context) {
        // TODO: How Command and Data look like for rust-analyzer?
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        _result: &mut Self::Result,
        _context: &mut Context,
    ) {
        // TODO: How Command and Data look like for rust-analyzer?
    }
}

impl ServerToClientNotification for lsp_types::notification::PublishDiagnostics {
    fn handle_server_to_client_notification(params: &mut Self::Params, context: &mut Context) {
        params.uri.handle_server_to_client(context);
        params
            .diagnostics
            .iter_mut()
            .for_each(|diagnostic| diagnostic.handle_server_to_client(context))
    }
}

// TODO: textDocument/diagnostic once lsp_types have its definition
// TODO: workspace/diagnostic once lsp_types have its definition
// TODO: workspace/diagnostic/refresh once lsp_types have its definition

impl ClientToServerRequest for lsp_types::request::SignatureHelpRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document_position_params.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::CodeActionRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context);
        params
            .context
            .diagnostics
            .iter_mut()
            .for_each(|diagnostic| diagnostic.handle_client_to_server(context))
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        for action in result.iter_mut().flat_map(|r| r.iter_mut()) {
            match action {
                lsp_types::CodeActionOrCommand::Command(_) => {
                    // TODO: How Command and Data look like for rust-analyzer?
                }
                lsp_types::CodeActionOrCommand::CodeAction(action) => {
                    action.handle_server_to_client(context)
                }
            }
        }
    }
}
impl ClientToServerRequest for lsp_types::request::CodeActionResolveRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.handle_client_to_server(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result.handle_server_to_client(context)
    }
}
impl ClientToServerRequest for lsp_types::request::DocumentColor {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }
}
impl ClientToServerRequest for lsp_types::request::ColorPresentationRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        // TODO: Handle partial result
        params.text_document.handle(context)
    }
}
impl ClientToServerRequest for lsp_types::request::Formatting {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}
impl ClientToServerRequest for lsp_types::request::RangeFormatting {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}
impl ClientToServerRequest for lsp_types::request::OnTypeFormatting {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document_position.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::Rename {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document_position.handle(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .for_each(|edit| edit.handle_server_to_client(context))
    }
}
impl ClientToServerRequest for lsp_types::request::PrepareRenameRequest {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document.handle(context)
    }
}
impl ClientToServerRequest for lsp_types::request::LinkedEditingRange {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.text_document_position_params.handle(context)
    }
}

impl ClientToServerRequest for lsp_types::request::WorkspaceSymbolRequest {
    fn handle_client_to_server_request(_params: &mut Self::Params, _context: &mut Context) {}

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result.iter_mut().for_each(|r| match r {
            lsp_types::WorkspaceSymbolResponse::Flat(infos) => infos
                .iter_mut()
                .for_each(|info| info.location.handle_server_to_client(context)),
            lsp_types::WorkspaceSymbolResponse::Nested(symbols) => symbols
                .iter_mut()
                .for_each(|symbol| symbol.handle_server_to_client(context)),
        })
    }
}
impl ClientToServerRequest for lsp_types::request::WorkspaceSymbolResolve {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params.handle_client_to_server(context)
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result.handle_client_to_server(context)
    }
}
impl ServerToClientRequest for lsp_types::request::WorkspaceConfiguration {
    fn handle_server_to_client_request(params: &mut Self::Params, context: &mut Context) {
        params
            .items
            .iter_mut()
            .flat_map(|item| item.scope_uri.iter_mut())
            .for_each(|uri| uri.handle_server_to_client(context))
    }

    fn handle_client_to_server_response(
        _params: &Self::Params,
        _result: &mut Self::Result,
        _context: &mut Context,
    ) {
        // TODO: What rust-analyzer suppose to see?
    }
}
impl ClientToServerNotification for lsp_types::notification::DidChangeConfiguration {
    fn handle_client_to_server_notification(_params: &mut Self::Params, _context: &mut Context) {
        // TODO: What rust-analyzer suppose to see?
    }
}
impl ServerToClientRequest for lsp_types::request::WorkspaceFoldersRequest {
    fn handle_server_to_client_request(_params: &mut Self::Params, _context: &mut Context) {}

    fn handle_client_to_server_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        // TODO: Change how workspace folders are configured?
        // or disable it with client capability?
        result
            .iter_mut()
            .flat_map(|v| v.iter_mut())
            .for_each(|f| f.uri.handle_client_to_server(context))
    }
}
impl ClientToServerNotification for lsp_types::notification::DidChangeWorkspaceFolders {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        // TODO: Change how workspace folders are configured?
        // or disable it with client capability?
        params
            .event
            .added
            .iter_mut()
            .for_each(|f| f.uri.handle_client_to_server(context));
        params
            .event
            .removed
            .iter_mut()
            .for_each(|f| f.uri.handle_client_to_server(context));
    }
}
impl ClientToServerRequest for lsp_types::request::WillCreateFiles {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params
            .files
            .iter_mut()
            .for_each(|file| file.handle(context))
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .for_each(|edit| edit.handle_server_to_client(context))
    }
}
impl ClientToServerNotification for lsp_types::notification::DidCreateFiles {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        params
            .files
            .iter_mut()
            .for_each(|file| file.handle(context))
    }
}
impl ClientToServerRequest for lsp_types::request::WillRenameFiles {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params
            .files
            .iter_mut()
            .for_each(|file| file.handle(context))
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .for_each(|edit| edit.handle_server_to_client(context))
    }
}
impl ClientToServerNotification for lsp_types::notification::DidRenameFiles {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        params
            .files
            .iter_mut()
            .for_each(|file| file.handle(context))
    }
}
impl ClientToServerRequest for lsp_types::request::WillDeleteFiles {
    fn handle_client_to_server_request(params: &mut Self::Params, context: &mut Context) {
        params
            .files
            .iter_mut()
            .for_each(|file| file.handle(context))
    }

    fn handle_server_to_client_response(
        _params: &Self::Params,
        result: &mut Self::Result,
        context: &mut Context,
    ) {
        result
            .iter_mut()
            .for_each(|edit| edit.handle_server_to_client(context))
    }
}
impl ClientToServerNotification for lsp_types::notification::DidDeleteFiles {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        params
            .files
            .iter_mut()
            .for_each(|file| file.handle(context))
    }
}

impl ClientToServerNotification for lsp_types::notification::DidChangeWatchedFiles {
    fn handle_client_to_server_notification(params: &mut Self::Params, context: &mut Context) {
        params
            .changes
            .iter_mut()
            .for_each(|file| file.handle(context))
    }
}
impl ClientToServerRequest for lsp_types::request::ExecuteCommand {
    fn handle_client_to_server_request(_params: &mut Self::Params, _context: &mut Context) {
        // TODO: How Command and Data look like for rust-analyzer?
    }
}

impl ServerToClientRequest for lsp_types::request::ApplyWorkspaceEdit {
    fn handle_server_to_client_request(params: &mut Self::Params, context: &mut Context) {
        params.edit.handle_server_to_client(context)
    }
}

impl ServerToClientNotification for lsp_types::notification::ShowMessage {
    fn handle_server_to_client_notification(_params: &mut Self::Params, _context: &mut Context) {}
}
impl ServerToClientRequest for lsp_types::request::ShowMessageRequest {
    fn handle_server_to_client_request(_params: &mut Self::Params, _context: &mut Context) {}
}
impl ServerToClientRequest for lsp_types::request::ShowDocument {
    fn handle_server_to_client_request(params: &mut Self::Params, context: &mut Context) {
        params.uri.handle_server_to_client(context)
    }
}
impl ServerToClientNotification for lsp_types::notification::LogMessage {
    fn handle_server_to_client_notification(_params: &mut Self::Params, _context: &mut Context) {}
}
impl ServerToClientRequest for lsp_types::request::WorkDoneProgressCreate {
    fn handle_server_to_client_request(_params: &mut Self::Params, _context: &mut Context) {}
}
impl ServerToClientNotification for lsp_types::notification::WorkDoneProgressCancel {
    fn handle_server_to_client_notification(_params: &mut Self::Params, _context: &mut Context) {}
}
impl ServerToClientNotification for lsp_types::notification::TelemetryEvent {
    fn handle_server_to_client_notification(_params: &mut Self::Params, _context: &mut Context) {}
}
