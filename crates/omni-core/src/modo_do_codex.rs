//! Fork (modo do Codex na Conversa): Perguntar, Sem perguntar, Acesso total e Somente leitura.
//!
//! O Codex não troca aprovação/sandbox por tecla como o `shift+tab` do Claude; o jeito estável é a
//! linha de comando. Opções conferidas no `codex --help` e `codex resume --help` (0.164.0, 23/10/2026):
//! `-a, --ask-for-approval <on-request|never>` e `-s, --sandbox <read-only|workspace-write|danger-full-access>`,
//! aceitas também depois de `resume <id>`. O nome de cada modo é o mesmo que o selo já mostra a partir do
//! `turn_context` do rollout (`plano_do_agente::modo_codex`), para a tela confirmar o que foi pedido.

/// Modos que a Conversa oferece, na ordem do seletor.
pub const MODOS: [&str; 4] = ["default", "semPerguntar", "acessoTotal", "somenteLeitura"];

pub fn valido(modo: &str) -> bool { MODOS.contains(&modo) }

/// As opções da linha de cada modo.
pub fn opcoes(modo: &str) -> Option<&'static str> {
    Some(match modo {
        "default" => "-a on-request -s workspace-write",
        "semPerguntar" => "-a never -s workspace-write",
        "acessoTotal" => "-a never -s danger-full-access",
        "somenteLeitura" => "-a on-request -s read-only",
        _ => return None,
    })
}

/// Opções de permissão que já podem estar na linha (com o valor separado ou com `=`).
const COM_VALOR: [&str; 4] = ["-a", "--ask-for-approval", "-s", "--sandbox"];
const SOZINHAS: [&str; 3] = ["--full-auto", "--dangerously-bypass-approvals-and-sandbox", "--approve-for-me"];

/// Pedaços da linha separados por espaço, sem quebrar o que está entre aspas.
fn pedacos(linha: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    let mut aspas: Option<char> = None;
    for c in linha.chars() {
        match (aspas, c) {
            (Some(a), c) if c == a => { aspas = None; atual.push(c); }
            (Some(_), c) => atual.push(c),
            (None, '"' | '\'') => { aspas = Some(c); atual.push(c); }
            (None, c) if c.is_whitespace() => { if !atual.is_empty() { saida.push(std::mem::take(&mut atual)); } }
            (None, c) => atual.push(c),
        }
    }
    if !atual.is_empty() { saida.push(atual); }
    saida
}

/// A linha com o modo pedido no lugar do que houver de aprovação/sandbox (o resto fica como estava).
/// `-c approval_policy=…`/`-c sandbox_mode=…` também saem: valeriam por cima do `-a`/`-s`.
/// `None` para modo desconhecido ou linha com aspas sem fechar.
pub fn com_modo(linha: &str, modo: &str) -> Option<String> {
    let novas = opcoes(modo)?;
    if linha.chars().filter(|c| *c == '"').count() % 2 == 1 || linha.chars().filter(|c| *c == '\'').count() % 2 == 1 { return None; }
    let todos = pedacos(linha);
    let mut mantidos: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < todos.len() {
        let p = todos[i].as_str();
        let config = |valor: &str| ["approval_policy=", "sandbox_mode="].iter().any(|k| valor.trim_matches(['"', '\'']).starts_with(k));
        if COM_VALOR.contains(&p) || (p == "-c" && todos.get(i + 1).is_some_and(|v| config(v))) { i += 2; continue; }
        if SOZINHAS.contains(&p) || COM_VALOR.iter().any(|f| f.starts_with("--") && p.starts_with(&format!("{f}="))) { i += 1; continue; }
        mantidos.push(p);
        i += 1;
    }
    if mantidos.is_empty() { return None; }
    Some(format!("{} {novas}", mantidos.join(" ")))
}

/// O modo que a linha abriu: o `-a`/`-s` dela (ou o atalho de acesso total). Sem as opções, `None`
/// (vale o `config.toml`, que a tela lê do `turn_context` depois do primeiro turno).
pub fn da_linha(linha: &str) -> Option<&'static str> {
    let todos = pedacos(linha);
    if todos.iter().any(|p| p == "--dangerously-bypass-approvals-and-sandbox") { return Some("acessoTotal"); }
    let valor = |curta: &str, longa: &str| -> Option<String> {
        todos.iter().enumerate().find_map(|(i, p)| {
            if p == curta || p == longa { return todos.get(i + 1).cloned(); }
            p.strip_prefix(&format!("{longa}=")).map(str::to_owned)
        })
    };
    match (valor("-a", "--ask-for-approval")?.as_str(), valor("-s", "--sandbox")?.as_str()) {
        ("never", "workspace-write") => Some("semPerguntar"),
        ("never", "danger-full-access") => Some("acessoTotal"),
        ("on-request", "read-only") => Some("somenteLeitura"),
        ("on-request", "workspace-write") => Some("default"),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_o_modo_da_linha() {
        for modo in MODOS { assert_eq!(da_linha(&com_modo("codex resume x -m y", modo).unwrap()), Some(modo)); }
        assert_eq!(da_linha("codex -m x"), None, "sem opção: vale o config.toml");
        assert_eq!(da_linha("codex --dangerously-bypass-approvals-and-sandbox"), Some("acessoTotal"));
        assert_eq!(da_linha("codex --ask-for-approval=never --sandbox=workspace-write"), Some("semPerguntar"));
        assert_eq!(da_linha("codex -a never"), None, "só metade");
    }

    #[test]
    fn troca_so_aprovacao_e_sandbox() {
        assert_eq!(com_modo("codex -m gpt-6.1-sol -c model_reasoning_effort=xhigh", "semPerguntar").as_deref(),
            Some("codex -m gpt-6.1-sol -c model_reasoning_effort=xhigh -a never -s workspace-write"));
        assert_eq!(com_modo("codex -m x -a never -s workspace-write", "somenteLeitura").as_deref(),
            Some("codex -m x -a on-request -s read-only"));
        assert_eq!(com_modo("codex resume 01a1-x --full-auto -m x", "acessoTotal").as_deref(),
            Some("codex resume 01a1-x -m x -a never -s danger-full-access"));
        assert_eq!(com_modo("codex --dangerously-bypass-approvals-and-sandbox --sandbox=read-only --ask-for-approval on-request", "default").as_deref(),
            Some("codex -a on-request -s workspace-write"));
        assert_eq!(com_modo("codex -c approval_policy=\"never\" -c 'sandbox_mode=\"read-only\"' -m x", "default").as_deref(),
            Some("codex -m x -a on-request -s workspace-write"));
    }

    #[test]
    fn nao_mexe_no_que_esta_entre_aspas_e_recusa_o_desconhecido() {
        assert_eq!(com_modo("codex \"use -a never e -s read-only\"", "default").as_deref(),
            Some("codex \"use -a never e -s read-only\" -a on-request -s workspace-write"));
        assert!(com_modo("codex", "auto").is_none(), "modo do Claude não é do Codex");
        assert!(com_modo("codex \"sem fechar", "default").is_none());
        assert!(com_modo("", "default").is_none());
        assert!(MODOS.iter().all(|m| valido(m) && opcoes(m).is_some()));
    }
}
