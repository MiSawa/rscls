use std::future::Future;

/// This type exists to wrap arguments for closures so that
/// the they will be moved even if they were [Copy].
/// See: [this article](https://zenn.dev/luma/articles/rust-why-and-how-force-move-copy-trait).
pub struct Move<T>(pub T);
impl<T> Move<T> {
    pub fn moved(self) -> T {
        self.0
    }
}

pub async fn handle_request<R: lsp_types::request::Request, F>(
    request: &mut lsp_server::Request,
    handler: impl FnOnce(Move<R::Params>) -> F,
) where
    F: Future<Output = R::Params>,
{
    if request.method != R::METHOD {
        return;
    }
    if let Ok((id, params)) = request.clone().extract::<R::Params>(R::METHOD) {
        let params = handler(Move(params)).await;
        *request = lsp_server::Request::new(id, std::mem::take(&mut request.method), params);
    }
}
pub async fn handle_response<R: lsp_types::request::Request, F>(
    request: &lsp_server::Request,
    response: &mut lsp_server::Response,
    handler: impl FnOnce(Move<R::Params>, Move<R::Result>) -> F,
) where
    F: Future<Output = R::Result>,
{
    if request.method != R::METHOD {
        return;
    }
    assert_eq!(request.id, response.id);
    if let Ok((id, request_params)) = request.clone().extract::<R::Params>(R::METHOD) {
        if let Some(value) = response.clone().result {
            if let Ok(result) = serde_json::from_value(value) {
                let result = handler(Move(request_params), Move(result)).await;
                *response = lsp_server::Response::new_ok(id, result);
            }
        }
    }
}
pub async fn handle_notification<N, F>(
    notification: &mut lsp_server::Notification,
    handler: impl FnOnce(Move<N::Params>) -> F,
) where
    N: lsp_types::notification::Notification,
    F: Future<Output = N::Params>,
{
    if notification.method != N::METHOD {
        return;
    }
    if let Ok(params) = notification.clone().extract::<N::Params>(N::METHOD) {
        let params = handler(Move(params)).await;
        *notification =
            lsp_server::Notification::new(std::mem::take(&mut notification.method), params);
    }
}
