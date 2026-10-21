//! Fork (trocar agente): passagem de bastão entre agentes **na mesma conversa**.
//!
//! Trocar de conta no mesmo provider é continuação literal (o transcript vai junto e a CLI retoma
//! pelo id). Entre providers não existe formato comum: o Codex não lê o `.jsonl` do Claude e vice-
//! versa. O que dá para fazer — e é o que isto faz — é escrever, a partir dos transcripts que as
//! próprias CLIs gravaram (de **todos** os trechos da conversa), um arquivo markdown com tudo o que o
//! agente novo precisa: objetivo, todas as mensagens do usuário na íntegra, as respostas anteriores
//! (inteiras ou cortadas quando enormes, com prioridade às recentes), decisões, arquivos alterados,
//! comandos com o resultado, tarefas em aberto e o estado do git. A CLI nova recebe só um pedido curto
//! para ler esse arquivo ([`prompt_curto`]).
//!
//! O arquivo fica em `<projeto>/.omni-agents/passagens/<conversa>-<n>.md`: dentro do projeto a CLI lê
//! sem pedir permissão de pasta (mesma razão dos anexos, `.omni-agents/anexos`), e `.omni-agents/`
//! entra no `.git/info/exclude` (como as worktrees), sem tocar no `.gitignore` do usuário.
//!
//! Limite honesto: o raciocínio interno do agente anterior (o que a CLI não grava ou grava cifrado),
//! o cache de prompt e as aprovações dadas na sessão não se transferem.

use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::conversations::{Conversation, Segment};
use crate::feed::{Item, Kind};
use crate::plano_do_agente::{EstadoDaConversa, EstadoDaTarefa, Tarefa};

/// Abre o pedido curto. Começa com `<` de propósito: as telas já escondem fala do usuário que começa
/// assim (`feed::injetado`), e [`sem_preambulo`] tira só a marca, deixando o que o usuário escreveu.
pub const MARCA: &str = "<passagem-de-bastao";
const FIM_DA_MARCA: &str = "</passagem-de-bastao>";
/// Pasta das passagens, dentro do projeto.
pub const PASTA: [&str; 2] = [".omni-agents", "passagens"];
/// Teto do arquivo (~20 mil tokens). Cabe com folga no contexto de qualquer CLI e ainda deixa espaço
/// para o trabalho; acima disso o corte começa pelas respostas antigas do agente.
pub const TETO_PADRAO: usize = 80_000;

pub fn nome_do_agente(provider: &str) -> String {
    match provider {
        "claude" => "Claude".into(),
        "codex" => "Codex".into(),
        "cursor" => "Cursor".into(),
        "antigravity" => "Antigravity".into(),
        outro => outro.to_owned(),
    }
}

/// "Codex · conta Trabalho" (ou só "Codex" sem conta conhecida).
pub fn rotulo(provider: &str, conta: Option<&str>) -> String {
    match conta.filter(|c| !c.trim().is_empty()) {
        Some(conta) => format!("{} · conta {conta}", nome_do_agente(provider)),
        None => nome_do_agente(provider),
    }
}

/// Um trecho da conversa e o arquivo que a CLI dele gravou.
#[derive(Debug, Clone)]
pub struct Fonte {
    pub provider: String,
    pub rotulo: String,
    /// `None`: a CLI não grava transcript legível (Cursor) ou o arquivo não foi achado.
    pub caminho: Option<PathBuf>,
}

/// As fontes da conversa na ordem dos trechos, com o nome da conta de cada um.
pub fn fontes(conversation: &Conversation, perfis: &[crate::Profile]) -> Vec<Fonte> {
    conversation.segments.iter().map(|segmento| {
        let corrigido = crate::pasta_do_claude::trecho_com_transcript_atual(segmento, conversation, perfis);
        let segmento: &Segment = corrigido.as_ref().unwrap_or(segmento);
        let conta = segmento.profile_id.as_deref().and_then(|id| perfis.iter().find(|p| p.id == id)).map(|p| p.name.as_str());
        let caminho = crate::conversations::resolve_transcript(segmento, conversation, perfis).filter(|p| p.is_file());
        Fonte { provider: segmento.provider.clone(), rotulo: rotulo(&segmento.provider, conta), caminho }
    }).collect()
}

/// Branch, worktree e mudanças da pasta da conversa, para o agente novo saber onde está pisando.
#[derive(Debug, Clone, Default)]
pub struct EstadoGit {
    pub raiz: String,
    pub branch: String,
    /// A pasta é uma worktree (não o checkout principal).
    pub worktree: bool,
    /// Linhas do `git status --porcelain` (sem `.omni-agents/`).
    pub status: Vec<String>,
    /// `git log --oneline` mais recentes.
    pub commits: Vec<String>,
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let saida = Command::new("git").args(args).current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0").env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
        .output().ok()?;
    saida.status.success().then(|| String::from_utf8_lossy(&saida.stdout).trim_end().to_owned())
}

/// `None` fora de um repositório git (ou sem `git` no PATH).
pub fn estado_git(cwd: &Path) -> Option<EstadoGit> {
    let raiz = git(cwd, &["rev-parse", "--show-toplevel"])?;
    let branch = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let dir = git(cwd, &["rev-parse", "--path-format=absolute", "--git-dir"]);
    let comum = git(cwd, &["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    let status = git(cwd, &["status", "--porcelain=v1"]).unwrap_or_default().lines()
        .filter(|l| !l.contains(".omni-agents/")).map(str::to_owned).collect();
    let commits = git(cwd, &["log", "--oneline", "-8"]).unwrap_or_default().lines().map(str::to_owned).collect();
    Some(EstadoGit { raiz, branch, worktree: dir.is_some() && dir != comum, status, commits })
}

/// O que gerar.
pub struct Pedido<'a> {
    /// Nome do app ("Medza Agents"), para o cabeçalho.
    pub produto: &'a str,
    pub conversa_id: &'a str,
    pub titulo: &'a str,
    pub cwd: &'a str,
    /// Rótulo de quem continua ("Codex · conta Trabalho").
    pub para: &'a str,
    pub fontes: &'a [Fonte],
    pub git: Option<&'a EstadoGit>,
    /// Data e hora legíveis da geração.
    pub gerado_em: &'a str,
    pub teto: usize,
}

/* ── Leitura dos transcripts ──────────────────────────────────────────────────────────────── */

#[derive(Debug, Clone)]
struct Resultado {
    ok: Option<bool>,
    resumo: String,
}

#[derive(Debug, Clone)]
enum Evento {
    Fala { usuario: bool, texto: String, quando: Option<String> },
    Acao { ferramenta: String, detalhe: String, arquivos: Vec<String>, comando: Option<String>, resultado: Option<Resultado> },
    Aviso(String),
    Comando(String),
    /// Começo de um trecho (o agente da conversa mudou ou a conta mudou).
    Trecho { indice: usize, rotulo: String, sem_transcript: bool },
}

#[derive(Default)]
struct Leitura {
    eventos: Vec<Evento>,
    tarefas: Option<Vec<Tarefa>>,
    plano: Option<String>,
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3))
}

/// Ferramentas que a conversa não mostra (`feed::ferramenta_claude`): ficam fora do pareamento por id.
const OCULTAS_CLAUDE: [&str; 4] = ["ToolSearch", "EnterPlanMode", "TaskList", "TaskGet"];
const EDICAO: [&str; 4] = ["Edit", "MultiEdit", "Write", "NotebookEdit"];

fn arquivos_do_patch(texto: &str) -> Vec<String> {
    texto.lines().filter_map(|linha| ["*** Update File: ", "*** Add File: ", "*** Delete File: ", "*** Move to: "].iter()
        .find_map(|p| linha.trim_start().strip_prefix(p)).map(|c| c.trim().to_owned())).collect()
}

fn texto_do_bloco(conteudo: &Value) -> String {
    if let Some(texto) = conteudo.as_str() { return texto.to_owned(); }
    conteudo.as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n")
}

/// Resultado de um comando em uma linha: deu certo? e as últimas linhas da saída.
fn resumir_saida(bruta: &str, erro: Option<bool>) -> Resultado {
    let (texto, mut codigo) = match serde_json::from_str::<Value>(bruta) {
        Ok(v) if v.is_object() => (v["output"].as_str().unwrap_or("").to_owned(), v["metadata"]["exit_code"].as_i64()),
        _ => (bruta.to_owned(), None),
    };
    let mut uteis = Vec::new();
    for linha in texto.lines().map(str::trim) {
        let numero = |prefixo: &str| linha.strip_prefix(prefixo).and_then(|r| r.trim().trim_end_matches('.').parse::<i64>().ok());
        if let Some(n) = numero("Exit code:").or_else(|| numero("Process exited with code")) { codigo = codigo.or(Some(n)); continue; }
        if linha.is_empty() || ["Wall time:", "Chunk ID:", "Original token count:", "Output:", "Total output lines:"].iter().any(|p| linha.starts_with(p)) { continue; }
        uteis.push(linha);
    }
    let fim: Vec<String> = uteis.iter().rev().take(2).rev().map(|l| cortar(l, 160)).collect();
    let ok = match (erro, codigo) { (Some(true), _) => Some(false), (_, Some(c)) => Some(c == 0), (Some(false), None) => Some(true), _ => None };
    Resultado { ok, resumo: fim.join(" ⏎ ") }
}

fn ler_jsonl(fonte: &Fonte, caminho: &Path, vistos: &mut HashSet<String>, leitura: &mut Leitura) {
    let Ok(arquivo) = File::open(caminho) else { return };
    let mut estado = EstadoDaConversa::default();
    let mut por_id: HashMap<String, usize> = HashMap::new();
    let provider = fonte.provider.as_str();
    for linha in BufReader::new(arquivo).lines().map_while(Result::ok) {
        let Ok(valor) = serde_json::from_str::<Value>(&linha) else { continue };
        let identidade = valor["uuid"].as_str().map(str::to_owned).unwrap_or_else(|| format!("{:016x}", fnv(linha.as_bytes())));
        // Troca de conta copia o transcript inteiro: a mesma linha aparece em dois trechos.
        if !vistos.insert(format!("{provider}:{identidade}")) { continue; }
        // Resultados de ferramenta (não viram item na conversa, mas dizem se o comando deu certo).
        match provider {
            "claude" if valor["type"] == "user" => {
                for bloco in valor["message"]["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "tool_result") {
                    let Some(&k) = bloco["tool_use_id"].as_str().and_then(|id| por_id.get(id)) else { continue };
                    if let Evento::Acao { resultado, .. } = &mut leitura.eventos[k] {
                        *resultado = Some(resumir_saida(&texto_do_bloco(&bloco["content"]), bloco["is_error"].as_bool()));
                    }
                }
            }
            "codex" if valor["type"] == "response_item" && matches!(valor["payload"]["type"].as_str(), Some("function_call_output" | "custom_tool_call_output")) => {
                let payload = &valor["payload"];
                if let Some(&k) = payload["call_id"].as_str().and_then(|id| por_id.get(id)) {
                    let saida = payload["output"].as_str().map(str::to_owned).unwrap_or_else(|| texto_do_bloco(&payload["output"]));
                    if let Evento::Acao { resultado, .. } = &mut leitura.eventos[k] { *resultado = Some(resumir_saida(&saida, None)); }
                }
                continue;
            }
            _ => {}
        }
        estado.observar(&valor, provider);
        let mut itens = crate::feed::items_em(&valor, provider, None);
        for item in itens.iter_mut() {
            if let Some(resumo) = estado.aplicar(&mut item.extras) { item.text = resumo; }
            if let Some(plano) = &item.extras.plano { leitura.plano = Some(plano.clone()); }
        }
        // Ids das chamadas, na ordem dos itens de ferramenta da mesma linha.
        let chamadas: Vec<(Option<String>, Vec<String>, Option<String>)> = match provider {
            "claude" => valor["message"]["content"].as_array().into_iter().flatten()
                .filter(|b| b["type"] == "tool_use" && !OCULTAS_CLAUDE.contains(&b["name"].as_str().unwrap_or("")))
                .map(|b| {
                    let nome = b["name"].as_str().unwrap_or("");
                    let arquivos = if EDICAO.contains(&nome) {
                        b["input"]["file_path"].as_str().or_else(|| b["input"]["notebook_path"].as_str()).map(str::to_owned).into_iter().collect()
                    } else { Vec::new() };
                    let comando = matches!(nome, "Bash" | "PowerShell").then(|| b["input"]["command"].as_str().map(|c| cortar(c.trim(), 400))).flatten();
                    (b["id"].as_str().map(str::to_owned), arquivos, comando)
                }).collect(),
            "codex" if valor["type"] == "response_item" => {
                let p = &valor["payload"];
                let entrada = p["input"].as_str().unwrap_or("");
                let arquivos = arquivos_do_patch(entrada).into_iter()
                    .chain(serde_json::from_str::<Value>(p["arguments"].as_str().unwrap_or("{}")).ok()
                        .and_then(|a| a["input"].as_str().map(arquivos_do_patch)).unwrap_or_default()).collect();
                vec![(p["call_id"].as_str().map(str::to_owned), arquivos, None)]
            }
            _ => Vec::new(),
        };
        let ferramentas = itens.iter().filter(|i| i.kind == Kind::Tool).count();
        let mut chamadas = if chamadas.len() == ferramentas { chamadas.into_iter() } else { Vec::new().into_iter() };
        for item in itens {
            // O `FileChange` do Codex repete o patch que acabou de aparecer.
            if item.kind == Kind::Tool && valor["type"] == "event_msg"
                && matches!(leitura.eventos.last(), Some(Evento::Acao { ferramenta, .. }) if ferramenta == "Edit") { continue; }
            let evento = evento_do_item(item, chamadas.next(), &mut por_id, leitura.eventos.len());
            if let Some(evento) = evento { leitura.eventos.push(evento); }
        }
    }
    if let Some(tarefas) = estado.tarefas() { leitura.tarefas = Some(tarefas); }
}

fn evento_do_item(item: Item, chamada: Option<(Option<String>, Vec<String>, Option<String>)>, por_id: &mut HashMap<String, usize>, posicao: usize) -> Option<Evento> {
    Some(match item.kind {
        Kind::Text if item.text.trim().is_empty() => return None,
        Kind::Text => Evento::Fala { usuario: item.role == "user", texto: item.text, quando: item.timestamp },
        Kind::Command => Evento::Comando(item.text),
        Kind::Notice => Evento::Aviso(item.text),
        Kind::Tool => {
            let ferramenta = item.tool.unwrap_or_default();
            let (id, mut arquivos, comando) = chamada.unwrap_or_default();
            if let Some(id) = id { por_id.insert(id, posicao); }
            if arquivos.is_empty() && EDICAO.contains(&ferramenta.as_str()) && !item.text.is_empty() {
                arquivos = item.text.trim_end_matches('…').split(", ").map(str::to_owned).collect();
            }
            let comando = comando.or_else(|| matches!(ferramenta.as_str(), "Shell" | "Bash" | "PowerShell").then(|| item.text.clone()));
            Evento::Acao { ferramenta, detalhe: item.text, arquivos, comando, resultado: None }
        }
    })
}

fn ler(fontes: &[Fonte]) -> Leitura {
    let mut leitura = Leitura::default();
    let mut vistos = HashSet::new();
    let mut anterior: Option<&str> = None;
    for (indice, fonte) in fontes.iter().enumerate() {
        // Mesmo agente e conta seguidos (retomada) não abrem trecho novo na linha do tempo.
        if anterior != Some(fonte.rotulo.as_str()) {
            leitura.eventos.push(Evento::Trecho { indice, rotulo: fonte.rotulo.clone(), sem_transcript: fonte.caminho.is_none() });
        }
        anterior = Some(&fonte.rotulo);
        let Some(caminho) = &fonte.caminho else { continue };
        if fonte.provider == "antigravity" {
            for entrada in crate::antigravity::itens(caminho).unwrap_or_default() {
                if !vistos.insert(entrada.identity.clone()) { continue; }
                let mut por_id = HashMap::new();
                if let Some(e) = evento_do_item(entrada.item, None, &mut por_id, 0) { leitura.eventos.push(e); }
            }
        } else {
            ler_jsonl(fonte, caminho, &mut vistos, &mut leitura);
        }
    }
    leitura
}

/* ── Montagem do markdown ─────────────────────────────────────────────────────────────────── */

fn cortar(texto: &str, max: usize) -> String {
    match texto.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", texto[..i].trim_end()),
        None => texto.to_owned(),
    }
}

/// Corta no meio do texto (começo e fim ficam): a conclusão de uma resposta longa costuma estar no fim.
fn cortar_no_meio(texto: &str, max: usize) -> String {
    let total = texto.chars().count();
    if total <= max { return texto.to_owned(); }
    let metade = max / 2;
    let inicio: String = texto.chars().take(metade).collect();
    let fim: String = texto.chars().skip(total - metade).collect();
    format!("{}\n\n[… {} caracteres omitidos …]\n\n{}", inicio.trim_end(), total - max, fim.trim_start())
}

/// Quanto cortar. Nível 0 é tudo; cada nível aperta mais as partes antigas.
#[derive(Clone, Copy)]
struct Nivel {
    /// Respostas do agente nos turnos recentes / antigos.
    agente_recente: usize,
    agente_antigo: usize,
    /// Mensagens do usuário nos turnos recentes / antigos.
    usuario_recente: usize,
    usuario_antigo: usize,
    /// Quantos turnos (a partir do fim) contam como recentes.
    recentes: usize,
    /// Ações listadas uma a uma por turno recente; nos antigos vira contagem.
    acoes_por_turno: usize,
    comandos: usize,
    lidos: usize,
}

const NIVEIS: [Nivel; 5] = [
    Nivel { agente_recente: 12_000, agente_antigo: 12_000, usuario_recente: 40_000, usuario_antigo: 40_000, recentes: usize::MAX, acoes_por_turno: 60, comandos: 80, lidos: 60 },
    Nivel { agente_recente: 6_000, agente_antigo: 1_500, usuario_recente: 20_000, usuario_antigo: 8_000, recentes: 6, acoes_por_turno: 30, comandos: 40, lidos: 30 },
    Nivel { agente_recente: 4_000, agente_antigo: 400, usuario_recente: 12_000, usuario_antigo: 4_000, recentes: 4, acoes_por_turno: 20, comandos: 30, lidos: 15 },
    Nivel { agente_recente: 3_000, agente_antigo: 150, usuario_recente: 8_000, usuario_antigo: 1_500, recentes: 3, acoes_por_turno: 12, comandos: 20, lidos: 0 },
    Nivel { agente_recente: 2_000, agente_antigo: 0, usuario_recente: 6_000, usuario_antigo: 600, recentes: 2, acoes_por_turno: 8, comandos: 12, lidos: 0 },
];

fn linha_de_acao(ferramenta: &str, detalhe: &str, resultado: Option<&Resultado>) -> String {
    let mut linha = if detalhe.is_empty() { format!("- {ferramenta}") } else { format!("- {ferramenta}: `{}`", cortar(&detalhe.replace('`', "'"), 200)) };
    if let Some(r) = resultado {
        let estado = match r.ok { Some(true) => "ok", Some(false) => "falhou", None => "terminou" };
        linha += &if r.resumo.is_empty() { format!(" → {estado}") } else { format!(" → {estado}: {}", r.resumo) };
    }
    linha
}

/// Frases que registram uma escolha. Heurística (pt/en), por isso o arquivo avisa que é automático.
fn decisoes(eventos: &[Evento]) -> Vec<String> {
    const CHAVES: [&str; 22] = ["decid", "optei", "optamos", "vamos usar", "vou usar", "escolhi", "escolhemos", "combinado",
        "não vamos", "nao vamos", "em vez de", "ao invés de", "prefiro", "preferi", "fica assim", "decided", "we'll use", "i'll use",
        "instead of", "going with", "chose", "let's use"];
    let mut achadas: Vec<String> = Vec::new();
    for evento in eventos {
        let Evento::Fala { usuario, texto, .. } = evento else { continue };
        for frase in texto.split(['\n', '.', ';']).map(str::trim).filter(|f| f.len() > 12 && !f.starts_with("```")) {
            let minuscula = frase.to_lowercase();
            if CHAVES.iter().any(|c| minuscula.contains(c)) {
                let frase = format!("{}{}", if *usuario { "(usuário) " } else { "" }, cortar(frase.trim_start_matches(['-', '*', '#', ' ']), 220));
                if !achadas.contains(&frase) { achadas.push(frase); }
            }
        }
    }
    let pular = achadas.len().saturating_sub(15);
    achadas.into_iter().skip(pular).collect()
}

fn montar(pedido: &Pedido, leitura: &Leitura, nivel: Nivel) -> String {
    let eventos = &leitura.eventos;
    let falas_do_usuario: Vec<&str> = eventos.iter().filter_map(|e| match e { Evento::Fala { usuario: true, texto, .. } => Some(texto.as_str()), _ => None }).collect();
    let trechos: Vec<&str> = eventos.iter().filter_map(|e| match e { Evento::Trecho { rotulo, .. } => Some(rotulo.as_str()), _ => None }).collect();
    let mut md = String::new();
    md += &format!("# Passagem de bastão — {}\n\n", pedido.titulo);
    md += &format!("> Gerado pelo {} em {} para continuar esta conversa com o **{}**. Fonte: os transcripts que as \
        próprias CLIs gravaram ({} trecho(s)). Saída longa de ferramenta foi resumida; o raciocínio interno do agente \
        anterior não é gravado pelas CLIs e não está aqui.\n\n", pedido.produto, pedido.gerado_em, pedido.para, pedido.fontes.len());

    md += "## Resumo\n\n";
    md += &format!("- Pasta do projeto: `{}`\n", pedido.cwd);
    md += &format!("- Conversa: `{}` · {} mensagem(ns) do usuário\n", pedido.conversa_id, falas_do_usuario.len());
    md += &format!("- Agentes até aqui: {}\n", if trechos.is_empty() { "—".to_owned() } else { trechos.join(" → ") });
    md += &format!("- Agora continua com: **{}**\n\n", pedido.para);

    md += "## Objetivo da conversa\n\n";
    match falas_do_usuario.first() {
        Some(primeira) => md += &format!("Primeiro pedido do usuário:\n\n{}\n\n", citar(&cortar_no_meio(primeira, nivel.usuario_recente))),
        None => md += "_(ainda não há mensagens do usuário)_\n\n",
    }

    // Onde parou: o último pedido e tudo que o agente respondeu depois dele.
    md += "## Onde parou\n\n";
    let ultimo_usuario = eventos.iter().rposition(|e| matches!(e, Evento::Fala { usuario: true, .. }));
    if let Some(k) = ultimo_usuario {
        if falas_do_usuario.len() > 1 {
            if let Evento::Fala { texto, .. } = &eventos[k] { md += &format!("Último pedido do usuário:\n\n{}\n\n", citar(&cortar_no_meio(texto, nivel.usuario_recente))); }
        }
        let resposta: Vec<&str> = eventos[k + 1..].iter().filter_map(|e| match e { Evento::Fala { usuario: false, texto, .. } => Some(texto.as_str()), _ => None }).collect();
        if resposta.is_empty() {
            md += "O agente anterior **não respondeu** a esse pedido (ou parou no meio): comece por ele.\n\n";
        } else {
            md += &format!("Última resposta do agente anterior:\n\n{}\n\n", citar(&cortar_no_meio(&resposta.join("\n\n"), nivel.agente_recente.max(3_000))));
        }
    } else {
        md += "_(nada ainda)_\n\n";
    }

    if let Some(tarefas) = leitura.tarefas.as_ref().filter(|t| !t.is_empty()) {
        md += "## Tarefas (a lista do agente como ficou)\n\n";
        for tarefa in tarefas.iter().take(40) {
            let marca = match tarefa.estado { EstadoDaTarefa::Feita => "[x]", EstadoDaTarefa::Andamento => "[~] (em andamento)", EstadoDaTarefa::Pendente => "[ ]" };
            md += &format!("- {marca} {}\n", cortar(&tarefa.texto, 200));
        }
        md += "\n";
    }
    if let Some(plano) = &leitura.plano {
        md += &format!("## Plano aprovado (plan mode)\n\n{}\n\n", cortar_no_meio(plano, nivel.agente_recente.max(2_000)));
    }

    let decididas = decisoes(eventos);
    if !decididas.is_empty() {
        md += "## Decisões e combinados\n\n_Extraídos automaticamente das falas (frases com \"decidi\", \"em vez de\", \"vamos usar\"…); na dúvida, vale a linha do tempo._\n\n";
        for d in decididas { md += &format!("- {d}\n"); }
        md += "\n";
    }

    // Arquivos: alterados (com quantas vezes) e lidos.
    let mut alterados: Vec<(String, usize)> = Vec::new();
    let mut lidos: Vec<String> = Vec::new();
    let mut comandos: Vec<(&str, Option<&Resultado>)> = Vec::new();
    for evento in eventos {
        let Evento::Acao { ferramenta, detalhe, arquivos, comando, resultado } = evento else { continue };
        for arquivo in arquivos.iter().filter(|a| !a.is_empty()) {
            match alterados.iter_mut().find(|(a, _)| a == arquivo) { Some((_, n)) => *n += 1, None => alterados.push((arquivo.clone(), 1)) }
        }
        if ferramenta == "Read" && !detalhe.is_empty() && !lidos.contains(detalhe) { lidos.push(detalhe.clone()); }
        if let Some(c) = comando { comandos.push((c.as_str(), resultado.as_ref())); }
    }
    md += "## Arquivos alterados\n\n";
    if alterados.is_empty() { md += "_(nenhuma edição registrada nos transcripts)_\n\n"; } else {
        for (arquivo, vezes) in &alterados { md += &format!("- `{arquivo}`{}\n", if *vezes > 1 { format!(" ({vezes}×)") } else { String::new() }); }
        md += "\n";
    }
    if nivel.lidos > 0 && !lidos.is_empty() {
        md += "## Arquivos lidos\n\n";
        let pular = lidos.len().saturating_sub(nivel.lidos);
        if pular > 0 { md += &format!("_({pular} leituras mais antigas omitidas)_\n"); }
        for arquivo in lidos.iter().skip(pular) { md += &format!("- `{arquivo}`\n"); }
        md += "\n";
    }
    md += "## Comandos rodados\n\n";
    if comandos.is_empty() { md += "_(nenhum)_\n\n"; } else {
        let pular = comandos.len().saturating_sub(nivel.comandos);
        if pular > 0 { md += &format!("_({pular} comandos mais antigos omitidos; os mais recentes abaixo)_\n"); }
        for (comando, resultado) in comandos.iter().skip(pular) { md += &linha_de_acao("", comando, *resultado).replacen("- : ", "- ", 1); md += "\n"; }
        md += "\n";
    }

    md += "## Git\n\n";
    match pedido.git {
        Some(git) => {
            md += &format!("- Branch: `{}`{}\n- Repositório: `{}`\n", git.branch, if git.worktree { " (worktree)" } else { "" }, git.raiz);
            if git.status.is_empty() { md += "- Sem mudanças pendentes.\n"; } else {
                md += &format!("- Mudanças não commitadas ({}):\n\n```\n{}\n```\n", git.status.len(), git.status.iter().take(50).cloned().collect::<Vec<_>>().join("\n"));
            }
            if !git.commits.is_empty() { md += &format!("\nÚltimos commits:\n\n```\n{}\n```\n", git.commits.join("\n")); }
            md += "\n";
        }
        None => md += "_(a pasta não é um repositório git)_\n\n",
    }

    md += "## Como continuar\n\n";
    md += &format!("- Para o usuário isto é **a mesma conversa**: continue no mesmo idioma e tom, sem se reapresentar e sem repetir o resumo.\n\
        - Não refaça o que já foi feito: os arquivos estão no disco como o agente anterior deixou (confira com `git status`/`git diff` se precisar).\n\
        - O que não estiver aqui (detalhe de uma saída, um trecho de código discutido) está nos transcripts originais abaixo; leia só se precisar.\n\
        - Se algo continuar ambíguo, pergunte ao usuário em vez de supor.\n\n");
    let originais: Vec<String> = pedido.fontes.iter().filter_map(|f| f.caminho.as_ref().map(|c| format!("- {}: `{}`", f.rotulo, c.display()))).collect();
    if !originais.is_empty() { md += &format!("Transcripts originais:\n\n{}\n\n", originais.join("\n")); }

    md += "## Linha do tempo\n\n";
    md += &linha_do_tempo(eventos, nivel);
    md
}

fn citar(texto: &str) -> String {
    texto.lines().map(|l| if l.is_empty() { ">".to_owned() } else { format!("> {l}") }).collect::<Vec<_>>().join("\n")
}

/// Turnos: cada mensagem do usuário abre um. Os recentes saem inteiros; os antigos, cortados.
fn linha_do_tempo(eventos: &[Evento], nivel: Nivel) -> String {
    let inicios: Vec<usize> = eventos.iter().enumerate().filter(|(_, e)| matches!(e, Evento::Fala { usuario: true, .. })).map(|(i, _)| i).collect();
    let total_turnos = inicios.len();
    let turno_de = |posicao: usize| inicios.iter().rposition(|&i| i <= posicao);
    let recente = |turno: Option<usize>| match turno { Some(t) => total_turnos - t <= nivel.recentes, None => total_turnos <= nivel.recentes };
    let mut md = String::new();
    let mut acoes: Vec<String> = Vec::new();
    let mut contagem: HashMap<String, usize> = HashMap::new();
    let mut omitidas = 0;
    let mut agente = "Agente".to_owned();
    let despejar = |md: &mut String, acoes: &mut Vec<String>, contagem: &mut HashMap<String, usize>, omitidas: &mut usize| {
        if !acoes.is_empty() {
            *md += &format!("{}\n", acoes.join("\n"));
            if *omitidas > 0 { *md += &format!("- _(+{} ações)_\n", omitidas); }
            *md += "\n";
        } else if !contagem.is_empty() {
            let mut partes: Vec<_> = contagem.iter().map(|(f, n)| format!("{n}× {f}")).collect();
            partes.sort();
            *md += &format!("_Ações: {}_\n\n", partes.join(", "));
        }
        acoes.clear(); contagem.clear(); *omitidas = 0;
    };
    for (posicao, evento) in eventos.iter().enumerate() {
        let turno = turno_de(posicao);
        let e_recente = recente(turno);
        match evento {
            Evento::Trecho { indice, rotulo, sem_transcript } => {
                despejar(&mut md, &mut acoes, &mut contagem, &mut omitidas);
                md += &format!("### Trecho {} — {}\n\n", indice + 1, rotulo);
                agente = rotulo.split(" · ").next().unwrap_or("Agente").to_owned();
                if *sem_transcript { md += "_(esta CLI não gravou transcript legível; o que aconteceu neste trecho não está aqui)_\n\n"; }
            }
            Evento::Fala { usuario: true, texto, quando } => {
                despejar(&mut md, &mut acoes, &mut contagem, &mut omitidas);
                let limite = if e_recente { nivel.usuario_recente } else { nivel.usuario_antigo };
                md += &format!("**Usuário**{}:\n\n{}\n\n", quando.as_deref().map(|q| format!(" ({})", hora_curta(q))).unwrap_or_default(), cortar_no_meio(texto, limite));
            }
            Evento::Fala { usuario: false, texto, .. } => {
                despejar(&mut md, &mut acoes, &mut contagem, &mut omitidas);
                let limite = if e_recente { nivel.agente_recente } else { nivel.agente_antigo };
                if limite == 0 { md += &format!("**{agente}:** _(resposta antiga omitida para caber)_\n\n"); } else { md += &format!("**{agente}:**\n\n{}\n\n", cortar_no_meio(texto, limite)); }
            }
            Evento::Acao { ferramenta, detalhe, resultado, .. } => {
                if e_recente {
                    if acoes.len() < nivel.acoes_por_turno { acoes.push(linha_de_acao(ferramenta, detalhe, resultado.as_ref())); } else { omitidas += 1; }
                } else {
                    *contagem.entry(ferramenta.clone()).or_default() += 1;
                }
            }
            Evento::Comando(texto) => { despejar(&mut md, &mut acoes, &mut contagem, &mut omitidas); md += &format!("_Usuário rodou `{}`_\n\n", cortar(texto, 120)); }
            Evento::Aviso(texto) => { despejar(&mut md, &mut acoes, &mut contagem, &mut omitidas); md += &format!("_{}_\n\n", texto); }
        }
    }
    despejar(&mut md, &mut acoes, &mut contagem, &mut omitidas);
    md
}

fn hora_curta(iso: &str) -> String {
    // Sem fuso local no core (chrono sem `clock`): UTC, dito no texto.
    chrono::DateTime::parse_from_rfc3339(iso).map(|d| d.with_timezone(&chrono::Utc).format("%d/%m %H:%M UTC").to_string()).unwrap_or_else(|_| iso.to_owned())
}

/// O markdown da passagem, dentro do teto: aperta nível a nível e, no limite, corta o começo da
/// linha do tempo (o objetivo e o "onde parou" ficam no topo de qualquer jeito).
pub fn gerar(pedido: &Pedido) -> String {
    let leitura = ler(pedido.fontes);
    let mut ultimo = String::new();
    for nivel in NIVEIS {
        ultimo = montar(pedido, &leitura, nivel);
        if ultimo.chars().count() <= pedido.teto { return ultimo; }
    }
    let Some(inicio) = ultimo.find("## Linha do tempo\n\n") else { return cortar(&ultimo, pedido.teto) };
    let (topo, tempo) = ultimo.split_at(inicio + "## Linha do tempo\n\n".len());
    let sobra = pedido.teto.saturating_sub(topo.chars().count() + 120);
    let total = tempo.chars().count();
    let fim: String = tempo.chars().skip(total.saturating_sub(sobra)).collect();
    let fim = fim.find("\n\n").map(|i| &fim[i + 2..]).unwrap_or(&fim);
    format!("{topo}_[… começo da linha do tempo cortado para caber; o primeiro pedido está em \"Objetivo\" e os transcripts originais estão listados acima …]_\n\n{fim}")
}

/* ── Arquivo e pedido curto ───────────────────────────────────────────────────────────────── */

fn id_seguro(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// `.omni-agents/passagens/<conversa>-<n>.md`, com `/` (vai no prompt igual em qualquer sistema).
pub fn caminho_relativo(conversa_id: &str, n: usize) -> String {
    format!("{}/{}/{conversa_id}-{n}.md", PASTA[0], PASTA[1])
}

/// Grava a passagem na pasta do projeto e tira `.omni-agents/` do `git status` (`.git/info/exclude`).
pub fn gravar(cwd: &Path, conversa_id: &str, n: usize, corpo: &str) -> std::io::Result<PathBuf> {
    if !id_seguro(conversa_id) { return Err(std::io::Error::other("Conversa inválida")); }
    let pasta = cwd.join(PASTA[0]).join(PASTA[1]);
    std::fs::create_dir_all(&pasta)?;
    let caminho = pasta.join(format!("{conversa_id}-{n}.md"));
    std::fs::write(&caminho, corpo)?;
    excluir_do_git(cwd);
    Ok(caminho)
}

fn excluir_do_git(cwd: &Path) {
    let Some(comum) = git(cwd, &["rev-parse", "--path-format=absolute", "--git-common-dir"]) else { return };
    let arquivo = Path::new(&comum).join("info").join("exclude");
    let atual = std::fs::read_to_string(&arquivo).unwrap_or_default();
    if atual.lines().any(|l| matches!(l.trim(), ".omni-agents/" | ".omni-agents" | "/.omni-agents/" | "/.omni-agents")) { return; }
    let _ = std::fs::create_dir_all(Path::new(&comum).join("info"));
    let _ = std::fs::write(&arquivo, format!("{atual}{}.omni-agents/\n", if atual.is_empty() || atual.ends_with('\n') { "" } else { "\n" }));
}

/// A primeira mensagem da CLI nova. Curta: o conteúdo está no arquivo. A próxima mensagem do usuário,
/// se ele já tinha escrito, vai junto **depois** da marca (e só ela aparece na conversa).
pub fn prompt_curto(de: &str, para: &str, relativo: &str, mensagem: Option<&str>) -> String {
    let depois = match mensagem.map(str::trim).filter(|m| !m.is_empty()) {
        Some(_) => "Depois de ler, atenda o pedido do usuário que vem logo abaixo.",
        None => "Depois de ler, responda em uma ou duas frases dizendo onde a conversa parou e espere o próximo pedido do usuário.",
    };
    let mut texto = format!("{MARCA} arquivo=\"{relativo}\">\nVocê está continuando esta conversa, que vinha com o {de}; o usuário trocou para você ({para}) e quer seguir \
        como se fosse o mesmo lugar. Antes de responder, leia o arquivo {relativo} (na pasta do projeto): ele tem o objetivo, todas as mensagens \
        do usuário, as respostas anteriores, os arquivos alterados, os comandos com o resultado, as tarefas em aberto e o estado do git. \
        Não refaça o que já foi feito; continue de onde parou. {depois}\n{FIM_DA_MARCA}");
    if let Some(m) = mensagem.map(str::trim).filter(|m| !m.is_empty()) { texto += &format!("\n\n{m}"); }
    texto
}

/// Tira a marca da passagem de uma fala do usuário: sobra só o que ele escreveu (ou nada).
pub fn sem_preambulo(texto: &str) -> String {
    let inicio = texto.trim_start();
    if !inicio.starts_with(MARCA) { return texto.to_owned(); }
    match inicio.find(FIM_DA_MARCA) {
        Some(fim) => inicio[fim + FIM_DA_MARCA.len()..].trim().to_owned(),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "passagem_testes.rs"]
mod testes;
