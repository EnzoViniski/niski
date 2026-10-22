//! Fork: o que aparece quando o usuário digita `/` no campo da Conversa — comandos que o chat sabe
//! rodar, skills e comandos personalizados da conta e do projeto.
//!
//! Só leitura de disco (nome e descrição do frontmatter); nada daqui é executado. Como cada CLI
//! chama uma skill (conferido em 2026-10-22):
//! - Claude Code: skills e `commands/*.md` viram `/nome` (https://code.claude.com/docs/en/skills,
//!   https://code.claude.com/docs/en/slash-commands).
//! - Codex: skill é mencionada com `$nome` (https://developers.openai.com/codex/skills) — texto
//!   comum, vai pela rota de prompt.
//! - Antigravity e outras: sem forma explícita documentada para o chat; insere um pedido em texto.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ItemDaBarra {
    /// O que aparece na lista e se filtra (`compact`, `medza-code-review`, `frontend:revisar`).
    pub nome: String,
    pub descricao: String,
    /// `cli` (comando da própria CLI que o chat roda), `skill` ou `comando` (personalizado).
    pub tipo: &'static str,
    /// `cli`, `conta` ou `projeto`.
    pub origem: &'static str,
    /// Texto que entra no campo ao escolher.
    pub insere: String,
}

/// Limite por pasta: uma pasta de skills enorme não pode travar o campo de mensagem.
const MAXIMO_POR_PASTA: usize = 200;

/// Nome que pode virar `/nome` na CLI: sem espaço, controle ou sintaxe de shell.
pub fn nome_seguro(nome: &str) -> bool {
    !nome.is_empty() && nome.len() <= 80
        && nome.chars().all(|c| c.is_ascii_alphanumeric() || "._-:".contains(c))
        && !nome.starts_with(['.', ':', '-'])
}

/// Os comandos da lista fechada (`comandos_oficiais::montar`) que o chat consegue rodar.
fn da_cli(provider: &str) -> Vec<ItemDaBarra> {
    let item = |nome: &str, descricao: &str, insere: &str| ItemDaBarra {
        nome: nome.into(), descricao: descricao.into(), tipo: "cli", origem: "cli", insere: insere.into(),
    };
    match provider {
        "claude" => vec![
            item("compact", "Resume a conversa para liberar contexto", "/compact"),
            item("rewind", "Volta a conversa e o código para um ponto anterior", "/rewind"),
            item("branch", "Abre uma ramificação da conversa a partir daqui", "/branch"),
            item("fast", "Liga ou desliga o modo rápido: /fast on ou /fast off", "/fast "),
        ],
        "codex" => vec![item("compact", "Resume a conversa para liberar contexto", "/compact")],
        _ => vec![],
    }
}

/// `name` e `description` do frontmatter. Aceita valor numa linha e blocos `|`/`>` simples.
pub fn frontmatter(texto: &str) -> (Option<String>, Option<String>) {
    let mut linhas = texto.lines();
    if linhas.next().map(str::trim) != Some("---") { return (None, None); }
    let (mut nome, mut descricao) = (None, None);
    let mut bloco: Option<(&str, Vec<String>)> = None;
    let fechar = |bloco: &mut Option<(&str, Vec<String>)>, nome: &mut Option<String>, descricao: &mut Option<String>| {
        if let Some((chave, partes)) = bloco.take() {
            let valor = partes.join(" ").trim().to_string();
            if !valor.is_empty() { if chave == "name" { *nome = Some(valor) } else { *descricao = Some(valor) } }
        }
    };
    for linha in linhas {
        if linha.trim() == "---" { break; }
        if bloco.is_some() && (linha.starts_with(' ') || linha.starts_with('\t') || linha.trim().is_empty()) {
            if let Some((_, partes)) = bloco.as_mut() { partes.push(linha.trim().to_string()); }
            continue;
        }
        fechar(&mut bloco, &mut nome, &mut descricao);
        let Some((chave, valor)) = linha.split_once(':') else { continue };
        let chave = chave.trim();
        if chave != "name" && chave != "description" { continue; }
        let valor = valor.trim();
        if valor.starts_with('|') || valor.starts_with('>') { bloco = Some((if chave == "name" { "name" } else { "description" }, vec![])); continue; }
        let valor = valor.trim_matches(|c| c == '"' || c == '\'').to_string();
        if valor.is_empty() { continue; }
        if chave == "name" { nome = Some(valor) } else { descricao = Some(valor) }
    }
    fechar(&mut bloco, &mut nome, &mut descricao);
    (nome, descricao)
}

fn ler_inicio(caminho: &Path) -> String {
    use std::io::Read;
    let mut texto = String::new();
    // Só o começo: o frontmatter está no topo, e um SKILL.md enorme não precisa ser lido inteiro.
    if let Ok(arquivo) = std::fs::File::open(caminho) { let _ = arquivo.take(16 * 1024).read_to_string(&mut texto); }
    texto
}

/// `skills/<nome>/SKILL.md`. Segue links (skill compartilhada entre CLIs costuma ser link).
fn skills_em(pasta: &Path) -> Vec<(String, String)> {
    let Ok(entradas) = std::fs::read_dir(pasta) else { return vec![] };
    let mut saida = Vec::new();
    for entrada in entradas.flatten().take(MAXIMO_POR_PASTA) {
        let caminho = entrada.path();
        let arquivo = caminho.join("SKILL.md");
        if !arquivo.is_file() { continue; }
        let Some(pasta_nome) = entrada.file_name().to_str().map(str::to_owned) else { continue };
        let (_, descricao) = frontmatter(&ler_inicio(&arquivo));
        // A pasta é o identificador que a CLI usa; o `name` do frontmatter é só rótulo.
        saida.push((pasta_nome, descricao.unwrap_or_default()));
    }
    saida
}

/// `commands/**/*.md` do Claude: subpasta vira prefixo `pasta:nome`.
fn comandos_em(pasta: &Path) -> Vec<(String, String)> {
    fn varrer(pasta: &Path, prefixo: &str, saida: &mut Vec<(String, String)>, fundo: u8) {
        let Ok(entradas) = std::fs::read_dir(pasta) else { return };
        for entrada in entradas.flatten() {
            if saida.len() >= MAXIMO_POR_PASTA { return; }
            let caminho = entrada.path();
            let Some(nome) = entrada.file_name().to_str().map(str::to_owned) else { continue };
            if caminho.is_dir() {
                if fundo < 3 { varrer(&caminho, &format!("{prefixo}{nome}:"), saida, fundo + 1); }
            } else if let Some(base) = nome.strip_suffix(".md") {
                let (_, descricao) = frontmatter(&ler_inicio(&caminho));
                let descricao = descricao.unwrap_or_else(|| primeira_linha(&ler_inicio(&caminho)));
                saida.push((format!("{prefixo}{base}"), descricao));
            }
        }
    }
    let mut saida = Vec::new();
    varrer(pasta, "", &mut saida, 0);
    saida
}

/// Sem frontmatter, o comando do Claude usa a primeira linha do arquivo como descrição.
fn primeira_linha(texto: &str) -> String {
    let mut linhas = texto.lines();
    if texto.starts_with("---") { for l in linhas.by_ref().skip(1) { if l.trim() == "---" { break; } } }
    linhas.map(str::trim).find(|l| !l.is_empty()).unwrap_or("").trim_start_matches('#').trim().chars().take(160).collect()
}

/// Pastas de skills de cada CLI: (pasta, origem).
fn pastas_de_skills(provider: &str, config: Option<&Path>, cwd: Option<&Path>, casa: Option<&Path>) -> Vec<(PathBuf, &'static str)> {
    let mut pastas = Vec::new();
    match provider {
        "claude" => {
            if let Some(c) = cwd { pastas.push((c.join(".claude/skills"), "projeto")); }
            if let Some(c) = config { pastas.push((c.join("skills"), "conta")); }
        }
        "codex" => {
            if let Some(c) = cwd { pastas.push((c.join(".agents/skills"), "projeto")); }
            if let Some(c) = config { pastas.push((c.join("skills"), "conta")); }
            if let Some(h) = casa { pastas.push((h.join(".agents/skills"), "conta")); }
        }
        "antigravity" => {
            if let Some(c) = cwd { pastas.push((c.join(".agents/skills"), "projeto")); }
            // A conta fica em `~/.gemini/antigravity-cli`; as skills do usuário, em `~/.gemini/config/skills`.
            if let Some(c) = config { pastas.push((c.join("skills"), "conta")); if let Some(p) = c.parent() { pastas.push((p.join("config/skills"), "conta")); } }
        }
        _ => {}
    }
    pastas
}

fn como_inserir(provider: &str, nome: &str) -> String {
    match provider {
        "claude" => format!("/{nome} "),
        "codex" => format!("${nome} "),
        _ => format!("Use a skill {nome}: "),
    }
}

/// Tudo o que a barra mostra para uma conversa. Projeto antes da conta; nome repetido aparece uma vez.
pub fn listar(provider: &str, config: Option<&Path>, cwd: Option<&Path>, casa: Option<&Path>) -> Vec<ItemDaBarra> {
    let mut itens = da_cli(provider);
    let mut vistos: std::collections::HashSet<String> = itens.iter().map(|i| i.nome.clone()).collect();
    let mut juntar = |nome: String, descricao: String, tipo: &'static str, origem: &'static str, insere: String, itens: &mut Vec<ItemDaBarra>| {
        if !nome_seguro(&nome) || !vistos.insert(nome.clone()) { return; }
        itens.push(ItemDaBarra { nome, descricao: descricao.chars().take(300).collect(), tipo, origem, insere });
    };
    for (pasta, origem) in pastas_de_skills(provider, config, cwd, casa) {
        let mut achadas = skills_em(&pasta);
        achadas.sort();
        for (nome, descricao) in achadas { let insere = como_inserir(provider, &nome); juntar(nome, descricao, "skill", origem, insere, &mut itens); }
    }
    if provider == "claude" {
        let pastas = [cwd.map(|c| (c.join(".claude/commands"), "projeto")), config.map(|c| (c.join("commands"), "conta"))];
        for (pasta, origem) in pastas.into_iter().flatten() {
            let mut achados = comandos_em(&pasta);
            achados.sort();
            for (nome, descricao) in achados { let insere = format!("/{nome} "); juntar(nome, descricao, "comando", origem, insere, &mut itens); }
        }
    }
    itens
}

/// Para a rota que digita `/nome resto` na CLI: o nome precisa ser uma skill ou comando listado
/// agora para esta conversa (Claude). Devolve o texto pronto ou o motivo da recusa.
pub fn texto_para_cli(itens: &[ItemDaBarra], texto: &str) -> Result<String, &'static str> {
    let texto = texto.trim();
    if texto.len() > 16_000 || texto.chars().any(|c| c.is_control() && c != '\n' && c != '\t') { return Err("Texto inválido"); }
    let Some(resto) = texto.strip_prefix('/') else { return Err("Não é um comando") };
    let (nome, argumento) = resto.split_once(char::is_whitespace).map_or((resto, ""), |(n, a)| (n, a.trim()));
    if !nome_seguro(nome) { return Err("Nome de comando inválido"); }
    let conhecido = itens.iter().any(|i| i.tipo != "cli" && i.nome == nome && i.insere.starts_with('/'));
    if !conhecido { return Err("Esse comando não está entre as skills e comandos desta conversa"); }
    Ok(if argumento.is_empty() { format!("/{nome}") } else { format!("/{nome} {argumento}") })
}

#[cfg(test)]
#[path = "comandos_da_barra_testes.rs"]
mod testes;
