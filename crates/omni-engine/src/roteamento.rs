//! Fork (Fase 3, frente 3B): a fila de contas no engine. A regra é pura e mora em
//! `omni_core::roteamento`; aqui só o que depende do disco e do estado vivo:
//! - `routing.json` na pasta do engine (política, ajustes por conta, pastas confidenciais);
//! - a cota de cada conta pelo que o app já lê (`usage.rs`: cache do Claude e statusline dos ganchos,
//!   o mais novo; rollout do Codex). Sem leitura, "desconhecida" com a fonte e a idade;
//! - as sessões vivas por conta, para o teto de concorrência;
//! - as rotas `GET|POST /roteamento`, `POST /roteamento/escolher` e `POST /roteamento/apos-falha`;
//! - a queda para a próxima conta, **só** em execução sem tela (fluxos). Sessão interativa recebe o
//!   aviso com a sugestão e o usuário decide.
//!
//! Filho de `mobile` (como a atividade) para usar `ApiError` e o estado sem torná-los públicos.
//! Preços não entram aqui: orçamento e gasto são números que o usuário digita na tela.
use super::*;
use omni_core::roteamento::{self as regra, Cota, Escolha, Pedido, Politica};
use omni_core::usage::AccountUsage;

pub(crate) const ARQUIVO: &str = "routing.json";
/// Valor de `conta` que pede a escolha pela fila (Novo agente, `criar_agente`, modelo de agente).
pub(crate) const CONTA_AUTOMATICA: &str = "auto";
/// Quantas contas um nó de fluxo tenta antes de desistir: cobre a fila inteira de quem tem poucas
/// contas sem virar laço com uma fila longa.
const TROCAS_MAXIMAS: usize = 4;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/roteamento", get(ler).post(gravar))
        .route("/roteamento/escolher", post(escolher_rota))
        .route("/roteamento/apos-falha", post(apos_falha_rota))
}

fn pasta(state: &EngineState) -> &Path { state.state_file.parent().expect("engine sem pasta") }

/// Arquivo ausente = política vazia. Inválido também (com aviso no log, sem o conteúdo): a fila
/// nunca impede abrir um agente por causa de um JSON quebrado.
pub(crate) fn ler_politica(dir: &Path) -> Politica {
    let Ok(texto) = fs::read_to_string(dir.join(ARQUIVO)) else { return Politica::default() };
    Politica::carregar(&texto).unwrap_or_else(|_| {
        registro::aviso("roteamento_arquivo_invalido", &[("arquivo", ARQUIVO)]);
        Politica::default()
    })
}

/// O uso que o app já lê, igual ao `GET /contas`: Claude pelo cache ou pela statusline (o mais
/// novo), Codex pelo rollout. Outras CLIs: sem fonte.
fn uso_da_conta(dir: &Path, perfil: &omni_core::Profile, agora: u64) -> Option<AccountUsage> {
    match perfil.provider.as_str() {
        "codex" => Some(omni_core::usage::read_codex(&perfil.id, Path::new(&perfil.config_dir), agora)),
        "claude" => omni_core::usage::mais_recente(
            omni_core::usage::read_claude_cache(&perfil.id, &omni_core::usage::claude_state_path(perfil), agora),
            crate::ganchos::uso_da_statusline(dir, &perfil.id)),
        _ => None,
    }
}

/// Leitura que falhou vira cota sem janelas, mas com a fonte: a explicação diz "desconhecida, fonte
/// X, idade Y" em vez de inventar 0%.
fn cota_de(uso: &AccountUsage) -> Cota {
    Cota::de_uso(uso).unwrap_or_else(|| Cota { janelas: Vec::new(), fonte: uso.source.clone(), observado_em_ms: uso.observed_at_ms })
}

fn cotas(dir: &Path, perfis: &[omni_core::Profile], agora: u64) -> HashMap<String, Cota> {
    perfis.iter().filter_map(|p| uso_da_conta(dir, p, agora).map(|uso| (p.id.clone(), cota_de(&uso)))).collect()
}

/// Sessões de agente vivas por conta (o terminal sem agente não usa conta).
pub(crate) fn sessoes_por_perfil(state: &EngineState) -> HashMap<String, u32> {
    let vivas: Vec<Arc<LiveSession>> = state.sessions.lock().expect("sessions poisoned").values()
        .filter_map(|entrada| match entrada { SessionEntry::Live(s) => Some(s.clone()), SessionEntry::Historical(_) => None })
        .collect();
    let mut contagem = HashMap::new();
    for sessao in vivas {
        // Trava ocupada (encerrando, redimensionando): conta, para nunca passar do teto.
        let viva = sessao.child.try_lock().map(|mut filho| matches!(filho.try_wait(), Ok(None))).unwrap_or(true);
        if !viva { continue; }
        let perfil = sessao.meta.lock().ok().and_then(|m| m.provider.as_ref().and(m.profile_id.clone()));
        if let Some(perfil) = perfil { *contagem.entry(perfil).or_insert(0) += 1; }
    }
    // Fork (fix-orquestracao, achado 9): execuções sem tela (fluxos) também ocupam a conta.
    let dir = pasta(state);
    for ((pasta_das_vagas, perfil), n) in SEM_TELA.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter() {
        if pasta_das_vagas == dir { *contagem.entry(perfil.clone()).or_insert(0) += n; }
    }
    contagem
}

/// Fork (fix-orquestracao, achado 2): a pasta e, se ela é uma worktree, a mesma pasta no checkout
/// principal do repositório — a regra de confidencial é do projeto, e um orquestrador (ou fluxo) numa
/// worktree irmã escapava dela. Pergunta ao git (worktree isolada do Medza ou `git worktree` comum); sem
/// git (worktree isolada já apagada), `<pai>/.medza-worktrees/<repo>/<nome>/<resto>` vira `<pai>/<repo>/<resto>`.
fn pastas_do_projeto(projeto: &str) -> Vec<String> {
    let caminho = Path::new(projeto.trim());
    let mut pastas = vec![projeto.to_owned()];
    pastas.extend(no_checkout_principal(caminho));
    pastas.extend(pela_worktree_isolada(caminho));
    pastas
}

fn no_checkout_principal(caminho: &Path) -> Option<String> {
    if !caminho.is_dir() { return None; }
    let saida = std::process::Command::new("git").args(["rev-parse", "--path-format=absolute", "--git-common-dir", "--show-prefix"])
        .current_dir(caminho).env("GIT_OPTIONAL_LOCKS", "0").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .output().ok().filter(|saida| saida.status.success())?;
    let texto = String::from_utf8_lossy(&saida.stdout);
    let mut linhas = texto.lines();
    let comum = PathBuf::from(linhas.next()?);
    let comum = comum.canonicalize().unwrap_or(comum);
    // Só o `.git` de um checkout (submódulo e repositório nu ficam de fora).
    if comum.file_name().is_none_or(|nome| nome != ".git") { return None; }
    let raiz = comum.parent()?;
    let prefixo = linhas.next().unwrap_or("").trim_end_matches('/');
    Some(if prefixo.is_empty() { raiz.to_path_buf() } else { raiz.join(prefixo) }.to_string_lossy().into_owned())
}

fn pela_worktree_isolada(caminho: &Path) -> Option<String> {
    let partes: Vec<std::path::Component> = caminho.components().collect();
    let i = partes.iter().rposition(|c| c.as_os_str() == omni_core::worktree_isolada::PASTA)?;
    let repo = partes.get(i + 1)?;
    partes.get(i + 2)?;
    let original: PathBuf = partes[..i].iter().chain(std::iter::once(repo)).chain(partes[i + 3..].iter()).collect();
    Some(original.to_string_lossy().into_owned())
}

/// Sem nenhuma conta deste provider na fila, valem todas as cadastradas, na ordem do cadastro:
/// "Automática" funciona antes de alguém configurar a fila.
fn politica_efetiva(politica: &Politica, perfis: &[omni_core::Profile], provider: &str) -> (Politica, Option<String>) {
    let tem = politica.etapas.iter().any(|id| perfis.iter().any(|p| &p.id == id && p.provider == provider));
    if tem { return (politica.clone(), None); }
    let mut efetiva = politica.clone();
    efetiva.etapas = perfis.iter().filter(|p| p.provider == provider).map(|p| p.id.clone()).collect();
    (efetiva, Some(format!("a fila não tem contas de {provider}: valem as contas cadastradas, na ordem")))
}

/// Escolhe a conta para `provider`. `falharam` não vazio = queda após falha (troca sozinha só com
/// `headless`; no interativo devolve o aviso com a sugestão).
pub(crate) fn escolher_para(state: &EngineState, provider: &str, projeto: Option<&str>, confidencial: bool, headless: bool,
    falharam: &[&str]) -> Escolha {
    let dir = pasta(state);
    let politica = ler_politica(dir);
    let perfis = omni_core::profiles(dir);
    let (efetiva, nota) = politica_efetiva(&politica, &perfis, provider);
    let agora = now_ms();
    let do_provider: Vec<omni_core::Profile> = perfis.iter().filter(|p| p.provider == provider).cloned().collect();
    let pedido = Pedido {
        provider: provider.to_owned(),
        // Fork (fix-orquestracao, achado 2): vale a pasta e a mesma pasta no projeto original (worktree).
        confidencial: confidencial || projeto.is_some_and(|p| pastas_do_projeto(p).iter().any(|pasta| politica.confidencial(pasta))),
        sessoes_em_uso: sessoes_por_perfil(state), agora_ms: agora, headless,
    };
    let contas = efetiva.contas(&perfis);
    let cotas = cotas(dir, &do_provider, agora);
    let mut escolha = if falharam.is_empty() { regra::escolher(&efetiva, &contas, &cotas, &pedido) }
        else { regra::proxima_apos_falha(&efetiva, &contas, &cotas, &pedido, falharam) };
    // O aviso da sessão interativa fica em primeiro; o contexto (fila vazia, confidencial) logo depois.
    let topo = usize::from(!headless && !falharam.is_empty()).min(escolha.explicacao.len());
    if let Some(nota) = nota { escolha.explicacao.insert(topo, nota); }
    if pedido.confidencial { escolha.explicacao.insert(topo, "projeto confidencial: só contas liberadas para ele".to_owned()); }
    // O teto de sessões do app (Configurações → Geral) vale para qualquer conta: avisa aqui, e quem
    // abre a sessão é que recusa.
    let limite = omni_core::limite_de_sessoes::ler(dir);
    if let Err(motivo) = omni_core::limite_de_sessoes::conferir(super::limite_de_sessoes::contar(state), limite) {
        escolha.explicacao.push(format!("atenção: {motivo}"));
    }
    escolha
}

fn nome_do_perfil(perfis: &[omni_core::Profile], id: Option<&str>) -> Option<String> {
    let id = id?;
    Some(perfis.iter().find(|p| p.id == id).map_or_else(|| id.to_owned(), |p| p.name.clone()))
}

fn escolha_em_json(state: &EngineState, escolha: &Escolha) -> Value {
    let perfis = omni_core::profiles(pasta(state));
    json!({"perfil_id": escolha.perfil_id, "nome": nome_do_perfil(&perfis, escolha.perfil_id.as_deref()),
        "sugestao": escolha.sugestao, "nome_da_sugestao": nome_do_perfil(&perfis, escolha.sugestao.as_deref()),
        "explicacao": escolha.explicacao})
}

/// Para o `listar_plataformas_e_contas` do orquestrador: o que "auto" escolheria agora.
pub(crate) fn previa(state: &EngineState, provider: &str) -> Value {
    escolha_em_json(state, &escolher_para(state, provider, None, false, false, &[]))
}

/* ── Vagas: escolha e reserva ────────────────────────────────────────────────────────────────── */

// Fork (fix-orquestracao, achados 8 e 9): o teto de sessões por conta contava só as PTYs; a CLI sem tela
// dos fluxos não ocupava vaga, e quem escolhia junto (dois fluxos, dois `criar_agente`) via a mesma conta
// livre. Agora a execução sem tela ocupa uma `Vaga` enquanto roda, e quem escolhe para usar já (fluxo,
// `criar_agente`) escolhe e ocupa sob `trava_da_escolha`.
static ESCOLHENDO: Mutex<()> = Mutex::new(());
static SEM_TELA: std::sync::LazyLock<Mutex<HashMap<(PathBuf, String), u32>>> = std::sync::LazyLock::new(Default::default);

/// Segure do momento de escolher a conta até a vaga dela estar ocupada (sessão aberta ou `Vaga` na mão).
pub(crate) fn trava_da_escolha() -> std::sync::MutexGuard<'static, ()> {
    ESCOLHENDO.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Vaga de uma execução sem tela numa conta; solta quando sai de escopo.
pub(crate) struct Vaga { chave: (PathBuf, String) }

impl Drop for Vaga {
    fn drop(&mut self) {
        let mut vagas = SEM_TELA.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(n) = vagas.get_mut(&self.chave) {
            *n = n.saturating_sub(1);
            if *n == 0 { vagas.remove(&self.chave); }
        }
    }
}

pub(crate) fn ocupar_sem_tela(state: &EngineState, perfil: &str) -> Vaga {
    let chave = (pasta(state).to_path_buf(), perfil.to_owned());
    *SEM_TELA.lock().unwrap_or_else(std::sync::PoisonError::into_inner).entry(chave.clone()).or_insert(0) += 1;
    Vaga { chave }
}

/* ── Fluxos (sem tela) ───────────────────────────────────────────────────────────────────────── */

/// Roda um agente de fluxo pela fila. `rodar(None)` = a escolha de conta de sempre (do modelo de
/// agente ou a padrão); `rodar(Some(perfil))` = a conta que a fila escolheu. A fila entra com
/// `conta: "auto"` no modelo de agente, ou sem conta com "usar nos fluxos" ligado. Falha por limite
/// ou login passa para a próxima conta da fila (até `TROCAS_MAXIMAS`); retomar uma sessão fica na
/// conta em que ela nasceu, porque a conversa só existe lá.
pub(crate) fn com_queda_sem_tela(state: &EngineState, provider: &str, conta: &str, projeto: &str, conta_anterior: Option<String>,
    retomando: bool, mut rodar: impl FnMut(Option<omni_core::Profile>) -> anyhow::Result<String>) -> anyhow::Result<String> {
    let dir = pasta(state);
    let pela_fila = conta.eq_ignore_ascii_case(CONTA_AUTOMATICA) || (conta.is_empty() && ler_politica(dir).usar_em_fluxos);
    if !pela_fila { return rodar(None); }
    let perfil_de = |id: &str| omni_core::profiles(dir).into_iter().find(|p| p.id == id && p.provider == provider);
    if retomando {
        let perfil = conta_anterior.as_deref().and_then(perfil_de);
        let _vaga = perfil.as_ref().map(|p| ocupar_sem_tela(state, &p.id)); // Fork (fix-orquestracao)
        return rodar(perfil);
    }
    let mut falharam: Vec<String> = Vec::new();
    loop {
        let refs: Vec<&str> = falharam.iter().map(String::as_str).collect();
        // Fork (fix-orquestracao, achado 9): escolher e ocupar a vaga na mesma seção crítica; a vaga
        // fica ocupada enquanto `rodar` roda (até o fim desta volta).
        let (escolha, _vaga) = {
            let _escolhendo = trava_da_escolha();
            let escolha = escolher_para(state, provider, Some(projeto), false, true, &refs);
            let vaga = escolha.perfil_id.as_deref().map(|id| ocupar_sem_tela(state, id));
            (escolha, vaga)
        };
        let Some(id) = escolha.perfil_id else {
            return Err(anyhow!("Fila de contas sem conta para {provider}: {}", escolha.explicacao.join("; ")));
        };
        let tentativa = (falharam.len() + 1).to_string();
        registro::info("roteamento_fluxo_conta", &[("provider", provider), ("perfil", &id), ("tentativa", &tentativa)]);
        match rodar(perfil_de(&id)) {
            Err(erro) if falharam.len() + 1 < TROCAS_MAXIMAS && regra::falha_de_conta(&erro.to_string()) => {
                registro::aviso("roteamento_queda_de_conta", &[("provider", provider), ("perfil", &id), ("tentativa", &tentativa)]);
                falharam.push(id);
            }
            resultado => return resultado,
        }
    }
}

/* ── Rotas ───────────────────────────────────────────────────────────────────────────────────── */

/// Tudo que a tela "Fila de contas" mostra: a política, cada conta com ajustes, cota (fonte e idade)
/// e sessões abertas, e o "por que esta conta" de cada provider que tem conta.
fn estado_em_json(state: &EngineState) -> Value {
    let dir = pasta(state);
    let politica = ler_politica(dir);
    let perfis = omni_core::profiles(dir);
    let agora = now_ms();
    let sessoes = sessoes_por_perfil(state);
    let contas: Vec<Value> = perfis.iter().map(|perfil| {
        let cota = uso_da_conta(dir, perfil, agora).map(|uso| cota_de(&uso));
        json!({"id": perfil.id, "nome": perfil.name, "provider": perfil.provider,
            "na_fila": politica.etapas.contains(&perfil.id),
            "ajustes": politica.contas.get(&perfil.id).cloned().unwrap_or_default(),
            "sessoes": sessoes.get(&perfil.id).copied().unwrap_or(0),
            "cota": cota.map(|c| json!({"conhecida": !c.janelas.is_empty(), "fonte": c.fonte, "observado_em_ms": c.observado_em_ms,
                "janelas": c.janelas})).unwrap_or_else(|| json!({"conhecida": false, "fonte": null, "observado_em_ms": null, "janelas": []}))})
    }).collect();
    let mut providers: Vec<&str> = Vec::new();
    for perfil in &perfis { if !providers.contains(&perfil.provider.as_str()) { providers.push(&perfil.provider); } }
    let escolhas: serde_json::Map<String, Value> = providers.iter()
        .map(|provider| ((*provider).to_owned(), escolha_em_json(state, &escolher_para(state, provider, None, false, false, &[])))).collect();
    json!({"politica": politica, "contas": contas, "escolhas": escolhas, "agora_ms": agora})
}

async fn ler(State(state): State<Arc<EngineState>>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || Json(estado_em_json(&state))).await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ler a fila de contas"))
}

async fn gravar(State(state): State<Arc<EngineState>>, corpo: axum::body::Bytes) -> Result<Json<Value>, ApiError> {
    let texto = std::str::from_utf8(&corpo).map_err(|_| error(StatusCode::BAD_REQUEST, "Corpo inválido"))?;
    let mut politica = Politica::carregar(texto).map_err(|erro| error(StatusCode::BAD_REQUEST, &erro))?;
    politica.etapas = politica.etapas.iter().map(|id| id.trim().to_owned()).collect();
    politica.validar().map_err(|erro| error(StatusCode::BAD_REQUEST, &erro))?;
    tokio::task::spawn_blocking(move || {
        let dir = pasta(&state).to_path_buf();
        atomic_write_json(&dir.join(ARQUIVO), &politica).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui gravar"))?;
        registro::info("roteamento_gravado", &[("modo", if politica.modo == regra::Modo::Auto { "auto" } else { "fila" }),
            ("etapas", &politica.etapas.len().to_string()), ("confidenciais", &politica.projetos_confidenciais.len().to_string()),
            ("fluxos", if politica.usar_em_fluxos { "sim" } else { "nao" })]);
        Ok(Json(estado_em_json(&state)))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao gravar"))?
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct EscolherBody { provider: String, #[serde(default)] projeto: Option<String>, #[serde(default)] confidencial: bool }

/// "Automática" do Novo agente: a conta e o porquê. Interativo, então nunca troca nada sozinho.
async fn escolher_rota(State(state): State<Arc<EngineState>>, Json(body): Json<EscolherBody>) -> Result<Json<Value>, ApiError> {
    if body.provider.trim().is_empty() { return Err(error(StatusCode::BAD_REQUEST, "Informe o provider")); }
    tokio::task::spawn_blocking(move || {
        let escolha = escolher_para(&state, &body.provider, body.projeto.as_deref(), body.confidencial, false, &[]);
        Json(escolha_em_json(&state, &escolha))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao escolher a conta"))
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct AposFalhaBody { provider: String, #[serde(default)] projeto: Option<String>, falharam: Vec<String>, #[serde(default)] headless: bool }

/// A conta `falharam` não serviu: sem tela, a próxima; com tela, só o aviso com a sugestão.
async fn apos_falha_rota(State(state): State<Arc<EngineState>>, Json(body): Json<AposFalhaBody>) -> Result<Json<Value>, ApiError> {
    if body.provider.trim().is_empty() || body.falharam.is_empty() { return Err(error(StatusCode::BAD_REQUEST, "Informe o provider e a conta que falhou")); }
    tokio::task::spawn_blocking(move || {
        let falharam: Vec<&str> = body.falharam.iter().map(String::as_str).collect();
        let escolha = escolher_para(&state, &body.provider, body.projeto.as_deref(), false, body.headless, &falharam);
        Json(escolha_em_json(&state, &escolha))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao escolher a conta"))
}

#[cfg(test)]
#[path = "roteamento_testes.rs"]
mod testes;
