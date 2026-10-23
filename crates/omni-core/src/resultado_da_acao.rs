//! Fork (troca-de-contexto): se a ação do agente deu certo, lido do resultado que a CLI grava logo
//! depois da chamada.
//!
//! - Claude: bloco `tool_result` (na linha `user` seguinte) com `tool_use_id` e `is_error`.
//! - Codex: `function_call_output` / `custom_tool_call_output` com `call_id` e o código de saída
//!   (`Exit code: N`, `Process exited with code N` ou `metadata.exit_code` no JSON).
//!
//! Só o veredito (`true`/`false`) entra na timeline, como `ok` do item da ação — nenhum texto da
//! saída. É o que deixa o cartão "onde parei" dizer "os testes falharam" sem ler o terminal. Mesma
//! regra de "deu certo?" do resumo da passagem de bastão (`passagem::resumir_saida`), aqui sem o
//! resumo da saída e lendo só o começo e o fim do texto (saída de comando pode ter milhares de linhas).

use serde_json::Value;

/// Linhas do começo e do fim da saída em que o código de saída é procurado.
const LINHAS_DO_INICIO: usize = 24;
const LINHAS_DO_FIM: usize = 6;

/// `(id da chamada, deu certo?)` de cada resultado de ação desta linha do transcript. Resultado sem
/// veredito (sem `is_error` nem código de saída) fica de fora: "não sei" não é "deu certo".
pub fn resultados(valor: &Value, provider: &str) -> Vec<(String, bool)> {
    match provider {
        "claude" if valor["type"] == "user" => valor["message"]["content"].as_array().into_iter().flatten()
            .filter(|bloco| bloco["type"] == "tool_result")
            .filter_map(|bloco| {
                let id = bloco["tool_use_id"].as_str()?;
                let erro = bloco["is_error"].as_bool();
                // Erro marcado pela CLI decide sozinho: não precisa ler a saída.
                let ok = if erro == Some(true) { Some(false) } else { deu_certo(&texto_do_bloco(&bloco["content"]), erro) };
                Some((id.to_owned(), ok?))
            })
            .collect(),
        "codex" if valor["type"] == "response_item"
            && matches!(valor["payload"]["type"].as_str(), Some("function_call_output" | "custom_tool_call_output")) => {
            let payload = &valor["payload"];
            let Some(id) = payload["call_id"].as_str() else { return Vec::new() };
            let saida = payload["output"].as_str().map(str::to_owned).unwrap_or_else(|| texto_do_bloco(&payload["output"]));
            // Fork: `output` em objeto (versões novas) pode trazer `success` direto.
            let ok = payload["output"]["success"].as_bool().or_else(|| deu_certo(&saida, None));
            ok.map(|ok| vec![(id.to_owned(), ok)]).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn texto_do_bloco(conteudo: &Value) -> String {
    if let Some(texto) = conteudo.as_str() { return texto.to_owned(); }
    if let Some(texto) = conteudo["output"].as_str() { return texto.to_owned(); }
    conteudo.as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n")
}

/// O veredito de uma saída: código de saída achado vale mais que o `is_error` falso (a CLI marca
/// sucesso em comando que saiu com 1 em algumas versões); sem código, vale o `is_error`.
pub fn deu_certo(saida: &str, erro: Option<bool>) -> Option<bool> {
    if erro == Some(true) { return Some(false); }
    let codigo = codigo_de_saida(saida);
    match (erro, codigo) {
        (_, Some(c)) => Some(c == 0),
        (Some(false), None) => Some(true),
        _ => None,
    }
}

fn codigo_de_saida(saida: &str) -> Option<i64> {
    let inicio = saida.trim_start();
    if inicio.starts_with('{') {
        if let Ok(valor) = serde_json::from_str::<Value>(inicio) {
            if let Some(c) = valor["metadata"]["exit_code"].as_i64().or_else(|| valor["exit_code"].as_i64()) { return Some(c); }
            if let Some(texto) = valor["output"].as_str() { return codigo_de_saida(texto); }
        }
    }
    let linhas: Vec<&str> = saida.lines().collect();
    let fim = linhas.len().saturating_sub(LINHAS_DO_FIM).max(LINHAS_DO_INICIO.min(linhas.len()));
    linhas[..LINHAS_DO_INICIO.min(linhas.len())].iter().chain(linhas[fim..].iter()).find_map(|linha| {
        let linha = linha.trim();
        ["Exit code:", "Process exited with code", "Exit code"].iter()
            .find_map(|prefixo| linha.strip_prefix(prefixo))
            .and_then(|resto| resto.trim().trim_end_matches('.').split_whitespace().next()?.parse::<i64>().ok())
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::feed::FeedCache;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn claude_le_is_error_e_codigo() {
        let linha = json!({"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "t1", "content": "ok", "is_error": false},
            {"type": "tool_result", "tool_use_id": "t2", "content": "Error: Exit code 1\nFAIL src/a.test.ts", "is_error": true},
            {"type": "tool_result", "tool_use_id": "t3", "content": [{"type": "text", "text": "sem veredito"}]},
            {"type": "tool_result", "tool_use_id": "t4", "content": "Exit code 2", "is_error": false},
        ]}});
        assert_eq!(resultados(&linha, "claude"), vec![("t1".into(), true), ("t2".into(), false), ("t4".into(), false)]);
    }

    #[test]
    fn codex_le_codigo_de_saida_em_texto_e_em_json() {
        let texto = json!({"type": "response_item", "payload": {"type": "function_call_output", "call_id": "c1",
            "output": "Chunk ID: 1\nWall time: 2.0 seconds\nProcess exited with code 0\nOriginal token count: 10\nOutput:\nok"}});
        assert_eq!(resultados(&texto, "codex"), vec![("c1".into(), true)]);
        let antigo = json!({"type": "response_item", "payload": {"type": "function_call_output", "call_id": "c2",
            "output": "{\"output\":\"2 failed\",\"metadata\":{\"exit_code\":1,\"duration_seconds\":3.1}}"}});
        assert_eq!(resultados(&antigo, "codex"), vec![("c2".into(), false)]);
        let exec = json!({"type": "response_item", "payload": {"type": "custom_tool_call_output", "call_id": "c3",
            "output": "Exit code: 0\nWall time: 1 seconds\nOutput:\nPASS"}});
        assert_eq!(resultados(&exec, "codex"), vec![("c3".into(), true)]);
        let sem = json!({"type": "response_item", "payload": {"type": "function_call_output", "call_id": "c4", "output": "texto qualquer"}});
        assert!(resultados(&sem, "codex").is_empty());
    }

    #[test]
    fn codigo_no_fim_de_saida_longa() {
        let mut saida = String::new();
        for i in 0..2000 { saida.push_str(&format!("linha {i}\n")); }
        saida.push_str("Exit code: 3\n");
        assert_eq!(deu_certo(&saida, None), Some(false));
        assert_eq!(deu_certo("nada aqui", None), None);
    }

    /// O feed incremental põe o `ok` no item da ação quando o resultado chega numa linha depois.
    #[test]
    fn feed_marca_a_acao_com_o_resultado() {
        let dir = tempfile::tempdir().unwrap();
        let caminho = dir.path().join("t.jsonl");
        let mut arquivo = std::fs::File::create(&caminho).unwrap();
        let chamada = json!({"type": "assistant", "uuid": "a1", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "npm test"}},
            {"type": "tool_use", "id": "t2", "name": "Read", "input": {"file_path": "/x/a.ts"}},
        ]}});
        writeln!(arquivo, "{chamada}").unwrap();
        let mut cache = FeedCache::default();
        let itens = cache.read(&caminho, "claude").unwrap();
        assert_eq!(itens[0].item.extras.ok, None);
        let resultado = json!({"type": "user", "uuid": "u1", "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "t1", "content": "1 failed", "is_error": true},
            {"type": "tool_result", "tool_use_id": "t2", "content": "conteúdo", "is_error": false},
        ]}});
        writeln!(arquivo, "{resultado}").unwrap();
        let itens = cache.read(&caminho, "claude").unwrap();
        assert_eq!(itens.len(), 2, "o resultado não vira item novo");
        assert_eq!(itens[0].item.extras.ok, Some(false));
        assert_eq!(itens[1].item.extras.ok, Some(true));
        let json = serde_json::to_value(&itens[0].item).unwrap();
        assert_eq!(json["ok"], json!(false));
        assert!(json.get("chamada").is_none(), "o id da chamada não vai para a timeline");
    }

    #[test]
    fn feed_do_codex_marca_o_shell() {
        let dir = tempfile::tempdir().unwrap();
        let caminho = dir.path().join("rollout.jsonl");
        let linhas = [
            json!({"type": "response_item", "timestamp": "2026-10-23T10:00:00Z", "payload": {"type": "function_call", "name": "exec_command", "call_id": "c1",
                "arguments": "{\"cmd\":\"cargo test\"}"}}),
            json!({"type": "response_item", "timestamp": "2026-10-23T10:00:05Z", "payload": {"type": "function_call_output", "call_id": "c1",
                "output": "Process exited with code 101\nOutput:\ntest result: FAILED"}}),
        ];
        std::fs::write(&caminho, linhas.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        let mut cache = FeedCache::default();
        let itens = cache.read(&caminho, "codex").unwrap();
        assert_eq!(itens.len(), 1);
        assert_eq!(itens[0].item.tool.as_deref(), Some("Shell"));
        assert_eq!(itens[0].item.extras.ok, Some(false));
    }
}
