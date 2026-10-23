//! Fork (Onda 2, itens 1–3): ganchos HTTP do Claude Code por sessão — a parte pura.
//!
//! O engine sobe um servidor próprio em `127.0.0.1:<porta do engine + 4>` e, a cada PTY de uma
//! conversa Claude, grava um arquivo de configuração **sem segredo** (`<engine>/ganchos/<terminal>.json`)
//! que a linha digitada passa com `--settings '<arquivo>'`. O token por PTY vai só no ambiente
//! (`MEDZA_GANCHO_TOKEN`); o Claude o põe no `Authorization` porque o nome da variável está em
//! `allowedEnvVars`. Doc oficial (code.claude.com/docs/en/hooks, conferida em 2026-10-20): `http`
//! existe desde a 2.1.63 (CHANGELOG), `StopFailure` desde a 2.1.78, `rate_limits` da statusline desde
//! a 2.1.80; 2xx vazio = sem decisão; não-2xx, falha de conexão e tempo esgotado não bloqueiam.
//!
//! Aqui: montar o JSON, pôr/tirar a flag do comando (idempotente), ler **só** as chaves que importam
//! das configurações do usuário (nunca o `.claude.json`), interpretar os eventos com tolerância e a
//! medição da statusline. Nada daqui faz rede nem guarda conteúdo de conversa.

use crate::usage::{AccountUsage, UsageWindow};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const VARIAVEL_DO_TOKEN: &str = "MEDZA_GANCHO_TOKEN";
/// Comando da statusline do usuário, resolvido pelo engine e posto no ambiente da PTY (nunca digitado).
pub const VARIAVEL_DA_STATUSLINE_DO_USUARIO: &str = "MEDZA_STATUSLINE_DO_USUARIO";
/// Primeiro argumento que põe o `omni-shim` no modo statusline.
pub const ARGUMENTO_DA_STATUSLINE: &str = "--medza-statusline";
pub const DESLOCAMENTO_DA_PORTA: u16 = 4;
/// Ganchos `type: "http"` entraram na 2.1.63 (CHANGELOG oficial: "Added HTTP hooks…").
pub const VERSAO_MINIMA_GANCHOS_HTTP: (u32, u32, u32) = (2, 1, 63);
/// Cada evento que só observa: um engine travado não pode segurar a CLI pelos 600 s padrão.
pub const TIMEOUT_DOS_EVENTOS_S: u64 = 5;
/// `SessionEnd` tem 1,5 s no total para todos os ganchos.
pub const TIMEOUT_DO_FIM_S: u64 = 2;
/// Prazos que a tela oferece para segurar um pedido de permissão (2, 5 e 10 min).
pub const ESPERAS_PERMITIDAS_S: [u64; 3] = [120, 300, 600];
pub const ESPERA_PADRAO_S: u64 = 300;
/// O engine solta o pedido (resposta vazia) este tanto antes do prazo: a CLI mostra o diálogo dela.
pub const FOLGA_ANTES_DO_PRAZO_S: u64 = 5;
pub const ARQUIVO_DE_AJUSTES: &str = "ganchos.json";
pub const PASTA_DOS_ARQUIVOS: &str = "ganchos";
pub const LIMITE_DO_CORPO: usize = 1024 * 1024;
pub const STATUS_DA_ESPERA: &str = "Aguardando sua resposta no Niski…";

/// Eventos que só observam (sem `PostToolUse`: o `tool_response` do Read/Write traz o arquivo inteiro).
/// `SessionStart` fica de fora: só aceita `command` e `mcp_tool` (doc de hooks, conferida em 23/10/2026).
pub const EVENTOS_OBSERVADOS: [&str; 15] = [
    "UserPromptSubmit", "PreToolUse", "PostToolUseFailure", "Notification", "Stop", "StopFailure",
    "SubagentStart", "SubagentStop", "TaskCreated", "TaskCompleted", "PreCompact", "PostCompact",
    "PostModelSwitch", "CwdChanged", "SessionEnd",
];
/// Ferramentas que a tela continua respondendo (cartão de pergunta/plano): o pedido é solto na hora.
pub const FERRAMENTAS_DA_TELA: [&str; 2] = ["AskUserQuestion", "ExitPlanMode"];

pub fn porta_dos_ganchos(porta_do_engine: u16) -> u16 { porta_do_engine.wrapping_add(DESLOCAMENTO_DA_PORTA) }
pub fn url_dos_eventos(porta: u16) -> String { format!("http://127.0.0.1:{porta}/ganchos") }
pub fn url_da_permissao(porta: u16) -> String { format!("http://127.0.0.1:{porta}/ganchos/permissao") }

/* ── Ajustes (Configurações → Agentes) ───────────────────────────────────────────────────────── */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ajustes {
    /// Fork (harness, A1): ganchos **só de observação** (início/fim de turno, aviso, erro, limite). Ligado
    /// por padrão: não decidem nada (2xx vazio = sem decisão; falha e prazo não bloqueiam, doc de hooks).
    /// Arquivo antigo, sem o campo, vira ligado. Desligado: nenhuma sessão nova ganha ganchos.
    #[serde(default = "verdadeiro")]
    pub eventos: bool,
    /// Segurar o pedido de permissão (`PermissionRequest`) para responder pelo app/celular/orquestrador.
    /// Fork: desligado por padrão até a rodada com a CLI real confirmar o diálogo de permissão com o
    /// gancho pendente (desenho-ondas-2-3.md, seção 6). Sem ele, a CLI mostra o diálogo dela, como antes.
    /// O nome continua `ligado` para o arquivo antigo (`{"ligado": true}`) seguir valendo igual.
    #[serde(default)]
    pub ligado: bool,
    #[serde(default = "espera_padrao")]
    pub espera_s: u64,
    /// Contas (ids de perfil) que ligaram "Ler uso pela statusline" quando ela era opcional. Mantida para o
    /// arquivo antigo e para uma versão anterior do app que abra este arquivo; quem decide agora é
    /// [`Ajustes::statusline_ligada`].
    #[serde(default)]
    pub statusline: Vec<String>,
    /// Fork (harness, decisão C5): a statusline é a única fonte do uso da conta Claude, então vem ligada
    /// em toda conta; aqui ficam só as contas que o usuário desligou. Arquivo antigo: lista vazia.
    #[serde(default)]
    pub statusline_desligada: Vec<String>,
}
fn espera_padrao() -> u64 { ESPERA_PADRAO_S }
fn verdadeiro() -> bool { true }
impl Default for Ajustes {
    fn default() -> Self { Self { eventos: true, ligado: false, espera_s: ESPERA_PADRAO_S, statusline: Vec::new(), statusline_desligada: Vec::new() } }
}
impl Ajustes {
    /// A conta lê o uso pela statusline (padrão ligado; desligada só se o usuário desligou).
    pub fn statusline_ligada(&self, perfil: &str) -> bool { !self.statusline_desligada.iter().any(|c| c == perfil) }
    /// Liga ou desliga a statusline da conta (as duas listas andam juntas).
    pub fn definir_statusline(&mut self, perfil: &str, ligada: bool) {
        self.statusline.retain(|c| c != perfil);
        self.statusline_desligada.retain(|c| c != perfil);
        if ligada { self.statusline.push(perfil.to_owned()) } else { self.statusline_desligada.push(perfil.to_owned()) }
    }
    /// Prazo válido (um dos três da tela); qualquer outro vira o padrão.
    pub fn espera(&self) -> u64 {
        if ESPERAS_PERMITIDAS_S.contains(&self.espera_s) { self.espera_s } else { ESPERA_PADRAO_S }
    }
}

pub fn ler_ajustes(dir: &Path) -> Ajustes {
    std::fs::read(dir.join(ARQUIVO_DE_AJUSTES)).ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

pub fn gravar_ajustes(dir: &Path, ajustes: &Ajustes) -> std::io::Result<()> {
    let destino = dir.join(ARQUIVO_DE_AJUSTES);
    let temporario = dir.join(format!("{ARQUIVO_DE_AJUSTES}.tmp"));
    std::fs::write(&temporario, serde_json::to_vec_pretty(ajustes).map_err(std::io::Error::other)?)?;
    std::fs::rename(temporario, destino)
}

/* ── O que as configurações do usuário já dizem ──────────────────────────────────────────────── */

/// Os arquivos de configuração que valem para uma sessão (sem o `.claude.json`, que nunca é lido).
#[derive(Clone, Debug)]
pub struct Arquivos { pub gerenciado: PathBuf, pub local: PathBuf, pub projeto: PathBuf, pub usuario: PathBuf }

impl Arquivos {
    pub fn de(pasta_da_conta: &Path, cwd: &Path) -> Self {
        Self {
            gerenciado: gerenciado_padrao(),
            local: cwd.join(".claude").join("settings.local.json"),
            projeto: cwd.join(".claude").join("settings.json"),
            usuario: pasta_da_conta.join("settings.json"),
        }
    }
}

/// Onde o administrador da máquina põe a política (doc de settings).
pub fn gerenciado_padrao() -> PathBuf {
    if cfg!(target_os = "macos") { PathBuf::from("/Library/Application Support/ClaudeCode/managed-settings.json") }
    else if cfg!(windows) { PathBuf::from(r"C:\Program Files\ClaudeCode\managed-settings.json") }
    else { PathBuf::from("/etc/claude-code/managed-settings.json") }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigDoUsuario {
    /// `disableAllHooks: true` em algum arquivo: não injetamos (e nunca sobrescrevemos com `false`).
    pub ganchos_desligados: bool,
    /// A política só deixa rodar os ganchos dela (`allowManagedHooksOnly`): os nossos nem chegariam.
    pub so_ganchos_gerenciados: bool,
    /// Algum arquivo já define `allowedHttpHookUrls`/`httpHookAllowedEnvVars`: aí (e só aí) entramos
    /// na lista. Definir sem ninguém ter definido **restringiria** os ganchos http do usuário.
    pub limita_urls: bool,
    pub limita_variaveis: bool,
    /// A `statusLine` que valeria sem nós (local > projeto > usuário).
    pub statusline: Option<Value>,
    /// A política define `statusLine`: ela vence o `--settings`, então a nossa não rodaria.
    pub statusline_gerenciada: bool,
}

/// Lê só as chaves que importam. Arquivo ausente ou inválido não conta.
pub fn ler_config_do_usuario(arquivos: &Arquivos) -> ConfigDoUsuario {
    let ler = |caminho: &Path| -> Option<Value> {
        let valor: Value = serde_json::from_slice(&std::fs::read(caminho).ok()?).ok()?;
        valor.is_object().then_some(valor)
    };
    let gerenciado = ler(&arquivos.gerenciado);
    let outros = [ler(&arquivos.local), ler(&arquivos.projeto), ler(&arquivos.usuario)];
    let todos: Vec<&Value> = gerenciado.iter().chain(outros.iter().flatten()).collect();
    let tem = |chave: &str| todos.iter().any(|v| v.get(chave).is_some());
    ConfigDoUsuario {
        ganchos_desligados: todos.iter().any(|v| v.get("disableAllHooks") == Some(&Value::Bool(true))),
        so_ganchos_gerenciados: gerenciado.as_ref().is_some_and(|v| v.get("allowManagedHooksOnly") == Some(&Value::Bool(true))),
        limita_urls: tem("allowedHttpHookUrls"),
        limita_variaveis: tem("httpHookAllowedEnvVars"),
        statusline: outros.iter().flatten().find_map(|v| v.get("statusLine").filter(|s| s.is_object()).cloned()),
        statusline_gerenciada: gerenciado.as_ref().is_some_and(|v| v.get("statusLine").is_some()),
    }
}

/// O comando da statusline do usuário, se for do tipo `command`.
pub fn statusline_do_usuario(config: &ConfigDoUsuario) -> Option<String> {
    let linha = config.statusline.as_ref()?;
    (linha.get("type").and_then(Value::as_str).unwrap_or("command") == "command")
        .then(|| linha.get("command").and_then(Value::as_str).map(str::to_owned)).flatten()
        .filter(|c| !c.trim().is_empty())
}

/* ── O arquivo de configuração da sessão ─────────────────────────────────────────────────────── */

/// Comando da nossa statusline: o `omni-shim` ao lado do engine, com a porta (não é segredo).
/// `None` se o caminho tiver aspas que quebrariam a linha.
pub fn comando_da_statusline(shim: &str, porta: u16) -> Option<String> {
    if cfg!(windows) {
        (!shim.contains('"')).then(|| format!("\"{shim}\" {ARGUMENTO_DA_STATUSLINE} {porta}"))
    } else {
        (!shim.contains('\'')).then(|| format!("'{shim}' {ARGUMENTO_DA_STATUSLINE} {porta}"))
    }
}

/// O JSON do `--settings` da sessão. Sem token: só URL e o **nome** da variável.
/// `espera_s`: `Some` só com "responder permissões pelo app" ligado; `None` = sem `PermissionRequest`
/// (a CLI mostra o diálogo dela, como sem ganchos).
pub fn montar_settings(porta: u16, espera_s: Option<u64>, config: &ConfigDoUsuario, statusline: Option<&str>) -> Value {
    let gancho = |url: String, timeout: u64| json!({
        "type": "http", "url": url, "timeout": timeout,
        "headers": {"Authorization": format!("Bearer ${{{VARIAVEL_DO_TOKEN}}}")},
        "allowedEnvVars": [VARIAVEL_DO_TOKEN],
    });
    let mut hooks = serde_json::Map::new();
    for evento in EVENTOS_OBSERVADOS {
        let timeout = if evento == "SessionEnd" { TIMEOUT_DO_FIM_S } else { TIMEOUT_DOS_EVENTOS_S };
        let mut grupo = json!({"hooks": [gancho(url_dos_eventos(porta), timeout)]});
        if matches!(evento, "PreToolUse" | "PostToolUseFailure") { grupo["matcher"] = json!("*"); }
        hooks.insert(evento.to_owned(), json!([grupo]));
    }
    if let Some(espera_s) = espera_s {
        let mut permissao = gancho(url_da_permissao(porta), espera_s + 10);
        permissao["statusMessage"] = json!(STATUS_DA_ESPERA);
        hooks.insert("PermissionRequest".into(), json!([{"matcher": "*", "hooks": [permissao]}]));
    }
    let mut settings = json!({"hooks": hooks});
    if config.limita_urls { settings["allowedHttpHookUrls"] = json!([format!("http://127.0.0.1:{porta}/*")]); }
    if config.limita_variaveis { settings["httpHookAllowedEnvVars"] = json!([VARIAVEL_DO_TOKEN]); }
    if let Some(comando) = statusline {
        let mut linha = json!({"type": "command", "command": comando});
        // O resto da aparência é o do usuário (não pomos `refreshInterval` nosso: custo de CPU).
        if let Some(dele) = config.statusline.as_ref() {
            for chave in ["padding", "refreshInterval", "hideVimModeIndicator"] {
                if let Some(valor) = dele.get(chave) { linha[chave] = valor.clone(); }
            }
        }
        settings["statusLine"] = linha;
    }
    settings
}

const FLAG: &str = " --settings '";

/// O comando sem o nosso `--settings` (só o que aponta para a pasta `ganchos`).
pub fn sem_settings(comando: &str) -> String {
    let mut saida = comando.to_owned();
    let mut desde = 0;
    while let Some(relativo) = saida[desde..].find(FLAG) {
        let inicio = desde + relativo;
        let corpo = inicio + FLAG.len();
        let Some(fim) = saida[corpo..].find('\'') else { break };
        let caminho = &saida[corpo..corpo + fim];
        let nosso = caminho.ends_with(".json")
            && Path::new(caminho).parent().and_then(Path::file_name).is_some_and(|n| n == PASTA_DOS_ARQUIVOS);
        if nosso { saida.replace_range(inicio..corpo + fim + 1, ""); } else { desde = corpo + fim + 1; }
    }
    saida
}

/// O comando com o `--settings` da sessão no fim. Idempotente. `None` se o caminho tiver `'`.
pub fn com_settings(comando: &str, arquivo: &Path) -> Option<String> {
    let caminho = arquivo.to_string_lossy();
    if caminho.contains('\'') { return None; }
    Some(format!("{}{FLAG}{caminho}'", sem_settings(comando)))
}

/* ── Versão da CLI ───────────────────────────────────────────────────────────────────────────── */

/// `2.1.304 (Claude Code)` → `(2, 1, 304)`.
pub fn versao_de(saida: &str) -> Option<(u32, u32, u32)> {
    let palavra = saida.split_whitespace().find(|p| p.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut partes = palavra.split('.').map(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u32>());
    Some((partes.next()?.ok()?, partes.next()?.ok()?, partes.next().unwrap_or(Ok(0)).ok()?))
}

pub fn suporta_ganchos_http(versao: (u32, u32, u32)) -> bool { versao >= VERSAO_MINIMA_GANCHOS_HTTP }

/* ── Eventos ─────────────────────────────────────────────────────────────────────────────────── */

/// Um evento de gancho, só com o que o engine usa. Campos de tipo inesperado viram `None`.
/// `tool_input` não fica guardado: vira o `resumo` (texto curto, só em memória).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Evento {
    pub nome: String,
    pub session_id: Option<String>,
    pub transcript_path: Option<String>,
    pub permission_mode: Option<String>,
    pub esforco: Option<String>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
    pub ferramenta: Option<String>,
    /// Ferramenta e o essencial da entrada ("Bash · npm test"), até `max` caracteres.
    pub resumo: Option<String>,
    pub servidor_mcp: Option<String>,
    pub notification_type: Option<String>,
    pub erro: Option<String>,
    pub gatilho: Option<String>,
}

fn texto(valor: &Value, chave: &str) -> Option<String> {
    valor.get(chave).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned)
}

impl Evento {
    /// `max`: tamanho do resumo da ferramenta (200 no `PreToolUse`; maior no pedido de permissão, que
    /// o usuário precisa ler inteiro para aprovar).
    pub fn de(valor: &Value, max: usize) -> Option<Self> {
        let nome = texto(valor, "hook_event_name")?;
        let ferramenta = texto(valor, "tool_name");
        let resumo = ferramenta.as_deref().map(|f| resumo_da_ferramenta(f, valor.get("tool_input").unwrap_or(&Value::Null), max));
        Some(Self {
            nome,
            session_id: texto(valor, "session_id"),
            transcript_path: texto(valor, "transcript_path"),
            permission_mode: texto(valor, "permission_mode"),
            esforco: valor.pointer("/effort/level").and_then(Value::as_str).map(str::to_owned),
            agent_id: texto(valor, "agent_id"),
            agent_type: texto(valor, "agent_type"),
            ferramenta,
            resumo,
            servidor_mcp: valor.get("mcp_server").and_then(|m| m.as_str().map(str::to_owned).or_else(|| texto(m, "name"))),
            notification_type: texto(valor, "notification_type"),
            erro: valor.get("error").and_then(|e| e.as_str().map(str::to_owned).or_else(|| texto(e, "type"))),
            gatilho: texto(valor, "trigger"),
        })
    }
}

/// "Bash · npm test", "Edit · src/a.ts"… O campo que diz o que a ferramenta vai fazer.
pub fn resumo_da_ferramenta(ferramenta: &str, entrada: &Value, max: usize) -> String {
    let campo = ["command", "file_path", "notebook_path", "path", "url", "pattern", "query", "description", "prompt"]
        .iter().find_map(|chave| texto(entrada, chave))
        .or_else(|| entrada.as_object().and_then(|o| o.values().find_map(|v| v.as_str().map(str::to_owned))));
    let completo = match campo {
        Some(c) => format!("{ferramenta} · {}", c.trim()),
        None => ferramenta.to_owned(),
    };
    cortar(&completo, max)
}

/// Fork (fix-orquestracao, achado 5): a linha da CLI do Claude com a sessão `sid` no lugar da que ela
/// tinha (`--session-id`, `--resume`/`-r <id>`, `--fork-session` saem; `--session-id <sid>` entra onde
/// estava o primeiro, ou no fim). O reinício (`comando_ao_reiniciar`) troca para `--resume <sid>` quando o
/// transcript dela já existe. Separa só por espaço simples, então o resto da linha volta igual.
pub fn comando_com_sessao(comando: &str, sid: &str) -> String {
    let partes: Vec<&str> = comando.split(' ').collect();
    let (mut saida, mut posicao, mut i) = (Vec::with_capacity(partes.len() + 2), None, 0);
    while i < partes.len() {
        let parte = partes[i];
        let com_valor = matches!(parte, "--session-id" | "--resume" | "-r")
            && partes.get(i + 1).is_some_and(|v| !v.is_empty() && !v.starts_with('-'));
        let sozinho = parte == "--fork-session" || parte.starts_with("--session-id=") || parte.starts_with("--resume=");
        if com_valor || sozinho {
            posicao.get_or_insert(saida.len());
            i += if com_valor { 2 } else { 1 };
            continue;
        }
        saida.push(parte.to_owned());
        i += 1;
    }
    let novo = format!("--session-id {sid}");
    match posicao { Some(p) => saida.insert(p, novo), None => saida.push(novo) }
    saida.join(" ")
}

pub fn cortar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max { return texto.to_owned(); }
    let mut saida: String = texto.chars().take(max.saturating_sub(1)).collect();
    saida.push('…');
    saida
}

/// Fork (para a frente "Atividade", 2B): o tipo de aviso que um evento de gancho implica, no mesmo
/// vocabulário da caixa de Atividade. `None` = o evento não pede aviso.
/// - `precisa_de_voce`: pedido de permissão, ou `Notification` de espera por você;
/// - `terminou`: `Stop`;
/// - `limite` / `erro`: `StopFailure` com `rate_limit` / qualquer outro erro.
pub fn tipo(evento: &str, notification_type: Option<&str>, erro: Option<&str>) -> Option<&'static str> {
    match evento {
        "PermissionRequest" => Some("precisa_de_voce"),
        "Notification" => matches!(notification_type, Some("permission_prompt" | "idle_prompt" | "agent_needs_input"
            | "elicitation_dialog" | "elicitation_url_dialog")).then_some("precisa_de_voce"),
        "Stop" => Some("terminou"),
        "StopFailure" => Some(if erro == Some("rate_limit") { "limite" } else { "erro" }),
        _ => None,
    }
}

/// O `notice` da sessão (o mesmo vocabulário da leitura de tela) para um `StopFailure`.
pub fn aviso_da_falha(erro: Option<&str>) -> &'static str {
    if erro == Some("rate_limit") { "usage_limit" } else { "api_error" }
}

/* ── Statusline ──────────────────────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Janela { pub pct: f64, pub reset_s: Option<i64> }

/// O que a statusline traz, só números e nomes (nunca o texto que ela imprime).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Medicao {
    pub session_id: Option<String>,
    pub modelo: Option<String>,
    pub contexto_pct: Option<f64>,
    pub contexto_tamanho: Option<u64>,
    /// Estimativa do cliente (`cost.total_cost_usd`): a tela rotula "estimado".
    pub custo_usd: Option<f64>,
    pub linhas_mais: Option<u64>,
    pub linhas_menos: Option<u64>,
    pub fast: Option<bool>,
    pub esforco: Option<String>,
    pub versao: Option<String>,
    pub cinco_horas: Option<Janela>,
    pub sete_dias: Option<Janela>,
    pub observado_em_ms: u64,
}

fn porcentagem(valor: Option<&Value>) -> Option<f64> {
    valor.and_then(Value::as_f64).filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
}

pub fn medicao(valor: &Value, agora_ms: u64) -> Medicao {
    let janela = |chave: &str| -> Option<Janela> {
        let item = valor.pointer(&format!("/rate_limits/{chave}"))?;
        Some(Janela { pct: porcentagem(item.get("used_percentage"))?, reset_s: item.get("resets_at").and_then(Value::as_i64).filter(|r| *r > 0) })
    };
    Medicao {
        session_id: texto(valor, "session_id"),
        modelo: valor.pointer("/model/display_name").or_else(|| valor.pointer("/model/id")).and_then(Value::as_str).map(str::to_owned),
        contexto_pct: porcentagem(valor.pointer("/context_window/used_percentage")),
        contexto_tamanho: valor.pointer("/context_window/context_window_size").and_then(Value::as_u64),
        custo_usd: valor.pointer("/cost/total_cost_usd").and_then(Value::as_f64).filter(|v| v.is_finite() && *v >= 0.0),
        linhas_mais: valor.pointer("/cost/total_lines_added").and_then(Value::as_u64),
        linhas_menos: valor.pointer("/cost/total_lines_removed").and_then(Value::as_u64),
        fast: valor.get("fast_mode").and_then(Value::as_bool),
        esforco: valor.pointer("/effort/level").and_then(Value::as_str).map(str::to_owned),
        versao: texto(valor, "version"),
        cinco_horas: janela("five_hour"),
        sete_dias: janela("seven_day"),
        observado_em_ms: agora_ms,
    }
}

impl Medicao {
    /// O uso da conta pela statusline (`rate_limits` só vem em Pro/Max, depois da 1ª resposta).
    pub fn uso(&self, perfil: &str) -> Option<AccountUsage> {
        let janela = |j: &Option<Janela>, minutos| j.as_ref().map(|j| UsageWindow { used_percent: j.pct, window_minutes: minutos, resets_at: j.reset_s, reset_label: None });
        let (primary, secondary) = (janela(&self.cinco_horas, 300), janela(&self.sete_dias, 10080));
        if primary.is_none() && secondary.is_none() { return None; }
        let vencida = [&primary, &secondary].into_iter().flatten()
            .any(|w| w.resets_at.is_some_and(|r| r > 0 && (r as u64) <= self.observado_em_ms / 1000));
        Some(AccountUsage { profile_id: perfil.into(), provider: "claude".into(), source: "claude_statusline".into(),
            status: if vencida { "stale" } else { "available" }.into(), observed_at_ms: Some(self.observado_em_ms), primary, secondary, reason: None })
    }
}

#[cfg(test)]
#[path = "ganchos_testes.rs"]
mod testes;
