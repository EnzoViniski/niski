//! Os modelos do Claude que dá para escolher numa conversa ou num agente.
//!
//! Antes eram só os quatro apelidos (`fable`, `opus`, `sonnet`, `haiku`), que o Claude Code resolve
//! para o modelo mais novo de cada linha — não havia como pedir o Opus 4.8, por exemplo. Agora os
//! apelidos continuam no topo e os IDs fixos vêm depois, como no seletor do app do Claude.
//!
//! IDs e esforços conferidos em 2026-10-19 na documentação oficial:
//! - https://platform.claude.com/docs/en/about-claude/models/overview (atuais e "legacy")
//! - https://platform.claude.com/docs/en/build-with-claude/effort (esforço por modelo)
//!
//! Ao sair modelo novo ou algum ser aposentado, mexer só aqui (e conferir na documentação).

/// Os cinco níveis; quem tem menos usa um recorte desta lista.
const TODOS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
/// Opus 4.6 e Sonnet 4.6: aceitam `max`, mas não `xhigh`.
const SEM_XHIGH: &[&str] = &["low", "medium", "high", "max"];
/// Opus 4.5: o esforço vai até `high`.
const ATE_HIGH: &[&str] = &["low", "medium", "high"];
/// Haiku 4.5: não aceita o parâmetro de esforço.
const SEM_ESFORCO: &[&str] = &[];

/// `(id para o --model, rótulo, esforços aceitos)`, na ordem do seletor.
pub const MODELOS: &[(&str, &str, &[&str])] = &[
    // Apelidos: o Claude Code escolhe o mais novo da linha (e acompanha os lançamentos).
    ("fable", "Fable (mais recente)", TODOS),
    ("opus", "Opus (mais recente)", TODOS),
    ("sonnet", "Sonnet (mais recente)", TODOS),
    ("haiku", "Haiku (mais recente)", TODOS),
    // Atuais.
    ("claude-fable-5-1", "Fable 5.1", TODOS),
    ("claude-opus-5-5", "Opus 5.5", TODOS),
    ("claude-sonnet-5-5", "Sonnet 5.5", TODOS),
    ("claude-haiku-5-5", "Haiku 5.5", TODOS),
    // Anteriores, ainda disponíveis.
    ("claude-fable-5", "Fable 5", TODOS),
    ("claude-opus-5", "Opus 5", TODOS),
    ("claude-opus-4-8", "Opus 4.8", TODOS),
    ("claude-opus-4-7", "Opus 4.7", TODOS),
    ("claude-opus-4-6", "Opus 4.6", SEM_XHIGH),
    ("claude-opus-4-5", "Opus 4.5", ATE_HIGH),
    ("claude-sonnet-5", "Sonnet 5", TODOS),
    ("claude-sonnet-4-6", "Sonnet 4.6", SEM_XHIGH),
    ("claude-haiku-4-5", "Haiku 4.5", SEM_ESFORCO),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_sao_seguros_para_o_shell_e_nao_repetem() {
        let mut vistos = std::collections::HashSet::new();
        for (id, rotulo, esforcos) in MODELOS {
            assert!(id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{id}");
            assert!(vistos.insert(*id), "repetido: {id}");
            assert!(!rotulo.is_empty());
            assert!(esforcos.iter().all(|e| TODOS.contains(e)));
        }
        assert!(MODELOS.iter().any(|(id, ..)| *id == "claude-opus-4-8"));
    }
}
