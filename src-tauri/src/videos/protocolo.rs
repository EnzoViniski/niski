//! Fork (Editor de vídeo): protocolo `medzavideo://` que serve os arquivos de um projeto para o
//! preview (`<hyperframes-player>` num iframe com sandbox opaco) e para as miniaturas da biblioteca.
//!
//! - Endereço: `medzavideo://localhost/<token>/p/<projeto>/<caminho>` (macOS/Linux) ou
//!   `http://medzavideo.localhost/...` (Windows). `r/<pasta>` serve uma pasta de referências.
//! - O token muda a cada execução do app: uma página aberta no navegador embutido não adivinha o
//!   endereço. Os arquivos de projeto vão com `Access-Control-Allow-Origin: *` porque o runtime busca
//!   as sub-composições com `fetch` de dentro do iframe de origem opaca; o segredo é o token.
//! - No HTML do projeto entra, antes de tudo, o runtime do HyperFrames na versão fixada (a mesma URL
//!   que o player usaria): os scripts da composição contam com `window.__timelines` já criado, como no
//!   render do CLI. O arquivo no disco não muda.
//! - Só arquivo comum dentro da pasta (sem `..`, sem link simbólico para fora). O chat
//!   (`conversa.jsonl`), o `projeto.json` e o `historico/` não são servidos.
//! - HTML vai com CSP fechada: scripts só da pasta e das CDNs que a composição usa, `connect-src`
//!   só da pasta (a composição não manda nada para a internet), sem frames nem formulários.
//! - Vídeo e áudio com `Range` (206), em pedaços de até 4 MB: arrastar a agulha não lê o arquivo todo.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const ESQUEMA: &str = "medzavideo";
const PEDACO: u64 = 4 * 1024 * 1024;

pub fn token() -> &'static str {
    static TOKEN: OnceLock<String> = OnceLock::new();
    TOKEN.get_or_init(|| format!("{:032x}", uuid::Uuid::new_v4().as_u128()))
}

pub struct Resposta {
    pub status: u16,
    pub cabecalhos: Vec<(&'static str, String)>,
    pub corpo: Vec<u8>,
}

fn erro(status: u16, texto: &str) -> Resposta {
    Resposta { status, cabecalhos: vec![("Content-Type", "text/plain; charset=utf-8".into()), ("Cache-Control", "no-store".into())], corpo: texto.as_bytes().to_vec() }
}

/// Política da composição. `'self'` não vale num iframe de origem opaca, por isso o esquema vai escrito.
pub const CSP_DA_COMPOSICAO: &str = "default-src 'none'; \
script-src medzavideo: http://medzavideo.localhost 'unsafe-inline' 'unsafe-eval' https://cdn.jsdelivr.net https://unpkg.com https://cdnjs.cloudflare.com; \
style-src medzavideo: http://medzavideo.localhost 'unsafe-inline' https://fonts.googleapis.com https://cdn.jsdelivr.net; \
font-src medzavideo: http://medzavideo.localhost data: https://fonts.gstatic.com https://cdn.jsdelivr.net; \
img-src medzavideo: http://medzavideo.localhost data: blob: https:; \
media-src medzavideo: http://medzavideo.localhost data: blob: https:; \
connect-src medzavideo: http://medzavideo.localhost data: blob:; \
worker-src blob:; frame-src 'none'; form-action 'none'; base-uri medzavideo: http://medzavideo.localhost";

/// Runtime que o `<hyperframes-player>` 0.8.183 injeta (`runtime-url.ts`); a mesma URL evita injetar duas vezes.
pub const RUNTIME: &str = "https://cdn.jsdelivr.net/npm/@hyperframes/core@0.8.183/dist/hyperframe.runtime.iife.js";

/// Põe o runtime no começo do `<head>` (ou do documento), se ainda não tiver.
pub fn com_runtime(html: &str) -> String {
    if html.contains("hyperframe.runtime") || html.contains("__hyperframes =") { return html.to_owned(); }
    let tag = format!("<script src=\"{RUNTIME}\"></script>");
    let baixo = html.to_ascii_lowercase();
    let depois_do_head = baixo.find("<head").and_then(|i| baixo[i..].find('>').map(|j| i + j + 1));
    match depois_do_head {
        Some(i) => format!("{}{tag}{}", &html[..i], &html[i..]),
        None => format!("{tag}{html}"),
    }
}

fn tipo(caminho: &Path) -> &'static str {
    match caminho.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "lottie") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("m4a" | "aac") => "audio/mp4",
        Some("ogg" | "opus") => "audio/ogg",
        Some("flac") => "audio/flac",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("vtt") => "text/vtt; charset=utf-8",
        Some("md" | "txt" | "srt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn decodificar(trecho: &str) -> Option<String> {
    let bytes = trecho.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            saida.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else { saida.push(bytes[i]); i += 1; }
    }
    String::from_utf8(saida).ok()
}

/// Pasta e caminho relativo pedidos, já conferidos. `None` = proibido ou inválido.
fn resolver(raiz: &Path, token_esperado: &str, caminho_da_url: &str) -> Option<(PathBuf, String)> {
    let caminho = caminho_da_url.split(['?', '#']).next().unwrap_or("");
    let mut partes = caminho.trim_start_matches('/').splitn(4, '/');
    if partes.next()? != token_esperado { return None; }
    let tipo = partes.next()?;
    let id = partes.next()?;
    if !super::arquivos::id_valido(id) { return None; }
    let resto = partes.next().filter(|r| !r.is_empty()).unwrap_or("index.html");
    let mut segmentos = Vec::new();
    for s in resto.split('/') {
        let s = decodificar(s)?;
        if s.is_empty() || s.starts_with('.') || s.contains(['\\', '\0']) { return None; }
        segmentos.push(s);
    }
    let rel = segmentos.join("/");
    let pasta = match tipo {
        "p" => {
            if matches!(rel.as_str(), "conversa.jsonl" | "projeto.json") || rel.starts_with("historico/") { return None; }
            raiz.join(super::projetos::PROJETOS).join(id)
        }
        "r" => {
            if !rel.starts_with(super::referencias::PREFIXO) { return None; }
            raiz.join(super::referencias::REFERENCIAS).join(id)
        }
        _ => return None,
    };
    Some((pasta, rel))
}

/// `bytes=a-b` → (início, fim inclusivo) dentro de `tamanho`.
fn intervalo(cabecalho: &str, tamanho: u64) -> Option<(u64, u64)> {
    let pedido = cabecalho.trim().strip_prefix("bytes=")?.split(',').next()?.trim();
    let (a, b) = pedido.split_once('-')?;
    if tamanho == 0 { return None; }
    let (inicio, fim) = if a.is_empty() {
        let ultimos: u64 = b.parse().ok()?;
        (tamanho.saturating_sub(ultimos), tamanho - 1)
    } else {
        let inicio: u64 = a.parse().ok()?;
        let fim = if b.is_empty() { tamanho - 1 } else { b.parse::<u64>().ok()?.min(tamanho - 1) };
        (inicio, fim)
    };
    (inicio <= fim && inicio < tamanho).then_some((inicio, fim))
}

pub fn responder(raiz: &Path, token_esperado: &str, metodo: &str, caminho_da_url: &str, range: Option<&str>) -> Resposta {
    if metodo != "GET" && metodo != "HEAD" { return erro(405, "Método não permitido"); }
    let Some((pasta, rel)) = resolver(raiz, token_esperado, caminho_da_url) else { return erro(404, "Não encontrado") };
    let do_projeto = caminho_da_url.trim_start_matches('/').split('/').nth(1) == Some("p");
    let alvo = pasta.join(&rel);
    // Sem link simbólico em nenhum ponto do caminho (podia apontar para fora da pasta).
    let mut atual = pasta.clone();
    if !super::arquivos::pasta_comum(&atual) { return erro(404, "Não encontrado"); }
    for s in rel.split('/') {
        atual.push(s);
        match fs::symlink_metadata(&atual) { Ok(m) if !m.file_type().is_symlink() => {} _ => return erro(404, "Não encontrado") }
    }
    let Some(tamanho) = super::arquivos::arquivo_comum(&alvo) else { return erro(404, "Não encontrado") };
    let tipo = tipo(&alvo);
    let mut cabecalhos: Vec<(&'static str, String)> = vec![
        ("Content-Type", tipo.into()),
        ("Cache-Control", "no-cache".into()),
        ("X-Content-Type-Options", "nosniff".into()),
        ("Accept-Ranges", "bytes".into()),
    ];
    if tipo.starts_with("text/html") { cabecalhos.push(("Content-Security-Policy", CSP_DA_COMPOSICAO.into())); }
    if do_projeto { cabecalhos.push(("Access-Control-Allow-Origin", "*".into())); }
    if do_projeto && tipo.starts_with("text/html") && tamanho <= super::projetos::TAMANHO_MAXIMO as u64 {
        let Ok(texto) = fs::read_to_string(&alvo) else { return erro(500, "Não consegui ler o arquivo") };
        let corpo = com_runtime(&texto).into_bytes();
        cabecalhos.retain(|(n, _)| *n != "Accept-Ranges");
        cabecalhos.push(("Content-Length", corpo.len().to_string()));
        return Resposta { status: 200, cabecalhos, corpo: if metodo == "HEAD" { Vec::new() } else { corpo } };
    }
    let Ok(mut arquivo) = fs::File::open(&alvo) else { return erro(404, "Não encontrado") };
    let pedido = range.and_then(|r| intervalo(r, tamanho));
    if range.is_some() && pedido.is_none() {
        return Resposta { status: 416, cabecalhos: vec![("Content-Range", format!("bytes */{tamanho}"))], corpo: Vec::new() };
    }
    let (status, inicio, fim) = match pedido {
        Some((a, b)) => (206, a, b.min(a + PEDACO - 1)),
        None if tamanho <= 16 * PEDACO => (200, 0, tamanho.saturating_sub(1)),
        // Arquivo grande sem Range (não acontece com <video>): só o começo, para não ler GB na memória.
        None => (206, 0, PEDACO - 1),
    };
    let quantos = if tamanho == 0 { 0 } else { fim - inicio + 1 };
    if status == 206 { cabecalhos.push(("Content-Range", format!("bytes {inicio}-{fim}/{tamanho}"))); }
    cabecalhos.push(("Content-Length", quantos.to_string()));
    if metodo == "HEAD" { return Resposta { status, cabecalhos, corpo: Vec::new() }; }
    let mut corpo = vec![0u8; quantos as usize];
    if arquivo.seek(SeekFrom::Start(inicio)).is_err() || arquivo.read_exact(&mut corpo).is_err() { return erro(500, "Não consegui ler o arquivo"); }
    Resposta { status, cabecalhos, corpo }
}

/// Liga o protocolo no app. A leitura roda fora da thread da janela.
pub fn atender<R: tauri::Runtime>(_ctx: tauri::UriSchemeContext<'_, R>, pedido: tauri::http::Request<Vec<u8>>, resposta: tauri::UriSchemeResponder) {
    let metodo = pedido.method().as_str().to_owned();
    let caminho = pedido.uri().path().to_owned();
    let range = pedido.headers().get("range").and_then(|v| v.to_str().ok()).map(str::to_owned);
    std::thread::spawn(move || {
        let r = match super::raiz() {
            Ok(raiz) => responder(&raiz, token(), &metodo, &caminho, range.as_deref()),
            Err(_) => erro(500, "Pasta de dados indisponível"),
        };
        let mut construtor = tauri::http::Response::builder().status(r.status);
        for (nome, valor) in r.cabecalhos { construtor = construtor.header(nome, valor); }
        match construtor.body(r.corpo) {
            Ok(corpo) => resposta.respond(corpo),
            Err(_) => resposta.respond(tauri::http::Response::builder().status(500).body(Vec::new()).unwrap_or_default()),
        }
    });
}

/// Base dos endereços para o front: `<esquema>://localhost/<token>` (o front escolhe a forma do Windows).
#[tauri::command]
pub fn videos_endereco_base() -> String { token().to_owned() }

#[cfg(test)]
mod testes {
    use super::*;

    fn projeto() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let id = "v-teste-1".to_owned();
        let pasta = dir.path().join("projetos").join(&id);
        fs::create_dir_all(pasta.join("assets")).unwrap();
        fs::create_dir_all(pasta.join("historico/000001")).unwrap();
        fs::write(pasta.join("index.html"), "<div></div>").unwrap();
        fs::write(pasta.join("conversa.jsonl"), "{}\n").unwrap();
        fs::write(pasta.join("projeto.json"), "{}").unwrap();
        fs::write(pasta.join("historico/000001/index.html"), "velho").unwrap();
        fs::write(pasta.join("assets/clip de teste.mp4"), (0u8..=255).collect::<Vec<_>>()).unwrap();
        (dir, id)
    }

    #[test]
    fn serve_html_com_csp_e_raiz_vira_index() {
        let (dir, id) = projeto();
        let r = responder(dir.path(), "tok", "GET", &format!("/tok/p/{id}/"), None);
        assert_eq!(r.status, 200);
        let corpo = String::from_utf8(r.corpo).unwrap();
        assert_eq!(corpo, format!("<script src=\"{RUNTIME}\"></script><div></div>"));
        assert!(r.cabecalhos.iter().any(|(n, v)| *n == "Content-Security-Policy" && v.contains("connect-src medzavideo:")));
        assert!(r.cabecalhos.iter().any(|(n, v)| *n == "Content-Length" && *v == corpo.len().to_string()));
    }

    #[test]
    fn runtime_entra_no_comeco_do_head_uma_vez_so() {
        let html = "<!doctype html><html><HEAD lang=x><script>window.__timelines['main']=1</script></head></html>";
        let com = com_runtime(html);
        assert!(com.starts_with("<!doctype html><html><HEAD lang=x><script src=\"https://cdn.jsdelivr.net/npm/@hyperframes/core@0.8.183/"));
        assert!(com.find("hyperframe.runtime").unwrap() < com.find("__timelines").unwrap());
        assert_eq!(com_runtime(&com), com);
    }

    #[test]
    fn recusa_token_errado_fuga_e_arquivos_internos() {
        let (dir, id) = projeto();
        for caminho in [
            format!("/outro/p/{id}/index.html"), format!("/tok/p/{id}/../{id}/index.html"), format!("/tok/p/{id}/%2e%2e/x"),
            format!("/tok/p/{id}/conversa.jsonl"), format!("/tok/p/{id}/projeto.json"), format!("/tok/p/{id}/historico/000001/index.html"),
            format!("/tok/p/../index.html"), format!("/tok/x/{id}/index.html"), format!("/tok/r/{id}/index.html"),
        ] {
            assert_eq!(responder(dir.path(), "tok", "GET", &caminho, None).status, 404, "{caminho}");
        }
        assert_eq!(responder(dir.path(), "tok", "POST", &format!("/tok/p/{id}/index.html"), None).status, 405);
    }

    #[cfg(unix)]
    #[test]
    fn nao_segue_link_para_fora() {
        let (dir, id) = projeto();
        fs::write(dir.path().join("segredo.txt"), "segredo").unwrap();
        std::os::unix::fs::symlink(dir.path().join("segredo.txt"), dir.path().join("projetos").join(&id).join("assets/link.txt")).unwrap();
        assert_eq!(responder(dir.path(), "tok", "GET", &format!("/tok/p/{id}/assets/link.txt"), None).status, 404);
    }

    #[test]
    fn range_devolve_206_e_nome_com_espaco_funciona() {
        let (dir, id) = projeto();
        let caminho = format!("/tok/p/{id}/assets/clip%20de%20teste.mp4");
        let r = responder(dir.path(), "tok", "GET", &caminho, Some("bytes=10-19"));
        assert_eq!(r.status, 206);
        assert_eq!(r.corpo, (10u8..20).collect::<Vec<_>>());
        assert!(r.cabecalhos.iter().any(|(n, v)| *n == "Content-Range" && v == "bytes 10-19/256"));
        assert!(r.cabecalhos.iter().any(|(n, v)| *n == "Content-Type" && v == "video/mp4"));
        let fim = responder(dir.path(), "tok", "GET", &caminho, Some("bytes=-6"));
        assert_eq!(fim.corpo, (250u8..=255).collect::<Vec<_>>());
        assert_eq!(responder(dir.path(), "tok", "GET", &caminho, Some("bytes=999-")).status, 416);
        let cabeca = responder(dir.path(), "tok", "HEAD", &caminho, None);
        assert!(cabeca.corpo.is_empty());
        assert!(cabeca.cabecalhos.iter().any(|(n, v)| *n == "Content-Length" && v == "256"));
    }
}
