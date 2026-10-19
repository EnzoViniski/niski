//! Fork: "Reiniciar para atualizar". Um build novo por cima do `.app` aberto (como o
//! `scripts/build-mac-local.mjs` faz) troca os executáveis no disco; a janela continua com o código
//! antigo até reabrir. Aqui o app compara a data dos executáveis com a do momento em que abriu e,
//! se mudou, a interface oferece o botão que reabre sozinho.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::OnceLock;

struct Marca { app: Option<u64>, engine: Option<u64> }

static NA_ABERTURA: OnceLock<Marca> = OnceLock::new();

fn executavel_do_app() -> Option<PathBuf> { std::env::current_exe().ok() }

fn agora() -> Marca {
    Marca {
        app: executavel_do_app().and_then(|c| omni_protocol::modified_ms(&c)),
        engine: crate::engine_client::engine_executable().ok().and_then(|c| omni_protocol::modified_ms(&c)),
    }
}

/// Chamado no começo do `run()`: o que estava no disco quando esta janela abriu.
pub fn marcar_abertura() { let _ = NA_ABERTURA.get_or_init(agora); }

#[derive(Serialize)]
pub struct VersaoNova {
    disponivel: bool,
    /// O engine também mudou: reabrir reinicia os agentes em execução (cada conversa volta ao
    /// clicar na aba). Interface só mudou: os agentes seguem rodando.
    reinicia_agentes: bool,
    /// Fork: agentes trabalhando agora. Com algum, reabrir só troca a interface: o engine (e os
    /// agentes) seguem rodando e a troca dele fica para quando todos pararem.
    agentes_ocupados: usize,
    /// Fork: o engine também é novo (agora ou adiado).
    engine_pendente: bool,
}

// Fork: async — consulta o engine (agentes ocupados) e não pode segurar a thread da janela.
#[tauri::command(async)]
pub fn versao_nova_no_disco() -> VersaoNova {
    let abertura = NA_ABERTURA.get_or_init(agora);
    let atual = agora();
    let mudou = |antes: Option<u64>, depois: Option<u64>| antes.is_some() && depois.is_some() && antes != depois;
    let app = mudou(abertura.app, atual.app);
    // Fork: engine com troca adiada (havia agente trabalhando na abertura) também conta como novo.
    let pendente = crate::engine_client::ENGINE_PENDENTE.load(std::sync::atomic::Ordering::SeqCst);
    let engine = mudou(abertura.engine, atual.engine) || pendente;
    let ocupados = if engine { crate::engine_client::agentes_ocupados() } else { 0 };
    VersaoNova { disponivel: app || engine, reinicia_agentes: engine && ocupados == 0, agentes_ocupados: ocupados, engine_pendente: engine }
}

#[tauri::command]
pub fn reiniciar_para_atualizar(app: tauri::AppHandle) {
    omni_core::registro::info("app_reiniciou_para_atualizar", &[]);
    // `restart` sai direto, sem `RunEvent::Exit`: solta o modelo de voz aqui.
    crate::voz::liberar();
    app.restart();
}
