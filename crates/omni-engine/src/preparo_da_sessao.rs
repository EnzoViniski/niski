//! Fork (Onda 2): o que a PTY de um agente ganha antes de o comando ser digitado, num lugar só.
//!
//! Encadeia o MCP do orquestrador (`mobile::orquestracao::preparar`) e os ganchos HTTP do Claude
//! (`ganchos::preparar`, que também põe a statusline quando a conta pediu). Cada um devolve o comando a
//! digitar e o ambiente extra da PTY (tokens), que nunca vai para o `sessions.json`.
use super::*;

pub(crate) fn preparar(state: &Arc<EngineState>, terminal: &str, origem: &SessionOrigin, cwd: &str, comando: Option<String>)
    -> (Option<String>, Vec<(String, String)>) {
    if origem.provider.is_some() { harness_resumo::iniciar(terminal); } // Fork (harness, A7)
    let (comando, mut ambiente) = mobile::orquestracao::preparar(state, origem.conversation_id.as_deref(), origem.provider.as_deref(), comando);
    let (comando, dos_ganchos) = ganchos::preparar(state, terminal, origem, cwd, comando);
    ambiente.extend(dos_ganchos);
    let comando = codex_app_server::preparar(state, terminal, origem, cwd, comando, &ambiente); // Fork: 3A.
    // Fork (harness, A3): a instrução do Codex vai no ambiente da PTY, não digitada.
    let (comando, da_memoria) = memoria_compartilhada::preparar(state, origem, comando);
    ambiente.extend(da_memoria);
    // Fork (harness, A2): Codex na PTY avisa o fim do turno pelo `notify` (depois da memória: mesma linha).
    let (comando, do_notify) = notify_codex::preparar(state, terminal, origem, comando);
    ambiente.extend(do_notify);
    (comando, ambiente)
}
