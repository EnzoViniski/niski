//! Lista fechada do que os controles da Conversa podem digitar na CLI.
//! Conferido em 2026-10-20:
//! - https://code.claude.com/docs/en/commands
//! - https://code.claude.com/docs/en/fast-mode (Opus 5.5, 5 e 4.8)
//! - https://code.claude.com/docs/en/cli-reference (`--permission-mode`)
//! - https://developers.openai.com/codex/cli/slash-commands
//!   (redireciona para https://learn.chatgpt.com/docs/developer-commands?surface=cli).
//! `/model` e `/config` ficam deliberadamente fora: ajustes usam `--resume`.

/// Valores que vão para a CLI: sem espaços, controles, escapes ou sintaxe de shell.
pub fn valor_seguro(valor: &str) -> bool {
    !valor.is_empty() && valor.len() <= 64
        && valor.chars().all(|c| c.is_ascii_alphanumeric() || "._-[]".contains(c))
}

pub fn modo_valido(modo: &str) -> bool {
    matches!(modo, "default" | "acceptEdits" | "plan" | "auto")
}

/// Não aceitar apelidos como `opus`: podem resolver para um modelo incompatível.
pub fn suporta_fast(modelo: &str) -> bool {
    ["claude-opus-5-5", "claude-opus-5", "claude-opus-4-8"].iter()
        .any(|id| modelo == *id || modelo.strip_prefix(id).is_some_and(|s| s.starts_with('-') && s[1..].len() == 8 && s[1..].bytes().all(|c| c.is_ascii_digit())))
}

/// O id vem da UI, nunca é usado diretamente como texto de comando.
pub fn montar(provider: &str, comando: &str, argumento: Option<&str>) -> Result<String, &'static str> {
    if argumento.is_some_and(|a| !valor_seguro(a)) { return Err("Argumento inválido"); }
    match (provider, comando, argumento) {
        ("claude", "fast", Some(a @ ("on" | "off"))) => Ok(format!("/fast {a}")),
        ("claude", "branch", nome) => Ok(nome.map_or_else(|| "/branch".into(), |n| format!("/branch {n}"))),
        ("claude", "rewind", None) => Ok("/rewind".into()),
        ("claude" | "codex", "compact", None) => Ok("/compact".into()),
        ("claude", "plano", Some(a)) => Ok(format!("/plan {a}")),
        _ => Err("Comando ou argumento não permitido para este agente"),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test] fn tabela_por_cli() {
        for (cli, id, arg, texto) in [
            ("claude", "fast", Some("on"), "/fast on"),
            ("claude", "fast", Some("off"), "/fast off"),
            ("claude", "branch", None, "/branch"),
            ("claude", "branch", Some("alternativa"), "/branch alternativa"),
            ("claude", "rewind", None, "/rewind"),
            ("claude", "compact", None, "/compact"),
            ("codex", "compact", None, "/compact"),
            ("claude", "plano", Some("revisar"), "/plan revisar"),
        ] { assert_eq!(montar(cli,id,arg).unwrap(), texto); }
    }
    #[test] fn lista_fechada_e_argumentos() {
        for cli in ["claude", "codex", "antigravity", "cursor", ""] {
            for comando in ["model", "/model", "config", "exit", "usage", "", "compact\r/model"] {
                assert!(montar(cli,comando,None).is_err());
            }
        }
        for a in ["", "on\r", "on\n", "on;exit", "$(id)", "a b", "\x1b[A", "/model"] {
            assert!(montar("claude","branch",Some(a)).is_err());
        }
        for (cli, id, arg) in [("codex","fast",Some("on")), ("codex","branch",None),
            ("claude","fast",None), ("claude","fast",Some("yes")), ("claude","rewind",Some("a")),
            ("codex","compact",Some("a"))] { assert!(montar(cli,id,arg).is_err()); }
    }
    #[test] fn fast_so_nos_modelos_documentados() {
        for m in ["claude-opus-5-5", "claude-opus-5", "claude-opus-4-8-20260528"] { assert!(suporta_fast(m)); }
        for m in ["opus", "claude-opus-4-7", "claude-sonnet-5", "claude-opus-5-9", ""] { assert!(!suporta_fast(m)); }
    }
}
