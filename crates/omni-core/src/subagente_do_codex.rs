//! Fork (conversa do Codex): rollout de **subagente** não é conversa de ninguém na tela.
//!
//! O Codex 0.164 grava cada subagente (`spawn_agent`) num rollout próprio, na mesma pasta e com o mesmo
//! `cwd` da sessão que o criou. A Conversa achava o rollout da sessão nova por "mesma pasta + criado
//! até 30 s depois de abrir": um subagente de outra aba do Codex, criado nesse intervalo, deixava dois
//! candidatos, nenhum era escolhido e a mensagem aparecia só no terminal ("Parte do histórico está
//! indisponível", visto no app em 23/10/2026). O `session_meta` do subagente diz que é subagente:
//! `thread_source: "subagent"`, `source: {"subagent": …}` e `parent_thread_id`.

use serde_json::Value;

/// `payload` do `session_meta` de um rollout é de subagente.
pub fn de_subagente(meta: &Value) -> bool {
    meta["thread_source"].as_str() == Some("subagent")
        || meta["source"].get("subagent").is_some()
        || meta["parent_thread_id"].as_str().is_some_and(|id| !id.is_empty())
}

/// Primeira linha de um rollout (`session_meta`) como valor, sem ler o resto do arquivo.
pub fn session_meta(caminho: &std::path::Path) -> Option<Value> {
    use std::io::BufRead;
    let arquivo = std::fs::File::open(caminho).ok()?;
    let linha = std::io::BufReader::new(arquivo).lines().next()?.ok()?;
    let valor: Value = serde_json::from_str(&linha).ok()?;
    (valor["type"] == "session_meta").then_some(valor)
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn reconhece_o_subagente_pelos_campos_do_codex_0_164() {
        // Campos como no rollout real (sem o resto do `session_meta`).
        let principal = json!({"id": "a", "source": "cli", "thread_source": "user", "originator": "codex-tui"});
        let subagente = json!({"id": "b", "thread_source": "subagent", "parent_thread_id": "a",
            "source": {"subagent": {"thread_spawn": {"parent_thread_id": "a", "depth": 1}}}});
        assert!(!de_subagente(&principal));
        assert!(de_subagente(&subagente));
        assert!(de_subagente(&json!({"source": {"subagent": {}}})), "versão sem thread_source");
        assert!(de_subagente(&json!({"parent_thread_id": "a"})));
        assert!(!de_subagente(&json!({"parent_thread_id": ""})));
        assert!(!de_subagente(&json!({})), "rollout antigo, sem os campos novos");
    }

    #[test]
    fn le_so_a_primeira_linha() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = dir.path().join("rollout-x.jsonl");
        std::fs::write(&rollout, "{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\"}}\n{\"type\":\"response_item\"}\n").unwrap();
        assert_eq!(session_meta(&rollout).unwrap()["payload"]["id"], "a");
        std::fs::write(&rollout, "{\"type\":\"response_item\"}\n").unwrap();
        assert!(session_meta(&rollout).is_none());
        assert!(session_meta(&dir.path().join("nao-existe.jsonl")).is_none());
    }
}
