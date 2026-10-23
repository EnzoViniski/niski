//! Fork (harness, A4): troca de conta no limite de uso também para a sessão **sem painel** aberto
//! (subagente do orquestrador, Telegram, Conversa sem o terminal montado, agendamento).
//!
//! A troca automática de sempre mora no `TerminalPane.tsx` e só roda com a aba do terminal montada. Aqui
//! um vigia no engine, a cada [`RITMO`], procura sessão de agente (Claude/Codex) com o aviso
//! `usage_limit` (pela tela ou pelo `StopFailure` dos ganchos) e **nenhum painel lendo** a PTY; escolhe
//! outra conta pela fila (`roteamento::escolher_para`, a atual conta como falha) e reabre a mesma
//! conversa nela com `reabrir_com` — o mesmo caminho do seletor de conta: transcript levado para a conta
//! nova, `--resume` do mesmo id, segmento novo no índice. Nada é digitado na conversa.
//!
//! **Uma troca por vez por conversa**: [`reservar`] é a trava que o vigia e o painel usam
//! (`POST /conversas/{id}/troca-de-conta/reservar` e `/soltar`). Quem não consegue a reserva não troca.
//! Sem outra conta, tenta de novo depois de [`ESPERA_SEM_CONTA_MS`]. Só troca quando bate no limite
//! (decisão C6, padrão seguro até o Enzo responder).
use super::*;
use std::sync::LazyLock;

pub(crate) const RITMO: std::time::Duration = std::time::Duration::from_secs(2);
/// Um painel que leu a PTY há menos que isto (ou está lendo agora, no long-poll) é dono da troca.
const PAINEL_RECENTE_MS: u64 = 5_000;
/// Uma reserva esquecida (processo que caiu no meio) solta sozinha depois disto.
const VALIDADE_DA_RESERVA_MS: u64 = 120_000;
pub(crate) const ESPERA_SEM_CONTA_MS: u64 = 5 * 60_000;

#[derive(Default)]
struct Painel { lendo: u32, ultimo_ms: u64 }

static PAINEIS: LazyLock<Mutex<HashMap<String, Painel>>> = LazyLock::new(Default::default);
/// Conversa → (quem reservou, até quando).
static RESERVAS: LazyLock<Mutex<HashMap<String, (String, u64)>>> = LazyLock::new(Default::default);
/// Terminal → (conta que estava no limite, não tentar antes de).
static ADIADAS: LazyLock<Mutex<HashMap<String, (String, u64)>>> = LazyLock::new(Default::default);
/// Terminal → conta para onde o vigia acabou de trocar. A retomada (`--resume`) pode reexibir a linha de
/// limite antiga na conta nova; até um turno novo começar nela, o aviso não dispara outra troca. É a
/// mesma guarda do painel (`staleLimitProfileRef` no `TerminalPane.tsx`) e evita o vaivém entre contas.
static RECEM_TROCADAS: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);

/* ── Painel lendo a PTY ──────────────────────────────────────────────────────────────────────── */

/// Um painel (xterm do desktop) começou a ler a PTY (`Snapshot`). Devolve a guarda que marca o fim.
pub(crate) fn painel_lendo(terminal: &str) -> LeituraDoPainel {
    if let Ok(mut p) = PAINEIS.lock() { p.entry(terminal.to_owned()).or_default().lendo += 1; }
    LeituraDoPainel(terminal.to_owned())
}

pub(crate) struct LeituraDoPainel(String);
impl Drop for LeituraDoPainel {
    fn drop(&mut self) {
        if let Ok(mut p) = PAINEIS.lock() {
            if let Some(painel) = p.get_mut(&self.0) { painel.lendo = painel.lendo.saturating_sub(1); painel.ultimo_ms = now_ms(); }
        }
    }
}

pub(crate) fn painel_ligado(terminal: &str) -> bool {
    PAINEIS.lock().ok().and_then(|p| p.get(terminal).map(|x| x.lendo > 0 || now_ms().saturating_sub(x.ultimo_ms) < PAINEL_RECENTE_MS))
        .unwrap_or(false)
}

/* ── A trava ─────────────────────────────────────────────────────────────────────────────────── */

/// Reserva a troca de conta da conversa para `dono`. `false` se outro já reservou (e a reserva vale).
/// Checar e gravar acontecem sob a mesma trava: dois pedidos ao mesmo tempo, só um ganha.
pub(crate) fn reservar(conversa: &str, dono: &str) -> bool {
    let Ok(mut reservas) = RESERVAS.lock() else { return false };
    let agora = now_ms();
    if reservas.get(conversa).is_some_and(|(quem, ate)| *ate > agora && quem != dono) { return false; }
    reservas.insert(conversa.to_owned(), (dono.to_owned(), agora + VALIDADE_DA_RESERVA_MS));
    true
}

pub(crate) fn soltar(conversa: &str, dono: &str) {
    if let Ok(mut reservas) = RESERVAS.lock() {
        if reservas.get(conversa).is_some_and(|(quem, _)| quem == dono) { reservas.remove(conversa); }
    }
}

/* ── O vigia ─────────────────────────────────────────────────────────────────────────────────── */

pub(crate) fn iniciar(state: Arc<EngineState>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(RITMO);
        uma_volta(&state);
    });
}

/// O que a volta fez com uma sessão (para o log e para os testes).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Resultado { Trocou { de: String, para: String }, SemOutraConta, Reservada, Falhou(String) }

/// Sessões candidatas: agente com conversa e conta, aviso de limite, sem painel lendo e não adiada.
fn candidatas(state: &EngineState) -> Vec<(String, String, String, String, String)> {
    let vivas: Vec<Arc<LiveSession>> = match state.sessions.lock() {
        Ok(s) => s.values().filter_map(|e| match e { SessionEntry::Live(v) => Some(v.clone()), _ => None }).collect(),
        Err(_) => return Vec::new(),
    };
    let agora = now_ms();
    vivas.iter().filter_map(|sessao| {
        let meta = sessao.meta.lock().ok()?.clone();
        let provider = meta.provider.clone().filter(|p| matches!(p.as_str(), "claude" | "codex"))?;
        let (conversa, perfil) = (meta.conversation_id.clone()?, meta.profile_id.clone()?);
        {
            let mut recem = RECEM_TROCADAS.lock().ok()?;
            // Só um turno novo na conta nova (sinal de turno pela tela ou pelo evento; o reinício zera o
            // sinal) prova que o aviso da tela é deste turno. Saída da retomada sozinha não conta.
            let turno_novo = || sessao.interaction.lock().is_ok_and(|i| i.turn_active);
            match recem.get(&meta.id) {
                Some(conta) if conta == &perfil && !turno_novo() => return None,
                Some(_) => { recem.remove(&meta.id); }
                None => {}
            }
        }
        if meta.notice.as_deref() != Some("usage_limit") || meta.pid.is_none() || painel_ligado(&meta.id) { return None; }
        let adiada = ADIADAS.lock().ok()?.get(&meta.id).is_some_and(|(conta, ate)| conta == &perfil && *ate > agora);
        (!adiada).then(|| (meta.id.clone(), conversa, provider, perfil, meta.cwd.clone()))
    }).collect()
}

/// Uma passada do vigia. Público no crate para os testes rodarem sem esperar o ritmo.
pub(crate) fn uma_volta(state: &Arc<EngineState>) -> Vec<(String, Resultado)> {
    let mut feitos = Vec::new();
    for (terminal, conversa, provider, perfil, cwd) in candidatas(state) {
        let resultado = trocar(state, &terminal, &conversa, &provider, &perfil, &cwd);
        let rotulo = match &resultado { Resultado::Trocou { .. } => "trocou", Resultado::SemOutraConta => "sem_outra_conta",
            Resultado::Reservada => "reservada", Resultado::Falhou(_) => "falhou" };
        registro::info("troca_no_limite", &[("terminal", &terminal), ("provider", &provider), ("de", &perfil), ("resultado", rotulo),
            ("para", match &resultado { Resultado::Trocou { para, .. } => para.as_str(), _ => "-" })]);
        feitos.push((terminal, resultado));
    }
    feitos
}

fn trocar(state: &Arc<EngineState>, terminal: &str, conversa: &str, provider: &str, perfil: &str, cwd: &str) -> Resultado {
    const DONO: &str = "engine";
    if !reservar(conversa, DONO) { return Resultado::Reservada; }
    // Com a reserva na mão, confere de novo: entre a lista de candidatas e a reserva, o painel pode ter
    // trocado (e soltado) a conta desta conversa. Aí a sessão já não está no limite nesta conta.
    if !ainda_no_limite(state, terminal, perfil) { soltar(conversa, DONO); return Resultado::Reservada; }
    let resultado = (|| {
        // A conta atual conta como falha: a fila escolhe a próxima (cota e teto de sessões valem).
        let escolha = {
            let _escolhendo = super::roteamento::trava_da_escolha();
            super::roteamento::escolher_para(state, provider, Some(cwd), false, true, &[perfil])
        };
        let Some(alvo) = escolha.perfil_id.filter(|alvo| alvo != perfil) else {
            if let Ok(mut adiadas) = ADIADAS.lock() { adiadas.insert(terminal.to_owned(), (perfil.to_owned(), now_ms() + ESPERA_SEM_CONTA_MS)); }
            return Resultado::SemOutraConta;
        };
        let ajustes = Ajustes { profile_id: Some(alvo.clone()), model: None, effort: None };
        match reabrir_com(state, conversa, &ajustes) {
            Ok(()) => {
                if let Ok(mut recem) = RECEM_TROCADAS.lock() { recem.insert(terminal.to_owned(), alvo.clone()); }
                crate::harness_resumo::troca_de_conta(terminal);
                Resultado::Trocou { de: perfil.to_owned(), para: alvo }
            }
            Err(erro) => {
                if let Ok(mut adiadas) = ADIADAS.lock() { adiadas.insert(terminal.to_owned(), (perfil.to_owned(), now_ms() + ESPERA_SEM_CONTA_MS)); }
                Resultado::Falhou(omni_core::registro::limpar(&erro.to_string()))
            }
        }
    })();
    soltar(conversa, DONO);
    resultado
}

fn ainda_no_limite(state: &EngineState, terminal: &str, perfil: &str) -> bool {
    let sessao = match state.sessions.lock() {
        Ok(s) => match s.get(terminal) { Some(SessionEntry::Live(v)) => v.clone(), _ => return false },
        Err(_) => return false,
    };
    let Ok(meta) = sessao.meta.lock() else { return false };
    meta.notice.as_deref() == Some("usage_limit") && meta.pid.is_some() && meta.profile_id.as_deref() == Some(perfil) && !painel_ligado(terminal)
}

/* ── Rotas do painel ─────────────────────────────────────────────────────────────────────────── */

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/conversas/{id}/troca-de-conta/reservar", post(reservar_rota))
        .route("/conversas/{id}/troca-de-conta/soltar", post(soltar_rota))
}

/// Quem pede a reserva pela rota: cada painel manda um id dele (`{"dono": "..."}`); sem corpo, "painel".
/// O nome "engine" é do vigia e não pode ser usado de fora.
#[derive(Deserialize, Default)] #[serde(deny_unknown_fields)] struct DonoBody { #[serde(default)] dono: Option<String> }

fn dono_do_corpo(corpo: &[u8]) -> Result<String, ApiError> {
    let dono = if corpo.iter().all(u8::is_ascii_whitespace) { None } else {
        serde_json::from_slice::<DonoBody>(corpo).map_err(|_| error(StatusCode::BAD_REQUEST, "Corpo inválido"))?.dono
    };
    let dono = dono.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty()).unwrap_or_else(|| "painel".into());
    if dono == "engine" || dono.len() > 80 || dono.chars().any(char::is_control) { return Err(error(StatusCode::BAD_REQUEST, "Dono inválido")); }
    Ok(format!("painel:{dono}"))
}

/// O painel pede a vez antes de trocar a conta. 409: o engine (ou outro painel) já está trocando.
async fn reservar_rota(RoutePath(id): RoutePath<String>, corpo: axum::body::Bytes) -> Result<Json<Value>, ApiError> {
    let dono = dono_do_corpo(&corpo)?;
    if reservar(&id, &dono) { Ok(Json(json!({"ok": true}))) }
    else { Err(error(StatusCode::CONFLICT, "A conta desta conversa já está sendo trocada")) }
}

async fn soltar_rota(RoutePath(id): RoutePath<String>, corpo: axum::body::Bytes) -> Result<Json<Value>, ApiError> {
    soltar(&id, &dono_do_corpo(&corpo)?);
    Ok(Json(json!({"ok": true})))
}

#[cfg(all(test, unix))]
#[path = "troca_no_limite_testes.rs"]
mod testes;
