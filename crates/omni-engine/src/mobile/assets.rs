//! Bundle da página (celular e `/pc`) embutido no engine e o tipo de cada arquivo.
//! Saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

#[cfg(mobile_assets)]
static ASSETS: include_dir::Dir<'_> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../dist-mobile");

/// `nosniff` está ligado em toda resposta, então o tipo tem de estar certo: ícone ou manifest
/// servidos como `application/octet-stream` são ignorados pelo celular.
pub(super) fn tipo_do_arquivo(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

pub(super) async fn asset(request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/');
    // `/` é a página do celular; `/pc` é a de computador — mesmas rotas de API, layout largo.
    let path = match path {
        "" => "index.html",
        "pc" | "pc/" => "pc.html",
        outro => outro,
    };
    #[cfg(mobile_assets)]
    if let Some(file) = ASSETS.get_file(path) {
        return ([("content-type",tipo_do_arquivo(path))],file.contents()).into_response();
    }
    let _ = path;
    (StatusCode::NOT_FOUND,"Página não encontrada; em desenvolvimento, execute npm run build:mobile antes de compilar o engine").into_response()
}
