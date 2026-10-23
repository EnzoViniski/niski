//! Fork (modo do Codex na Conversa): trocar aprovação/sandbox do Codex pelo seletor do campo.
//!
//! O Codex não troca o modo por tecla; é a linha de comando (`omni_core::modo_do_codex`). Trocar é o
//! mesmo que trocar modelo/esforço (`ajustes::reabrir_com`): com o agente parado, a sessão reabre na
//! **mesma** conversa (`codex resume <id>`, `retomar_codex`) com as opções novas; sem mensagem ainda,
//! abre de novo já no modo pedido. A escolha fica guardada por conversa (`modos-das-conversas.json`) e
//! vale também ao retomar pelo histórico.
use super::*;

pub(super) fn trocar(state: &Arc<EngineState>, action: &Action, alvo: &str) -> Result<()> {
    anyhow::ensure!(omni_core::modo_do_codex::valido(alvo), "Modo inválido");
    anyhow::ensure!(now_ms().saturating_sub(action.created_at_ms) < 60_000, "Ação expirada");
    let (_, sessao) = current(state, &action.conversation_id).map_err(|_| anyhow!("Conversa removida"))?;
    let sessao = sessao.ok_or_else(|| anyhow!("Sessão encerrada"))?;
    {
        let sessions = state.sessions.lock().map_err(|_| anyhow!("sessions poisoned"))?;
        anyhow::ensure!(matches!(sessions.get(&action.session_id), Some(SessionEntry::Live(s)) if Arc::ptr_eq(s, &sessao)), "Sessão trocada");
    }
    let (sessao_id, base) = {
        let meta = sessao.meta.lock().map_err(|_| anyhow!("metadata poisoned"))?;
        anyhow::ensure!(meta.provider.as_deref() == Some("codex"), "Este controle é do Codex");
        (meta.id.clone(), meta.initial_command.clone().ok_or_else(|| anyhow!("Sessão sem comando do Codex"))?)
    };
    // App-server (`--remote`): a política é do servidor, não da linha da TUI.
    anyhow::ensure!(!crate::codex_app_server::ativo(&sessao_id) && !base.contains("--remote"), "No Codex pelo app-server, troque o modo no terminal");
    {
        let context = sessao.interaction.lock().map_err(|_| anyhow!("screen poisoned"))?;
        // Reabrir no meio de um turno mataria o trabalho em andamento.
        anyhow::ensure!(!sessao.reserved.load(Ordering::SeqCst) && interaction::ready_no(&context, "codex"),
            "O Codex está trabalhando; troque o modo quando ele terminar");
    }
    let novo = omni_core::modo_do_codex::com_modo(&base, alvo).ok_or_else(|| anyhow!("Não deu para montar o comando do Codex com este modo"))?;
    comandos::guardar_modo(state, &action.conversation_id, alvo)?;
    sessao.meta.lock().map_err(|_| anyhow!("metadata poisoned"))?.initial_command = Some(novo);
    state.persist()?;
    registro::info("codex_modo_trocado", &[("sessao", &sessao_id), ("modo", alvo)]);
    restart_session(state, &sessao_id)?;
    Ok(())
}
