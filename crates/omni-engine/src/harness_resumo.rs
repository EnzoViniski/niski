//! Fork (harness, A7): uma linha `harness_resumo` no log quando a CLI de uma sessão de agente termina (ou a
//! sessão é fechada), só com números: de onde veio o estado (canal, eventos, turnos por evento × pela
//! tela), quantas vezes a Conversa não conseguiu digitar (motivos distintos), trocas de conta e
//! compactações. É a medida para comparar antes e depois no uso real, sem conteúdo nenhum.
use super::*;
use std::{collections::HashSet, sync::LazyLock};

#[derive(Default)]
struct Conta { sem_entrada: HashSet<String>, trocas_de_conta: u64 }

static SESSOES: LazyLock<Mutex<HashMap<String, Conta>>> = LazyLock::new(Default::default);

/// Uma sessão de agente começou (spawn ou reinício): zera a conta dela.
pub(crate) fn iniciar(terminal: &str) {
    if let Ok(mut s) = SESSOES.lock() { s.insert(terminal.to_owned(), Conta::default()); }
}

/// A Conversa não conseguiu digitar por este motivo (cada motivo conta uma vez por sessão).
pub(crate) fn sem_entrada(terminal: &str, motivo: &str) {
    if let Ok(mut s) = SESSOES.lock() {
        if let Some(c) = s.get_mut(terminal) { if c.sem_entrada.len() < 64 { c.sem_entrada.insert(motivo.to_owned()); } }
    }
}

pub(crate) fn troca_de_conta(terminal: &str) {
    if let Ok(mut s) = SESSOES.lock() { if let Some(c) = s.get_mut(terminal) { c.trocas_de_conta += 1; } }
}

/// Grava a linha e esquece a sessão. Uma vez por processo da CLI: a segunda chamada não acha nada.
pub(crate) fn registrar(terminal: &str, provider: &str, turnos_pela_tela: u64) -> Option<Vec<(String, String)>> {
    let conta = SESSOES.lock().ok()?.remove(terminal)?;
    let eventos = ganchos::contadores(terminal);
    let campos: Vec<(String, String)> = vec![
        ("terminal".into(), terminal.to_owned()),
        ("provider".into(), provider.to_owned()),
        ("canal".into(), eventos.as_ref().map_or("nenhum", |e| e.canal).to_owned()),
        ("eventos".into(), eventos.as_ref().map_or(0, |e| e.eventos).to_string()),
        ("turnos_evento".into(), eventos.as_ref().map_or(0, |e| e.turnos).to_string()),
        ("turnos_tela".into(), turnos_pela_tela.to_string()),
        ("sem_entrada".into(), conta.sem_entrada.len().to_string()),
        ("trocas_de_conta".into(), conta.trocas_de_conta.to_string()),
        ("compactacoes".into(), eventos.as_ref().map_or(0, |e| e.compactacoes).to_string()),
    ];
    let referencias: Vec<(&str, &str)> = campos.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    registro::info("harness_resumo", &referencias);
    Some(campos)
}

/// Atalho para quem tem a sessão viva na mão.
pub(crate) fn registrar_da_sessao(sessao: &LiveSession) {
    let turnos = sessao.interaction.lock().map(|i| i.turnos_iniciados).unwrap_or(0);
    let Ok(meta) = sessao.meta.lock().map(|m| (m.id.clone(), m.provider.clone())) else { return };
    if let (id, Some(provider)) = meta { registrar(&id, &provider, turnos); }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_numeros_uma_vez_por_sessao() {
        let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
        assert!(registrar(&terminal, "claude", 0).is_none(), "sessão que não começou não registra");
        iniciar(&terminal);
        sem_entrada(&terminal, "campo de digitação com texto");
        sem_entrada(&terminal, "campo de digitação com texto");
        sem_entrada(&terminal, "agente trabalhando");
        troca_de_conta(&terminal);
        let campos = registrar(&terminal, "codex", 3).unwrap();
        let valor = |k: &str| campos.iter().find(|(c, _)| c == k).map(|(_, v)| v.as_str());
        assert_eq!((valor("sem_entrada"), valor("trocas_de_conta"), valor("turnos_tela"), valor("canal")), (Some("2"), Some("1"), Some("3"), Some("nenhum")));
        assert!(campos.iter().all(|(k, v)| k == "terminal" || k == "provider" || k == "canal" || v.parse::<u64>().is_ok()), "só números");
        assert!(registrar(&terminal, "codex", 3).is_none(), "uma vez só");
    }
}
