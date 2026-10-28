//! Fork (trocar agente): "Continuar com…" — trocar o agente (ou a conta) **sem sair da conversa**.
//!
//! - `GET  /conversas/{id}/trocar-agente`: agentes instalados (os que o PC publicou) com as contas de
//!   cada um, o agente/conta atual e se dá para trocar agora.
//! - `POST /conversas/{id}/trocar-agente` `{provider, profile_id?, mensagem?}`:
//!   - mesmo agente, outra conta: o caminho de sempre (`reabrir_com`/`retomar`: o transcript vai junto
//!     e a CLI retoma pelo mesmo id). Nada muda aqui.
//!   - outro agente: gera a passagem de bastão (`omni_core::passagem`) a partir dos transcripts de
//!     todos os trechos, grava em `<projeto>/.omni-agents/passagens/<conversa>-<n>.md`, troca a CLI na
//!     **mesma PTY** (o id do terminal não muda: a aba do PC, a lateral e o celular continuam ligados
//!     nela), abre um trecho novo no índice e, quando a CLI nova fica pronta, manda o pedido curto
//!     ("leia o arquivo…") pela fila do chat — com a próxima mensagem do usuário junto, se ele já tinha
//!     escrito.
//!
//! A troca só acontece com o agente parado: no meio de um turno, encerrar a CLI mataria o trabalho.
//! O que já foi feito está no transcript que a CLI gravou e nos arquivos do disco; nada se perde.
use super::*;

/// Conversas com troca em andamento: um toque duplo não abre duas CLIs.
static EM_ANDAMENTO: std::sync::LazyLock<Mutex<HashSet<String>>> = std::sync::LazyLock::new(Default::default);

/// Agentes que a visão Conversa lê (transcript + tela reconhecida). O Cursor troca, mas segue no terminal.
const COM_CONVERSA: [&str; 3] = ["claude", "codex", "antigravity"];
/// Quanto esperar a CLI nova ficar pronta para receber a passagem.
const PRAZO_DA_PASSAGEM: std::time::Duration = std::time::Duration::from_secs(180);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PedidoDeTroca {
    provider: String,
    #[serde(default)] profile_id: Option<String>,
    /// O que o usuário já tinha escrito no campo: vai depois da passagem, na mesma mensagem.
    #[serde(default)] mensagem: Option<String>,
}

fn sessao_ocupada(sessao: &Arc<LiveSession>) -> Option<&'static str> {
    if sessao.reserved.load(Ordering::SeqCst) { return Some("Consulta de uso em andamento; tente de novo em alguns segundos"); }
    // A CLI que mostra esta trava não está trabalhando. A passagem não mexe na trava externa.
    if sessao.interaction.lock().is_ok_and(|i| !i.turn_active && crate::conversa_bloqueada::reconhece(&i.parser.screen().contents())) { return None; }
    let cap = interaction::capabilities(sessao);
    if cap.approve || cap.escolha.is_some() || cap.pedido.is_some() { return Some("Responda a pergunta do agente antes de trocar"); }
    if cap.prompt { return None; }
    if codex_app_server::ativo(&sessao.meta.lock().map(|m| m.id.clone()).unwrap_or_default()) {
        return Some("O agente está trabalhando; espere terminar (ou pare) antes de trocar");
    }
    let (pid, provider) = sessao.meta.lock().map(|m| (m.pid, m.provider.clone().unwrap_or_default())).unwrap_or((None, String::new()));
    // CLI fora do ar (saiu, travou) não tem turno a perder: troca. Rodando sem o campo pronto, não.
    interaction::provider_running(pid, &provider)
        .then_some("O agente está trabalhando (ou a tela não foi reconhecida); espere terminar ou pare antes de trocar")
}

pub(super) fn opcoes_de_troca(state: &EngineState, id: &str) -> Result<Value, ApiError> {
    let (conversa, viva) = current(state, id)?;
    let atual = conversa.segments.last();
    let perfis = omni_core::profiles(state.state_file.parent().unwrap());
    let agentes = state.mobile.workspace.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "workspace poisoned"))?.agents.clone();
    let lista: Vec<Value> = agentes.iter().map(|agente| json!({
        "id": agente.id, "label": agente.label,
        "conversa": COM_CONVERSA.contains(&agente.id.as_str()),
        "profiles": perfis.iter().filter(|p| p.provider == agente.id).map(|p| json!({"id": p.id, "name": p.name})).collect::<Vec<_>>(),
    })).collect();
    let motivo = viva.as_ref().and_then(sessao_ocupada);
    Ok(json!({
        "provider": atual.map(|s| s.provider.clone()),
        "profile_id": atual.and_then(|s| s.profile_id.clone()),
        "agentes": lista,
        "pode_trocar": motivo.is_none(),
        "motivo": motivo,
        "live": viva.is_some(),
    }))
}

async fn opcoes(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || opcoes_de_troca(&state, &id).map(Json))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ler as opções"))?
}

fn mensagem_valida(mensagem: &str) -> bool {
    mensagem.len() <= 16_000 && !mensagem.trim_start().starts_with('/') && !mensagem.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}

/// Texto que vai entre aspas duplas na linha de comando (só o Cursor, que não tem campo reconhecido):
/// nada que bash, zsh, PowerShell ou cmd interpretem dentro de aspas (`$`, `` ` ``, `!`, `"`, `\`, `%`).
fn para_a_linha_de_comando(texto: &str) -> String {
    texto.chars().map(|c| if c.is_alphanumeric() || " .,;:()/-_·".contains(c) { c } else { ' ' }).collect::<String>()
        .split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn trocar(state: &Arc<EngineState>, id: &str, pedido: PedidoDeTroca) -> Result<Value, ApiError> {
    if !valor_seguro(&pedido.provider) || pedido.profile_id.as_deref().is_some_and(|p| !valor_seguro(p)) {
        return Err(error(StatusCode::BAD_REQUEST, "Valor inválido"));
    }
    let mensagem = pedido.mensagem.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(str::to_owned);
    if mensagem.as_deref().is_some_and(|m| !mensagem_valida(m)) {
        return Err(error(StatusCode::BAD_REQUEST, "Mensagem inválida: até 16000 bytes, sem comandos slash ou caracteres de controle"));
    }
    // Uma troca por conversa de cada vez.
    if !EM_ANDAMENTO.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "trava poisoned"))?.insert(id.to_owned()) {
        return Err(error(StatusCode::CONFLICT, "Já estou trocando o agente desta conversa"));
    }
    let resultado = trocar_travado(state, id, &pedido, mensagem);
    if let Ok(mut trava) = EM_ANDAMENTO.lock() { trava.remove(id); }
    resultado
}

fn trocar_travado(state: &Arc<EngineState>, id: &str, pedido: &PedidoDeTroca, mensagem: Option<String>) -> Result<Value, ApiError> {
    let dir = state.state_file.parent().unwrap().to_path_buf();
    let (conversa, viva) = current(state, id)?;
    let atual = conversa.segments.last().cloned().ok_or_else(|| error(StatusCode::CONFLICT, "Conversa sem sessão"))?;
    let agente = state.mobile.workspace.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "workspace poisoned"))?
        .agents.iter().find(|a| a.id == pedido.provider).cloned()
        .ok_or_else(|| error(StatusCode::CONFLICT, "Esse agente não está instalado neste PC (ou o app do PC ainda não publicou a lista)"))?;
    let perfis = omni_core::profiles(&dir);
    let alvo = match &pedido.profile_id {
        Some(conta) => Some(perfis.iter().find(|p| &p.id == conta && p.provider == pedido.provider).cloned()
            .ok_or_else(|| error(StatusCode::CONFLICT, "Conta desconhecida para este agente"))?),
        None if pedido.provider == atual.provider => atual.profile_id.as_ref().and_then(|c| perfis.iter().find(|p| &p.id == c).cloned()),
        None => perfis.iter().find(|p| p.provider == pedido.provider && p.builtin).or_else(|| perfis.iter().find(|p| p.provider == pedido.provider)).cloned(),
    };
    let alvo_id = alvo.as_ref().map(|p| p.id.clone());
    if pedido.provider == atual.provider && alvo_id == atual.profile_id {
        return Err(error(StatusCode::CONFLICT, "A conversa já está com esse agente e essa conta"));
    }
    if !Path::new(&conversa.cwd).is_dir() { return Err(retomada_rotas::erro_pasta_sumiu(state, &conversa.cwd)); }
    if let Some(motivo) = viva.as_ref().and_then(sessao_ocupada) { return Err(error(StatusCode::CONFLICT, motivo)); }

    // Mesmo agente, outra conta: continuação literal, como sempre foi.
    if pedido.provider == atual.provider {
        let ajustes = Ajustes { profile_id: alvo_id.clone(), model: None, effort: None };
        let falha = |e: anyhow::Error| error(StatusCode::CONFLICT, &format!("Não consegui trocar a conta: {e}"));
        let terminal = if viva.is_some() {
            reabrir_com(state, id, &ajustes).map_err(falha)?;
            atual.terminal_session_id.clone()
        } else {
            limite_de_sessoes::conferir(state)?;
            let plano = plano_de_retomada_em(state, id, ajustes, None)?;
            let acao = Action { id: state.next_id(), conversation_id: id.into(), session_id: String::new(), state: "executing".into(),
                created_at_ms: now_ms(), error: None, key: String::new(), revision: String::new(), kind: ActionKind::Retomar(plano.clone()) };
            retomar(state, &acao, &plano).map_err(falha)?;
            omni_core::conversations::read_index(&dir).into_iter().find(|c| c.id == id).and_then(|c| c.segments.last().and_then(|s| s.terminal_session_id.clone()))
        };
        if let Some(texto) = mensagem.clone() { mandar_quando_pronta(state, id, texto, format!("conta-{id}-{}", now_ms())); }
        registro::info("agente_trocado", &[("modo", "conta"), ("provider", &atual.provider)]);
        return Ok(json!({"conversation_id": id, "terminal_session_id": terminal, "modo": "conta", "provider": atual.provider,
            "profile_id": alvo_id, "passagem": null, "mensagem_enviada": mensagem.is_some()}));
    }

    // Outro agente: passagem de bastão. Codex ainda sem caminho de rollout é descoberto agora, para
    // a passagem ler o trecho dele.
    let mut indice = omni_core::conversations::read_index(&dir);
    omni_core::conversations::discover_codex_transcripts(&mut indice, &perfis);
    let conversa = indice.into_iter().find(|c| c.id == id).unwrap_or(conversa);
    let para = omni_core::passagem::rotulo(&pedido.provider, alvo.as_ref().map(|p| p.name.as_str()));
    let de = omni_core::passagem::rotulo(&atual.provider, atual.profile_id.as_deref().and_then(|c| perfis.iter().find(|p| p.id == c)).map(|p| p.name.as_str()));
    let com_arquivo = omni_core::arquivo_de_conversas::com_arquivo(&dir, &perfis);
    let fontes = omni_core::passagem::fontes(&conversa, &com_arquivo);
    let git = omni_core::passagem::estado_git(Path::new(&conversa.cwd));
    let n = conversa.segments.len() + 1;
    let gerado_em = chrono::Local::now().format("%d/%m/%Y %H:%M").to_string();
    let titulo = omni_core::conversations::display_title(&conversa);
    let corpo = omni_core::passagem::gerar(&omni_core::passagem::Pedido {
        produto: omni_protocol::NOME_DO_PRODUTO, conversa_id: id, titulo: &titulo, cwd: &conversa.cwd, para: &para,
        fontes: &fontes, git: git.as_ref(), gerado_em: &gerado_em, teto: omni_core::passagem::TETO_PADRAO,
    });
    let arquivo = omni_core::passagem::gravar(Path::new(&conversa.cwd), id, n, &corpo)
        .map_err(|e| error(StatusCode::CONFLICT, &format!("Não consegui gravar a passagem de bastão na pasta do projeto: {e}")))?;
    let relativo = omni_core::passagem::caminho_relativo(id, n);

    // A linha de comando da CLI nova: a mesma que abrir a conversa do zero (`resolve_spawn`).
    let base = agente.command.clone();
    let (mut comando, externo, transcript) = match pedido.provider.as_str() {
        "claude" => {
            let novo = uuid::Uuid::new_v4().to_string();
            let caminho = alvo.as_ref().map(|p| omni_core::conversations::claude_transcript_path(Path::new(&p.config_dir), &conversa.cwd, &novo)
                .to_string_lossy().into_owned());
            let mut comando = format!("{base} --session-id {novo}");
            if let Some(modo) = comandos::modo_salvo(&dir, id) { comando = comandos::com_modo(&comando, &modo).unwrap_or(comando); }
            (comando, Some(novo), caminho)
        }
        _ => (base.clone(), None, None),
    };
    // Cursor: o campo dele não é reconhecido, então o pedido curto vai como argumento (uma linha, sem
    // nada que o shell interprete). A mensagem do usuário fica no campo da tela para ele mandar.
    let pela_fila = COM_CONVERSA.contains(&pedido.provider.as_str());
    if !pela_fila {
        let linha = para_a_linha_de_comando(&format!("Você está continuando uma conversa que vinha com o {de}. Antes de responder, leia o arquivo {relativo} \
            na pasta do projeto: ele tem o objetivo, as mensagens, os arquivos alterados, os comandos e as tarefas em aberto. Não refaça o que já foi feito; \
            depois de ler, diga em uma frase onde a conversa parou e espere o próximo pedido."));
        comando = format!("{comando} \"{linha}\"");
    }

    // Saindo do Antigravity: o trecho dele fica preso à conversa que ele tinha (o log da sessão vai
    // ser de outra CLI). Entrando no Antigravity: o log da PTY começa limpo, senão ele reabriria a
    // conversa antiga do agy com `--conversation`.
    let congelar_agy = atual.provider == "antigravity";
    let caminho_agy = congelar_agy.then(|| omni_core::conversations::resolve_transcript(&atual, &conversa, &perfis)).flatten();
    if pedido.provider == "antigravity" {
        if let (Some(config), Some(terminal)) = (crate::config_do_antigravity(), atual.terminal_session_id.as_deref()) {
            let _ = std::fs::remove_file(omni_core::antigravity::log_da_sessao(&config, terminal));
        }
    }

    let agora = now_ms();
    let segmento = |terminal: &str| omni_core::conversations::Segment {
        provider: pedido.provider.clone(), profile_id: alvo_id.clone(), external_session_id: externo.clone(),
        transcript_path: transcript.clone(), terminal_session_id: Some(terminal.to_owned()), started_at_ms: agora, ended_at_ms: None,
    };
    let empurrar = |terminal: &str| omni_core::indice_de_conversas::mudar_indice(&dir, |indice| {
        let Some(c) = indice.iter_mut().find(|c| c.id == id) else { return false };
        if let Some(ultimo) = c.segments.last_mut() {
            ultimo.ended_at_ms = Some(agora);
            if congelar_agy {
                if let Some(caminho) = &caminho_agy {
                    ultimo.transcript_path = Some(caminho.to_string_lossy().into_owned());
                    ultimo.external_session_id = caminho.file_stem().map(|s| s.to_string_lossy().into_owned()).or(ultimo.external_session_id.clone());
                }
                ultimo.terminal_session_id = None;
            }
        }
        c.segments.push(segmento(terminal));
        true
    });

    let terminal = match viva {
        Some(sessao) => {
            let (terminal, anterior) = {
                let mut meta = sessao.meta.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "metadata poisoned"))?;
                let anterior = meta.clone();
                // Só as variáveis de conta mudam; o resto do ambiente da sessão fica.
                meta.env.retain(|(chave, _)| chave != "CLAUDE_CONFIG_DIR" && chave != "CODEX_HOME");
                if let Some(alvo) = &alvo { meta.env.extend(omni_core::env_for(alvo)); }
                meta.provider = Some(pedido.provider.clone());
                meta.profile_id = alvo_id.clone();
                meta.initial_command = Some(comando.clone());
                meta.external_session_id = externo.clone();
                (meta.id.clone(), anterior)
            };
            empurrar(&terminal).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
            // A CLI anterior sai: o que ela fez já está no transcript dela (gravado a cada mensagem).
            if atual.provider == "codex" { codex_app_server::esquecer(&terminal); }
            // Uma passagem entre provedores nunca retoma uma sessão descoberta por horário.
            if let Err(e) = restart_session_com_comando(state, &terminal, Some(comando.clone())) {
                // Volta tudo como estava: a conversa segue com o agente anterior.
                if let Ok(mut meta) = sessao.meta.lock() {
                    meta.env = anterior.env; meta.provider = anterior.provider; meta.profile_id = anterior.profile_id;
                    meta.initial_command = anterior.initial_command; meta.external_session_id = anterior.external_session_id;
                }
                let _ = omni_core::indice_de_conversas::mudar_indice(&dir, |indice| {
                    let Some(c) = indice.iter_mut().find(|c| c.id == id) else { return false };
                    c.segments.pop();
                    if let Some(ultimo) = c.segments.last_mut() { ultimo.ended_at_ms = None; ultimo.terminal_session_id = atual.terminal_session_id.clone(); }
                    true
                });
                return Err(error(StatusCode::CONFLICT, &format!("Não consegui abrir o {}: {e}", omni_core::passagem::nome_do_agente(&pedido.provider))));
            }
            terminal
        }
        None => {
            limite_de_sessoes::conferir(state)?;
            let sessao = spawn_terminal_inner(state, state.next_id(), conversa.project_id.clone(), titulo.clone(), conversa.cwd.clone(),
                None, Some(comando.clone()), 30, 120, agora,
                SessionOrigin { headless: true, env: alvo.as_ref().map(omni_core::env_for).unwrap_or_default(), provider: Some(pedido.provider.clone()),
                    profile_id: alvo_id.clone(), conversation_id: Some(id.to_owned()), external_session_id: externo.clone() })
                .map_err(|e| error(StatusCode::CONFLICT, &format!("Não consegui abrir o {}: {e}", omni_core::passagem::nome_do_agente(&pedido.provider))))?;
            empurrar(&sessao.id).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
            sessao.id
        }
    };
    if pedido.provider == "claude" { passar_pela_confianca(state, &terminal); }
    if pela_fila {
        let texto = omni_core::passagem::prompt_curto(&de, &para, &relativo, mensagem.as_deref());
        mandar_quando_pronta(state, id, texto, format!("passagem-{id}-{n}"));
    } else if let Ok(sessoes) = state.sessions.lock() {
        // O pedido curto foi digitado uma vez; reabrir a sessão depois não o repete.
        if let Some(SessionEntry::Live(sessao)) = sessoes.get(&terminal) {
            if let Ok(mut meta) = sessao.meta.lock() { meta.initial_command = Some(base.clone()); }
        }
    }
    let _ = state.persist();
    registro::info("agente_trocado", &[("modo", "passagem"), ("de", &atual.provider), ("para", &pedido.provider), ("trecho", &n.to_string())]);
    Ok(json!({
        "conversation_id": id, "terminal_session_id": terminal, "modo": "passagem", "provider": pedido.provider, "profile_id": alvo_id,
        "passagem": arquivo.to_string_lossy(), "relativo": relativo,
        "mensagem_enviada": pela_fila && mensagem.is_some(),
        "conversa_no_chat": pela_fila,
    }))
}

/// Espera a CLI chegar ao campo vazio e manda o texto pela fila do chat (mesma porta das mensagens).
fn mandar_quando_pronta(state: &Arc<EngineState>, conversa: &str, texto: String, chave: String) {
    let (state, conversa) = (state.clone(), conversa.to_owned());
    std::thread::spawn(move || {
        let limite = std::time::Instant::now() + PRAZO_DA_PASSAGEM;
        // A tela da CLI anterior ainda pode estar no parser por um instante depois do reinício.
        std::thread::sleep(std::time::Duration::from_millis(800));
        while std::time::Instant::now() < limite {
            if let Ok((_, Some(sessao))) = current(&state, &conversa) {
                if interaction::capabilities(&sessao).prompt {
                    let mut headers = HeaderMap::new();
                    if let Ok(valor) = chave.parse() { headers.insert("idempotency-key", valor); }
                    if enqueue(&state, &conversa, &headers, ActionKind::Prompt(texto.clone())).is_ok() {
                        registro::info("passagem_enviada", &[("conversa", &conversa)]);
                        return;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        registro::aviso("passagem_nao_enviada", &[("conversa", &conversa)]);
    });
}

async fn trocar_rota(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(pedido): Json<PedidoDeTroca>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || trocar(&state, &id, pedido).map(Json))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao trocar o agente"))?
}

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/conversas/{id}/trocar-agente", get(opcoes).post(trocar_rota))
}
