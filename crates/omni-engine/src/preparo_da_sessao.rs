//! Fork (Onda 2): o que a PTY de um agente ganha antes de o comando ser digitado, num lugar só.
//!
//! Encadeia o MCP do orquestrador (`mobile::orquestracao::preparar`) e os ganchos HTTP do Claude
//! (`ganchos::preparar`, que também põe a statusline quando a conta pediu). Cada um devolve o comando a
//! digitar e o ambiente extra da PTY (tokens), que nunca vai para o `sessions.json`.
use super::*;

pub(crate) fn preparar(state: &Arc<EngineState>, terminal: &str, origem: &SessionOrigin, cwd: &str, comando: Option<String>)
    -> (Option<String>, Vec<(String, String)>) {
    let (comando, mut ambiente) = mobile::orquestracao::preparar(state, origem.conversation_id.as_deref(), origem.provider.as_deref(), comando);
    let (comando, dos_ganchos) = ganchos::preparar(state, terminal, origem, cwd, comando);
    ambiente.extend(dos_ganchos);
    let comando = codex_app_server::preparar(state, terminal, origem, cwd, comando, &ambiente); // Fork: 3A.
    let comando = memoria_compartilhada::preparar(state, origem, comando);
    (comando, ambiente)
}
