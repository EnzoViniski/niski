//! Conta, modelo e esforço da conversa: opções, ajustes, retomada (`--resume`), `reabrir_com` e levar o transcript para outra conta.
//! Fork: saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// Uma conversa sem sessão aberta (do índice do OMNI ou só do histórico da CLI) e como reabri-la.
/// Resolvido no servidor antes da fila, como o `SpawnPlan`: o corpo do request só escolhe conta,
/// modelo e esforço.
#[derive(Clone, PartialEq)]
pub(super) struct Retomada {
    pub(super) conversation_id: String,
    pub(super) project_id: String,
    pub(super) cwd: String,
    pub(super) title: String,
    pub(super) provider: String,
    pub(super) profile_id: Option<String>,
    pub(super) session_id: String,
    /// O arquivo da conversa na conta de origem — é ele que vai junto se a conta mudar.
    pub(super) transcript: Option<String>,
    pub(super) ajustes: Ajustes,
}

/// Conta, modelo e esforço da conversa. `None` em modelo/esforço = o padrão da CLI.
#[derive(Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Ajustes { pub(super) profile_id: Option<String>, pub(super) model: Option<String>, pub(super) effort: Option<String> }

/* ── Conta, modelo e esforço ─────────────────────────────────────────────────────────────────
   Trocar qualquer um dos três é a mesma operação: reabrir a **mesma** sessão (mesmo id — a aba do
   PC continua ligada nela) com `--resume` e os parâmetros novos. As duas CLIs aceitam modelo e
   esforço na linha de comando; trocar por comando interno (`/model`) muda de versão para versão.
   Trocar de conta leva o transcript junto para o config dir da conta nova, como o failover. */


/// Só letras, números e `._-[]`: o valor vira argumento de um comando digitado num shell.
pub(super) fn valor_seguro(valor: &str) -> bool {
    omni_core::comandos_oficiais::valor_seguro(valor)
}

/// Modelos do Codex que a conta enxerga (`models_cache.json` do CODEX_HOME), com os esforços de cada um.
pub(super) fn modelos_codex(config_dir: &Path) -> Vec<Value> {
    let Ok(bytes) = std::fs::read(config_dir.join("models_cache.json")) else { return Vec::new() };
    let Ok(cache) = serde_json::from_slice::<Value>(&bytes) else { return Vec::new() };
    cache["models"].as_array().into_iter().flatten()
        .filter(|modelo| modelo["visibility"].as_str() != Some("hide"))
        .filter_map(|modelo| {
            let id = modelo["slug"].as_str()?;
            let esforcos: Vec<&str> = modelo["supported_reasoning_levels"].as_array().into_iter().flatten()
                .filter_map(|nivel| nivel["effort"].as_str().or_else(|| nivel.as_str())).collect();
            Some(json!({"id":id,"label":modelo["display_name"].as_str().unwrap_or(id),"efforts":esforcos}))
        }).collect()
}

/// Saída do `agy models` (slug e nome de cada variante). A lista vem da internet e leva uns segundos,
/// então fica guardada um dia em `modelos-antigravity.txt` no engine. Falhou e não há cópia: vazio, e
/// o seletor some em vez de travar a conversa.
pub(super) fn saida_agy_models(dir: &Path) -> String {
    let arquivo = dir.join("modelos-antigravity.txt");
    let fresca = std::fs::metadata(&arquivo).and_then(|m| m.modified()).ok()
        .and_then(|quando| quando.elapsed().ok()).is_some_and(|idade| idade < std::time::Duration::from_secs(24 * 3600));
    let guardada = std::fs::read_to_string(&arquivo).ok();
    if fresca { if let Some(texto) = guardada.clone() { return texto; } }
    omni_core::cli_path::ampliar_path();
    let mut comando = std::process::Command::new("agy");
    comando.arg("models").stdin(std::process::Stdio::null());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; comando.creation_flags(0x08000000); }
    match comando.output() {
        Ok(saida) if saida.status.success() && !omni_core::antigravity::variantes(&String::from_utf8_lossy(&saida.stdout)).is_empty() => {
            let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
            let _ = std::fs::write(&arquivo, &texto);
            texto
        }
        _ => guardada.unwrap_or_default(),
    }
}

/// Agente, conta, pasta e id nativo da última sessão — do índice do OMNI ou do histórico da CLI.
pub(super) fn ultima_sessao(state: &EngineState, id: &str) -> Result<omni_core::conversations::Segment, ApiError> {
    if let Ok((conversa,_)) = current(state,id) {
        return conversa.segments.last().cloned().ok_or_else(||error(StatusCode::CONFLICT,"Conversa sem sessão"));
    }
    let entrada = historical_entries(state).into_iter().find(|entrada| history_id(entrada) == id)
        .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
    Ok(omni_core::conversations::Segment {
        provider: entrada.provider, profile_id: Some(entrada.profile_id), external_session_id: Some(entrada.session_id),
        transcript_path: Some(entrada.path), terminal_session_id: None, started_at_ms: 0, ended_at_ms: None,
    })
}

pub(super) fn pasta_da_conversa(state: &EngineState, id: &str) -> Option<String> {
    current(state,id).map(|(c,_)| c.cwd).ok()
        .or_else(|| historical_entries(state).into_iter().find(|e| history_id(e) == id).and_then(|e| e.cwd))
}

pub(super) fn opcoes_da_conversa(state: &EngineState, id: &str) -> Result<Value, ApiError> {
    let dir = state.state_file.parent().unwrap();
    let segmento = ultima_sessao(state,id)?;
    let cwd = pasta_da_conversa(state,id).unwrap_or_default();
    let segmento = &segmento;
    let perfis: Vec<_> = omni_core::profiles(dir).into_iter().filter(|p| p.provider == segmento.provider).collect();
    let atual = perfis.iter().find(|p| Some(&p.id) == segmento.profile_id.as_ref());
    let runtime = match (segmento.provider.as_str(), atual) {
        ("claude", Some(perfil)) => segmento.external_session_id.as_deref()
            .and_then(|sid| omni_core::runtime::claude_runtime(Path::new(&perfil.config_dir),&cwd,sid)),
        ("codex", Some(perfil)) => omni_core::runtime::codex_runtime(Path::new(&perfil.config_dir),&cwd),
        _ => None,
    };
    let saida_agy = (segmento.provider == "antigravity").then(|| saida_agy_models(dir)).unwrap_or_default();
    // Antigravity: o modelo em uso sai do log da própria sessão, já como família + esforço.
    let (modelo_agy, esforco_agy) = match (segmento.provider.as_str(), atual, segmento.terminal_session_id.as_deref()) {
        ("antigravity", Some(perfil), Some(sessao)) => omni_core::antigravity::modelo_da_sessao(Path::new(&perfil.config_dir), sessao, &saida_agy)
            .map_or((None, None), |(familia, esforco)| (Some(familia), esforco)),
        _ => (None, None),
    };
    let modelos: Vec<Value> = match segmento.provider.as_str() {
        // Fork: apelidos e versões fixas, cada um com os esforços que aceita (`omni_core::modelos_claude`).
        "claude" => omni_core::modelos_claude::MODELOS.iter().map(|(id,label,esforcos)| json!({"id":id,"label":label,"efforts":esforcos})).collect(),
        "codex" => atual.map(|perfil| modelos_codex(Path::new(&perfil.config_dir))).unwrap_or_default(),
        "antigravity" => omni_core::antigravity::familias(&saida_agy).into_iter()
            .map(|f| json!({"id":f.id,"label":f.label,"efforts":f.esforcos})).collect(),
        _ => Vec::new(),
    };
    Ok(json!({
        "provider": segmento.provider,
        "profile_id": segmento.profile_id,
        "model": runtime.as_ref().and_then(|r| r.model.clone()).or(modelo_agy),
        "fast_supported": segmento.provider == "claude" && runtime.as_ref().and_then(|r| r.model.as_deref()).is_some_and(omni_core::comandos_oficiais::suporta_fast),
        "effort": runtime.as_ref().and_then(|r| r.effort.clone()).or(esforco_agy),
        "profiles": perfis.iter().map(|p| json!({"id":p.id,"name":p.name})).collect::<Vec<_>>(),
        "models": modelos,
        "can_resume": segmento.external_session_id.is_some(),
    }))
}

pub(super) async fn opcoes(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || opcoes_da_conversa(&state,&id).map(Json))
        .await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler as opções"))?
}

pub(super) async fn ajustar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<Ajustes>) -> Result<(StatusCode,Json<Value>),ApiError> {
    for valor in [&body.model, &body.effort, &body.profile_id].into_iter().flatten() {
        if !valor_seguro(valor) { return Err(error(StatusCode::BAD_REQUEST,"Valor inválido")); }
    }
    enqueue_async(state,id,headers,ActionKind::Ajustes(body)).await
}

/// Linha de comando que retoma a conversa com o modelo e o esforço pedidos.
pub(super) fn comando_de_retomada(base: &str, provider: &str, sessao: &str, ajustes: &Ajustes, modo: Option<&str>) -> Result<String> {
    // Antigravity: família + esforço viram a variante (`--model gemini-4-argon --effort high`). As
    // opções vão **antes** do `--conversation`: o reinício troca só o que vem depois dele.
    if provider == "antigravity" {
        let mut comando = base.to_owned();
        if let Some(modelo) = &ajustes.model { comando += &format!(" --model {modelo}"); }
        if let Some(esforco) = &ajustes.effort { comando += &format!(" --effort {esforco}"); }
        return Ok(format!("{comando} --conversation {sessao}"));
    }
    let mut comando = match provider {
        "claude" => format!("{base} --resume {sessao}"),
        "codex" => format!("{base} resume {sessao}"),
        _ => return Err(anyhow!("Este agente não permite trocar conta, modelo ou esforço")),
    };
    if let Some(modelo) = &ajustes.model {
        comando += &if provider == "claude" { format!(" --model {modelo}") } else { format!(" -m {modelo}") };
    }
    if let Some(esforco) = &ajustes.effort {
        // Sem aspas de propósito: o `-c` do Codex lê o valor como TOML e, se não for TOML válido,
        // usa o texto cru — e aspas mudariam de sentido entre bash e PowerShell.
        comando += &if provider == "claude" { format!(" --effort {esforco}") } else { format!(" -c model_reasoning_effort={esforco}") };
    }
    // Fork: modo confirmado por conversa, preservado na retomada sem mudar o padrão global.
    if provider == "claude" {
        if let Some(modo) = modo { comando = comandos::com_modo(&comando,modo)?; }
    }
    Ok(comando)
}

/// Copia o arquivo da conversa para onde a CLI da conta de destino procura: Claude por pasta do
/// projeto + id; Codex na mesma posição relativa dentro do CODEX_HOME. Devolve o caminho novo.
pub(super) fn levar_transcript(perfis: &[omni_core::Profile], provider: &str, conta_origem: Option<&str>, origem: &Path,
    alvo: &omni_core::Profile, cwd: &str, sessao: &str) -> Result<String> {
    let destino = match provider {
        "claude" => omni_core::conversations::claude_transcript_path(Path::new(&alvo.config_dir),cwd,sessao),
        _ => {
            let raiz = perfis.iter().find(|p| Some(p.id.as_str()) == conta_origem)
                .and_then(|p| Path::new(&p.config_dir).canonicalize().ok()).ok_or_else(||anyhow!("Conta de origem não encontrada"))?;
            let relativo = origem.canonicalize()?.strip_prefix(&raiz).map(Path::to_path_buf)
                .map_err(|_|anyhow!("Transcript fora da conta de origem"))?;
            Path::new(&alvo.config_dir).join(relativo)
        }
    };
    if destino != origem {
        if let Some(pasta) = destino.parent() { std::fs::create_dir_all(pasta)?; }
        std::fs::copy(origem,&destino)?;
    }
    Ok(destino.to_string_lossy().into_owned())
}

/// Conversa do histórico que vai ser retomada noutra conta: copia o transcript para lá antes do
/// `--resume` (a CLI só acha conversas da própria conta). O pedido nomeia a conversa pelos campos
/// do histórico, nunca por caminho — quem acha o arquivo é a varredura do próprio engine.
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
pub(super) struct LevarBody { provider: String, profile_id: String, session_id: String, destino: String }
pub(super) async fn levar_do_historico(State(state): State<Arc<EngineState>>, Json(body): Json<LevarBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let entrada = historical_entries(&state).into_iter()
            .find(|e| e.provider == body.provider && e.profile_id == body.profile_id && e.session_id == body.session_id)
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada no histórico"))?;
        if body.destino == entrada.profile_id { return Ok(Json(json!({"ok":true}))); }
        let perfis = omni_core::profiles(state.state_file.parent().unwrap());
        let alvo = perfis.iter().find(|p| p.id == body.destino && p.provider == entrada.provider)
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Conta não encontrada para este agente"))?;
        let cwd = entrada.cwd.clone().ok_or_else(||error(StatusCode::CONFLICT,"Essa conversa não registrou a pasta onde rodou"))?;
        levar_transcript(&perfis,&entrada.provider,Some(&entrada.profile_id),Path::new(&entrada.path),alvo,&cwd,&entrada.session_id)
            .map_err(|e|error(StatusCode::CONFLICT,&format!("Não deu para levar a conversa para a conta {}: {e}",alvo.name)))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao copiar a conversa"))?
}

/* ── Retomar uma conversa sem sessão aberta ──────────────────────────────────────────────────
   Conversa do histórico (ou do OMNI, com a sessão já fechada) não tem o que "reabrir": abre uma
   sessão nova na pasta do projeto com `--resume`, já com a conta, o modelo e o esforço escolhidos. */

pub(super) fn plano_de_retomada(state: &EngineState, id: &str, ajustes: Ajustes) -> Result<Retomada, ApiError> {
    let dir = state.state_file.parent().unwrap();
    if let Ok((conversa, viva)) = current(state,id) {
        if viva.is_some() { return Err(error(StatusCode::CONFLICT,"Esta conversa já está aberta no PC")); }
        let segmento = conversa.segments.last().ok_or_else(||error(StatusCode::CONFLICT,"Conversa sem sessão"))?;
        let session_id = segmento.external_session_id.clone().ok_or_else(||error(StatusCode::CONFLICT,"Esta conversa não chegou a ter mensagens para retomar"))?;
        let transcript = omni_core::conversations::resolve_transcript(segmento,&conversa,&omni_core::profiles(dir))
            .map(|p| p.to_string_lossy().into_owned()).or(segmento.transcript_path.clone());
        return Ok(Retomada { conversation_id: conversa.id.clone(), project_id: conversa.project_id.clone(), cwd: conversa.cwd.clone(),
            title: omni_core::conversations::display_title(&conversa), provider: segmento.provider.clone(),
            profile_id: segmento.profile_id.clone(), session_id, transcript, ajustes });
    }
    let entrada = historical_entries(state).into_iter().find(|entrada| history_id(entrada) == id)
        .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
    let cwd = entrada.cwd.clone().filter(|cwd| Path::new(cwd).is_dir())
        .ok_or_else(||error(StatusCode::CONFLICT,"A pasta desta conversa não existe mais neste PC"))?;
    let project_id = known_projects(state).into_iter().find(|p| same_path(&p.path,&cwd)).map(|p| p.id)
        .unwrap_or_else(|| history_project_id(&cwd));
    Ok(Retomada {
        // Id estável: um segundo toque (mesma chave) acha a mesma conversa em vez de criar outra.
        conversation_id: stable_id("conv",&[id]), project_id, cwd,
        title: entrada.title.clone().or(entrada.first_prompt.clone()).unwrap_or_else(||"Conversa retomada".into()),
        provider: entrada.provider.clone(), profile_id: Some(entrada.profile_id.clone()), session_id: entrada.session_id.clone(),
        transcript: Some(entrada.path.clone()), ajustes,
    })
}

pub(super) async fn retomar_rota(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<Ajustes>) -> Result<(StatusCode,Json<Value>),ApiError> {
    for valor in [&body.model, &body.effort, &body.profile_id].into_iter().flatten() {
        if !valor_seguro(valor) { return Err(error(StatusCode::BAD_REQUEST,"Valor inválido")); }
    }
    tokio::task::spawn_blocking(move || {
        limite_de_sessoes::conferir(&state)?; // Fork: teto único e configurável.
        let plano = plano_de_retomada(&state,&id,body)?;
        let conversa = plano.conversation_id.clone();
        enqueue(&state,&conversa,&headers,ActionKind::Retomar(plano))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha na fila"))?
}

pub(super) fn retomar(state: &Arc<EngineState>, action: &Action, plano: &Retomada) -> Result<()> {
    if now_ms().saturating_sub(action.created_at_ms) >= 60_000 { return Err(anyhow!("Ação expirada")); }
    let dir = state.state_file.parent().unwrap().to_path_buf();
    let perfis = omni_core::profiles(&dir);
    let alvo_id = plano.ajustes.profile_id.clone().or(plano.profile_id.clone());
    let alvo = perfis.iter().find(|p| Some(&p.id) == alvo_id.as_ref() && p.provider == plano.provider);
    if alvo_id.is_some() && alvo.is_none() { return Err(anyhow!("Conta não encontrada para este agente")); }

    let mut transcript = plano.transcript.clone();
    if alvo_id != plano.profile_id {
        let alvo = alvo.ok_or_else(||anyhow!("Conta não encontrada"))?;
        let origem = plano.transcript.as_deref().map(PathBuf::from).filter(|p| p.is_file())
            .ok_or_else(||anyhow!("Não achei o transcript desta conversa para levar à outra conta"))?;
        transcript = Some(levar_transcript(&perfis,&plano.provider,plano.profile_id.as_deref(),&origem,alvo,&plano.cwd,&plano.session_id)?);
    }
    let base = state.mobile.workspace.lock().map_err(|_|anyhow!("workspace poisoned"))?.agents.iter()
        .find(|agente| agente.id == plano.provider).map(|agente| agente.command.clone())
        .unwrap_or_else(|| plano.provider.clone());
    let comando = comando_de_retomada(&base,&plano.provider,&plano.session_id,&plano.ajustes,comandos::modo_salvo(&dir,&plano.conversation_id).as_deref())?;

    let agora = now_ms();
    let sessao = spawn_terminal_inner(state, state.next_id(), plano.project_id.clone(), plano.title.clone(), plano.cwd.clone(),
        None, Some(comando), 30, 120, agora,
        SessionOrigin {
            headless: true,
            env: alvo.map(omni_core::env_for).unwrap_or_default(),
            provider: Some(plano.provider.clone()),
            profile_id: alvo_id.clone(),
            conversation_id: Some(plano.conversation_id.clone()),
            external_session_id: Some(plano.session_id.clone()),
        })?;
    let segmento = omni_core::conversations::Segment {
        provider: plano.provider.clone(), profile_id: alvo_id, external_session_id: Some(plano.session_id.clone()),
        transcript_path: transcript, terminal_session_id: Some(sessao.id.clone()), started_at_ms: agora, ended_at_ms: None,
    };
    // Fork (fix-orquestracao): ler-alterar-gravar sob a trava do índice.
    omni_core::indice_de_conversas::mudar_indice(&dir, |indice| {
        match indice.iter_mut().find(|c| c.id == plano.conversation_id) {
            Some(conversa) => {
                if let Some(ultimo) = conversa.segments.last_mut() { ultimo.ended_at_ms.get_or_insert(agora); }
                conversa.segments.push(segmento);
            }
            None => indice.push(omni_core::conversations::Conversation {
                id: plano.conversation_id.clone(), project_id: plano.project_id.clone(), cwd: plano.cwd.clone(),
                title: plano.title.clone(), title_source: omni_core::conversations::TitleSource::Auto,
                created_at_ms: agora, segments: vec![segmento],
            }),
        }
        true
    })?;
    if plano.provider == "claude" { passar_pela_confianca(state,&sessao.id); }
    Ok(())
}

pub(super) fn reabrir_com(state: &Arc<EngineState>, conversation_id: &str, ajustes: &Ajustes) -> Result<()> {
    let dir = state.state_file.parent().unwrap().to_path_buf();
    let (_, sessao) = current(state,conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
    let sessao = sessao.ok_or_else(||anyhow!("Sessão encerrada"))?;
    let mut indice = omni_core::conversations::read_index(&dir);
    let conversa = indice.iter_mut().find(|c| c.id == conversation_id).ok_or_else(||anyhow!("Conversa removida"))?;
    let segmento = conversa.segments.last().cloned().ok_or_else(||anyhow!("Conversa sem sessão"))?;
    let sessao_externa = segmento.external_session_id.clone()
        .ok_or_else(||anyhow!("Mande a primeira mensagem antes: ainda não há conversa para retomar"))?;
    let perfis = omni_core::profiles(&dir);
    let alvo_id = ajustes.profile_id.clone().or(segmento.profile_id.clone());
    let alvo = perfis.iter().find(|p| Some(&p.id) == alvo_id.as_ref() && p.provider == segmento.provider);
    if alvo_id.is_some() && alvo.is_none() { return Err(anyhow!("Conta não encontrada para este agente")); }

    // Conversa que ainda não teve mensagem: o Claude não gravou nada, e `--resume` respondia "No
    // conversation found" — a sessão morria ao trocar o modelo antes de escrever. Sem transcript, o
    // Claude reabre com o **mesmo** id via `--session-id`, já com os ajustes novos.
    let gravada = omni_core::conversations::resolve_transcript(&segmento,conversa,&perfis).is_some_and(|p| p.is_file())
        || perfis.iter().find(|p| Some(&p.id) == segmento.profile_id.as_ref())
            .is_some_and(|p| omni_core::conversations::claude_transcript_path(Path::new(&p.config_dir),&conversa.cwd,&sessao_externa).is_file());
    let nova_sem_mensagem = segmento.provider == "claude" && !gravada;

    // Outra conta: o transcript vai junto, no lugar onde a CLI da conta nova procura.
    let trocou_conta = alvo_id != segmento.profile_id;
    let mut transcript = segmento.transcript_path.clone();
    if trocou_conta && !nova_sem_mensagem {
        let alvo = alvo.ok_or_else(||anyhow!("Conta não encontrada"))?;
        let origem = omni_core::conversations::resolve_transcript(&segmento,conversa,&perfis)
            .ok_or_else(||anyhow!("Não achei o transcript desta conversa para levar à outra conta"))?;
        transcript = Some(levar_transcript(&perfis,&segmento.provider,segmento.profile_id.as_deref(),&origem,alvo,&conversa.cwd,&sessao_externa)?);
    }

    let base = state.mobile.workspace.lock().map_err(|_|anyhow!("workspace poisoned"))?.agents.iter()
        .find(|agente| agente.id == segmento.provider).map(|agente| agente.command.clone())
        .unwrap_or_else(|| segmento.provider.clone());
    let comando = if nova_sem_mensagem {
        comando_de_retomada(&base,&segmento.provider,&sessao_externa,ajustes,comandos::modo_da_sessao(&sessao).map(str::to_owned).or_else(||comandos::modo_salvo(&dir,conversation_id)).as_deref())?.replacen(" --resume "," --session-id ",1)
    } else {
        comando_de_retomada(&base,&segmento.provider,&sessao_externa,ajustes,comandos::modo_da_sessao(&sessao).map(str::to_owned).or_else(||comandos::modo_salvo(&dir,conversation_id)).as_deref())?
    };
    let sessao_id = {
        let mut meta = sessao.meta.lock().map_err(|_|anyhow!("metadata poisoned"))?;
        // Só as variáveis de conta mudam; o resto do ambiente da sessão fica como estava.
        meta.env.retain(|(chave,_)| chave != "CLAUDE_CONFIG_DIR" && chave != "CODEX_HOME");
        if let Some(alvo) = alvo { meta.env.extend(omni_core::env_for(alvo)); }
        meta.profile_id = alvo_id.clone();
        meta.initial_command = Some(comando);
        meta.external_session_id = Some(sessao_externa.clone());
        meta.id.clone()
    };
    if trocou_conta {
        let agora = now_ms();
        let novo = omni_core::conversations::Segment {
            provider: segmento.provider.clone(), profile_id: alvo_id, external_session_id: Some(sessao_externa),
            transcript_path: transcript, terminal_session_id: Some(sessao_id.clone()), started_at_ms: agora, ended_at_ms: None,
        };
        // Fork (fix-orquestracao): o segmento novo entra no índice relido sob a trava.
        omni_core::indice_de_conversas::mudar_indice(&dir, |indice| {
            let Some(conversa) = indice.iter_mut().find(|c| c.id == conversation_id) else { return false };
            if let Some(ultimo) = conversa.segments.last_mut() { ultimo.ended_at_ms = Some(agora); }
            conversa.segments.push(novo);
            true
        })?;
    }
    restart_session(state,&sessao_id)?;
    // Conta nova numa pasta que ela nunca abriu: o Claude pergunta se confia nela.
    if segmento.provider == "claude" { passar_pela_confianca(state,&sessao_id); }
    Ok(())
}
