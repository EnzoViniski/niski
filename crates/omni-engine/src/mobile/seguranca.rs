//! Checagem de Host/Origin, token de dispositivo e a lista fechada de caminhos públicos.
//! Fork: saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// Comparação de tempo constante: `==` em string sai no primeiro byte diferente, e isso vaza o
/// prefixo correto para quem conseguir medir. Três linhas resolvem sem dependência nova.
pub(super) fn token_confere(recebido: &str, esperado: &str) -> bool {
    recebido.len() == esperado.len()
        && recebido.bytes().zip(esperado.bytes()).fold(0_u8,|acumulado,(a,b)| acumulado | (a ^ b)) == 0
}

/// Hosts aceitos. No modo serve o `Host` que chega é o nome MagicDNS, não o bind de loopback —
/// mas o nome continua sendo uma lista fechada, não um curinga: o check existe para barrar CSRF e
/// DNS rebinding, e aceitar qualquer `Host` desfaria exatamente isso.
pub(super) fn hosts_aceitos(state: &EngineState) -> (Vec<String>, Vec<String>) {
    let config = state.mobile.config.lock().expect("config poisoned").clone();
    let mut hosts = vec![config.bind.clone()];
    let mut origens = vec![format!("http://{}",config.bind)];
    if config.serve {
        if let Some(nome) = state.mobile.magic_dns.lock().expect("magic poisoned").clone() {
            // O serve termina TLS na 443, então o `Host` chega sem porta.
            hosts.push(nome.clone());
            origens.push(format!("https://{nome}"));
        }
    }
    (hosts,origens)
}

pub(super) async fn same_origin(State(state): State<Arc<EngineState>>, request: Request, next: Next) -> Response {
    let (hosts,origens) = hosts_aceitos(&state);
    let host = request.headers().get("host").and_then(|v|v.to_str().ok()).unwrap_or("");
    if !hosts.iter().any(|aceito| aceito == host) {
        // A mensagem nomeia a causa real: o Host tem que bater com o bind literal, então abrir pelo
        // nome MagicDNS do Tailscale cai aqui. Afrouxar a checagem não é opção — ela é a defesa
        // contra CSRF e DNS rebinding.
        return error(StatusCode::FORBIDDEN,"Host não permitido; abra pelo IP do Tailscale (100.x.x.x:porta), não pelo nome MagicDNS").into_response();
    }
    if request.method() != axum::http::Method::GET {
        let origin = request.headers().get("origin").and_then(|v|v.to_str().ok()).unwrap_or("");
        if !origens.iter().any(|aceita| aceita == origin) {
            return error(StatusCode::FORBIDDEN,"Origem não permitida").into_response();
        }
    }
    // Token de dispositivo, em cima do Tailscale. Deny-by-default com uma allow-list de duas
    // entradas: o bundle precisa carregar antes de existir token na mão do celular, e todo o resto
    // é API. Rota nova nasce protegida sem ninguém lembrar de protegê-la.
    let token = state.mobile.config.lock().expect("config poisoned").token.clone();
    let path = request.uri().path();
    let publico = caminho_publico(path);
    if !publico && !token.is_empty()
        && !token_confere(request.headers().get("x-omni-token").and_then(|v|v.to_str().ok()).unwrap_or(""),&token) {
        return error(StatusCode::UNAUTHORIZED,"Dispositivo não autorizado; leia o QR de novo no PC").into_response();
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("cache-control","no-store".parse().unwrap());
    headers.insert("x-content-type-options","nosniff".parse().unwrap());
    headers.insert("referrer-policy","no-referrer".parse().unwrap());
    // `style-src 'unsafe-inline'`: o espelho do terminal (xterm) cria o próprio `<style>` com as
    // cores e o tamanho da fonte. Script continua só `'self'` — é ele que segura injeção de código;
    // estilo inline no máximo muda a aparência, e o markdown das mensagens já passa por sanitize.
    headers.insert("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data: blob:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'".parse().unwrap());
    response
}

/// Caminhos servidos sem código de acesso. Lista **fechada**, não prefixo: todo o resto é API e nasce
/// protegido.
///
/// Ícones e manifest precisam estar aqui porque o navegador os busca **sozinho**, sem o
/// `X-Omni-Token` — ao adicionar à tela inicial, e antes mesmo de o JavaScript rodar. Antes eles
/// caíam no 401 e o celular ficava sem ícone.
// `/parear` é pública porque quem chama ainda não tem token; quem a protege é a trava de tentativas.
pub(super) const ARQUIVOS_PUBLICOS: [&str; 6] =
    ["/manifest.webmanifest", "/apple-touch-icon.png", "/icon-192.png", "/icon-512.png", "/favicon.png", "/parear"];

pub(super) fn caminho_publico(path: &str) -> bool {
    // A casca das duas páginas é pública (só HTML estático); quem protege os dados é a API.
    matches!(path, "/" | "/pc" | "/pc/" | "/pc.html")
        || path.starts_with("/assets/")
        || ARQUIVOS_PUBLICOS.contains(&path)
}
