//! Fork (harness, decisão C2 do Enzo, 23/10/2026): subagente do orquestrador recebe **só a memória
//! curta**, sem o Sobre mim inteiro. A recomendação oficial é contexto mínimo e focado para subagente
//! (anthropic.com/engineering/effective-context-engineering-for-ai-agents); a tarefa já chega no briefing.
//!
//! O subagente entra no registro (`orquestracao.json`) só depois que a PTY abre, mas o contexto é montado
//! antes, na abertura. Por isso a conversa fica marcada como "nascendo subagente" durante a abertura;
//! depois, o registro responde (reinício, troca de conta, retomada).
use super::*;
use std::collections::HashSet;
use std::sync::LazyLock;

static NASCENDO: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

/// Guarda da abertura: enquanto existir, a conversa conta como subagente.
pub(crate) struct Nascendo(String);
impl Drop for Nascendo {
    fn drop(&mut self) { if let Ok(mut n) = NASCENDO.lock() { n.remove(&self.0); } }
}

pub(crate) fn nascendo(conversa: &str) -> Nascendo {
    if let Ok(mut n) = NASCENDO.lock() { n.insert(conversa.to_owned()); }
    Nascendo(conversa.to_owned())
}

/// A conversa é subagente de algum orquestrador (abrindo agora ou já no registro).
pub(crate) fn eh_subagente(state: &EngineState, conversa: &str) -> bool {
    NASCENDO.lock().is_ok_and(|n| n.contains(conversa)) || ler_registro(state).dono_de(conversa).is_some()
}
