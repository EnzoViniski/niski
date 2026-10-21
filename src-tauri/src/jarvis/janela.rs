//! Fork: Jarvis. Janela flutuante, sempre por cima, em todas as mesas; encolhe para uma bolinha e
//! volta. Posição, tamanho e "bolinha" ficam em `<pasta de dados>/engine/jarvis-janela.json`
//! (arquivo novo; nada do estado da interface muda). Também o atalho global e o fechamento junto da
//! janela principal.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent};

pub const ROTULO: &str = "jarvis";
/// ⌃⌥J abre e fecha o Jarvis de qualquer app.
pub const ATALHO: &str = "ctrl+alt+KeyJ";
const LARGURA: f64 = 380.0;
const ALTURA: f64 = 560.0;
const BOLINHA: f64 = 88.0;
const MARGEM: f64 = 24.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Geometria {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub largura: f64,
    pub altura: f64,
    pub bolinha: bool,
}

impl Default for Geometria {
    fn default() -> Self { Self { x: None, y: None, largura: LARGURA, altura: ALTURA, bolinha: false } }
}

static GEOMETRIA: Mutex<Option<Geometria>> = Mutex::new(None);
static GRAVADA_EM: Mutex<Option<Instant>> = Mutex::new(None);

fn arquivo() -> Option<PathBuf> { crate::engine_client::engine_dir().ok().map(|d| d.join("jarvis-janela.json")) }

fn ler() -> Geometria {
    arquivo().and_then(|a| std::fs::read(a).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn gravar(geometria: &Geometria) {
    let Some(arquivo) = arquivo() else { return };
    if let Ok(json) = serde_json::to_vec_pretty(geometria) { let _ = std::fs::write(arquivo, json); }
}

#[derive(Clone, Serialize)]
struct Aberta { aberta: bool }

fn avisar_principal(app: &AppHandle, aberta: bool) { let _ = app.emit_to("main", "jarvis-janela", Aberta { aberta }); }

/// Ao encolher para bolinha, a borda direita fica onde estava (o Jarvis mora no canto direito).
pub fn x_ao_trocar(x: f64, de: f64, para: f64) -> f64 { (x + de - para).max(0.0) }

/// O ponto (canto de cima) cai em algum monitor? Monitor desligado desde a última vez → posição padrão.
fn dentro_de_um_monitor(app: &AppHandle, x: f64, y: f64) -> bool {
    app.available_monitors().unwrap_or_default().iter().any(|m| {
        let escala = m.scale_factor();
        let (px, py) = (m.position().x as f64 / escala, m.position().y as f64 / escala);
        let (w, h) = (m.size().width as f64 / escala, m.size().height as f64 / escala);
        x >= px && x < px + w - 40.0 && y >= py && y < py + h - 40.0
    })
}

fn posicao_padrao(app: &AppHandle, largura: f64) -> (f64, f64) {
    app.primary_monitor().ok().flatten().map(|m| {
        let escala = m.scale_factor();
        let x = m.position().x as f64 / escala + m.size().width as f64 / escala - largura - MARGEM;
        (x, m.position().y as f64 / escala + 48.0)
    }).unwrap_or((MARGEM, 48.0))
}

pub fn abrir(app: &AppHandle) -> Result<(), String> {
    if let Some(janela) = app.get_webview_window(ROTULO) { return janela.set_focus().map_err(|e| e.to_string()); }
    let mut geometria = ler();
    let (largura, altura) = if geometria.bolinha { (BOLINHA, BOLINHA) } else { (geometria.largura.max(280.0), geometria.altura.max(320.0)) };
    let (x, y) = match (geometria.x, geometria.y) {
        (Some(x), Some(y)) if dentro_de_um_monitor(app, x, y) => (x, y),
        _ => posicao_padrao(app, largura),
    };
    (geometria.x, geometria.y) = (Some(x), Some(y));
    let construtor = WebviewWindowBuilder::new(app, ROTULO, WebviewUrl::App("index.html".into()))
        .title("Jarvis")
        .inner_size(largura, altura)
        .min_inner_size(BOLINHA, BOLINHA)
        .position(x, y)
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .resizable(!geometria.bolinha)
        .shadow(false)
        .focused(true);
    #[cfg(target_os = "macos")]
    let construtor = construtor.transparent(true);
    construtor.build().map_err(|e| format!("A janela do Jarvis não abriu: {e}"))?;
    *GEOMETRIA.lock().unwrap_or_else(|e| e.into_inner()) = Some(geometria);
    avisar_principal(app, true);
    Ok(())
}

pub fn alternar(app: &AppHandle) -> Result<(), String> {
    match app.get_webview_window(ROTULO) {
        Some(janela) => janela.close().map_err(|e| e.to_string()),
        None => abrir(app),
    }
}

#[tauri::command]
pub fn jarvis_alternar(app: AppHandle) -> Result<(), String> { alternar(&app) }

#[tauri::command]
pub fn jarvis_fechar(app: AppHandle) -> Result<(), String> {
    app.get_webview_window(ROTULO).map(|j| j.close().map_err(|e| e.to_string())).unwrap_or(Ok(()))
}

#[tauri::command]
pub fn jarvis_janela_aberta(app: AppHandle) -> bool { app.get_webview_window(ROTULO).is_some() }

#[tauri::command]
pub fn jarvis_bolinha(app: AppHandle, ligada: bool) -> Result<(), String> {
    let janela = app.get_webview_window(ROTULO).ok_or("O Jarvis não está aberto")?;
    let mut guarda = GEOMETRIA.lock().unwrap_or_else(|e| e.into_inner());
    let geometria = guarda.get_or_insert_with(ler);
    if geometria.bolinha == ligada { return Ok(()); }
    let escala = janela.scale_factor().map_err(|e| e.to_string())?;
    let posicao = janela.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(escala);
    let (de, para) = if ligada { (geometria.largura, BOLINHA) } else { (BOLINHA, geometria.largura) };
    let (altura, x) = (if ligada { BOLINHA } else { geometria.altura }, x_ao_trocar(posicao.x, de, para));
    geometria.bolinha = ligada;
    (geometria.x, geometria.y) = (Some(x), Some(posicao.y));
    janela.set_resizable(!ligada).map_err(|e| e.to_string())?;
    janela.set_size(LogicalSize::new(para, altura)).map_err(|e| e.to_string())?;
    janela.set_position(LogicalPosition::new(x, posicao.y)).map_err(|e| e.to_string())?;
    gravar(geometria);
    Ok(())
}

/// Guarda posição e tamanho enquanto o Enzo arrasta (no máximo a cada meio segundo) e ao fechar.
fn lembrar(janela: &Window, forcar: bool) {
    let mut gravada = GRAVADA_EM.lock().unwrap_or_else(|e| e.into_inner());
    if !forcar && gravada.is_some_and(|t| t.elapsed() < Duration::from_millis(500)) { return; }
    let Ok(escala) = janela.scale_factor() else { return };
    let mut guarda = GEOMETRIA.lock().unwrap_or_else(|e| e.into_inner());
    let Some(geometria) = guarda.as_mut() else { return };
    if let Ok(posicao) = janela.outer_position() {
        let p = posicao.to_logical::<f64>(escala);
        (geometria.x, geometria.y) = (Some(p.x), Some(p.y));
    }
    if !geometria.bolinha {
        if let Ok(tamanho) = janela.inner_size() {
            let t = tamanho.to_logical::<f64>(escala);
            (geometria.largura, geometria.altura) = (t.width, t.height);
        }
    }
    gravar(geometria);
    *gravada = Some(Instant::now());
}

/// Ligado em `on_window_event` (lib.rs).
pub fn ao_evento_da_janela(janela: &Window, evento: &WindowEvent) {
    if janela.label() == ROTULO {
        match evento {
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => lembrar(janela, false),
            WindowEvent::CloseRequested { .. } => lembrar(janela, true),
            WindowEvent::Destroyed => {
                super::jarvis_encerrar();
                avisar_principal(janela.app_handle(), false);
            }
            _ => {}
        }
    } else if janela.label() == "main" && matches!(evento, WindowEvent::Destroyed) {
        // Sem a janela principal o app fecha, como antes do Jarvis existir.
        if let Some(jarvis) = janela.app_handle().get_webview_window(ROTULO) { let _ = jarvis.close(); }
    }
}

/// Atalho global ⌃⌥J. Se outro app já usa a combinação, o Jarvis segue pelo botão da barra.
pub fn instalar(app: &tauri::App) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _atalho, evento| {
            if evento.state == ShortcutState::Pressed { let _ = alternar(app); }
        })
        .build();
    if app.handle().plugin(plugin).is_err() { return; }
    if app.global_shortcut().register(ATALHO).is_err() {
        omni_core::registro::aviso("jarvis_atalho_ocupado", &[("atalho", ATALHO)]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bolinha_mantem_a_borda_direita() {
        assert_eq!(x_ao_trocar(1000.0, LARGURA, BOLINHA), 1000.0 + LARGURA - BOLINHA);
        assert_eq!(x_ao_trocar(1292.0, BOLINHA, LARGURA), 1000.0);
        assert_eq!(x_ao_trocar(10.0, BOLINHA, LARGURA), 0.0);
    }

    #[test]
    fn geometria_antiga_ou_parcial_le_com_padrao() {
        let g: Geometria = serde_json::from_str(r#"{"x":10}"#).unwrap();
        assert_eq!(g, Geometria { x: Some(10.0), ..Geometria::default() });
    }

    #[test]
    fn atalho_e_valido() {
        assert!(ATALHO.parse::<tauri_plugin_global_shortcut::Shortcut>().is_ok());
    }
}
