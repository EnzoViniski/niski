//! Fork (perguntas na Conversa): **todas** as opções do diálogo de permissão da CLI, não só Permitir/Negar.
//!
//! O cartão mostrava dois botões, o "sim de uma vez" e o "não" que o `interaction::approval` escolhe. O
//! Enzo (23/10/2026): "é uma pergunta que eu tenho que escolher entre três opções e ele só dá a opção de
//! permitir" — a terceira (permitir nesta sessão, sempre permitir este comando no agy…) só existia no
//! terminal. Aqui o mesmo diálogo vira a lista inteira, com a tecla de cada opção; o cartão mostra as
//! que não são o Permitir/Negar como botões a mais, com o texto da própria CLI.
//!
//! A resposta passa pela mesma fila e pela mesma versão da tela da aprovação (`ActionKind::OpcaoDaPermissao`).

use crate::interaction;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct OpcaoDaPermissao {
    pub numero: u8,
    pub rotulo: String,
    /// `permitir` (o sim de uma vez, o botão Permitir), `negar` (o botão Negar) ou `outra`.
    pub papel: &'static str,
    /// Setas até a opção (num write) e a tecla que confirma (noutro). Claude: o número escolhe.
    #[serde(skip)]
    pub setas: Option<String>,
    #[serde(skip)]
    pub fim: String,
}

const MARCAS: [char; 3] = ['❯', '›', '>'];

/// `(marcada, número, rótulo)` de uma linha de opção; o agy da confiança na pasta não numera.
fn opcao(linha: &str, posicao: u8) -> Option<(bool, u8, String)> {
    let marcada = linha.starts_with(MARCAS);
    let resto = linha.trim_start_matches(MARCAS).trim();
    match resto.split_once(". ") {
        Some((n, rotulo)) if n.parse::<u8>().is_ok() => Some((marcada, n.parse().ok()?, rotulo.trim().to_owned())),
        _ if resto.starts_with("Yes") || resto.starts_with("No") => Some((marcada, posicao, resto.to_owned())),
        _ => None,
    }
}

/// O mesmo diálogo que o `interaction::approval` reconhece, com todas as opções. Vazio sem diálogo.
pub fn opcoes(texto: &str, provider: &str) -> Vec<OpcaoDaPermissao> {
    if interaction::approval(texto, provider).is_none() { return Vec::new(); }
    let linhas: Vec<String> = texto.lines()
        .map(|l| l.trim().trim_start_matches('│').trim_end_matches('│').trim().to_owned()).collect();
    let pergunta = |l: &str| match provider {
        "claude" => l.starts_with("Do you want to") && l.ends_with('?'),
        "codex" => (l.starts_with("Would you like to") || l.starts_with("Do you want to approve")) && l.ends_with('?'),
        _ => l.ends_with('?') && opcao(l, 1).is_none(),
    };
    let Some(inicio) = linhas.iter().rposition(|l| pergunta(l)) else { return Vec::new() };
    let mut achadas: Vec<(bool, u8, String)> = Vec::new();
    for linha in linhas[inicio + 1..].iter().take(14) {
        match opcao(linha, achadas.len() as u8 + 1) {
            Some(o) => achadas.push(o),
            // Explicação entre a pergunta e as opções (a confiança na pasta do agy) ou linha vazia.
            None if achadas.is_empty() || linha.is_empty() => continue,
            None => break,
        }
    }
    if achadas.len() < 2 || achadas.iter().enumerate().any(|(i, o)| o.1 as usize != i + 1) { return Vec::new(); }
    let marcada = achadas.iter().position(|o| o.0);
    let uma_vez = |r: &str| { let r = r.to_lowercase(); !r.contains("always") && !r.contains("persist") && !r.contains(" tell ") };
    let permitir = match provider {
        "claude" => achadas.iter().position(|o| o.2 == "Yes"),
        "codex" => Some(0),
        _ => achadas.iter().position(|o| o.2.starts_with("Yes") && uma_vez(&o.2)),
    };
    let negar = match provider {
        "claude" | "codex" => achadas.iter().position(|o| o.2 == "No" || o.2.starts_with("No,") || o.2.starts_with("No ")),
        _ => achadas.iter().position(|o| o.2.starts_with("No") && uma_vez(&o.2)),
    };
    achadas.iter().enumerate().map(|(i, (_, numero, rotulo))| {
        let (setas, fim) = match (provider, marcada) {
            ("claude", _) | (_, None) => (None, format!("{numero}\r")),
            (_, Some(m)) => {
                let seta = if i >= m { "\x1b[B" } else { "\x1b[A" };
                ((i != m).then(|| seta.repeat(i.abs_diff(m))), "\r".to_owned())
            }
        };
        let papel = if Some(i) == permitir { "permitir" } else if Some(i) == negar { "negar" } else { "outra" };
        OpcaoDaPermissao { numero: *numero, rotulo: rotulo.clone(), papel, setas, fim }
    }).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn claude_mostra_a_terceira_opcao_com_o_numero_dela() {
        let tela = "Edit file\n────────────\nsrc/a.ts\n\nDo you want to make this edit to a.ts?\n❯ 1. Yes\n  2. Yes, allow all edits during this session (shift+tab)\n  3. No, and tell Claude what to do differently (esc)\n";
        let opcoes = opcoes(tela, "claude");
        let resumo: Vec<_> = opcoes.iter().map(|o| (o.numero, o.papel, o.setas.clone(), o.fim.as_str())).collect();
        assert_eq!(resumo, [(1, "permitir", None, "1\r"), (2, "outra", None, "2\r"), (3, "negar", None, "3\r")]);
        assert_eq!(opcoes[1].rotulo, "Yes, allow all edits during this session (shift+tab)");
    }

    #[test]
    fn codex_e_agy_andam_pelas_setas_a_partir_da_marcada() {
        let codex = "Would you like to run the following command?\n\n  $ npm test\n\n› 1. Yes, proceed (y)\n  2. Yes, and don't ask again for this command (a)\n  3. No, and tell Codex what to do differently (esc)\n";
        let o = opcoes(codex, "codex");
        assert_eq!(o.iter().map(|o| o.papel).collect::<Vec<_>>(), ["permitir", "outra", "negar"]);
        assert_eq!((o[1].setas.as_deref(), o[1].fim.as_str()), (Some("\x1b[B"), "\r"));
        let agy = "Run command\n──────────\nnpm test\n\nRun this command?\n  1. Yes, and always allow for commands that start with 'npm' (Persist to settings.json)\n> 2. Yes, run command\n  3. No, and always deny 'npm test' in this conversation\n  4. No, deny\n";
        let o = opcoes(agy, "antigravity");
        assert_eq!(o.iter().map(|o| o.papel).collect::<Vec<_>>(), ["outra", "permitir", "outra", "negar"]);
        assert_eq!(o[0].setas.as_deref(), Some("\x1b[A"), "sempre permitir fica a uma seta para cima");
        assert_eq!(o[3].setas.as_deref(), Some("\x1b[B\x1b[B"));
    }

    #[test]
    fn sem_dialogo_de_permissao_nada() {
        assert!(opcoes("Qual opção?\n❯ 1. Criar\n  2. Corrigir\n", "claude").is_empty());
        assert!(opcoes("", "codex").is_empty());
    }
}
