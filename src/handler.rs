pub fn handle_request<R: lsp_types::request::Request>(
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
pub fn handle_response<R: lsp_types::request::Request>(
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
pub fn handle_notification<N: lsp_types::notification::Notification>(
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
