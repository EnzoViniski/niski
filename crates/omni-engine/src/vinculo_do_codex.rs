//! Fork (conversa do Codex): o `thread-id` que o próprio Codex manda no fim de cada turno (`notify`,
//! ver `notify_codex.rs`) liga a conversa da aba ao rollout certo.
//!
//! Antes, a única ligação era a busca por "mesma pasta + criado até 30 s depois de a aba abrir"
//! (`conversations::resolve_transcript`). Ela falhava em dois casos reais (23/10/2026): um subagente
//! de outra aba criado na mesma pasta no mesmo intervalo (empate) e o Codex reaberto na mesma aba
//! minutos depois (o rollout novo nasce fora da janela). A mensagem ia para o terminal e a Conversa
//! mostrava "Parte do histórico está indisponível". O `thread-id` do aviso não depende de horário.
//!
//! Só vale rollout da conta do trecho e que não seja de subagente; um `/new` ou um `fork` dentro do
//! Codex troca o thread da aba, e a conversa passa a seguir o novo (como o `/clear` no Claude).

use super::*;

/// Id de thread do Codex (UUID): só letras, números e `-`.
fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 100 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Liga o último trecho da conversa desta PTY ao thread `sid`, se ele for o da sessão principal.
pub(crate) fn vincular(state: &EngineState, sessao: &LiveSession, terminal: &str, sid: &str) {
    if !id_valido(sid) { return; }
    let (provider, conversa, perfil) = match sessao.meta.lock() {
        Ok(meta) => (meta.provider.clone(), meta.conversation_id.clone(), meta.profile_id.clone()),
        Err(_) => return,
    };
    if provider.as_deref() != Some("codex") { return; }
    let Some(conversa) = conversa else { return };
    let dir = state.state_file.parent().map(Path::to_path_buf).unwrap_or_default();
    // Chamado a cada fim de turno: já ligado a este thread, nada a procurar.
    let ja_ligado = omni_core::conversations::read_index(&dir).iter().find(|c| c.id == conversa)
        .and_then(|c| c.segments.last())
        .is_some_and(|t| t.external_session_id.as_deref() == Some(sid) && t.transcript_path.as_deref().is_some_and(|p| Path::new(p).is_file()));
    if ja_ligado { return; }
    let perfis = omni_core::profiles(&dir);
    let Some(conta) = perfis.iter().find(|p| p.provider == "codex" && Some(&p.id) == perfil.as_ref()) else { return };
    let Some(caminho) = omni_core::achar_transcript::codex_na_conta(Path::new(&conta.config_dir), sid) else { return };
    let Some(meta_do_rollout) = omni_core::subagente_do_codex::session_meta(&caminho) else { return };
    if omni_core::subagente_do_codex::de_subagente(&meta_do_rollout["payload"]) { return; }
    let caminho = caminho.canonicalize().unwrap_or(caminho).to_string_lossy().into_owned();
    let mudou = omni_core::indice_de_conversas::mudar_indice(&dir, |indice| {
        let Some(trecho) = indice.iter_mut().find(|c| c.id == conversa).and_then(|c| c.segments.last_mut()) else { return false };
        if trecho.provider != "codex" || trecho.terminal_session_id.as_deref().is_some_and(|t| t != terminal) { return false; }
        if trecho.external_session_id.as_deref() == Some(sid) && trecho.transcript_path.as_deref() == Some(caminho.as_str()) { return false; }
        trecho.external_session_id = Some(sid.to_owned());
        trecho.transcript_path = Some(caminho.clone());
        true
    });
    if !mudou.unwrap_or(false) { return; }
    // O reinício (`retomar_codex::comando`) retoma pelo id da sessão quando ele existe.
    if let Ok(mut meta) = sessao.meta.lock() { meta.external_session_id = Some(sid.to_owned()); }
    let _ = state.persist();
    registro::info("codex_thread_vinculado", &[("terminal", terminal), ("conversa", &conversa)]);
}

#[cfg(all(test, unix))]
#[path = "vinculo_do_codex_testes.rs"]
mod testes;
