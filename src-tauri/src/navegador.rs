//! Aba "Navegador" do layout e a trava contra link que tomava a janela inteira.
//!
//! - Link clicado no app (`<a href>` da conversa, de um markdown…) navegava a própria janela do app:
//!   a página cobria tudo e não havia como voltar. O plugin `navegacao` recusa qualquer navegação da
//!   janela principal para fora do app e manda o endereço para a interface (`abrir-link`), que abre
//!   uma aba de navegador no layout (`features/navegador`).
//! - A aba é um **webview filho de verdade** (não iframe: a maioria dos sites recusa iframe),
//!   posicionado por cima do retângulo do painel. A interface informa posição e tamanho; esconde o
//!   webview quando a aba sai da tela.
//! - A página externa não tem IPC: as capabilities valem só para a origem local do app (sem `remote`).

use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Runtime, Url, Webview, WebviewBuilder, WebviewUrl};

const PREFIXO: &str = "nav-";

#[derive(Clone, serde::Serialize)]
struct Endereco { rotulo: String, url: String }

#[derive(Clone, serde::Serialize)]
struct Titulo { rotulo: String, titulo: String }

/// Endereço do próprio app (dev e build), que a janela principal pode carregar.
///
/// Fork (segurança, 0.34.0): `asset:`/`ipc:` (e `asset.localhost`/`ipc.localhost` no Windows) saíram.
/// O Tauri trata esses esquemas como origem local e injeta o IPC: um link `asset://localhost/<arquivo>`
/// (posto por um agente num `midias.json`, por exemplo) abria um `.html` qualquer do disco **como o
/// app**, com acesso a `spawn_terminal`. Imagem, vídeo e `fetch` de `asset:` não passam por aqui.
/// `localhost`/`127.0.0.1` só no dev (servidor do Vite); no app publicado a origem é `tauri://localhost`.
fn eh_do_app(url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "about" | "data" | "blob" => true,
        "http" | "https" => url.host_str() == Some("tauri.localhost")
            || (cfg!(debug_assertions) && matches!(url.host_str(), Some("localhost" | "127.0.0.1")))
            || preview_de_video(url),
        // Fork (Editor de vídeo): o iframe do preview. Esta trava vale também para iframes; sem isto o
        // Tauri cancelava em silêncio a navegação para `medzavideo://` e o preview nunca abria
        // (só o app real tem a trava; no navegador e nos testes ele abria).
        crate::videos::protocolo::ESQUEMA => preview_de_video(url),
        _ => false,
    }
}

/// Endereço do preview com o token desta execução (macOS/Linux `medzavideo://localhost/<token>/…`,
/// Windows `http://medzavideo.localhost/<token>/…`). Outro token ou host não passa.
fn preview_de_video(url: &Url) -> bool {
    let host_certo = match url.scheme() {
        "http" => url.host_str() == Some("medzavideo.localhost"),
        s if s == crate::videos::protocolo::ESQUEMA => url.host_str() == Some("localhost"),
        _ => false,
    };
    host_certo && url.path().trim_start_matches('/').split('/').next() == Some(crate::videos::protocolo::token())
}

fn externo(url: &Url) -> bool { matches!(url.scheme(), "http" | "https") }

/// Fork (segurança): `*.localhost` dos protocolos do app (no Windows o `asset:` vira
/// `http://asset.localhost/…`) não é site: na aba ou na janela, viraria página com o IPC do app.
fn protocolo_do_app(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        let host = host.to_ascii_lowercase();
        ["asset.localhost", "ipc.localhost", "tauri.localhost", "medzavideo.localhost"].iter()
            .any(|app| host == *app || host.ends_with(&format!(".{app}")))
    })
}

/// Fork (segurança): o que a aba de navegador pode abrir — site (http/https) que não seja protocolo do
/// app, e `about:blank`. Antes ela aceitava qualquer coisa, e uma página externa fazia
/// `location = 'asset://localhost/<arquivo>'`: o arquivo abria **dentro da janela do app** com o IPC.
fn aba_pode_abrir(url: &Url) -> bool {
    (externo(url) && !protocolo_do_app(url)) || url.as_str() == "about:blank"
}

/// Plugin com a trava: a janela principal nunca sai do app.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("navegacao")
        .on_navigation(|webview: &Webview<R>, url: &Url| {
            if webview.label().starts_with(PREFIXO) { return aba_pode_abrir(url); }
            if eh_do_app(url) { return true; }
            if externo(url) {
                omni_core::registro::info("link_aberto_em_aba", &[]);
                let _ = webview.app_handle().emit_to("main", "abrir-link", url.as_str());
            }
            false
        })
        .build()
}

fn rotulo_valido(rotulo: &str) -> Result<(), String> {
    let ok = rotulo.starts_with(PREFIXO) && rotulo.len() <= 80
        && rotulo.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok { Ok(()) } else { Err("Aba de navegador inválida".into()) }
}

fn url_valida(url: &str) -> Result<Url, String> {
    let url = Url::parse(url).map_err(|_| "Endereço inválido".to_string())?;
    if externo(&url) && !protocolo_do_app(&url) { Ok(url) } else { Err("Só endereços http e https abrem na aba".into()) }
}

/// Abre (ou reposiciona e mostra) o webview da aba `rotulo` no retângulo dado, em pixels lógicos.
#[tauri::command]
pub fn navegador_abrir<R: Runtime>(app: tauri::AppHandle<R>, rotulo: String, url: String, x: f64, y: f64, largura: f64, altura: f64) -> Result<(), String> {
    rotulo_valido(&rotulo)?;
    let url = url_valida(&url)?;
    let posicao = LogicalPosition::new(x.max(0.0), y.max(0.0));
    let tamanho = LogicalSize::new(largura.max(1.0), altura.max(1.0));
    if let Some(existente) = app.get_webview(&rotulo) {
        existente.set_position(posicao).map_err(|e| e.to_string())?;
        existente.set_size(tamanho).map_err(|e| e.to_string())?;
        return existente.show().map_err(|e| e.to_string());
    }
    let janela = app.get_window("main").ok_or("Janela principal não encontrada")?;
    let app_titulo = app.clone();
    let rotulo_titulo = rotulo.clone();
    let app_url = app.clone();
    let rotulo_url = rotulo.clone();
    let app_nova = app.clone();
    let rotulo_nova = rotulo.clone();
    let builder = WebviewBuilder::new(&rotulo, WebviewUrl::External(url))
        .on_navigation(move |destino| {
            if !aba_pode_abrir(destino) { return false; } // Fork (segurança)
            let _ = app_url.emit_to("main", "navegador-endereco", Endereco { rotulo: rotulo_url.clone(), url: destino.to_string() });
            true
        })
        .on_document_title_changed(move |_, titulo| {
            let _ = app_titulo.emit_to("main", "navegador-titulo", Titulo { rotulo: rotulo_titulo.clone(), titulo });
        })
        // `target="_blank"` dentro da página abre na própria aba, em vez de não fazer nada.
        .on_new_window(move |destino, _| {
            let (app, rotulo) = (app_nova.clone(), rotulo_nova.clone());
            // Fora do callback: navegar de dentro dele reentra no webview.
            if aba_pode_abrir(&destino) { // Fork (segurança)
                std::thread::spawn(move || { if let Some(w) = app.get_webview(&rotulo) { let _ = w.navigate(destino); } });
            }
            tauri::webview::NewWindowResponse::Deny
        });
    janela.add_child(builder, posicao, tamanho).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn navegador_posicionar<R: Runtime>(app: tauri::AppHandle<R>, rotulo: String, x: f64, y: f64, largura: f64, altura: f64, visivel: bool) -> Result<(), String> {
    rotulo_valido(&rotulo)?;
    let Some(webview) = app.get_webview(&rotulo) else { return Ok(()) };
    if !visivel { return webview.hide().map_err(|e| e.to_string()); }
    webview.set_position(LogicalPosition::new(x.max(0.0), y.max(0.0))).map_err(|e| e.to_string())?;
    webview.set_size(LogicalSize::new(largura.max(1.0), altura.max(1.0))).map_err(|e| e.to_string())?;
    webview.show().map_err(|e| e.to_string())
}

/// `voltar`, `avancar`, `recarregar` ou `ir` (com `url`).
#[tauri::command]
pub fn navegador_acao<R: Runtime>(app: tauri::AppHandle<R>, rotulo: String, acao: String, url: Option<String>) -> Result<(), String> {
    rotulo_valido(&rotulo)?;
    let webview = app.get_webview(&rotulo).ok_or("Aba de navegador fechada")?;
    match acao.as_str() {
        "voltar" => webview.eval("history.back()").map_err(|e| e.to_string()),
        "avancar" => webview.eval("history.forward()").map_err(|e| e.to_string()),
        "recarregar" => webview.reload().map_err(|e| e.to_string()),
        "ir" => webview.navigate(url_valida(url.as_deref().unwrap_or(""))?).map_err(|e| e.to_string()),
        _ => Err("Ação desconhecida".into()),
    }
}

#[tauri::command]
pub fn navegador_fechar<R: Runtime>(app: tauri::AppHandle<R>, rotulo: String) -> Result<(), String> {
    rotulo_valido(&rotulo)?;
    if let Some(webview) = app.get_webview(&rotulo) { webview.close().map_err(|e| e.to_string())?; }
    Ok(())
}

/// Rede de segurança contra página de navegador sobrando na tela. O Enzo (23/10/2026) abriu um link
/// do Hyperframes, fechou a aba e ficou um quadrado branco por cima do app que não saía. A interface manda
/// as abas de navegador que existem (`vivos`) e as que estão desenhadas agora (`montados`): webview de aba
/// que não existe mais é fechado; o de aba que não está na tela, escondido. Devolve quantos mexeu.
#[tauri::command]
pub fn navegador_conferir<R: Runtime>(app: tauri::AppHandle<R>, vivos: Vec<String>, montados: Vec<String>) -> Result<usize, String> {
    let mut mexidos = 0;
    for (rotulo, webview) in app.webviews() {
        if !rotulo.starts_with(PREFIXO) { continue; }
        if !vivos.contains(&rotulo) {
            let _ = webview.hide();
            let _ = webview.close();
            mexidos += 1;
        } else if !montados.contains(&rotulo) {
            let _ = webview.hide();
        }
    }
    if mexidos > 0 { omni_core::registro::info("navegador_sobrando_fechado", &[("quantos", &mexidos.to_string())]); }
    Ok(mexidos)
}

/// "Abrir no navegador do Mac": só http/https.
#[tauri::command]
pub fn navegador_abrir_fora(url: String) -> Result<(), String> {
    let url = url_valida(&url)?;
    tauri_plugin_opener::open_url(url.as_str(), None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_o_app_fica_na_janela_e_so_http_vai_para_a_aba() {
        assert!(eh_do_app(&Url::parse("tauri://localhost/index.html").unwrap()));
        assert!(eh_do_app(&Url::parse("http://localhost:1420/").unwrap()));
        assert!(!eh_do_app(&Url::parse("https://claude.ai/artifact/x").unwrap()));
        assert!(url_valida("https://claude.ai/x").is_ok());
        assert!(url_valida("file:///etc/passwd").is_err());
        assert!(url_valida("javascript:alert(1)").is_err());
        assert!(rotulo_valido("nav-tab-abc_1").is_ok());
        assert!(rotulo_valido("main").is_err());
        assert!(rotulo_valido("nav-../x").is_err());
    }

    /// Fork (segurança): arquivo do disco (`asset:`) nunca vira página com o IPC do app.
    #[test]
    fn asset_e_ipc_nao_abrem_na_janela_nem_na_aba() {
        for url in ["asset://localhost/%2FUsers%2Fx%2Fa.html", "ipc://localhost/x", "http://asset.localhost/C%3A%2Fx.html",
            "https://asset.localhost/x", "http://ipc.localhost/x"] {
            let url = Url::parse(url).unwrap();
            assert!(!eh_do_app(&url), "{url}");
            assert!(!aba_pode_abrir(&url), "{url}");
        }
        assert!(url_valida("http://asset.localhost/C%3A%2Fx.html").is_err());
        assert!(url_valida("http://x.asset.localhost/").is_err());
        assert!(url_valida("http://localhost:3000/").is_ok(), "preview de dev continua abrindo na aba");
        assert!(aba_pode_abrir(&Url::parse("about:blank").unwrap()));
        assert!(!aba_pode_abrir(&Url::parse("file:///etc/passwd").unwrap()));
        assert!(!aba_pode_abrir(&Url::parse("tauri://localhost/index.html").unwrap()));
    }

    #[test]
    fn preview_de_video_passa_so_com_o_token_desta_execucao() {
        let tok = crate::videos::protocolo::token();
        assert!(eh_do_app(&Url::parse(&format!("medzavideo://localhost/{tok}/p/v-1/index.html?v=3")).unwrap()));
        assert!(eh_do_app(&Url::parse(&format!("http://medzavideo.localhost/{tok}/p/v-1/index.html")).unwrap()));
        assert!(!eh_do_app(&Url::parse("medzavideo://localhost/outro/p/v-1/index.html").unwrap()));
        assert!(!eh_do_app(&Url::parse(&format!("medzavideo://evil/{tok}/p/v-1/index.html")).unwrap()));
        assert!(!eh_do_app(&Url::parse(&format!("https://medzavideo.localhost/{tok}/x")).unwrap()));
    }
}
