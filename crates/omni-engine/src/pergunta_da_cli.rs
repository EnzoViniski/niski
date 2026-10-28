//! Fork (pergunta-da-cli): diálogos da própria CLI que esperam o usuário e que o
//! `interaction::escolha` (rodapé "Enter to confirm · Esc to cancel") e o `interaction::approval`
//! não reconhecem — confiar na pasta, limite de uso, troca de modelo, MCP novo, atualização, o menu
//! do rewind ("Summarize from here"), etc.
//!
//! Duas saídas, nesta ordem:
//! - `menu`: menu de opções no pé da tela (numerado, só com o cursor `❯`/`›`, "(y/n)" ou "Press
//!   enter to continue") vira uma `Escolha` com `da_cli`, respondida pela mesma fila do `/escolher`
//!   com a tecla certa (`Escolha::teclas`);
//! - `espera`: algo esperando entrada que não dá para interpretar (rodapé de teclas, pergunta sem o
//!   campo da CLI embaixo) vira um aviso com as últimas linhas da tela — nunca silêncio.
//!
//! O risco é o falso positivo: uma lista numerada que é só saída de comando, ou o prompt normal `❯`
//! da CLI. A regra que segura os dois: o diálogo está **no pé da tela**. Com a CLI ociosa ou
//! trabalhando, o campo de digitação (`❯`/`›`/`>`) é a última coisa além do rodapé dele; quando o
//! Claude desenha um menu em cima do campo, o menu termina numa linha de teclas ("Enter to confirm"),
//! que o rodapé do campo nunca tem.
use crate::interaction::{Escolha, Opcao};

/// Quantas linhas úteis do pé da tela contam como "o diálogo".
const PE_DA_TELA: usize = 14;
/// Linhas mostradas no aviso de espera.
const LINHAS_DA_ESPERA: usize = 12;
/// A última opção do menu fica no máximo a tantas linhas úteis do fim do diálogo (explicação dela e
/// o rodapé de teclas). Uma lista no meio da saída fica bem mais longe.
const OPCAO_PERTO_DO_FIM: usize = 4;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Espera {
    /// As últimas linhas do diálogo (sem bordas), para a pessoa saber o que a CLI está pedindo.
    pub texto: String,
    /// A linha de teclas do rodapé ("Enter to confirm · Esc to cancel"), quando há.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dica: Option<String>,
}

/// Só Claude e Codex: o Antigravity tem diálogo próprio (`antigravity_permissoes`) e o rodapé dele
/// parado/trabalhando ("esc to cancel") confundiria a leitura genérica.
fn suportado(provider: &str) -> bool { matches!(provider, "claude" | "codex") }

/// Linha sem a moldura lateral (`│ … │`) e sem espaço nas pontas.
fn limpa(linha: &str) -> &str {
    linha.trim().trim_start_matches(['│', '┃']).trim_end_matches(['│', '┃']).trim()
}

/// Borda horizontal da caixa do diálogo (`────`, `╭───╮`).
fn borda(linha: &str) -> bool {
    let linha = linha.trim();
    linha.chars().count() >= 10 && linha.chars().all(|c| matches!(c, '─' | '━' | '═' | '╭' | '╮' | '╰' | '╯' | '┌' | '┐' | '└' | '┘' | ' '))
}

/// Linha útil: nem vazia nem borda.
fn util(linha: &str) -> bool { !limpa(linha).is_empty() && !borda(linha) }

/// Linha da largura do terminal quebra sem `\n` no texto da tela e cola na de baixo (a borda gruda
/// no texto seguinte). Cada sequência longa de `─` vira linha própria, como no `escolha`.
fn linhas(texto: &str) -> Vec<&str> {
    texto.lines().flat_map(|linha| {
        let mut partes = Vec::new();
        let mut resto = linha;
        while let Some(inicio) = resto.find("──────────") {
            let fim = resto[inicio..].find(|c| c != '─').map_or(resto.len(), |i| inicio + i);
            if !resto[..inicio].trim().is_empty() { partes.push(&resto[..inicio]); }
            partes.push(&resto[inicio..fim]);
            resto = &resto[fim..];
        }
        if partes.is_empty() || !resto.trim().is_empty() { partes.push(resto); }
        partes
    }).collect()
}

/// Cursor de seleção no começo da linha.
const MARCAS: [char; 4] = ['❯', '›', '>', '▶'];

fn marcada(linha: &str) -> bool { limpa(linha).starts_with(MARCAS) }

/// `❯ 1. Yes` / `  2. No` → (marcada, número, rótulo).
fn opcao(linha: &str) -> Option<(bool, u8, String)> {
    let linha = limpa(linha);
    let sel = linha.starts_with(MARCAS);
    let resto = linha.trim_start_matches(MARCAS).trim();
    let (numero, rotulo) = resto.split_once(". ").or_else(|| resto.split_once(") "))?;
    let numero: u8 = numero.parse().ok()?;
    // Fork (perguntas na Conversa): a explicação à direita, em coluna, sai em `numerado` com todas as
    // opções juntas (`coluna_da_descricao`); cortar no vão de três espaços deixava ela grudada no rótulo.
    let rotulo = rotulo.trim();
    (!rotulo.is_empty() && (1..=9).contains(&numero)).then(|| (sel, numero, rotulo.to_owned()))
}

/// O campo de digitação da CLI (ou um prompt antigo): `❯`, `›` ou `>` sem número de opção depois.
fn campo(linha: &str) -> bool {
    let linha = limpa(linha);
    linha.starts_with(['❯', '›', '>']) && opcao(linha).is_none()
}

/// Rodapé com as teclas de um diálogo aberto. Nenhuma destas aparece com o campo livre: o rodapé do
/// campo é "? for shortcuts", o modo, "⏎ send" ou a linha de status. Textos da doc do Claude Code e
/// das telas do Codex (`codex-rs/tui`): "Press enter to confirm or esc to cancel", "enter continue ·
/// esc skip", "enter select · esc back", "enter/esc confirm", "enter to submit | esc to cancel".
pub(crate) fn dica_de_tecla(linha: &str) -> bool {
    let l = linha.to_lowercase();
    const DICAS: [&str; 26] = [
        "enter to confirm", "enter to select", "enter to continue", "enter to submit", "press enter",
        "enter continue", "enter select", "enter confirm", "enter/esc",
        "esc to cancel", "esc to go back", "esc to exit", "esc to close", "esc to skip",
        "esc skip", "esc back", "esc quit", "esc to continue working",
        "to navigate", "↑/↓", "↑↓", "(y/n)", "[y/n]", "(yes/no)", "press any key", "tab to amend",
    ];
    DICAS.iter().any(|dica| l.contains(dica))
}

/// Pergunta de sim/não de linha ("Continue? (y/n)").
fn sim_ou_nao(linha: &str) -> bool {
    let l = linha.to_lowercase();
    ["(y/n)", "[y/n]", "(yes/no)"].iter().any(|d| l.contains(d))
}

/// Só "Enter para continuar" ("Your usage limit has reset" / "Press enter to continue").
fn so_enter(linha: &str) -> bool {
    let l = limpa(linha).to_lowercase();
    (l.starts_with("press enter to continue") || l == "enter to continue") && !l.contains("esc")
}

/// Pedido de permissão que o `approval` recusou (por exemplo, sem a opção de uso único): mostrar as
/// opções como botões mandaria "não perguntar de novo" — fica como espera, para o terminal.
fn pedido_de_permissao(caixa: &[&str]) -> bool {
    caixa.iter().any(|l| {
        let l = limpa(l);
        l.starts_with("Do you want to") || l.starts_with("Would you like to")
    })
}

/// O agente no meio de um turno: o que aparece é saída dele, não pergunta. A pergunta do Codex
/// (`request_user_input`) também diz "esc to interrupt", mas na linha das teclas, com o Enter.
fn trabalhando(texto: &str) -> bool {
    texto.lines().map(str::to_lowercase).any(|l| l.contains("esc to interrupt") && !l.contains("enter"))
}

/// O campo de digitação parado (o da CLI ou um prompt antigo no histórico): a linha com `❯`/`›`/`>`
/// sem uma irmã logo embaixo. Opção marcada de um menu sem número ("❯ No, exit") tem a próxima
/// opção na linha de baixo; o campo tem borda, linha vazia, a resposta (`●`) ou nada.
fn campo_parado(todas: &[&str], i: usize) -> bool {
    campo(todas[i]) && todas.get(i + 1).is_none_or(|abaixo| {
        let limpo = limpa(abaixo);
        limpo.is_empty() || borda(abaixo) || limpo.starts_with(['●', '⏺', '•', '✻', '✢', '✶', '✳', '·'])
    })
}

/// O trecho do pé da tela onde o diálogo pode estar, como fatia contígua de `todas`. Três casos:
/// - o campo da CLI no pé (só o rodapé dele embaixo, até 3 linhas): o diálogo é o que está logo
///   acima e precisa terminar numa linha de teclas — o Claude desenha alguns menus em cima do campo
///   ("Teach auto mode…"). Sem ela, é a CLI ociosa;
/// - o campo mais acima (um prompt antigo): o diálogo é o que vem depois dele;
/// - campo nenhum: as últimas linhas da tela.
fn regiao<'a>(todas: &'a [&'a str]) -> Option<&'a [&'a str]> {
    let uteis: Vec<usize> = (0..todas.len()).filter(|&i| util(todas[i])).collect();
    let mut abaixo = 0;
    let mut fim = todas.len();
    let mut campo_no_pe = false;
    let mut inicio = 0;
    for &i in uteis.iter().rev() {
        if campo_parado(todas, i) {
            if !campo_no_pe && abaixo <= 3 { campo_no_pe = true; fim = i; abaixo = 0; continue; }
            inicio = i + 1;
            break;
        }
        abaixo += 1;
        if abaixo >= PE_DA_TELA { inicio = i; break; }
    }
    let fatia = todas.get(inicio..fim)?;
    let ultima = fatia.iter().rposition(|l| util(l))?;
    if campo_no_pe && !dica_de_tecla(fatia[ultima]) { return None; }
    Some(fatia)
}

/// Linhas úteis depois do índice `i` da fatia.
fn uteis_depois(caixa: &[&str], i: usize) -> usize { caixa[i + 1..].iter().filter(|l| util(l)).count() }

/// Menu da própria CLI no pé da tela. `cursor`: linha do cursor visível, para o "(y/n)" de linha
/// (que precisa de Enter) — `None` com o cursor escondido.
pub fn menu_no_texto(texto: &str, provider: &str, cursor: Option<usize>) -> Option<Escolha> {
    if !suportado(provider) || trabalhando(texto) { return None; }
    let todas = linhas(texto);
    let caixa = regiao(&todas)?;
    if pedido_de_permissao(caixa) { return None; }
    let rodape = caixa.iter().any(|l| dica_de_tecla(l));

    if let Some(menu) = numerado(caixa, provider, rodape) { return Some(menu); }
    if let Some(menu) = so_com_cursor(caixa, rodape) { return Some(menu); }

    let ultimas: Vec<usize> = (0..caixa.len()).rev().filter(|&i| util(caixa[i])).take(3).collect();
    // "(y/n)" numa das três últimas linhas úteis.
    if let Some(i) = ultimas.iter().copied().find(|&i| sim_ou_nao(caixa[i])) {
        // Cursor na mesma linha da pergunta: é leitura de linha (precisa de Enter).
        // Também com o cursor na linha vazia logo abaixo — o `eprintln!` + `read_line` do Codex
        // ("Continue anyway? [y/N]:") pula a linha antes de ler, e sem Enter o botão só escrevia o "y".
        // `rposition`: a pergunta repetida na tela conta a última vez que apareceu.
        let linhas: Vec<&str> = texto.lines().collect();
        let linha_original = linhas.iter().rposition(|l| *l == caixa[i]);
        let vazia = |c: usize| linhas.get(c).is_none_or(|l| l.trim().is_empty());
        let enter = match (cursor, linha_original) {
            (Some(c), Some(l)) if c == l || (c == l + 1 && vazia(c)) => "\r",
            _ => "",
        };
        return Some(Escolha {
            pergunta: limpa(caixa[i]).to_owned(), detalhe: None,
            opcoes: vec![
                Opcao { numero: 1, rotulo: "Sim".into(), descricao: Some("tecla y".into()), marcada: None },
                Opcao { numero: 2, rotulo: "Não".into(), descricao: Some("tecla n".into()), marcada: None },
            ],
            multipla: false, etapas: Vec::new(), previa: false, da_cli: true,
            teclas: Some(vec![format!("y{enter}"), format!("n{enter}")]),
        });
    }
    // "Press enter to continue" sozinho na última linha: um botão, o Enter.
    let ultima = *ultimas.first()?;
    if !so_enter(caixa[ultima]) { return None; }
    // A pergunta é a linha logo acima ("Your usage limit has reset"), não o começo da fatia.
    let pergunta = caixa[..ultima].iter().rev().find(|l| util(l) && !campo(l)).map_or("A CLI pede Enter para continuar", |l| limpa(l)).to_owned();
    Some(Escolha {
        pergunta, detalhe: None,
        opcoes: vec![Opcao { numero: 1, rotulo: "Continuar".into(), descricao: Some("tecla Enter".into()), marcada: None }],
        multipla: false, etapas: Vec::new(), previa: false, da_cli: true, teclas: Some(vec!["\r".into()]),
    })
}

/// Pergunta e detalhe: o texto do diálogo acima da primeira opção (até a borda, um prompt antigo ou
/// 6 linhas). A numeração de perguntas do Codex ("Question 1/1 (1 unanswered)") não é a pergunta.
fn pergunta_acima(caixa: &[&str], primeira: usize) -> (String, Option<String>) {
    let inicio = caixa[..primeira].iter().rposition(|l| borda(l) || campo(l)).map_or(0, |i| i + 1);
    // Fork (perguntas na Conversa): a resposta do agente que ficou em cima ("• Vou perguntar.") não é a
    // pergunta — no Codex 0.164 ela virava o título do cartão. A pergunta começa depois dela.
    let inicio = caixa[inicio..primeira].iter().rposition(|l| limpa(l).starts_with(['•', '●', '⏺', '■']))
        .map_or(inicio, |i| inicio + i + 1);
    let textos: Vec<&str> = caixa[inicio..primeira].iter().map(|l| limpa(l))
        .filter(|l| !l.is_empty() && !(l.starts_with("Question ") && l.contains('/'))).collect();
    let textos = &textos[textos.len().saturating_sub(6)..];
    match textos.split_first() {
        Some((pergunta, resto)) => ((*pergunta).to_owned(), (!resto.is_empty()).then(|| resto.join("\n"))),
        None => ("A CLI está esperando uma escolha".into(), None),
    }
}

/// Teclas para chegar na opção `alvo` andando com as setas a partir da `atual`, e Enter.
fn setas(atual: u8, alvo: u8) -> String {
    let seta = if alvo >= atual { "\x1b[B" } else { "\x1b[A" };
    format!("{}\r", seta.repeat(alvo.abs_diff(atual) as usize))
}

/// Opções "1. … 2. …" seguidas no pé do diálogo, com o cursor numa delas ou um rodapé de teclas.
fn numerado(caixa: &[&str], provider: &str, tem_rodape: bool) -> Option<Escolha> {
    let ultima = caixa.iter().rposition(|l| opcao(l).is_some())?;
    if uteis_depois(caixa, ultima) > OPCAO_PERTO_DO_FIM { return None; }
    // Sobe da última opção até a 1, aceitando até 3 linhas de explicação entre elas.
    let mut achadas: Vec<(usize, bool, u8, String)> = Vec::new();
    let mut vao = 0;
    for i in (0..=ultima).rev() {
        let linha = caixa[i];
        if borda(linha) { break; }
        match opcao(linha) {
            Some((sel, numero, rotulo)) => {
                if let Some(seguinte) = achadas.last() { if numero + 1 != seguinte.2 { return None; } }
                achadas.push((i, sel, numero, rotulo));
                vao = 0;
                if numero == 1 { break; }
            }
            None => { if util(linha) { vao += 1; if vao > 3 { break; } } }
        }
    }
    achadas.reverse();
    if achadas.len() < 2 || achadas[0].2 != 1 { return None; }
    let selecionadas: Vec<u8> = achadas.iter().filter(|o| o.1).map(|o| o.2).collect();
    if selecionadas.len() > 1 { return None; }
    // Sem cursor e sem rodapé de teclas, uma lista numerada no pé é só texto.
    if selecionadas.is_empty() && !tem_rodape { return None; }
    let (pergunta, detalhe) = pergunta_acima(caixa, achadas[0].0);
    // Explicação embaixo de cada opção (linhas entre ela e a próxima; a última não leva o rodapé).
    // Fork (perguntas na Conversa): rótulo e descrição em coluna na mesma linha (Codex).
    let separadas = crate::coluna_da_descricao::separar(&achadas.iter().map(|o| o.3.clone()).collect::<Vec<_>>());
    let opcoes: Vec<Opcao> = achadas.iter().zip(separadas).enumerate().map(|(k, ((i, _, numero, _), (rotulo, coluna)))| {
        let fim = achadas.get(k + 1).map_or(*i + 1, |o| o.0);
        let abaixo = caixa[i + 1..fim].iter().map(|l| limpa(l)).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        let descricao = [coluna.unwrap_or_default(), abaixo].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
        Opcao { numero: *numero, rotulo, descricao: (!descricao.is_empty()).then_some(descricao), marcada: None }
    }).collect();
    // Claude: o número escolhe (o seletor dele numera para isso). Codex e outros: setas a partir da
    // opção marcada e Enter — sem marca visível não há de onde partir, e fica a espera.
    let teclas = match (provider, selecionadas.first()) {
        ("claude", _) => None,
        (_, Some(&atual)) => Some(opcoes.iter().map(|o| setas(atual, o.numero)).collect()),
        (_, None) => return None,
    };
    Some(Escolha { pergunta, detalhe, opcoes, multipla: false, etapas: Vec::new(), previa: false, da_cli: true, teclas })
}

/// Menu sem número ("❯ No, exit" / "Yes, I trust this folder"): o cursor numa linha e as irmãs
/// alinhadas na coluna do rótulo, logo acima ou abaixo. Exige o rodapé de teclas.
fn so_com_cursor(caixa: &[&str], tem_rodape: bool) -> Option<Escolha> {
    if !tem_rodape { return None; }
    let i = caixa.iter().rposition(|l| marcada(l) && opcao(l).is_none() && limpa(l).chars().count() > 2)?;
    let sem_moldura = |l: &str| l.trim_start_matches(['│', '┃']).to_owned();
    let coluna = |l: &str| sem_moldura(l).chars().take_while(|c| c.is_whitespace()).count();
    // Coluna do rótulo da linha marcada: depois da marca e do espaço.
    let marca = sem_moldura(caixa[i]);
    let recuo = marca.chars().take_while(|c| c.is_whitespace()).count();
    let depois_da_marca = marca.trim_start().chars().skip(1).take_while(|c| c.is_whitespace()).count();
    let rotulo_col = recuo + 1 + depois_da_marca;
    let irma = |l: &str| util(l) && !marcada(l) && !dica_de_tecla(l) && coluna(l) == rotulo_col;
    let mut inicio = i;
    while inicio > 0 && irma(caixa[inicio - 1]) { inicio -= 1; }
    let mut fim = i;
    while fim + 1 < caixa.len() && irma(caixa[fim + 1]) { fim += 1; }
    let total = fim - inicio + 1;
    if !(2..=9).contains(&total) || uteis_depois(caixa, fim) > OPCAO_PERTO_DO_FIM { return None; }
    let atual = (i - inicio + 1) as u8;
    let opcoes: Vec<Opcao> = caixa[inicio..=fim].iter().enumerate().map(|(k, l)| Opcao {
        numero: k as u8 + 1, rotulo: limpa(l).trim_start_matches(MARCAS).trim().to_owned(), descricao: None, marcada: None,
    }).collect();
    let (pergunta, detalhe) = pergunta_acima(caixa, inicio);
    let teclas = Some(opcoes.iter().map(|o| setas(atual, o.numero)).collect());
    Some(Escolha { pergunta, detalhe, opcoes, multipla: false, etapas: Vec::new(), previa: false, da_cli: true, teclas })
}

/// Algo esperando entrada no pé da tela que não virou menu: uma linha de teclas, ou uma pergunta
/// ("…?") nas últimas linhas sem o campo da CLI embaixo, e sem turno em andamento.
pub fn espera_no_texto(texto: &str, provider: &str) -> Option<Espera> {
    if !suportado(provider) || trabalhando(texto) { return None; }
    let todas = linhas(texto);
    let caixa = regiao(&todas)?;
    let uteis: Vec<&str> = caixa.iter().copied().filter(|l| util(l)).map(limpa).collect();
    let dica = uteis.iter().rev().find(|l| dica_de_tecla(l)).map(|l| (*l).to_owned());
    let pergunta = uteis.iter().rev().take(4).any(|l| l.ends_with('?'));
    if dica.is_none() && !pergunta { return None; }
    let ultimas = &uteis[uteis.len().saturating_sub(LINHAS_DA_ESPERA)..];
    Some(Espera { texto: ultimas.join("\n"), dica })
}

/// Teclas para responder a opção `n` de um menu com `teclas`: (setas, fim) — as setas num primeiro
/// write e o Enter (ou a tecla da opção) no segundo. `None`: o menu responde pelo número.
pub fn teclas_da_opcao(menu: &Escolha, n: u8) -> Option<(Option<String>, String)> {
    let i = menu.opcoes.iter().position(|o| o.numero == n)?;
    let teclas = menu.teclas.as_ref()?.get(i)?;
    Some(match teclas.strip_suffix('\r').filter(|setas| setas.starts_with('\x1b')) {
        Some(setas) => (Some(setas.to_owned()), "\r".into()),
        None => (None, teclas.clone()),
    })
}

/// Menu genérico na tela viva (o cursor só conta se estiver visível).
pub fn menu(screen: &vt100::Screen, provider: &str) -> Option<Escolha> {
    let cursor = (!screen.hide_cursor()).then(|| screen.cursor_position().0 as usize);
    menu_no_texto(&crate::interaction::linha_a_linha(screen), provider, cursor)
}

/// Espera na tela viva. `parada_ms`: há quanto tempo a tela não muda — um quadro no meio de um
/// redesenho não vira aviso.
pub fn espera(screen: &vt100::Screen, provider: &str, parada_ms: u64) -> Option<Espera> {
    if parada_ms < 1_500 { return None; }
    espera_no_texto(&crate::interaction::linha_a_linha(screen), provider)
}

#[cfg(test)]
#[path = "pergunta_da_cli_testes.rs"]
mod testes;
