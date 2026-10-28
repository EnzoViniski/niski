//! O que o agente mudou num arquivo, como um mini-diff para a conversa.
//!
//! Antes a ação de editar mostrava só o caminho ("Editou src/a.ts"); no terminal do PC e nos apps
//! do Claude Code/Codex dá para ver as linhas. As fontes, todas do próprio transcript:
//!
//! - **Claude** `Edit`/`MultiEdit`: `old_string` → `new_string` (diff de linhas calculado aqui);
//!   `Write`: o `content` inteiro, como linhas novas.
//! - **Codex** `apply_patch` (o patch em texto, `*** Update File:` / `*** Add File:` …), chamado
//!   direto ou de dentro do `exec`; e o item `FileChange` (`unified_diff`) que o rollout grava.
//!
//! A timeline é consultada a cada 1,5 s, então o diff vai **cortado**: no máximo
//! [`LINHAS_MAX`] linhas por ação, cada uma com até [`LARGURA_MAX`] caracteres. As contagens
//! (+/−) são do diff inteiro.

use serde::Serialize;

/// Linhas de diff por ação (somando os arquivos). O resto vira `omitidas`.
pub const LINHAS_MAX: usize = 60;
/// Caracteres por linha; linha maior termina em `…`.
pub const LARGURA_MAX: usize = 240;
/// Acima disto (linhas antigas × novas) o diff de linhas não é calculado: vira "tudo saiu, tudo
/// entrou", que é honesto e não custa nada.
const CELULAS_MAX: usize = 40_000;
/// Linhas iguais mostradas antes e depois de cada mudança.
const CONTEXTO: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDeMudanca {
    /// Trecho trocado num arquivo que já existia.
    Editado,
    /// Arquivo criado pelo patch.
    Novo,
    /// Arquivo apagado pelo patch.
    Apagado,
    /// Arquivo escrito por inteiro (`Write` do Claude): pode ser novo ou sobrescrito.
    Escrito,
}

/// O diff de um arquivo. Cada linha começa com `+`, `-`, ` ` (contexto) ou `@` (início de trecho,
/// o resto da linha é o rótulo do trecho, pode ser vazio).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffDeArquivo {
    pub arquivo: String,
    pub tipo: TipoDeMudanca,
    pub adicionadas: usize,
    pub removidas: usize,
    pub linhas: Vec<String>,
    /// Linhas do diff que ficaram de fora pelo limite.
    #[serde(skip_serializing_if = "is_zero")]
    pub omitidas: usize,
}

fn is_zero(n: &usize) -> bool { *n == 0 }

fn cortar_linha(linha: &str) -> String {
    if linha.chars().count() <= LARGURA_MAX { return linha.to_owned(); }
    let mut saida: String = linha.chars().take(LARGURA_MAX).collect();
    saida.push('…');
    saida
}

impl DiffDeArquivo {
    fn novo(arquivo: &str, tipo: TipoDeMudanca) -> Self {
        Self { arquivo: arquivo.to_owned(), tipo, adicionadas: 0, removidas: 0, linhas: Vec::new(), omitidas: 0 }
    }

    /// Acrescenta uma linha já com o marcador; conta `+`/`-` mesmo quando ela não cabe mais.
    fn linha(&mut self, marca: char, texto: &str) {
        match marca { '+' => self.adicionadas += 1, '-' => self.removidas += 1, _ => {} }
        self.linhas.push(cortar_linha(&format!("{marca}{texto}")));
    }
}

/// Corta os diffs de uma ação para caberem juntos em [`LINHAS_MAX`] linhas.
pub fn limitar(diffs: &mut [DiffDeArquivo]) {
    let mut sobra = LINHAS_MAX;
    for diff in diffs {
        if diff.linhas.len() > sobra {
            diff.omitidas += diff.linhas.len() - sobra;
            diff.linhas.truncate(sobra);
        }
        sobra -= diff.linhas.len();
    }
}

/// Diff de linhas: `(marca, linha)` com `' '`, `'-'` ou `'+'`, na ordem do arquivo.
fn diff_de_linhas<'a>(antes: &[&'a str], depois: &[&'a str]) -> Vec<(char, &'a str)> {
    // Prefixo e sufixo iguais saem antes: o diff de verdade costuma ser pequeno no meio.
    let prefixo = antes.iter().zip(depois).take_while(|(a, b)| a == b).count();
    let sufixo = antes[prefixo..].iter().rev().zip(depois[prefixo..].iter().rev()).take_while(|(a, b)| a == b).count();
    let a = &antes[prefixo..antes.len() - sufixo];
    let b = &depois[prefixo..depois.len() - sufixo];
    let mut meio: Vec<(char, &str)> = Vec::new();
    if a.len().saturating_mul(b.len()) > CELULAS_MAX {
        meio.extend(a.iter().map(|l| ('-', *l)));
        meio.extend(b.iter().map(|l| ('+', *l)));
    } else {
        // LCS clássico: tabela (n+1)×(m+1) de quanto ainda casa a partir de (i, j).
        let (n, m) = (a.len(), b.len());
        let mut tabela = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                tabela[i * (m + 1) + j] = if a[i] == b[j] { tabela[(i + 1) * (m + 1) + j + 1] + 1 }
                    else { tabela[(i + 1) * (m + 1) + j].max(tabela[i * (m + 1) + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            // Empate: o que sai vem antes do que entra, como no `git diff`.
            if i < n && j < m && a[i] == b[j] { meio.push((' ', a[i])); i += 1; j += 1; }
            else if i < n && (j == m || tabela[(i + 1) * (m + 1) + j] >= tabela[i * (m + 1) + j + 1]) { meio.push(('-', a[i])); i += 1; }
            else { meio.push(('+', b[j])); j += 1; }
        }
    }
    let mut saida: Vec<(char, &str)> = antes[..prefixo].iter().map(|l| (' ', *l)).collect();
    saida.extend(meio);
    saida.extend(antes[antes.len() - sufixo..].iter().map(|l| (' ', *l)));
    saida
}

/// Só as linhas iguais perto de uma mudança ficam; um vão maior vira um marcador `@` vazio.
fn com_contexto(ops: Vec<(char, &str)>, diff: &mut DiffDeArquivo) {
    let mudou: Vec<usize> = ops.iter().enumerate().filter(|(_, (m, _))| *m != ' ').map(|(i, _)| i).collect();
    let perto = |i: usize| mudou.iter().any(|&k| k.abs_diff(i) <= CONTEXTO);
    let mut pulou = false;
    for (i, (marca, texto)) in ops.iter().enumerate() {
        if *marca == ' ' && !perto(i) { pulou = true; continue; }
        if pulou && !diff.linhas.is_empty() { diff.linhas.push("@".into()); }
        pulou = false;
        diff.linha(*marca, texto);
    }
}

/// `Edit` do Claude: o trecho antigo pelo novo.
pub fn de_edicao(arquivo: &str, antigo: &str, novo: &str) -> DiffDeArquivo {
    de_edicoes(arquivo, &[(antigo, novo)])
}

/// `MultiEdit` do Claude: cada edição é um trecho, separado por `@`.
pub fn de_edicoes(arquivo: &str, edicoes: &[(&str, &str)]) -> DiffDeArquivo {
    let mut diff = DiffDeArquivo::novo(arquivo, TipoDeMudanca::Editado);
    for (n, (antigo, novo)) in edicoes.iter().enumerate() {
        if n > 0 { diff.linhas.push("@".into()); }
        let antes: Vec<&str> = antigo.lines().collect();
        let depois: Vec<&str> = novo.lines().collect();
        com_contexto(diff_de_linhas(&antes, &depois), &mut diff);
    }
    diff
}

/// `Write` do Claude: o conteúdo inteiro como linhas novas. Não lê o resto: arquivo de milhares de
/// linhas só conta.
pub fn de_escrita(arquivo: &str, conteudo: &str) -> DiffDeArquivo {
    let mut diff = DiffDeArquivo::novo(arquivo, TipoDeMudanca::Escrito);
    for (n, linha) in conteudo.lines().enumerate() {
        if n < LINHAS_MAX { diff.linha('+', linha); } else { diff.adicionadas += 1; diff.omitidas += 1; }
    }
    diff
}

/// Patch do `apply_patch` do Codex, um diff por arquivo.
pub fn do_patch(patch: &str) -> Vec<DiffDeArquivo> {
    let mut diffs: Vec<DiffDeArquivo> = Vec::new();
    for linha in patch.lines() {
        let cabecalho = [("*** Update File: ", TipoDeMudanca::Editado), ("*** Add File: ", TipoDeMudanca::Novo), ("*** Delete File: ", TipoDeMudanca::Apagado)]
            .into_iter().find_map(|(prefixo, tipo)| linha.strip_prefix(prefixo).map(|c| (c.trim(), tipo)));
        if let Some((arquivo, tipo)) = cabecalho { diffs.push(DiffDeArquivo::novo(arquivo, tipo)); continue; }
        if let Some(destino) = linha.strip_prefix("*** Move to: ") {
            if let Some(atual) = diffs.last_mut() { atual.arquivo = format!("{} → {}", atual.arquivo, destino.trim()); }
            continue;
        }
        if linha.starts_with("*** ") { continue; }
        let Some(atual) = diffs.last_mut() else { continue };
        if atual.tipo == TipoDeMudanca::Apagado { continue; }
        if let Some(rotulo) = linha.strip_prefix("@@") {
            atual.linhas.push(format!("@{}", rotulo.trim()));
            continue;
        }
        let mut letras = linha.chars();
        match letras.next() {
            Some(marca @ ('+' | '-' | ' ')) => atual.linha(marca, letras.as_str()),
            // Linha de contexto vazia às vezes perde o espaço do começo.
            None if atual.tipo == TipoDeMudanca::Editado => atual.linha(' ', ""),
            _ => {}
        }
    }
    // Primeiro trecho sem rótulo (`@@` sozinho logo no começo) não diz nada.
    for diff in &mut diffs { if diff.linhas.first().is_some_and(|l| l == "@") { diff.linhas.remove(0); } }
    diffs
}

/// `unified_diff` do item `FileChange` do Codex (`@@ -1,3 +1,4 @@` e as linhas).
pub fn do_unified(arquivo: &str, tipo: TipoDeMudanca, unified: &str) -> DiffDeArquivo {
    let mut diff = DiffDeArquivo::novo(arquivo, tipo);
    let mut dentro = false;
    for linha in unified.lines() {
        if let Some(resto) = linha.strip_prefix("@@") {
            dentro = true;
            // `@@ -1,3 +1,4 @@ fn x` → o rótulo é o que vem depois do segundo `@@`.
            let rotulo = resto.split_once("@@").map_or("", |(_, r)| r.trim());
            if !diff.linhas.is_empty() || !rotulo.is_empty() { diff.linhas.push(format!("@{rotulo}")); }
            continue;
        }
        if !dentro { continue; }
        let mut letras = linha.chars();
        if let Some(marca @ ('+' | '-' | ' ')) = letras.next() { diff.linha(marca, letras.as_str()); }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edicao_vira_linhas_trocadas_com_contexto_curto() {
        let antigo = "a\nb\nc\nd\ne\nf\ng\nh\nvelho\ni";
        let novo = "a\nb\nc\nd\ne\nf\ng\nh\nnovo\noutro\ni";
        let diff = de_edicao("/p/x.ts", antigo, novo);
        assert_eq!((diff.adicionadas, diff.removidas, diff.tipo), (2, 1, TipoDeMudanca::Editado));
        // Só 3 linhas iguais antes da mudança; as de cima viram um corte (`@` no começo não entra).
        assert_eq!(diff.linhas, vec![" f", " g", " h", "-velho", "+novo", "+outro", " i"]);
    }

    #[test]
    fn edicao_no_meio_e_no_fim_separa_trechos() {
        let antigo = (1..=20).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
        let novo = antigo.replace("\n2\n", "\ndois\n").replace("\n19\n", "\ndezenove\n");
        let diff = de_edicao("x", &antigo, &novo);
        assert_eq!((diff.adicionadas, diff.removidas), (2, 2));
        assert!(diff.linhas.contains(&"@".to_owned()), "dois trechos longe um do outro: {:?}", diff.linhas);
        assert_eq!(diff.linhas.first().map(String::as_str), Some(" 1"));
    }

    #[test]
    fn multiedit_e_escrita() {
        let diff = de_edicoes("x", &[("a", "b"), ("c\nd", "c")]);
        assert_eq!(diff.linhas, vec!["-a", "+b", "@", " c", "-d"]);
        let grande = (0..500).map(|n| format!("linha {n}")).collect::<Vec<_>>().join("\n");
        let escrito = de_escrita("novo.md", &grande);
        assert_eq!((escrito.adicionadas, escrito.linhas.len(), escrito.omitidas, escrito.tipo), (500, LINHAS_MAX, 500 - LINHAS_MAX, TipoDeMudanca::Escrito));
    }

    #[test]
    fn edicao_enorme_nao_calcula_lcs() {
        let antigo = (0..300).map(|n| format!("a{n}")).collect::<Vec<_>>().join("\n");
        let novo = (0..300).map(|n| format!("b{n}")).collect::<Vec<_>>().join("\n");
        let diff = de_edicao("x", &antigo, &novo);
        assert_eq!((diff.adicionadas, diff.removidas), (300, 300));
    }

    #[test]
    fn linha_longa_e_limite_por_acao() {
        let diff = de_edicao("x", "", &"y".repeat(1000));
        assert_eq!(diff.linhas[0].chars().count(), LARGURA_MAX + 1);
        assert!(diff.linhas[0].ends_with('…'));
        let mut varios = vec![de_escrita("a", &"x\n".repeat(50)), de_escrita("b", &"x\n".repeat(50))];
        limitar(&mut varios);
        assert_eq!((varios[0].linhas.len(), varios[1].linhas.len(), varios[1].omitidas), (50, LINHAS_MAX - 50, 50 - (LINHAS_MAX - 50)));
    }

    #[test]
    fn patch_do_codex_um_diff_por_arquivo() {
        let patch = "*** Begin Patch\n*** Update File: src/a.ts\n@@ function x\n contexto\n-velho\n+novo\n*** Add File: src/b.ts\n+um\n+dois\n*** Delete File: c.txt\n*** Update File: d.ts\n*** Move to: e.ts\n@@\n-1\n+2\n*** End of File\n*** End Patch";
        let diffs = do_patch(patch);
        assert_eq!(diffs.iter().map(|d| (d.arquivo.as_str(), d.tipo, d.adicionadas, d.removidas)).collect::<Vec<_>>(), vec![
            ("src/a.ts", TipoDeMudanca::Editado, 1, 1),
            ("src/b.ts", TipoDeMudanca::Novo, 2, 0),
            ("c.txt", TipoDeMudanca::Apagado, 0, 0),
            ("d.ts → e.ts", TipoDeMudanca::Editado, 1, 1),
        ]);
        assert_eq!(diffs[0].linhas, vec!["@function x", " contexto", "-velho", "+novo"]);
        assert_eq!(diffs[3].linhas, vec!["-1", "+2"]);
    }

    #[test]
    fn unified_do_file_change() {
        let unified = "--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n a\n-b\n+c\n@@ -10,1 +10,2 @@ fn y\n z\n+w\n";
        let diff = do_unified("/p/x", TipoDeMudanca::Editado, unified);
        assert_eq!((diff.adicionadas, diff.removidas), (2, 1));
        assert_eq!(diff.linhas, vec![" a", "-b", "+c", "@fn y", " z", "+w"]);
    }
}
