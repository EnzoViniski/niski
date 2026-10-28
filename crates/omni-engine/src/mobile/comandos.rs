//! Controles da Conversa. Só comandos da lista do core chegam à PTY.
use super::*;
use omni_core::comandos_oficiais as oficiais;

#[derive(Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Comando { comando: String, argumento: Option<String> }

#[derive(Clone, PartialEq)]
pub(super) enum Pedido { Slash(Comando), Modo(String),
    /// `/skill argumento` do Claude, já conferido contra a lista da conversa (`barra.rs`).
    Barra(String),
    /// Aprovação/sandbox do Codex (`modo_codex.rs`), que reabre a mesma conversa.
    ModoCodex(String) }

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/conversas/{id}/comando",post(comando))
        .route("/conversas/{id}/modo",post(modo))
}

async fn comando(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<Comando>) -> Result<(StatusCode,Json<Value>),ApiError> {
    let segmento = ultima_sessao(&state,&id)?;
    oficiais::montar(&segmento.provider,&body.comando,body.argumento.as_deref())
        .map_err(|e| error(StatusCode::BAD_REQUEST,e))?;
    if body.comando == "fast" && opcoes_da_conversa(&state,&id)?["fast_supported"] != true {
        return Err(error(StatusCode::CONFLICT,"O modelo atual não confirma suporte a fast"));
    }
    enqueue_async(state,id,headers,ActionKind::Comando(Pedido::Slash(body))).await
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct ModoBody { modo: String }
async fn modo(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<ModoBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    // O Codex também escolhe o modo pela Conversa (Perguntar, Sem perguntar, Acesso total, Somente leitura).
    // Valor que não é de nenhum dos dois é recusado antes de procurar a sessão, como antes.
    let do_codex = omni_core::modo_do_codex::valido(&body.modo);
    if !oficiais::modo_valido(&body.modo) && !do_codex { return Err(error(StatusCode::BAD_REQUEST,"Modo deve ser default, acceptEdits, plan ou auto")); }
    let provider = ultima_sessao(&state,&id)?.provider;
    if provider == "codex" {
        if !do_codex { return Err(error(StatusCode::BAD_REQUEST,"Modo do Codex deve ser default, semPerguntar, acessoTotal ou somenteLeitura")); }
        return enqueue_async(state,id,headers,ActionKind::Comando(Pedido::ModoCodex(body.modo))).await;
    }
    if !oficiais::modo_valido(&body.modo) { return Err(error(StatusCode::BAD_REQUEST,"Modo deve ser default, acceptEdits, plan ou auto")); }
    if provider != "claude" { return Err(error(StatusCode::BAD_REQUEST,"Este controle de modo é do Claude Code")); }
    enqueue_async(state,id,headers,ActionKind::Comando(Pedido::Modo(body.modo))).await
}

/// Só reconhece default com o composer vazio e a dica do rodapé visível. Ausência de selo
/// sozinha nunca é confirmação (pode ser um seletor, turno ativo ou rodapé fora da tela).
pub(super) fn modo_na_tela(screen: &vt100::Screen) -> Option<&'static str> {
    if let Some(modo) = interaction::modo_atual(screen) { return Some(modo); }
    let texto = screen.contents().to_lowercase();
    if texto.contains("bypass permissions") || texto.contains("don't ask") || texto.contains("dontask") { return None; }
    (interaction::ready(screen,"claude") && (texto.contains("? for shortcuts") || texto.contains("shift+tab to cycle"))).then_some("default")
}

pub(super) fn modo_da_sessao(session: &Arc<LiveSession>) -> Option<&'static str> {
    let (provider, linha) = session.meta.lock().ok().map(|m| (m.provider.clone(), m.initial_command.clone()))?;
    // Fork (modo do Codex na Conversa): o modo do Codex é o da linha com que ele abriu.
    if provider.as_deref() == Some("codex") { return linha.as_deref().and_then(omni_core::modo_do_codex::da_linha); }
    if provider.as_deref() != Some("claude") { return None; }
    // A frente de ganchos pode fornecer aqui o modo confirmado pelo evento da CLI.
    if let Some(modo) = interaction::modo_da_sessao(session) { return Some(modo); }
    modo_na_tela(session.interaction.lock().ok()?.parser.screen())
}

/// `None` = a CLI ainda não confirmou. Não deduzimos o estado do último clique da UI.
pub(super) fn fast_da_sessao(session: &Arc<LiveSession>) -> Option<bool> {
    let context = session.interaction.lock().ok()?;
    let texto = context.parser.screen().contents();
    let rodape = texto.lines().rev().take(5).collect::<Vec<_>>().join(" ").to_lowercase();
    if rodape.contains('↯') || rodape.contains("fast mode on") { Some(true) }
    else if rodape.contains("fast mode off") { Some(false) } else { None }
}

fn arquivo_de_modos(dir: &Path) -> std::path::PathBuf { dir.join("modos-das-conversas.json") }
/// Mantém os outros argumentos do lançamento; troca apenas uma flag conhecida de permissão.
pub(super) fn com_modo(base: &str, modo: &str) -> Result<String> {
    if !oficiais::modo_valido(modo) { return Err(anyhow!("Modo inválido")); }
    let mut comando = base.to_owned();
    for anterior in ["default", "acceptEdits", "plan", "auto", "bypassPermissions", "dontAsk", "manual"] {
        let flag = format!(" --permission-mode {anterior}");
        comando = comando.replace(&format!("{flag} "), " ");
        if comando.ends_with(&flag) { comando.truncate(comando.len()-flag.len()); }
    }
    Ok(format!("{comando} --permission-mode {modo}"))
}
pub(super) fn modo_salvo(dir: &Path, id: &str) -> Option<String> {
    let mapa: HashMap<String,String> = serde_json::from_slice(&std::fs::read(arquivo_de_modos(dir)).ok()?).ok()?;
    // O modo do Codex mora no mesmo arquivo (cada uso confere se o valor é do seu agente).
    mapa.get(id).filter(|m| oficiais::modo_valido(m) || omni_core::modo_do_codex::valido(m)).cloned()
}
pub(super) fn guardar_modo(state: &EngineState, id: &str, modo: &str) -> Result<()> {
    let _lock = state.persist_lock.lock().map_err(|_|anyhow!("persist poisoned"))?;
    let arquivo = arquivo_de_modos(state.state_file.parent().unwrap());
    let mut mapa: HashMap<String,String> = match std::fs::read(&arquivo) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => return Err(e.into()),
    };
    mapa.insert(id.into(),modo.into());
    atomic_write_json(&arquivo,&serde_json::to_value(mapa)?)?;
    Ok(())
}

pub(super) fn executar(state: &Arc<EngineState>, action: &Action, pedido: &Pedido) -> Result<()> {
    executar_com(state,action,pedido,interaction::provider_running)
}

fn executar_com(state: &Arc<EngineState>, action: &Action, pedido: &Pedido, rodando: impl Fn(Option<u32>, &str) -> bool) -> Result<()> {
    match pedido {
        Pedido::Slash(body) => {
            let (_,session) = current(state,&action.conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
            let session = session.ok_or_else(||anyhow!("Sessão encerrada"))?;
            let provider = session.meta.lock().map_err(|_|anyhow!("metadata poisoned"))?.provider.clone().unwrap_or_default();
            let texto = oficiais::montar(&provider,&body.comando,body.argumento.as_deref()).map_err(|e|anyhow!(e))?;
            // /compact é comando da TUI; no app-server usamos o RPC oficial, sem turn/start.
            if body.comando == "compact" && codex_app_server::ativo(&action.session_id) {
                anyhow::ensure!(now_ms().saturating_sub(action.created_at_ms) < 60_000, "Ação expirada");
                let sessions = state.sessions.lock().map_err(|_|anyhow!("sessions poisoned"))?;
                anyhow::ensure!(matches!(sessions.get(&action.session_id),Some(SessionEntry::Live(s)) if Arc::ptr_eq(s,&session)), "Sessão trocada");
                anyhow::ensure!(!session.reserved.load(Ordering::SeqCst), "Sessão reservada");
                drop(sessions);
                return codex_app_server::compactar(&action.session_id,&action.revision);
            }
            if body.comando == "fast" && opcoes_da_conversa(state,&action.conversation_id).map_err(|_|anyhow!("Opções indisponíveis"))?["fast_supported"] != true {
                return Err(anyhow!("O modelo atual não confirma suporte a fast"));
            }
            let antes = (body.comando == "branch").then(|| transcritos_da_conversa(state,&action.conversation_id)).transpose()?;
            // Reutiliza integralmente a porta do prompt: sessão, revisão, CLI ativa, campo vazio,
            // bracketed paste e Enter separado. O texto acima veio exclusivamente da lista fechada.
            let prompt = Action { kind: ActionKind::Prompt(texto), ..action.clone() };
            execute_checked(state,&prompt,rodando)?;
            if let Some(antes) = antes { acompanhar_branch(state.clone(),action.conversation_id.clone(),session,antes); }
            Ok(())
        }
        // Skill/comando personalizado do Claude. O texto já passou por `texto_para_cli` na rota;
        // aqui só confere que a CLI viva ainda é o Claude e usa a mesma porta do prompt.
        Pedido::Barra(texto) => {
            let (_,session) = current(state,&action.conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
            let session = session.ok_or_else(||anyhow!("Sessão encerrada"))?;
            let provider = session.meta.lock().map_err(|_|anyhow!("metadata poisoned"))?.provider.clone().unwrap_or_default();
            if provider != "claude" { return Err(anyhow!("Skills por / só no Claude Code")); }
            let prompt = Action { kind: ActionKind::Prompt(texto.clone()), ..action.clone() };
            execute_checked(state,&prompt,rodando)
        }
        Pedido::ModoCodex(alvo) => modo_codex::trocar(state,action,alvo),
        Pedido::Modo(alvo) => {
            if !oficiais::modo_valido(alvo) { return Err(anyhow!("Modo inválido")); }
            let (_,session) = current(state,&action.conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
            let session = session.ok_or_else(||anyhow!("Sessão encerrada"))?;
            let meta = session.meta.lock().map_err(|_|anyhow!("metadata poisoned"))?.clone();
            if meta.provider.as_deref() != Some("claude") || !rodando(meta.pid,"claude") { return Err(anyhow!("CLI não está mais ativo")); }
            {
                let context = session.interaction.lock().map_err(|_|anyhow!("screen poisoned"))?;
                if now_ms().saturating_sub(action.created_at_ms) >= 60_000 || context.revision(&session) != action.revision
                    || session.reserved.load(Ordering::SeqCst) || !interaction::ready(context.parser.screen(),"claude") {
                    return Err(anyhow!("Contexto mudou antes da execução"));
                }
            }
            ciclar_modo(alvo, || modo_da_sessao(&session), |tecla| {
                let sessions = state.sessions.lock().map_err(|_|anyhow!("sessions poisoned"))?;
                if !matches!(sessions.get(&action.session_id),Some(SessionEntry::Live(s)) if Arc::ptr_eq(s,&session)) { return Err(anyhow!("Sessão trocada")); }
                let mut context = session.interaction.lock().map_err(|_|anyhow!("screen poisoned"))?;
                if session.reserved.load(Ordering::SeqCst) || !interaction::ready(context.parser.screen(),"claude") { return Err(anyhow!("Campo ocupado")); }
                context.input_revision += 1;
                let mut writer = session.writer.lock().map_err(|_|anyhow!("writer poisoned"))?;
                writer.write_all(tecla)?; writer.flush()?;
                Ok(())
            }, || std::thread::sleep(std::time::Duration::from_millis(80)))?;
            guardar_modo(state,&action.conversation_id,alvo)?;
            // O reinício comum reutiliza initial_command; persistimos também nele a escolha.
            let mut meta = session.meta.lock().map_err(|_|anyhow!("metadata poisoned"))?;
            if let Some(base) = &meta.initial_command { meta.initial_command = Some(com_modo(base,alvo)?); }
            drop(meta);
            state.persist()?;
            Ok(())
        }
    }
}

/// Relê o selo a cada tecla; não pressupõe ordem do ciclo nem disponibilidade de Auto.
fn ciclar_modo(alvo: &str, ler: impl Fn() -> Option<&'static str>, escrever: impl Fn(&[u8]) -> Result<()>, pausa: impl Fn()) -> Result<()> {
    if ler() == Some(alvo) { return Ok(()); }
    let mut tecla: &[u8] = b"\x1b[Z";
    for volta in 0..8 {
        let antes = ler();
        escrever(tecla)?;
        let mut agora = antes;
        for _ in 0..15 { pausa(); agora = ler(); if agora != antes { break; } }
        if agora == Some(alvo) { return Ok(()); }
        if agora == antes && volta >= 1 { tecla = b"\x1bm"; }
    }
    Err(anyhow!("A CLI não confirmou o modo pedido; ele pode estar indisponível nesta conta"))
}

struct Transcritos {
    pasta: PathBuf,
    arquivos: std::collections::HashSet<PathBuf>,
    sessao_original: Option<String>,
}
fn transcritos_da_conversa(state: &EngineState, id: &str) -> Result<Transcritos> {
    let (c,_) = current(state,id).map_err(|_|anyhow!("Conversa removida"))?;
    let s = c.segments.last().ok_or_else(||anyhow!("Conversa sem sessão"))?;
    let perfis = omni_core::profiles(state.state_file.parent().unwrap());
    let perfil = perfis.iter().find(|p| Some(&p.id) == s.profile_id.as_ref()).ok_or_else(||anyhow!("Conta não encontrada"))?;
    let pasta = omni_core::conversations::claude_transcript_path(Path::new(&perfil.config_dir),&c.cwd,s.external_session_id.as_deref().unwrap_or("sessao")).parent().unwrap().to_path_buf();
    let arquivos = listar_transcritos(&pasta);
    Ok(Transcritos { pasta, arquivos, sessao_original:s.external_session_id.clone() })
}
fn listar_transcritos(pasta: &Path) -> std::collections::HashSet<std::path::PathBuf> {
    std::fs::read_dir(pasta).into_iter().flatten().flatten().map(|e|e.path())
        .filter(|p|p.extension().is_some_and(|e|e == "jsonl")).collect()
}
fn acompanhar_branch(state: Arc<EngineState>, id: String, session: Arc<LiveSession>, antes: Transcritos) {
    std::thread::spawn(move || {
        for _ in 0..120 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            // Ganchos têm prioridade: se já trocaram a identidade, nada para descobrir aqui.
            let Ok((c,Some(atual))) = current(&state,&id) else { return };
            if !Arc::ptr_eq(&atual,&session) { return; }
            let novo_id = session.meta.lock().ok().and_then(|m|m.external_session_id.clone());
            if novo_id != antes.sessao_original || c.segments.last().and_then(|s|s.external_session_id.clone()) != antes.sessao_original { return; }
            let novos: Vec<_> = listar_transcritos(&antes.pasta).difference(&antes.arquivos).cloned().collect();
            // Duas CLIs na mesma pasta: é melhor manter o ponteiro que atribuir a conversa errada.
            if novos.len() > 1 { return; }
            let Some(path) = novos.first() else { continue };
            let Some(sid) = path.file_stem().and_then(|s|s.to_str()) else { return };
            if !oficiais::valor_seguro(sid) { return; }
            let Ok(_lock) = state.persist_lock.lock() else { return };
            let dir = state.state_file.parent().unwrap();
            // Ler, alterar e gravar numa transação só (trava do índice, achado 4 da revisão).
            let trocou = omni_core::indice_de_conversas::mudar_indice(dir, |indice| {
                let Some(c) = indice.iter_mut().find(|c|c.id == id) else { return false };
                let Some(ultimo) = c.segments.last_mut() else { return false };
                if ultimo.external_session_id != antes.sessao_original { return false; }
                if ultimo.external_session_id.as_deref() == Some(sid) { return false; }
                let mut segmento = ultimo.clone();
                ultimo.ended_at_ms = Some(now_ms());
                segmento.external_session_id = Some(sid.into()); segmento.transcript_path = Some(path.to_string_lossy().into_owned());
                segmento.started_at_ms = now_ms(); segmento.ended_at_ms = None;
                c.segments.push(segmento);
                true
            });
            if matches!(trocou, Ok(true)) {
                if let Ok(mut meta) = session.meta.lock() { meta.external_session_id = Some(sid.into()); }
            }
            return;
        }
    });
}

#[cfg(test)]
#[path = "comandos_testes.rs"]
mod testes;
