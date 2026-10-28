//! Typed actions over existing PTYs. Recognition deliberately fails closed.
use super::*;
use omni_core::usage::AccountUsage;
use std::{collections::hash_map::DefaultHasher, hash::{Hash, Hasher}, time::{Duration, Instant}};

/// `turn_active`: o agente mostrou sinal de turno (hint de interromper ou diálogo de aprovação) desde
/// o último fim de turno. É o que separa "terminou" de "está ocioso" em `settle`.
/// `aprovacao_contada`: o diálogo de aprovação na tela já virou aviso (`attention_seq`). A TUI
/// redesenha o diálogo em pedaços, e um quadro parcial não é reconhecido — sem esta trava, cada
/// redesenho contava um pedido novo e o aviso já visto voltava sozinho.
///
/// `avisos_antes_do_turno`: quantos avisos (limite, erro de API) já estavam na tela quando o turno
/// atual começou. Só os que aparecerem depois disso são deste turno — ver `notice_since`.
///
/// `turnos_iniciados`: antes do primeiro turno, nada na tela é aviso novo — ver `append_output`.
pub struct Interaction { pub parser: vt100::Parser, pub input_revision: u64, pub turn_active: bool, pub aprovacao_contada: bool, pub avisos_antes_do_turno: (usize, usize), pub turnos_iniciados: u64, generation: u64,
    /// Fork (chat-estavel): o cabeçalho "OpenAI Codex" já apareceu desde que a tela nasceu (zera no reinício).
    pub codex_visto: bool }
impl Interaction {
    pub fn new(rows: u16, cols: u16) -> Self { Self { parser: vt100::Parser::new(rows,cols,0), input_revision: 0, turn_active: false, aprovacao_contada: false, avisos_antes_do_turno: (0, 0), turnos_iniciados: 0, generation: rand::random(), codex_visto: false } }
    pub fn revision(&self, session: &Arc<LiveSession>) -> String {
        let mut hash = DefaultHasher::new();
        // Sem os quadros de animação (o Codex ocioso gira um indicador e a versão nunca parava).
        crate::revisao_estavel::texto_para_revisao(&self.parser.screen().contents()).hash(&mut hash);
        self.input_revision.hash(&mut hash);
        self.generation.hash(&mut hash);
        (Arc::as_ptr(session) as usize).hash(&mut hash);
        format!("\"{:x}\"",hash.finish())
    }
}

#[derive(Clone, serde::Serialize)]
pub struct Capabilities {
    pub prompt: bool, pub approve: bool, pub revision: String, pub approval_text: Option<String>, pub reason: Option<String>,
    /// O agente está no meio de um turno, mas o campo da CLI está vazio e aceita digitação: uma
    /// mensagem mandada agora entra na fila da própria CLI, como digitar no terminal.
    #[serde(default)]
    pub enfileirar: bool,
    /// Menu de escolha da própria CLI na tela (ex.: "Teach auto mode about your environment?"): sem
    /// responder, o campo fica travado. A conversa mostra as opções e manda o número.
    pub escolha: Option<Escolha>,
    /// Fork (Onda 2, item 2): pedido de permissão segurado pelo gancho `PermissionRequest` (ferramenta,
    /// resumo, prazo). Responder com o `id` dele (`/aprovar` com `pedido`): outro id → 412.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pedido: Option<crate::ganchos::PedidoVisivel>,
    /// Fork (pergunta-da-cli): a CLI parada esperando algo no terminal que não virou menu nem pedido.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub espera: Option<crate::pergunta_da_cli::Espera>,
    /// Fork (perguntas na Conversa): todas as opções do diálogo de permissão na tela (`opcoes_da_permissao`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub approval_options: Vec<crate::opcoes_da_permissao::OpcaoDaPermissao>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Escolha {
    pub pergunta: String,
    /// Texto entre a pergunta e as opções, uma linha da tela por linha.
    pub detalhe: Option<String>,
    pub opcoes: Vec<Opcao>,
    /// Marcar várias (AskUserQuestion com `multiSelect`): o número marca/desmarca e "avançar" (Tab)
    /// segue para a próxima pergunta ou para a revisão.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub multipla: bool,
    /// Abas de várias perguntas seguidas ("☐ Cor  ☒ Fruta"), sem a de enviar.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub etapas: Vec<Etapa>,
    /// Pergunta com prévia ("n to add notes"): ali o número só move a seleção, e é o Enter que escolhe.
    #[serde(skip)]
    pub previa: bool,
    /// Fork (pergunta-da-cli): menu da própria CLI achado pela leitura genérica
    /// (`pergunta_da_cli::menu`), não pergunta do agente: a conversa mostra "A CLI está perguntando".
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub da_cli: bool,
    /// Fork (pergunta-da-cli): teclas de cada opção, na ordem de `opcoes` (setas + Enter, "y"…).
    /// `None` = o número da opção, o caminho de sempre.
    #[serde(skip)]
    pub teclas: Option<Vec<String>>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Opcao {
    pub numero: u8,
    pub rotulo: String,
    /// A linha de explicação embaixo da opção, quando o agente escreveu uma.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descricao: Option<String>,
    /// Só em `multipla`: se está marcada agora.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marcada: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Etapa { pub titulo: String, pub respondida: bool }

/// Texto da tela com uma linha por linha do terminal. O `contents()` junta linha que quebrou por
/// largura com a de baixo — a pergunta do agente ocupa a largura toda e a opção "1." colava nela,
/// e o menu inteiro deixava de ser reconhecido.
pub fn linha_a_linha(screen: &vt100::Screen) -> String {
    screen.rows(0, screen.size().1).collect::<Vec<_>>().join("\n")
}

/// Fork (pergunta-da-cli): o menu na tela — o reconhecido pelo rodapé (`escolha`) ou, sem ele, o
/// genérico da CLI (`pergunta_da_cli::menu`). Quem mostra e quem responde usam este mesmo.
pub fn menu_na_tela(screen: &vt100::Screen, provider: &str) -> Option<Escolha> {
    escolha(&linha_a_linha(screen)).or_else(|| crate::pergunta_da_cli::menu(screen, provider))
}

/// Menu numerado da CLI com o rodapé "Enter to confirm · Esc to cancel": a caixa vai da última
/// linha de borda (`────`) acima do rodapé até ele. Precisa de pelo menos duas opções numeradas em
/// sequência a partir de 1. Pedido de permissão ("Do you want to…") fica de fora: é o `approval`,
/// que só aceita as escolhas de uso único — a conversa nunca manda "não perguntar de novo".
pub fn escolha(text: &str) -> Option<Escolha> {
    // Linha da largura do terminal quebra sem `\n` no texto da tela e cola na de baixo: a borda
    // gruda na barra de abas ("────… ☐ Cor") e, depois de um turno, também no prompt destacado de
    // cima ("❯ manda de novo…   ────… ☐ Cor"). Cada sequência longa de `─` vira linha própria.
    let linhas: Vec<&str> = text.lines().flat_map(|linha| {
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
    }).collect();
    let borda = |linha: &str| { let linha = linha.trim(); linha.chars().count() >= 10 && linha.chars().all(|c| c == '─') };
    // Pergunta com prévia (`preview` do AskUserQuestion): o desenho fica à direita das opções, na
    // mesma linha ("1. Lado a lado     ┌────┐"), e embaixo delas ("│ | claude | │"). Corta no
    // primeiro caractere de caixa ou num vão largo.
    const CAIXA: [char; 11] = ['│', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼', '─'];
    let sem_previa = |texto: &str| -> String {
        let fim = [texto.find(CAIXA), texto.find("   ")].into_iter().flatten().min().unwrap_or(texto.len());
        texto[..fim].trim().to_owned()
    };
    let opcao = |linha: &str| -> Option<Opcao> {
        let linha = linha.trim().trim_start_matches(['❯', '›', '>']).trim();
        let (numero, rotulo) = linha.split_once(". ")?;
        let numero: u8 = numero.parse().ok()?;
        // Fork (perguntas na Conversa): só a caixa da prévia sai aqui; a descrição em coluna (Codex) é
        // separada depois, com todas as opções juntas (`coluna_da_descricao`).
        let rotulo = rotulo[..rotulo.find(CAIXA).unwrap_or(rotulo.len())].trim().to_owned();
        let rotulo = rotulo.as_str();
        // Caixa de marcar ("[ ] Uva", "[✔] Uva"): vira `marcada`.
        let (marcada, rotulo) = match rotulo.strip_prefix('[').and_then(|r| r.split_once("] ")) {
            Some((dentro, resto)) if dentro.chars().count() == 1 => (Some(dentro != " "), resto.trim()),
            _ => (None, rotulo),
        };
        (!rotulo.is_empty()).then(|| Opcao { numero, rotulo: rotulo.to_owned(), descricao: None, marcada })
    };
    let rodape = linhas.iter().rposition(|linha| {
        let linha = linha.trim().to_lowercase();
        (linha.contains("esc to cancel") && linha.contains("enter"))
            // Antigravity: "↑/↓ Navigate · enter Select · esc Skip".
            || (linha.contains("enter select") && linha.contains("navigate"))
    }).or_else(|| {
        // Revisão no fim de várias perguntas ("Submit answers / Cancel") vem sem rodapé: a caixa vai
        // até a última opção depois de "Ready to submit your answers?".
        let pronto = linhas.iter().rposition(|l| l.trim() == "Ready to submit your answers?")?;
        (pronto + 1..linhas.len()).filter(|&i| opcao(linhas[i]).is_some()).last().map(|i| i + 1)
    })?;
    // A caixa começa na borda acima da opção 1: a pergunta do agente (AskUserQuestion) tem outra
    // borda antes de "Chat about this", e começar na última deixava só essa opção.
    let um = linhas[..rodape].iter().rposition(|l| opcao(l).is_some_and(|o| o.numero == 1))?;
    // Fork (perguntas na Conversa): sem borda acima (formulário de aprovação do MCP no Codex 0.164), a
    // caixa é o bloco de texto colado na opção 1 — antes subia 20 linhas e a "pergunta" virava uma linha
    // velha do histórico.
    let inicio = linhas[..um].iter().rposition(|l| borda(l)).map(|i| i + 1).unwrap_or_else(|| {
        let mut i = um;
        while i > 0 && linhas[i - 1].trim().is_empty() && um - i < 3 { i -= 1; }
        while i > 0 && !linhas[i - 1].trim().is_empty() && um - i < 20 { i -= 1; }
        i
    });
    let caixa: Vec<&str> = linhas[inicio..rodape].iter().copied().filter(|l| !borda(l)).collect();
    if caixa.iter().any(|l| l.contains("Do you want to")) { return None; }
    // Opções só da opção 1 para baixo: o que vem antes é a pergunta, mesmo que tenha lista numerada.
    let primeira = caixa.iter().rposition(|l| opcao(l).is_some_and(|o| o.numero == 1))?;
    // Cada opção com a explicação das linhas de baixo (até a próxima opção). Uma borda encerra a
    // explicação: o que vem depois dela ("Chat about this" sem número, na prévia) não é da opção.
    let mut todas: Vec<Opcao> = Vec::new();
    let mut explicando = false;
    for linha in &linhas[um..rodape] {
        if borda(linha) { explicando = false; continue; }
        match opcao(linha) {
            Some(nova) => { todas.push(nova); explicando = true; }
            None => if let (true, Some(atual)) = (explicando, todas.last_mut()) {
                let texto = sem_previa(linha.trim());
                if !texto.is_empty() && !texto.starts_with("Notes:") {
                    atual.descricao = Some(match atual.descricao.take() { Some(antes) => format!("{antes} {texto}"), None => texto });
                }
            },
        }
    }
    // Fork (perguntas na Conversa): descrição na mesma linha, em coluna ("2. Allow for this session  Run…").
    let separadas = crate::coluna_da_descricao::separar(&todas.iter().map(|o| o.rotulo.clone()).collect::<Vec<_>>());
    for (opcao, (rotulo, coluna)) in todas.iter_mut().zip(separadas) {
        opcao.rotulo = rotulo;
        if let Some(coluna) = coluna {
            opcao.descricao = Some(match opcao.descricao.take() { Some(abaixo) => format!("{coluna} {abaixo}"), None => coluna });
        }
    }
    if todas.len() < 2 || todas.len() > 9 || todas.iter().enumerate().any(|(i, o)| o.numero as usize != i + 1) { return None; }
    let multipla = todas.iter().any(|o| o.marcada.is_some());
    // "Type something" abre um campo de texto dentro do menu, que a conversa não preenche; para
    // responder livre existe "Chat about this", que volta ao campo normal. A linha "Submit" do
    // marcar-várias fica pendurada nele e some junto.
    // "Write-in..." é o mesmo campo livre no Antigravity.
    let opcoes: Vec<Opcao> = todas.into_iter().filter(|o| !o.rotulo.starts_with("Type something") && !o.rotulo.starts_with("Write-in")).collect();
    // A barra de abas das perguntas ("← ☐ Cor  ☒ Fruta  ✔ Submit →") não é texto da pergunta; as
    // respostas da revisão ("→ Azul") são, e vão no detalhe.
    let aba = |l: &str| l.contains(['☐', '☒', '✔']);
    let etapas: Vec<Etapa> = caixa[..primeira].iter().find(|l| aba(l)).map(|barra| {
        barra.split(['☐', '☒']).skip(1).zip(barra.match_indices(['☐', '☒']))
            .map(|(resto, (_, marca))| Etapa {
                titulo: resto.split("  ").next().unwrap_or("").split('✔').next().unwrap_or("").trim().to_owned(),
                respondida: marca == "☒",
            })
            .filter(|e| !e.titulo.is_empty()).collect()
    }).filter(|e: &Vec<Etapa>| e.len() > 1).unwrap_or_default();
    // O contador do formulário do Codex ("Field 1/1", "Question 1/2 (2 unanswered)") não é a pergunta.
    let contador = |l: &str| ["Field ", "Question "].iter().any(|p| l.strip_prefix(p)
        .and_then(|r| r.split_whitespace().next()).is_some_and(|n| n.contains('/') && n.chars().all(|c| c.is_ascii_digit() || c == '/')));
    let mut textos = caixa[..primeira].iter().filter(|l| !aba(l)).map(|l| l.trim()).filter(|l| !l.is_empty() && !contador(l));
    // Antigravity numera as perguntas ("Question 1/1: O que…"): a numeração não é texto da pergunta.
    let pergunta = textos.next()?;
    let pergunta = pergunta.strip_prefix("Question ").and_then(|resto| resto.split_once(": "))
        .filter(|(n, _)| n.contains('/') && n.chars().all(|c| c.is_ascii_digit() || c == '/'))
        .map_or(pergunta, |(_, texto)| texto).to_owned();
    let detalhe = textos.collect::<Vec<_>>().join("\n");
    let previa = linhas[um..].iter().take(rodape + 1 - um).any(|l| l.contains("to add notes"));
    Some(Escolha { pergunta, detalhe: (!detalhe.is_empty()).then_some(detalhe), opcoes, multipla, etapas, previa, da_cli: false, teclas: None })
}

/// Require provider chrome plus an empty composer at the actual terminal cursor.
pub fn ready(screen: &vt100::Screen, provider: &str) -> bool { ready_com(screen, provider, false) }

/// Fork (chat-estavel): o cabeçalho do Codex só existe na abertura e rola para fora depois de algumas
/// trocas — exigir que ele estivesse na tela travava a conversa ("cabeçalho do Codex fora da tela"), e
/// cada envio caía no `destravar`, que reabria o agente. Basta tê-lo visto desde que a CLI abriu: isso
/// ainda impede colar no shell antes de o Codex subir.
pub fn ready_no(context: &Interaction, provider: &str) -> bool { ready_com(context.parser.screen(), provider, context.codex_visto) }
pub fn pode_enfileirar_no(context: &Interaction, provider: &str) -> bool { pode_enfileirar_com(context.parser.screen(), provider, context.codex_visto) }
/// Chamado pelo leitor da PTY a cada pedaço de saída do Codex.
pub fn notar_codex(context: &mut Interaction, texto: &str) { if !context.codex_visto && texto.contains("OpenAI Codex") { context.codex_visto = true; } }

fn ready_com(screen: &vt100::Screen, provider: &str, codex_visto: bool) -> bool {
    let text = screen.contents();
    // "Claude Code" only shows in the startup banner; it scrolls out of the visible screen after
    // the first exchange, so requiring it here made this permanently false for any session with
    // history (confirmed against a real running session — the idle composer never re-shows it).
    // The structural checks below are strict enough on their own to recognize the composer.
    // Antigravity: "? for shortcuts" no rodapé só existe parado; trabalhando vira "esc to cancel".
    let brand = match provider { "claude" => true, "codex" => codex_visto || text.contains("OpenAI Codex"), "antigravity" => text.contains("? for shortcuts"), _ => false };
    brand && screen.bracketed_paste() && composer_vazio(screen)
        && !text.contains("Esc to cancel") && !text.contains("esc to interrupt") && !text.contains("esc to cancel")
        && approval(&text,provider).is_none()
}

/// Turno em andamento com o campo livre: o Claude Code (e o Codex) aceitam digitar e mandar
/// durante o turno — a mensagem fica na fila e entra quando o turno acaba. É o mesmo campo vazio
/// do `ready`, só que com o "esc to interrupt" na tela em vez de proibido por ele.
pub fn pode_enfileirar(screen: &vt100::Screen, provider: &str) -> bool { pode_enfileirar_com(screen, provider, false) }

fn pode_enfileirar_com(screen: &vt100::Screen, provider: &str, codex_visto: bool) -> bool {
    let text = screen.contents();
    let brand = match provider { "claude" => true, "codex" => codex_visto || text.contains("OpenAI Codex"), _ => false };
    brand && screen.bracketed_paste() && composer_vazio(screen)
        && text.to_lowercase().contains("esc to interrupt")
        && approval(&text,provider).is_none()
}

/// Tokens do turno em andamento, como a CLI mostra na linha de status:
/// `· Propagating… (5s · ↓ 113 tokens · thinking with low effort)` → `"113"`; `"1.2k"` também vale.
/// A linha mais de baixo vence (a de cima pode ser de um turno anterior que ficou na tela).
pub fn tokens_do_turno(texto: &str) -> Option<String> {
    texto.lines().rev().find_map(|linha| {
        let fim = linha.find(" tokens").or_else(|| linha.find("tokens"))?;
        let antes = linha[..fim].trim_end();
        let seta = antes.rfind(['↓', '↑'])?;
        let numero = antes[seta..].trim_start_matches(['↓', '↑']).trim();
        (!numero.is_empty() && numero.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',' || c == 'k' || c == 'K'))
            .then(|| numero.to_owned())
    })
}

/// O campo de digitação está vazio no cursor real.
///
/// À esquerda do cursor, só o símbolo do prompt; à direita, só célula vazia ou texto `dim`. Texto
/// digitado de verdade não é `dim`, então um rascunho a meio continua bloqueando — colar um prompt no
/// meio do que a pessoa escreveu no PC seria pior que recusar.
///
/// A tolerância a `dim` cobre o exemplo em cinza que o Claude Code desenha no campo vazio
/// (`❯ Try "edit <filepath> to..."`). Na tela capturada ele já vinha apagado por um `ESC[K`
/// logo depois, então não era o que travava; a regra antiga (linha inteira igual a `❯`) quebraria
/// no dia em que o redesenho vier sem esse `ESC[K`.
pub(crate) fn composer_vazio(screen: &vt100::Screen) -> bool {
    let (row, col) = screen.cursor_position();
    let (_, cols) = screen.size();
    let antes = screen.contents_between(row,0,row,col);
    matches!(antes.trim(), "❯" | "›" | ">")
        // Espaço em branco conta como vazio: a 2.1.302 apaga cada palavra do exemplo em separado e deixa
        // os espaços entre elas sem `dim` — `Try "fix lint errors"` virava "rascunho" e travava a conversa.
        && ((col..cols).all(|c| screen.cell(row,c).is_none_or(|cell| !cell.has_contents() || cell.dim() || cell.contents().trim().is_empty()))
            // O agy mostra a dica do modo no campo vazio ("Accept-edits mode: … (shift+tab to cycle)").
            || crate::antigravity_permissoes::dica_de_modo(&screen.contents_between(row,col,row,cols)))
}

/// Fork (chat-estavel): a forma da linha do cursor, só metadados (posição e contagens, nunca o texto),
/// para o log dizer por que `composer_vazio` recusou. O caso "campo com texto" do Claude não se
/// reproduziu com a CLI real numa tela limpa; isto mostra na próxima vez qual parte falhou.
pub(crate) fn forma_do_campo(screen: &vt100::Screen) -> String {
    let (row, col) = screen.cursor_position();
    let (rows, cols) = screen.size();
    let antes = screen.contents_between(row, 0, row, col);
    let esquerda = match antes.trim() { "❯" | "›" | ">" => "prompt", "" => "vazio", _ => "texto" };
    let (mut normal, mut cinza) = (0, 0);
    for c in col..cols {
        let Some(cell) = screen.cell(row, c) else { continue };
        if !cell.has_contents() || cell.contents().trim().is_empty() { continue; }
        if cell.dim() { cinza += 1 } else { normal += 1 }
    }
    let prompt_acima = (0..row).rev().take(6).any(|r| matches!(screen.contents_between(r, 0, r, 2).trim(), "❯" | "›"));
    format!("cursor={row}/{rows},{col}/{cols} esquerda={esquerda} direita_normal={normal} direita_cinza={cinza} oculto={} prompt_acima={}",
        if screen.hide_cursor() { "sim" } else { "nao" }, if prompt_acima { "sim" } else { "nao" })
}

/// Quanto esperar a colagem aparecer no campo antes de mandar o Enter assim mesmo.
const ESPERA_DA_COLAGEM: Duration = Duration::from_millis(1_200);
/// Folga depois que o texto aparece: a CLI ainda está montando o "[Pasted text #1 +N lines]" no
/// instante em que a primeira parte dele chega à tela.
const FOLGA_ANTES_DO_ENTER: Duration = Duration::from_millis(80);

/// Manda o Enter que submete um prompt **já colado** no campo.
///
/// Enter no mesmo `write` da colagem não submete: a CLI recebe tudo como uma rajada só e trata o
/// `\r` como parte do texto colado. Com uma linha isso passava despercebido; com várias (é o caso
/// de um prompt com anexos) o texto ficava digitado no terminal e nunca era enviado — e o celular
/// ficava em "trabalhando" para sempre.
///
/// Espera o texto **aparecer no campo** em vez de dormir um tempo fixo: é o sinal de que a CLI
/// terminou de processar a colagem. Fora dos locks de propósito — quem atualiza a tela é o leitor
/// da PTY, que precisa do mesmo `interaction` no intervalo.
pub fn submeter_colagem(session: &Arc<LiveSession>) -> Result<()> {
    let limite = Instant::now() + ESPERA_DA_COLAGEM;
    loop {
        let vazio = session.interaction.lock().map_err(|_| anyhow!("screen poisoned"))
            .map(|context| composer_vazio(context.parser.screen()))?;
        // Campo preenchido: a colagem chegou. Sem eco até o limite, manda assim mesmo — é o único
        // jeito de submeter, e uma CLI que não desenha o campo não pode travar o envio para sempre.
        if !vazio || Instant::now() >= limite { break; }
        std::thread::sleep(Duration::from_millis(30));
    }
    std::thread::sleep(FOLGA_ANTES_DO_ENTER);
    let mut writer = session.writer.lock().map_err(|_| anyhow!("writer poisoned"))?;
    writer.write_all(b"\r")?;
    writer.flush()?;
    Ok(())
}

/// Only single-use choices with exact labels. Persistent permission choices are never sent.
pub fn approval(text: &str, provider: &str) -> Option<(String, String, String)> {
    // Reescrito. O antigo exigia a opção escrita "3. No" (o Claude Code atual escreve "3. No, and
    // tell Claude what to do differently (esc)"), só a pergunta "Do you want to proceed?" (editar e
    // criar arquivo perguntam "Do you want to make this edit to x?") e exatamente um `❯` na tela
    // inteira — o prompt anterior, que fica na tela como "❯ texto", já derrubava. O pedido ficava só
    // no terminal e o chat não mostrava nada para aprovar.
    // Borda de caixa (`│ … │`) sai antes: versões com o diálogo emoldurado.
    // Antigravity tem diálogo próprio (menu de setas), em `antigravity_permissoes`.
    if provider == "antigravity" { return crate::antigravity_permissoes::aprovacao(text); }
    let lines: Vec<String> = text.lines()
        .map(|l| l.trim().trim_start_matches('│').trim_end_matches('│').trim().to_string()).collect();
    let pergunta = |l: &str| match provider {
        "claude" => l.starts_with("Do you want to") && l.ends_with('?'),
        "codex" => l == "Would you like to run the following command?" || (l.starts_with("Would you like to") && l.ends_with('?'))
            // Fork (perguntas na Conversa): acesso à rede no Codex 0.164 (`approval_overlay.rs`); as 4 opções vão ao cartão.
            || (l.starts_with("Do you want to approve") && l.ends_with('?')),
        _ => false,
    };
    let start = lines.iter().rposition(|l| pergunta(l))?;
    let opcao = |l: &str| -> Option<(bool, u8, String)> {
        let selecionada = l.starts_with('❯') || l.starts_with('›');
        let resto = l.trim_start_matches(['❯', '›', '>', ' ']);
        let (n, rotulo) = resto.split_once(". ")?;
        Some((selecionada, n.parse().ok()?, rotulo.trim().to_string()))
    };
    // As opções vêm logo abaixo da pergunta, sem outra pergunta no meio, e numeradas desde 1.
    let opcoes: Vec<(bool, u8, String)> = lines[start + 1..].iter().take(12).filter_map(|l| opcao(l)).collect();
    if opcoes.len() < 2 || opcoes.iter().enumerate().any(|(i, o)| o.1 as usize != i + 1) { return None; }
    if opcoes.iter().filter(|o| o.0).count() != 1 { return None; }
    let sim = &opcoes[0];
    let sim_ok = match provider { "codex" => sim.2.starts_with("Yes"), _ => sim.2 == "Yes" };
    let nao = opcoes.iter().find(|o| o.2 == "No" || o.2.starts_with("No,") || o.2.starts_with("No "))?;
    if !sim_ok { return None; }
    // Texto mostrado no chat: o diálogo (ferramenta, comando, pergunta e opções), não a tela inteira.
    let fim = lines[start + 1..].iter().rposition(|l| opcao(l).is_some()).map_or(lines.len(), |i| start + 2 + i);
    let inicio = lines[..start].iter().rposition(|l| l.chars().count() >= 10 && l.chars().all(|c| matches!(c, '─' | '╭' | '╮' | '╰' | '╯')))
        .map_or(start.saturating_sub(10), |i| i + 1);
    let dialogo = lines[inicio..fim.min(lines.len())].iter().map(|l| l.as_str()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    Some((if provider == "codex" { "y".into() } else { "1\r".into() },
        if provider == "codex" { "\x1b".into() } else { format!("{}\r", nao.1) }, dialogo))
}

/// Nome do executável de cada CLI quando difere do id do provider.
fn executavel(provider: &str) -> &str {
    match provider { "antigravity" => "agy", outro => outro }
}

#[cfg(windows)]
pub fn provider_running(pid: Option<u32>, provider: &str) -> bool {
    let provider = executavel(provider);
    use windows_sys::Win32::{Foundation::{CloseHandle, INVALID_HANDLE_VALUE}, System::Diagnostics::ToolHelp::*};
    let Some(pid) = pid else { return false };
    // A PTY shell survives when its CLI exits. Confirm the CLI is still its descendant.
    unsafe {
        let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0);
        if handle == INVALID_HANDLE_VALUE { return false; }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed(); entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut processes = HashMap::new(); let mut candidates = Vec::new();
        let mut valid = Process32FirstW(handle,&mut entry) != 0;
        while valid {
            let end = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            processes.insert(entry.th32ProcessID,entry.th32ParentProcessID);
            if name.eq_ignore_ascii_case(&format!("{provider}.exe")) { candidates.push(entry.th32ProcessID); }
            valid = Process32NextW(handle,&mut entry) != 0;
        }
        CloseHandle(handle);
        candidates.into_iter().any(|mut child| { for _ in 0..32 {
            if child == pid { return true; }
            let Some(parent) = processes.get(&child) else { break }; if *parent == child { break; } child = *parent;
        } false })
    }
}
/// O shell tem um processo filho rodando (`npm run dev`, `node`…): silêncio aí não é "parado".
/// Shell no prompt não tem filho. conhost/OpenConsole são da ConPTY, não do usuário.
#[cfg(windows)]
pub fn has_children(pid: Option<u32>) -> bool {
    use windows_sys::Win32::{Foundation::{CloseHandle, INVALID_HANDLE_VALUE}, System::Diagnostics::ToolHelp::*};
    let Some(pid) = pid else { return false };
    unsafe {
        let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0);
        if handle == INVALID_HANDLE_VALUE { return false; }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed(); entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut valid = Process32FirstW(handle,&mut entry) != 0;
        while valid && !found {
            if entry.th32ParentProcessID == pid && entry.th32ProcessID != pid {
                let end = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
                found = name != "conhost.exe" && name != "openconsole.exe";
            }
            valid = Process32NextW(handle,&mut entry) != 0;
        }
        CloseHandle(handle);
        found
    }
}
#[cfg(not(windows))]
pub fn has_children(pid: Option<u32>) -> bool {
    let Some(pid) = pid else { return false };
    // `settle` roda em todo snapshot; a leitura do `ps` vale 1 s (`economia.rs`).
    let Some(output) = crate::economia::ps(&["ppid="]) else { return false };
    output.lines().any(|line| line.trim().parse::<u32>() == Ok(pid))
}

#[cfg(not(windows))]
pub fn provider_running(pid: Option<u32>, provider: &str) -> bool {
    let provider = executavel(provider);
    let Some(pid) = pid else { return false };
    // `ps` existe igual no macOS e no Linux (não há /proc no macOS). Duas leituras:
    // - `args`: instalado pelo npm o CLI roda como `node .../claude`, e `comm` diria só `node`;
    // - `comm`: o Claude Code troca o `process.title` pelo título da conversa, e no macOS/Linux isso
    //   reescreve o `args` ("✳ Corrigir login") — sem o `comm` (no macOS, o caminho do executável)
    //   o agente "sumia" depois do primeiro título e a conversa travava em "entrada não reconhecida".
    let ps = |coluna: &str| std::process::Command::new("ps").args(["-A", "-o", "pid=", "-o", "ppid=", "-o", coluna]).output()
        .map(|saida| String::from_utf8_lossy(&saida.stdout).into_owned()).unwrap_or_default();
    agent_descends_from(&format!("{}\n{}", ps("args="), ps("comm=")), pid, provider)
}

/// `provider_running` com a lista de processos de até 1 s atrás (`economia::ps`). Para estado
/// de tela; quem vai digitar ou esperar o CLI sair continua com a leitura na hora.
#[cfg(not(windows))]
pub fn provider_running_recente(pid: Option<u32>, provider: &str) -> bool {
    let provider = executavel(provider);
    let Some(pid) = pid else { return false };
    let ps = |coluna: &str| crate::economia::ps(&["pid=", "ppid=", coluna]).map(|s| s.to_string()).unwrap_or_default();
    agent_descends_from(&format!("{}\n{}", ps("args="), ps("comm=")), pid, provider)
}
#[cfg(windows)]
pub fn provider_running_recente(pid: Option<u32>, provider: &str) -> bool { provider_running(pid, provider) }

/// Linhas `pid ppid comando` do `ps` (de `args` e de `comm`, misturadas). O agente conta quando o
/// executável ou o script que o node roda se chama `provider` e o processo descende da PTY.
#[cfg(any(not(windows), test))]
fn agent_descends_from(ps: &str, pid: u32, provider: &str) -> bool {
    let mut parents = HashMap::new(); let mut candidates = Vec::new();
    for line in ps.lines() {
        fn campo(s: &str) -> (Result<u32, std::num::ParseIntError>, &str) { let s = s.trim_start(); let fim = s.find(char::is_whitespace).unwrap_or(s.len()); (s[..fim].parse(), &s[fim..]) }
        let (Ok(child), resto) = campo(line) else { continue };
        let (Ok(parent), resto) = campo(resto) else { continue };
        let resto = resto.trim();
        parents.insert(child, parent);
        // `comm` do macOS é o caminho inteiro e pode ter espaço ("/Users/Ana Lima/.local/bin/claude").
        if resto.rsplit('/').next() == Some(provider)
            || resto.split_whitespace().take(2).any(|arg| arg.rsplit('/').next() == Some(provider)) { candidates.push(child); }
    }
    candidates.into_iter().any(|mut child| { for _ in 0..32 {
        if child == pid { return true; }
        let Some(parent) = parents.get(&child) else { break }; if *parent == child { break; } child = *parent;
    } false })
}

pub fn capabilities(session: &Arc<LiveSession>) -> Capabilities {
    if let Some(cap) = crate::codex_app_server::capabilities(session) { return cap; } // 3A, protocolo antes da tela.
    let context = session.interaction.lock().expect("screen poisoned");
    let meta = session.meta.lock().expect("metadata poisoned");
    let provider = meta.provider.as_deref().unwrap_or("");
    // `capabilities` entra em cada leitura da conversa; aqui o `ps` de até 1 s atrás basta.
    let available = !session.reserved.load(Ordering::SeqCst) && provider_running_recente(meta.pid,provider);
    let dialog = approval(&context.parser.screen().contents(),provider);
    // Fork (Onda 2): pedido do gancho vale como o diálogo (a conexão aberta prova que a CLI está viva),
    // e o turno dos ganchos vence a tela, que com statusline própria não mostra "esc to interrupt".
    let pedido = crate::ganchos::pedido(&meta.id);
    let em_turno = crate::ganchos::em_turno(&meta.id);
    let approve = pedido.is_some() || (available && dialog.is_some());
    let prompt = available && pedido.is_none() && em_turno != Some(true) && ready_no(&context,provider);
    let enfileirar = available && !prompt && pedido.is_none() && (pode_enfileirar_no(&context,provider)
        || (em_turno == Some(true) && pode_enfileirar_pelos_ganchos(context.parser.screen(),provider)));
    // Fork (pergunta-da-cli): menu genérico da CLI e, sem nada reconhecível, a espera — nunca silêncio.
    let escolha = (available && !approve && !prompt).then(|| menu_na_tela(context.parser.screen(), provider)).flatten();
    let espera = (available && !approve && !prompt && escolha.is_none() && pedido.is_none())
        .then(|| crate::pergunta_da_cli::espera(context.parser.screen(), provider, now_ms().saturating_sub(meta.last_activity_at_ms))).flatten();
    let reason = (!(approve || prompt || escolha.is_some() || espera.is_some())).then(|| match provider {
        // ponytail: diagnóstico detalhado enquanto a tela do agy não está calibrada em todos os casos;
        // volta para o texto genérico quando a Conversa do Antigravity estiver estável.
        "antigravity" => motivo_antigravity(context.parser.screen(), available, session.reserved.load(Ordering::SeqCst)),
        // Diz qual condição falhou (e grava no log, só metadados) — o texto genérico não dava
        // pista nenhuma de por que a conversa travava. O chat se destrava sozinho (`destravar`).
        _ => motivo_geral(context.parser.screen(), provider, available, session.reserved.load(Ordering::SeqCst), &meta.id, context.codex_visto),
    });
    // Fork (perguntas na Conversa): pedido do gancho tem cartão próprio; o diálogo na tela leva as opções todas.
    let approval_options = if available && pedido.is_none() && dialog.is_some() {
        crate::opcoes_da_permissao::opcoes(&context.parser.screen().contents(), provider)
    } else { Vec::new() };
    let approval_text = pedido.as_ref().map(|p| p.resumo.clone()).or_else(|| dialog.map(|d|d.2));
    Capabilities { prompt, approve, revision: context.revision(session), approval_text, reason, enfileirar, escolha, pedido, espera, approval_options }
}

/// Campo vazio no meio de um turno que só os ganchos enxergam (sem o rodapé "esc to interrupt").
fn pode_enfileirar_pelos_ganchos(screen: &vt100::Screen, provider: &str) -> bool {
    provider == "claude" && screen.bracketed_paste() && composer_vazio(screen) && approval(&screen.contents(),provider).is_none()
}

/// Por que o campo do Claude/Codex não foi reconhecido. Mesma ideia do `motivo_antigravity`.
fn motivo_geral(screen: &vt100::Screen, provider: &str, rodando: bool, reservada: bool, sessao: &str, codex_visto: bool) -> String {
    let texto = screen.contents();
    let mut faltas = Vec::new();
    if reservada { faltas.push("consulta de uso em andamento"); }
    if !rodando { faltas.push("o agente não está rodando no terminal"); }
    if trabalhando(&texto) { faltas.push("agente trabalhando ou com menu aberto"); }
    if approval(&texto,provider).is_some() { faltas.push("pedido de aprovação na tela"); }
    if provider == "codex" && !codex_visto && !texto.contains("OpenAI Codex") { faltas.push("Codex ainda não abriu na tela"); }
    if !screen.bracketed_paste() { faltas.push("modo de colar desligado"); }
    if !composer_vazio(screen) { faltas.push("campo de digitação com texto ou cursor fora dele"); }
    if faltas.is_empty() { faltas.push("tela não reconhecida"); }
    let motivo = faltas.join(" · ");
    // Fork (chat-estavel): forma do campo junto, quando é ele que falhou.
    let campo = if composer_vazio(screen) { String::new() } else { forma_do_campo(screen) };
    let mut campos = vec![("sessao", sessao), ("provider", provider), ("motivo", motivo.as_str())];
    if !campo.is_empty() { campos.push(("campo", &campo)); }
    omni_core::registro::aviso_sem_repetir(&format!("sem_entrada:{sessao}:{motivo}"), "conversa_sem_entrada", &campos);
    crate::harness_resumo::sem_entrada(sessao, &motivo); // Fork (harness, A7): conta o motivo uma vez por sessão.
    format!("Entrada não reconhecida: {motivo}")
}

/// Turno ou menu em andamento: interromper aqui mataria o trabalho.
pub fn trabalhando(texto: &str) -> bool {
    texto.contains("Esc to cancel") || texto.contains("esc to cancel") || texto.to_lowercase().contains("esc to interrupt")
}

/// Por que o campo do Antigravity não foi reconhecido, em palavras: qual das condições do `ready`
/// falhou. Sem isso a única pista era "não reconhecida", e a tela do terminal parecia normal.
fn motivo_antigravity(screen: &vt100::Screen, rodando: bool, reservada: bool) -> String {
    let texto = screen.contents();
    let mut faltas = Vec::new();
    if reservada { faltas.push("consulta de uso em andamento"); }
    if !rodando { faltas.push("processo agy não encontrado"); }
    if texto.contains("esc to cancel") { faltas.push("agente trabalhando ou com pergunta aberta"); }
    else if !texto.contains("? for shortcuts") { faltas.push("rodapé \"? for shortcuts\" fora da tela"); }
    if !screen.bracketed_paste() { faltas.push("modo de colar desligado"); }
    if !composer_vazio(screen) { faltas.push("campo de digitação vazio não encontrado no cursor"); }
    if faltas.is_empty() { "Antigravity: entrada não reconhecida".into() } else { format!("Antigravity: {}", faltas.join(" · ")) }
}

/// Uso da conta Claude. Fork (harness, decisão C5 do Enzo, 23/10/2026): a fonte é **só** a statusline do
/// Claude Code (`rate_limits`, gravada pelo `omni-shim` em `uso-statusline.json`). O app não lê mais o
/// `.claude.json` (o `cachedUsageUtilization`) nem digita `/usage` numa sessão escondida. A statusline só
/// traz os limites depois da primeira resposta da conta (doc da statusline): antes disso, sem número.
/// `refresh` não muda nada aqui (não há o que consultar); fica no protocolo para Codex e Antigravity.
pub fn account_usage(state: &Arc<EngineState>, profile_id: &str, refresh: bool) -> Result<EngineResponse> {
    let dir = state.state_file.parent().ok_or_else(|| anyhow!("engine dir unavailable"))?.to_path_buf();
    if let Some(perfil) = omni_core::profiles(&dir).into_iter().find(|p| p.id == profile_id && p.provider == "antigravity") {
        return uso_antigravity(state, &dir, &perfil, refresh);
    }
    Ok(EngineResponse::AccountUsage { usage: crate::ganchos::uso_da_statusline(&dir, profile_id).unwrap_or_else(|| {
        AccountUsage::unavailable(profile_id, "claude", "O uso aparece depois da primeira resposta do Claude nesta conta.")
    }) })
}

/// Uso da conta do Antigravity. A CLI não grava o uso em arquivo nenhum: só o painel do `/usage`
/// mostra. Então a leitura é a tela, e a última fica em `uso-antigravity-<conta>.json` no engine para
/// o rodapé abrir com número sem digitar nada (`refresh: false`).
/// - `refresh: true`: usa um agy ocioso da conta; sem nenhum, abre um **oculto e temporário** na pasta
///   do engine (confiada antes, senão ele para perguntando), consulta e encerra. `/usage` não manda
///   mensagem ao modelo e não cria conversa.
fn uso_antigravity(state: &Arc<EngineState>, dir: &Path, perfil: &omni_core::Profile, refresh: bool) -> Result<EngineResponse> {
    let arquivo = dir.join(format!("uso-antigravity-{}.json", perfil.id));
    let salvo = || std::fs::read(&arquivo).ok().and_then(|bytes| serde_json::from_slice::<AccountUsage>(&bytes).ok());
    let com_motivo = |uso: Option<AccountUsage>, motivo: &str| {
        let mut uso = uso.unwrap_or_else(|| AccountUsage::unavailable(&perfil.id, "antigravity", motivo));
        uso.reason = Some(motivo.into());
        Ok(EngineResponse::AccountUsage { usage: uso })
    };
    let antes = salvo();
    if !refresh {
        return match antes { Some(usage) => Ok(EngineResponse::AccountUsage { usage }), None => com_motivo(None, "Sem consulta de uso ainda. Clique em atualizar.") };
    }
    if let Some(usage) = antes.as_ref().filter(|u| u.observed_at_ms.is_some_and(|at| now_ms().saturating_sub(at) < 60_000)) {
        return Ok(EngineResponse::AccountUsage { usage: usage.clone() });
    }
    let ocioso = state.sessions.lock().map_err(|_|anyhow!("sessions poisoned"))?.values().filter_map(|entry| {
        let SessionEntry::Live(session) = entry else { return None };
        let meta = session.meta.lock().ok()?;
        (meta.provider.as_deref() == Some("antigravity")).then(|| session.clone())
    }).find(|session| capabilities(session).prompt);
    let lido = match ocioso {
        Some(session) => consultar_usage_antigravity(state, &session, &perfil.id)?,
        None => {
            let pasta = dir.to_string_lossy().into_owned();
            let _ = omni_core::antigravity::confiar(Path::new(&perfil.config_dir), &pasta);
            let id = state.next_id();
            let oculto = spawn_terminal_inner(state, id.clone(), String::new(), "Antigravity · consulta de uso".into(),
                pasta, None, Some("agy".into()), 40, 120, now_ms(),
                SessionOrigin { headless: true, env: omni_core::env_for(perfil), provider: Some("antigravity".into()),
                    profile_id: Some(perfil.id.clone()), conversation_id: None, external_session_id: None });
            let resultado = oculto.and_then(|_| {
                let session = match state.sessions.lock().map_err(|_|anyhow!("sessions poisoned"))?.get(&id) {
                    Some(SessionEntry::Live(session)) => session.clone(),
                    _ => return Ok(None),
                };
                let inicio = Instant::now();
                while inicio.elapsed() < Duration::from_secs(40) && !capabilities(&session).prompt {
                    if session.meta.lock().map(|meta| meta.pid.is_none()).unwrap_or(true) { return Ok(None); }
                    std::thread::sleep(Duration::from_millis(300));
                }
                consultar_usage_antigravity(state, &session, &perfil.id)
            });
            end_hidden(state, &id, "antigravity");
            resultado?
        }
    };
    match lido {
        Some(usage) => {
            if let Ok(json) = serde_json::to_vec(&usage) { let _ = std::fs::write(&arquivo, json); }
            Ok(EngineResponse::AccountUsage { usage })
        }
        None => com_motivo(antes, "O Antigravity não mostrou o uso a tempo; mostrando a última leitura."),
    }
}

/// Digita `/usage` num agy ocioso, lê o painel "Models & Quota" e fecha com Esc.
fn consultar_usage_antigravity(state: &Arc<EngineState>, session: &Arc<LiveSession>, profile_id: &str) -> Result<Option<AccountUsage>> {
    {
        let sessions = state.sessions.lock().map_err(|_|anyhow!("sessions poisoned"))?;
        if !sessions.values().any(|entry| matches!(entry,SessionEntry::Live(s) if Arc::ptr_eq(s,session))) { return Ok(None); }
        let mut context = session.interaction.lock().map_err(|_|anyhow!("screen poisoned"))?;
        if !ready(context.parser.screen(),"antigravity") || session.reserved.swap(true,Ordering::SeqCst) { return Ok(None); }
        context.input_revision += 1;
    }
    struct Release<'a>(&'a LiveSession);
    impl Drop for Release<'_> { fn drop(&mut self) { self.0.reserved.store(false,Ordering::SeqCst); } }
    let _release = Release(session);
    let write = |bytes: &[u8]| -> Result<()> {
        let mut writer = session.writer.lock().map_err(|_|anyhow!("writer poisoned"))?;
        writer.write_all(bytes)?; writer.flush()?; Ok(())
    };
    write(b"/usage")?;
    // Enter separado (mesmo motivo do Claude: na mesma rajada vira texto colado).
    std::thread::sleep(Duration::from_millis(300));
    write(b"\r")?;
    let inicio = Instant::now();
    let mut resultado = None;
    while inicio.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(300));
        let tela = session.interaction.lock().map_err(|_|anyhow!("screen poisoned"))?.parser.screen().contents();
        let uso = omni_core::usage::parse_antigravity_screen(&tela, profile_id, now_ms());
        if uso.status == "available" { resultado = Some(uso); break; }
    }
    write(b"\x1b")?;
    Ok(resultado)
}

/// Encerra a CLI oculta. Só matar a sessão não basta no Windows: o shell morre e o `.exe` neto fica
/// órfão (visto no teste com o Claude). `/exit` primeiro; se ele ainda estiver vivo, derruba a árvore.
fn end_hidden(state: &Arc<EngineState>, id: &str, provider: &str) {
    let session = match state.sessions.lock().ok().as_ref().and_then(|sessions| sessions.get(id)) {
        Some(SessionEntry::Live(session)) => Some(session.clone()),
        _ => None,
    };
    if let Some(session) = session {
        let write = |bytes: &[u8]| if let Ok(mut writer) = session.writer.lock() { let _ = writer.write_all(bytes); let _ = writer.flush(); };
        let pid = session.meta.lock().ok().and_then(|meta| meta.pid);
        write(b"\x1b");
        std::thread::sleep(Duration::from_millis(300));
        write(b"/exit");
        std::thread::sleep(Duration::from_millis(250));
        write(b"\r");
        let started = Instant::now();
        while provider_running(pid, provider) && started.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(200));
        }
        #[cfg(windows)]
        if let Some(pid) = pid.filter(|pid| provider_running(Some(*pid), provider)) {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()])
                .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                .creation_flags(0x0800_0000).status();
        }
    }
    let _ = close_session(state, id);
}

/// Espera o Claude recém-aberto chegar no composer vazio, passando pelo "confiar nesta pasta" (a
/// pasta é a do próprio engine; o Claude grava a confiança, então só acontece na primeira vez).
pub(crate) fn wait_claude_ready(session: &Arc<LiveSession>, limit: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(300));
        if session.meta.lock().map(|meta| meta.pid.is_none()).unwrap_or(true) { return false; }
        let text = session.interaction.lock().map(|c| c.parser.screen().contents()).unwrap_or_default();
        if let Some(keys) = trust_dialog_keys(&text) {
            if let Ok(mut writer) = session.writer.lock() { let _ = writer.write_all(keys.as_bytes()); let _ = writer.flush(); }
            continue;
        }
        if capabilities(session).prompt { return true; }
    }
    false
}

/// Teclas para o diálogo de confiança de pasta, ou `None` se ele não está na tela. Na 2.1.272 o
/// padrão selecionado é "No, exit" — um Enter às cegas **fecharia** o Claude. Então: seta até a opção
/// "Yes", e Enter só quando a tela já mostra o `❯` nela (a próxima volta do loop confere).
fn trust_dialog_keys(text: &str) -> Option<&'static str> {
    // Rótulo sem o `❯` e sem numeração ("1. Yes, proceed" nas versões antigas).
    let label = |line: &str| line.trim_start_matches('❯').trim_start().trim_start_matches(|c: char| c.is_ascii_digit() || c == '.').trim_start().to_owned();
    let options: Vec<&str> = text.lines().map(str::trim)
        .filter(|line| label(line).starts_with("Yes, ") || label(line).starts_with("No, exit"))
        .collect();
    let yes = options.iter().position(|line| label(line).starts_with("Yes, "))?;
    let selected = options.iter().position(|line| line.starts_with('❯'))?;
    if !options.iter().any(|line| label(line).starts_with("No, exit")) { return None; }
    Some(match selected.cmp(&yes) {
        std::cmp::Ordering::Equal => "\r",
        std::cmp::Ordering::Less => "\x1b[B",
        std::cmp::Ordering::Greater => "\x1b[A",
    })
}

/// `shift+tab`, que é o que o Claude Code liga a `chat:cycleMode`. **Cicla** entre os quatro modos
/// (default, aceitar edições, plano, automático), então chegar num modo específico é apertar e
/// conferir, nunca apertar uma vez.
const CICLAR_MODO: &[u8] = b"\x1b[Z";
/// Atalho alternativo do próprio CLI (`meta+m`), usado quando ele decide que o terminal não entrega
/// shift+tab. Só entra em cena se o rodapé não mudar com a tecla principal.
const CICLAR_MODO_ALTERNATIVO: &[u8] = b"\x1bm";
/// Quantas vezes vale apertar: o ciclo tem quatro estados, e uma volta inteira é o suficiente.
const VOLTAS_DO_CICLO: usize = 4;

/// Modo de permissão que o rodapé do Claude Code está mostrando.
///
/// O rodapé é a fonte **imediata**: o transcript também registra a troca, mas com atraso, e este
/// laço precisa saber se a tecla pegou antes de apertar de novo. Os rótulos vêm do próprio binário
/// (`default`, `accept edits on`, `plan mode on`, `auto mode on`); `default` não escreve nada no
/// rodapé, então a ausência das outras marcas é o que o identifica.
pub fn modo_atual(screen: &vt100::Screen) -> Option<&'static str> {
    let texto = screen.contents().to_lowercase();
    if texto.contains("plan mode on") {
        return Some("plan");
    }
    if texto.contains("auto mode on") {
        return Some("auto");
    }
    if texto.contains("accept edits on") {
        return Some("acceptEdits");
    }
    // Sem nenhuma marca não dá para afirmar que é `default`: pode ser uma tela em que o rodapé nem
    // está visível. Quem chama trata `None` como "não sei" e não como um modo.
    None
}

/// Modo de uma sessão viva, lido da tela dela. `None` quando o rodapé não está visível.
pub fn modo_da_sessao(session: &Arc<LiveSession>) -> Option<&'static str> {
    let context = session.interaction.lock().ok()?;
    modo_atual(context.parser.screen())
}

/// Alterna o modo de permissão até chegar em `alvo` (`plan` ou `auto`), como o shift+tab faz no
/// terminal.
///
/// Mesmo formato do `trust_dialog_keys`: manda tecla, relê a tela, repete. Falha honesta — se o
/// rodapé não reagir, devolve erro com motivo em vez de seguir apertando às cegas.
pub fn alternar_modo(state: &Arc<EngineState>, session: &Arc<LiveSession>, alvo: &str) -> Result<String> {
    let ler_modo = || -> Result<Option<&'static str>> {
        let context = session.interaction.lock().map_err(|_| anyhow!("screen poisoned"))?;
        Ok(modo_atual(context.parser.screen()))
    };
    let escrever = |bytes: &[u8]| -> Result<()> {
        let mut context = session.interaction.lock().map_err(|_| anyhow!("screen poisoned"))?;
        context.input_revision += 1;
        let mut writer = session.writer.lock().map_err(|_| anyhow!("writer poisoned"))?;
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
    };
    let viva = || -> Result<bool> {
        Ok(state
            .sessions
            .lock()
            .map_err(|_| anyhow!("sessions poisoned"))?
            .values()
            .any(|entry| matches!(entry, SessionEntry::Live(s) if Arc::ptr_eq(s, session))))
    };

    let inicial = ler_modo()?;
    if inicial == Some(alvo) {
        return Ok(alvo.to_string());
    }
    let mut tecla = CICLAR_MODO;
    let mut sem_reacao = 0;
    for _ in 0..VOLTAS_DO_CICLO {
        if !viva()? {
            return Err(anyhow!("A sessão não está mais aberta"));
        }
        let antes = ler_modo()?;
        escrever(tecla)?;
        // Uma volta do Ink leva alguns quadros; sem esta pausa a leitura seguinte ainda vê o modo
        // antigo e o laço apertaria de novo, passando do alvo.
        let limite = Instant::now() + Duration::from_millis(1_200);
        let mut agora = antes;
        while Instant::now() < limite {
            std::thread::sleep(Duration::from_millis(80));
            agora = ler_modo()?;
            if agora != antes {
                break;
            }
        }
        if agora == Some(alvo) {
            return Ok(alvo.to_string());
        }
        if agora == antes {
            sem_reacao += 1;
            // Duas teclas sem qualquer mudança no rodapé: provavelmente esta CLI está com o atalho
            // alternativo. Vale uma tentativa antes de desistir.
            if sem_reacao == 2 && tecla == CICLAR_MODO {
                tecla = CICLAR_MODO_ALTERNATIVO;
                continue;
            }
            if sem_reacao >= 3 {
                return Err(anyhow!("O agente não respondeu ao atalho de modo"));
            }
        }
    }
    Err(anyhow!("Não foi possível chegar no modo pedido"))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Pergunta do Antigravity 1.4.2 (`ask_question`), copiada da tela real.
    #[test] fn escolha_do_antigravity() {
        let tela = "> me pergunte alguma coisa pra eu escolher alguma opcao\n\n? O que você gostaria de fazer agora?\n\nQuestion\n────────────────────────────────────────\n\nQuestion 1/1: O que você gostaria de fazer agora?\n\n> 1. (Recommended) Criar um novo projeto do zero\n  2. Corrigir um bug ou melhorar um código existente\n  3. Aprender sobre uma tecnologia ou ferramenta nova\n  4. Automatizar alguma tarefa repetitiva\n  5. Write-in...\n\n  ↑/↓ Navigate · enter Select · esc Skip\nesc to cancel                                   Gemini 4 Argon · medium\n";
        let menu = escolha(tela).expect("menu do agy");
        assert_eq!(menu.pergunta, "O que você gostaria de fazer agora?");
        assert_eq!(menu.opcoes.iter().map(|o| o.numero).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
        assert_eq!(menu.opcoes[0].rotulo, "(Recommended) Criar um novo projeto do zero");
    }
    #[test] fn shell_com_filho_vivo_conta_como_ocupado() {
        let mut filho = if cfg!(windows) {
            std::process::Command::new("cmd").args(["/c", "ping -n 4 127.0.0.1 >nul"]).spawn()
        } else {
            std::process::Command::new("sleep").arg("3").spawn()
        }.expect("spawn");
        assert!(has_children(Some(std::process::id())));
        let _ = filho.kill(); let _ = filho.wait();
        assert!(!has_children(None));
    }
    #[test] fn reconstructs_split_ansi_and_checks_cursor_not_old_prompts() {
        let mut context = Interaction::new(10,80);
        context.parser.process(b"\x1b[?2004hClaude Code\r\n> ");
        assert!(ready(context.parser.screen(),"claude"));
        context.parser.process(b"draft"); assert!(!ready(context.parser.screen(),"claude"));
        context.parser.process(b"\x1b[2"); context.parser.process(b"J\x1b[Hshell> ");
        assert!(!ready(context.parser.screen(),"claude"));
    }
    /// O rodapé é a fonte imediata do modo — o transcript só registra depois. Rótulos tirados do
    /// binário do Claude Code: `plan mode on`, `auto mode on`, `accept edits on`.
    #[test] fn modo_sai_do_rodape_e_tela_sem_rodape_nao_chuta() {
        let tela = |rodape: &str| {
            let mut context = Interaction::new(10, 80);
            context.parser.process(format!("> \r\n{rodape}").as_bytes());
            context
        };
        assert_eq!(modo_atual(tela("⏸ plan mode on (shift+tab to cycle)").parser.screen()), Some("plan"));
        assert_eq!(
            modo_atual(tela("⏵⏵ auto mode on (shift+tab to cycle) · ← for agents").parser.screen()),
            Some("auto")
        );
        assert_eq!(modo_atual(tela("⏵ accept edits on").parser.screen()), Some("acceptEdits"));
        // Sem marca nenhuma não dá para afirmar o modo: pode ser só um rodapé fora da tela.
        assert_eq!(modo_atual(tela("nada aqui").parser.screen()), None);
    }

    /// Tela **real** do Claude Code ocioso, capturada de uma sessão aberta pelo celular (30×120).
    /// Os fixtures anteriores eram todos sintéticos; este é o que o engine realmente recebe.
    #[test] fn claude_ocioso_de_verdade_aceita_prompt() {
        let fixture = include_bytes!("../tests-fixtures/claude-composer-vazio.ansi");
        // O stream real da PTY usa CRLF. Em checkouts com `core.autocrlf=false` (Linux/macOS), o Git
        // normaliza este fixture textual para LF; o VT então mantém a coluna anterior a cada quebra
        // e o cursor termina no lugar errado. Normalize primeiro para LF para exercitar o mesmo
        // caminho em todos os sistemas, depois reponha o CR que a PTY entregou na captura real.
        let tela: Vec<_> = fixture.iter().copied().enumerate()
            .filter_map(|(index, byte)| (!(byte == b'\r' && fixture.get(index + 1) == Some(&b'\n'))).then_some(byte))
            .collect();
        let mut stream = Vec::with_capacity(tela.len() + 32);
        for byte in tela {
            if byte == b'\n' { stream.push(b'\r'); }
            stream.push(byte);
        }
        let mut context = Interaction::new(30,120);
        context.parser.process(&stream);
        assert!(ready(context.parser.screen(),"claude"));

        // Digitar de verdade na frente do cursor volta a bloquear.
        context.parser.process(b"abc");
        assert!(!ready(context.parser.screen(),"claude"));
    }

    /// Tela real do Claude Code 2.1.302 depois do primeiro turno: o menu travava o campo e a conversa
    /// só dizia "Entrada do CLI não reconhecida".
    #[test] fn menu_da_cli_vira_escolha_e_permissao_nao() {
        let borda = "─".repeat(40);
        let tela = format!("❯ responda apenas: ok\n● ok\n{borda}\n  Teach auto mode about your environment?\n\n  Auto mode works better when it knows your environment. Takes about a minute.\n\n  ❯ 1. Yes\n    2. Not now\n    3. Don't show again\n\n  Enter to confirm · Esc to cancel\n{borda}\n❯\n{borda}\n");
        let menu = escolha(&tela).expect("menu reconhecido");
        assert_eq!(menu.pergunta, "Teach auto mode about your environment?");
        assert_eq!(menu.detalhe.as_deref(), Some("Auto mode works better when it knows your environment. Takes about a minute."));
        assert_eq!(menu.opcoes.iter().map(|o| (o.numero, o.rotulo.as_str())).collect::<Vec<_>>(), vec![(1, "Yes"), (2, "Not now"), (3, "Don't show again")]);
        let permissao = format!("{borda}\n Bash command\n Do you want to proceed?\n ❯ 1. Yes\n   2. Yes, and don't ask again\n   3. No\n\n Esc to cancel · Enter to confirm\n");
        assert_eq!(escolha(&permissao), None);
        assert_eq!(escolha("● ok\n❯ \n"), None);
    }

    /// Pergunta do agente (AskUserQuestion, Claude Code 2.1.302): abas no topo, descrição embaixo de
    /// cada opção e uma borda antes de "Chat about this". Antes a conversa não mostrava nada.
    #[test] fn pergunta_do_agente_vira_escolha() {
        let borda = "─".repeat(40);
        let tela = format!("● Antes de começar, preciso de uma decisão.\n{borda}\n ←  ☐ Banco  ✔ Submit  →\n\nQual banco de dados usar?\n\n❯ 1. Postgres\n     Relacional, já usado no projeto\n  2. SQLite\n     Arquivo local, sem servidor\n  3. Type something.\n{borda}\n  4. Chat about this\n\nEnter to select · Tab/Arrow keys to navigate · Esc to cancel\n");
        let menu = escolha(&tela).expect("pergunta reconhecida");
        assert_eq!(menu.pergunta, "Qual banco de dados usar?");
        assert_eq!(menu.opcoes.iter().map(|o| (o.numero, o.rotulo.as_str())).collect::<Vec<_>>(), vec![(1, "Postgres"), (2, "SQLite"), (4, "Chat about this")]);
        // Borda da largura toda colada na linha de baixo, como sai do texto da tela.
        let colada = tela.replacen(&format!("{borda}\n ←"), &format!("{borda} ←"), 1);
        assert_eq!(escolha(&colada).expect("pergunta com borda colada").pergunta, "Qual banco de dados usar?");
        // Revisão no fim de duas perguntas: sem rodapé (tela real, 2.1.302).
        let revisao = format!("{borda}
←  ☒ Cor  ☒ Fruta  ✔ Submit  →

Review your answers

 ● Qual cor?
   → Azul
 ● Qual fruta?
   → Manga

Ready to submit your answers?

❯ 1. Submit answers
  2. Cancel
");
        let menu = escolha(&revisao).expect("revisão reconhecida");
        assert_eq!(menu.pergunta, "Review your answers");
        assert_eq!(menu.detalhe.as_deref(), Some("● Qual cor?\n→ Azul\n● Qual fruta?\n→ Manga\nReady to submit your answers?"));
        assert_eq!(menu.opcoes.iter().map(|o| o.rotulo.as_str()).collect::<Vec<_>>(), vec!["Submit answers", "Cancel"]);
        assert_eq!(menu.etapas, vec![Etapa { titulo: "Cor".into(), respondida: true }, Etapa { titulo: "Fruta".into(), respondida: true }]);
    }

    /// Marcar várias (multiSelect), tela real do 2.1.302 depois de marcar a 1 e a 3.
    #[test] fn pergunta_de_marcar_varias() {
        let borda = "─".repeat(40);
        let tela = format!("{borda}\n←  ☐ Frutas  ✔ Submit  →\n\nQuais frutas você gosta?\n\n❯ 1. [✔] Manga\n         Doce e suculenta\n  2. [ ] Uva\n         Pequena, em cachos\n  3. [✔] Banana\n         Prática e energética\n  4. [ ] Type something\n     Submit\n{borda}\n  5. Chat about this\n\nEnter to select · ↑/↓ to navigate · Esc to cancel\n");
        let menu = escolha(&tela).expect("marcar várias reconhecido");
        assert!(menu.multipla);
        assert!(menu.etapas.is_empty(), "uma pergunta só não mostra etapas");
        assert_eq!(menu.opcoes.iter().map(|o| (o.numero, o.rotulo.as_str(), o.marcada, o.descricao.as_deref())).collect::<Vec<_>>(), vec![
            (1, "Manga", Some(true), Some("Doce e suculenta")),
            (2, "Uva", Some(false), Some("Pequena, em cachos")),
            (3, "Banana", Some(true), Some("Prática e energética")),
            (5, "Chat about this", None, None),
        ]);
    }

    /// Print do usuário: depois de um turno, o prompt destacado (largura toda), a borda e a barra de
    /// abas chegam numa linha só. Sem achar a borda, a "pergunta" virava 20 linhas da conversa — e,
    /// com uma lista numerada antiga na janela, o menu sumia no clique ("Sessão ocupada…").
    #[test] fn pergunta_depois_de_um_turno_nao_pega_a_conversa() {
        let borda = "─".repeat(120);
        let tela = format!("● Testei três tipos:\n  1. Escolha única\n  2. Seleção múltipla\n\n✻ Baked for 11s · done 0:02\n\n❯ manda de novo a primeira pergunta{}{borda}←  ☐ Cor  ✔ Submit  →\n\nQual cor você mais gosta?\n\n❯ 1. Azul\n     Calma, céu e mar\n  2. Verde\n     Natureza e equilíbrio\n  3. Type something.\n{borda}\n  4. Chat about this\n\nEnter to select · ↑/↓ to navigate · Esc to cancel\n", " ".repeat(85));
        let menu = escolha(&tela).expect("pergunta reconhecida");
        assert_eq!(menu.pergunta, "Qual cor você mais gosta?");
        assert!(!menu.previa, "sem prévia o número já escolhe: Enter extra cairia na tela seguinte");
        assert_eq!(menu.detalhe, None);
        assert_eq!(menu.opcoes.iter().map(|o| (o.numero, o.rotulo.as_str(), o.descricao.as_deref())).collect::<Vec<_>>(), vec![
            (1, "Azul", Some("Calma, céu e mar")), (2, "Verde", Some("Natureza e equilíbrio")), (4, "Chat about this", None),
        ]);
    }

    /// Print do usuário: pergunta com prévia lado a lado. O desenho não é rótulo nem explicação, e o
    /// "Chat about this" sem número (depois da borda) não gruda na última opção.
    #[test] fn pergunta_com_previa_ignora_o_desenho() {
        let borda = "─".repeat(120);
        let tela = format!("{borda}\n☐ Layout\n\nQual layout de painel você prefere?\n\n❯ 1. Lado a lado          ┌────────────────────────────┐\n  2. Empilhado            │ +-----------+-----------+  │\n  3. Grade 2x2            │ | claude    | codex     |  │\n                          │ |  > _      |  > _      |  │\n                          │ +-----------+-----------+  │\n                          └────────────────────────────┘\n\n                          Notes: press n to add notes\n\n{borda}\n  Chat about this\n\nEnter to select · ↑/↓ to navigate · n to add notes · Esc to cancel\n");
        let menu = escolha(&tela).expect("pergunta com prévia reconhecida");
        assert_eq!(menu.pergunta, "Qual layout de painel você prefere?");
        assert!(menu.previa, "com prévia o número só move: precisa de Enter");
        assert_eq!(menu.opcoes.iter().map(|o| (o.numero, o.rotulo.as_str(), o.descricao.as_deref())).collect::<Vec<_>>(), vec![
            (1, "Lado a lado", None), (2, "Empilhado", None), (3, "Grade 2x2", None),
        ]);
    }

    #[test] fn le_os_tokens_do_turno_da_linha_de_status() {
        assert_eq!(tokens_do_turno("· Propagating… (5s · ↓ 113 tokens · thinking with low effort)").as_deref(), Some("113"));
        assert_eq!(tokens_do_turno("✻ Pensando… (1m 12s · ↑ 1.2k tokens · esc to interrupt)").as_deref(), Some("1.2k"));
        assert_eq!(tokens_do_turno("antigo (↓ 5 tokens)\n✢ Agora (3s · ↓ 40 tokens)").as_deref(), Some("40"));
        assert_eq!(tokens_do_turno("• Working (12s • esc to interrupt)"), None);
        assert_eq!(tokens_do_turno("fala sobre tokens em geral"), None);
    }

    /// Exemplo em cinza depois do cursor não é rascunho; o mesmo texto sem `dim` é.
    #[test] fn exemplo_em_cinza_nao_bloqueia_mas_texto_normal_bloqueia() {
        let mut cinza = Interaction::new(5,40);
        // Prompt, exemplo em `dim` (SGR 2), e o cursor volta para logo depois do símbolo.
        cinza.parser.process("[?2004h❯ [2mTry this[22m[1;3H".as_bytes());
        assert!(ready(cinza.parser.screen(),"claude"));

        let mut digitado = Interaction::new(5,40);
        digitado.parser.process("[?2004h❯ Try this[1;3H".as_bytes());
        assert!(!ready(digitado.parser.screen(),"claude"));
    }

    /// Tela real da 2.1.302 num agente recém-aberto: cada palavra do exemplo em `dim` separado, os
    /// espaços entre elas sem estilo e um NBSP depois do `❯`.
    #[test] fn exemplo_com_palavras_apagadas_em_separado_continua_vazio() {
        let mut real = Interaction::new(5,60);
        real.parser.process("\x1b[?2004h\u{276f}\u{a0}\x1b[2mTry\x1b[22m \x1b[2m\"fix\x1b[22m \x1b[2mlint\x1b[22m \x1b[2merrors\"\x1b[22m\x1b[K\x1b[1;3H".as_bytes());
        assert!(ready(real.parser.screen(),"claude"));

        let mut rascunho = Interaction::new(5,60);
        rascunho.parser.process("\x1b[?2004h\u{276f}\u{a0}fix lint\x1b[1;3H".as_bytes());
        assert!(!ready(rascunho.parser.screen(),"claude"));
    }

    #[test] fn trust_dialog_moves_to_yes_before_confirming() {
        // Tela real da 2.1.272: "No, exit" vem selecionado.
        let screen = " Quick safety check: Is this a project you created or one you trust?\n ❯ No, exit\n   Yes, I trust this folder\n Enter to confirm · Esc to cancel";
        assert_eq!(trust_dialog_keys(screen), Some("\x1b[B"));
        assert_eq!(trust_dialog_keys(&screen.replace("❯ No, exit\n   Yes", "  No, exit\n ❯ Yes")), Some("\r"));
        assert_eq!(trust_dialog_keys(" ❯ 1. Yes, proceed\n   2. No, exit"), Some("\r"));
        assert_eq!(trust_dialog_keys("❯ "), None);
        assert_eq!(trust_dialog_keys("Do you want to proceed?\n❯ 1. Yes\n3. No"), None);
    }
    #[test] fn unix_agent_process_must_descend_from_pty() {
        let ps = "  10     1 -zsh\n  20    10 node /Users/a/.npm/bin/claude --session-id x\n  30     1 /opt/homebrew/bin/codex";
        assert!(agent_descends_from(ps, 10, "claude"));
        assert!(!agent_descends_from(ps, 10, "codex")); // codex existe, mas fora desta PTY
        assert!(!agent_descends_from("  40    10 vim notes/claude.md", 10, "claude"));
        // Claude Code com título de conversa: o `args` vira o título; o `comm` do macOS (caminho
        // inteiro, com espaço no nome da pasta) é o que ainda diz quem é.
        let titulo = "  10     1 -zsh\n  20    10 ✳ Corrigir login\n  10     1 -zsh\n  20    10 /Users/Ana Lima/.local/bin/claude";
        assert!(agent_descends_from(titulo, 10, "claude"));
        assert!(!agent_descends_from("  20    10 ✳ Corrigir login", 10, "claude"));
    }
    #[test] fn never_approves_generic_output_or_persistent_choice() {
        assert!(approval("Allow? (y/n)","claude").is_none());
        assert!(approval("Do you want to proceed?\n1. Always allow\n3. No","claude").is_none());
        assert_eq!(approval("Do you want to proceed?\n❯ 1. Yes\n2. Yes, always\n3. No","claude").unwrap().0,"1\r");
    }
    /// Telas do Claude Code atual. O prompt anterior fica na tela com `❯`, a opção "No" vem
    /// com explicação, e editar/criar arquivo perguntam outra coisa.
    #[test] fn aprovacao_do_claude_atual() {
        let bash = "❯ roda os testes\n\n⏺ Bash(npm test)\n\n────────────────────────────────────────\n Bash command\n\n   npm test\n   Run the test suite\n\n Do you want to proceed?\n ❯ 1. Yes\n   2. Yes, and don't ask again for npm test commands in /Users/x/app\n   3. No, and tell Claude what to do differently (esc)\n";
        let (sim, nao, texto) = approval(bash, "claude").expect("pedido do Bash");
        assert_eq!((sim.as_str(), nao.as_str()), ("1\r", "3\r"));
        assert!(texto.starts_with("Bash command") && texto.contains("npm test") && !texto.contains("roda os testes"));

        let edicao = " Edit file\n src/App.tsx\n Do you want to make this edit to App.tsx?\n ❯ 1. Yes\n   2. Yes, allow all edits during this session (shift+tab)\n   3. No, and tell Claude what to do differently (esc)\n";
        assert_eq!(approval(edicao, "claude").unwrap().1, "3\r");

        let duas = "│ Do you want to allow Claude to fetch this content? │\n│ ❯ 1. Yes │\n│   2. No, and tell Claude what to do differently (esc) │";
        assert_eq!(approval(duas, "claude").unwrap().1, "2\r");

        // Opção 1 que não é "Yes" (sempre permitir) nunca vira o botão de aprovar.
        assert!(approval("Do you want to proceed?\n❯ 1. Yes, and don't ask again\n2. No", "claude").is_none());
        // Pergunta que ficou no histórico, sem opções embaixo, não conta.
        assert!(approval("Do you want to proceed?\n\n⏺ Feito.\n\n❯ ", "claude").is_none());
    }

    /// Fork (chat-estavel): o cabeçalho rola para fora depois de algumas trocas; o Codex continua pronto.
    /// Antes de ele aparecer (shell ainda subindo a CLI), não.
    #[test] fn codex_continua_pronto_depois_que_o_cabecalho_rola() {
        let mut tela = Interaction::new(5,60);
        tela.parser.process("\x1b[?2004h$ codex\r\n\u{203a} \x1b[5;3H".as_bytes());
        let texto = tela.parser.screen().contents(); notar_codex(&mut tela, &texto);
        assert!(!ready_no(&tela,"codex"), "sem o Codex na tela ainda");
        tela.parser.process("\x1b[2J\x1b[1;1H>_ OpenAI Codex (v0.163.0)\r\n\u{203a} \x1b[2;3H".as_bytes());
        let texto = tela.parser.screen().contents(); notar_codex(&mut tela, &texto);
        assert!(ready_no(&tela,"codex"));
        tela.parser.process("\x1b[2J\x1b[1;1H\u{2022} resposta longa\r\n\r\n\u{203a} \x1b[3;3H".as_bytes());
        let texto = tela.parser.screen().contents(); notar_codex(&mut tela, &texto);
        assert!(!tela.parser.screen().contents().contains("OpenAI Codex"));
        assert!(ready_no(&tela,"codex"), "cabeçalho fora da tela não trava mais");
        assert!(!ready(tela.parser.screen(),"codex"), "a leitura antiga, só da tela, travava");
    }
}
