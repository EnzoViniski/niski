//! Fork: as ferramentas MCP do orquestrador (ver `orquestracao.rs`).
//!
//! Plataformas, contas e modelos saem do que o app já publica (`workspace.agents`, perfis,
//! `modelos_claude`, cache de modelos do Codex, `agy models`): uma CLI nova que o app passar a
//! suportar aparece aqui sem mudar nada. Todo acesso a um subagente passa por `do_orquestrador`:
//! o token só enxerga os subagentes da própria conversa.

use super::*;
use omni_core::orquestracao::briefing;
use omni_core::worktree_isolada;

fn caminhos(descricao: &str) -> Value {
    json!({"type": "array", "items": {"type": "string"}, "description": format!("{descricao}. Relativos à pasta do orquestrador")})
}

pub(super) fn lista() -> Value {
    let agente = json!({"type": "string", "description": "Id do subagente (o campo `agente` devolvido por criar_agente ou listar_agentes)"});
    json!([
        {"name": "listar_plataformas_e_contas",
         "description": "Lista as plataformas de agente que o app suporta (Claude Code, Codex, Antigravity…), as contas de cada uma e os modelos conhecidos. Um modelo fora da lista também é aceito.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "criar_agente",
         "description": "Abre um subagente focado numa tarefa, na mesma pasta deste orquestrador. O engine monta o briefing dele (tarefa, contexto, arquivos para ler, para editar e para criar, o que é dos outros subagentes, critérios de pronto e regras) e manda assim que a CLI ficar pronta. Arquivos de `editar`/`criar` não podem sobrepor os de outro subagente vivo: divida o trabalho em pedaços disjuntos. Respeita o limite de subagentes vivos. Com `worktree`, ele roda numa worktree isolada do git (pasta própria fora do repo, branch `medza/<nome>`, com os arquivos do `.worktreeinclude`): use para subagentes que EDITAM código em paralelo, assim um não pisa no outro; no fim ele commita na branch dele e você mescla (git merge medza/<nome>). Depois use esperar e ler_conversa.",
         "inputSchema": {"type": "object", "required": ["plataforma", "tarefa"], "properties": {
            "plataforma": {"type": "string", "description": "Id da plataforma (ex.: claude, codex, antigravity)"},
            "conta": {"type": "string", "description": "Id ou nome da conta; sem ela, a conta padrão da plataforma"},
            "modelo": {"type": "string", "description": "Modelo para a CLI (ex.: haiku). Sem ele, o padrão da CLI"},
            "esforco": {"type": "string", "description": "Esforço de raciocínio (ex.: low, medium, high)"},
            "tarefa": {"type": "string", "description": "O objetivo do subagente, claro e único"},
            "contexto": {"type": "string", "description": "O que ele precisa saber: decisões, interfaces, convenções, comandos de teste"},
            "ler": caminhos("Arquivos para ler antes de começar"),
            "editar": caminhos("Arquivos ou pastas (`src/x/`) que ele pode EDITAR — só estes"),
            "criar": caminhos("Arquivos ou pastas que ele pode CRIAR — só estes"),
            "criterios": {"type": "array", "items": {"type": "string"}, "description": "Como saber que acabou (ex.: 'npm test passa')"},
            "somente_leitura": {"type": "boolean", "description": "Tarefa só de leitura/análise, sem editar nem criar"},
            "worktree": {"type": ["boolean", "string"], "description": "true (nome tirado da tarefa) ou um nome: roda numa worktree isolada do git, na branch medza/<nome>. A pasta do orquestrador precisa ser um repositório git com ao menos um commit"},
            "titulo": {"type": "string", "description": "Nome da conversa do subagente"}}}},
        {"name": "conferir_arquivos",
         "description": "Cruza os arquivos alterados na pasta (git status) com o que cada subagente podia mexer: por subagente, sem dono (fora de todas as listas) e em disputa. Subagentes em worktree isolada vêm à parte (em_worktrees), com a branch e quantas mudanças não commitadas têm.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "mandar_prompt",
         "description": "Manda uma mensagem a um subagente. Com ele trabalhando, entra na fila da CLI dele.",
         "inputSchema": {"type": "object", "required": ["agente", "texto"], "properties": {"agente": agente, "texto": {"type": "string"}}}},
        {"name": "ler_conversa",
         "description": "Últimas mensagens de um subagente e o estado dele.",
         "inputSchema": {"type": "object", "required": ["agente"], "properties": {"agente": agente,
            "ultimas": {"type": "integer", "minimum": 1, "maximum": regra::ULTIMAS_MAXIMO, "description": "Quantas mensagens (padrão 5)"}}}},
        {"name": "esperar",
         "description": "Espera o subagente terminar o turno (ou pedir algo ao usuário), até `segundos` (máx. 120). Se estourar, devolve 'ainda trabalhando': chame de novo.",
         "inputSchema": {"type": "object", "required": ["agente"], "properties": {"agente": agente,
            "segundos": {"type": "integer", "minimum": 1, "maximum": regra::ESPERA_MAXIMA_S}}}},
        {"name": "listar_agentes",
         "description": "Os subagentes deste orquestrador, com plataforma, conta, modelo, tarefa, estado e última mensagem.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "responder_pedido",
         "description": "Responde um pedido que um subagente fez na tela: permissão (permitir true/false) ou pergunta/menu da CLI (opcao = número). Aprove o que está dentro da tarefa e dos arquivos dele; negue e oriente com mandar_prompt o que sair disso. Pedido arriscado (apagar em massa, push/publicar, instalar global, sudo, credenciais) só o usuário aprova: avise-o, ele responde no fluxo.",
         "inputSchema": {"type": "object", "required": ["agente"], "properties": {"agente": agente,
            "permitir": {"type": "boolean", "description": "Para pedido de permissão"},
            "opcao": {"type": "integer", "minimum": 1, "description": "Para pergunta ou menu: o número da opção"},
            "mensagem": {"type": "string", "description": "Ao negar (permitir: false) um pedido que veio pelo gancho do Claude: o que ele deve fazer em vez disso"}}}},
        {"name": "parar_agente",
         "description": "Encerra a CLI de um subagente (a conversa continua no histórico). Libera vaga no limite. A worktree isolada dele (se tiver) e a branch ficam: mescle e apague pelo app.",
         "inputSchema": {"type": "object", "required": ["agente"], "properties": {"agente": agente}}},
    ])
}

fn texto_do_campo<'a>(args: &'a Value, campo: &str) -> Option<&'a str> {
    args.get(campo).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty())
}

fn obrigatorio<'a>(args: &'a Value, campo: &str) -> Result<&'a str, String> {
    texto_do_campo(args, campo).ok_or_else(|| format!("Falta o campo `{campo}`"))
}

fn mensagem_do_erro(erro: ApiError) -> String {
    erro.1 .0["error"].as_str().unwrap_or("Falha").to_owned()
}

fn em_json(valor: Value) -> String { serde_json::to_string_pretty(&valor).unwrap_or_default() }

/// O subagente **deste** orquestrador, ou erro. É a trava de escopo do token.
fn do_orquestrador(state: &EngineState, orquestrador: &str, agente: &str) -> Result<regra::Agente, String> {
    ler_registro(state).agente_de(orquestrador, agente).cloned()
        .ok_or_else(|| "Esse agente não existe ou não é deste orquestrador. Use listar_agentes.".to_owned())
}

pub(super) async fn chamar(state: &Arc<EngineState>, orquestrador: &str, nome: &str, args: &Value) -> Result<String, String> {
    let args = if args.is_object() { args.clone() } else { json!({}) };
    let (state, orquestrador) = (state.clone(), orquestrador.to_owned());
    match nome {
        "esperar" => esperar(&state, &orquestrador, &args).await,
        "mandar_prompt" => {
            let agente = obrigatorio(&args, "agente")?.to_owned();
            let texto = obrigatorio(&args, "texto")?.to_owned();
            bloqueante(move || mandar(&state, &orquestrador, &agente, &texto)).await
        }
        "listar_plataformas_e_contas" => bloqueante(move || Ok(em_json(catalogo(&state)))).await,
        "criar_agente" => bloqueante(move || criar(&state, &orquestrador, &args)).await,
        "ler_conversa" => bloqueante(move || ler(&state, &orquestrador, &args)).await,
        "listar_agentes" => bloqueante(move || Ok(em_json(listar_agentes(&state, &orquestrador)))).await,
        "conferir_arquivos" => bloqueante(move || conferir(&state, &orquestrador)).await,
        "responder_pedido" => bloqueante(move || responder(&state, &orquestrador, &args)).await,
        "parar_agente" => {
            let agente = obrigatorio(&args, "agente")?.to_owned();
            bloqueante(move || parar(&state, &orquestrador, &agente)).await
        }
        _ => Err(format!("Ferramenta desconhecida: {nome}")),
    }
}

async fn bloqueante(f: impl FnOnce() -> Result<String, String> + Send + 'static) -> Result<String, String> {
    tokio::task::spawn_blocking(f).await.map_err(|_| "Falha interna".to_owned())?
}

/* ── listar_plataformas_e_contas ─────────────────────────────────────────────────────────────── */

/// Modelos conhecidos de uma plataforma, como a tela de ajustes mostra. Plataforma sem lista: vazio
/// (o modelo pedido passa mesmo assim, se for um argumento seguro).
fn modelos_de(state: &EngineState, plataforma: &str, perfil: Option<&omni_core::Profile>) -> Vec<Value> {
    match plataforma {
        "claude" => omni_core::modelos_claude::MODELOS.iter().map(|(id, _, esforcos)| json!({"id": id, "esforcos": esforcos})).collect(),
        "codex" => perfil.map(|p| modelos_codex(Path::new(&p.config_dir))).unwrap_or_default().into_iter()
            .map(|m| json!({"id": m["id"], "esforcos": m["efforts"]})).collect(),
        "antigravity" => omni_core::antigravity::familias(&saida_agy_models(dir(state))).into_iter()
            .map(|f| json!({"id": f.id, "esforcos": f.esforcos})).collect(),
        _ => Vec::new(),
    }
}

fn catalogo(state: &EngineState) -> Value {
    let agentes = state.mobile.workspace.lock().expect("workspace poisoned").agents.clone();
    let perfis = omni_core::profiles(dir(state));
    json!({
        "plataformas": agentes.iter().map(|agente| {
            let contas: Vec<&omni_core::Profile> = perfis.iter().filter(|p| p.provider == agente.id).collect();
            let padrao = contas.iter().find(|p| p.builtin).or(contas.first()).copied();
            json!({
                "plataforma": agente.id, "nome": agente.label,
                "contas": contas.iter().map(|p| json!({"id": p.id, "nome": p.name, "padrao": p.builtin})).collect::<Vec<_>>(),
                "modelos": modelos_de(state, &agente.id, padrao),
            })
        }).collect::<Vec<_>>(),
        "limite_de_subagentes_vivos": ler_registro(state).limite(),
    })
}

/* ── criar_agente ────────────────────────────────────────────────────────────────────────────── */

/// Campo que é lista de textos (aceita também um texto só).
fn textos(args: &Value, campo: &str) -> Vec<String> {
    match args.get(campo) {
        Some(Value::Array(itens)) => itens.iter().filter_map(Value::as_str).map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned).collect(),
        Some(Value::String(texto)) if !texto.trim().is_empty() => vec![texto.trim().to_owned()],
        _ => Vec::new(),
    }
}

/// Nome curto de um subagente para mensagens e para o "não mexa" do briefing.
fn nome_do_agente(a: &regra::Agente) -> String {
    format!("{} · {}", a.plataforma, regra::resumir(&a.tarefa, 40))
}

/// `(nome, caminhos)` dos subagentes vivos deste orquestrador que têm arquivos.
fn arquivos_dos_vivos(state: &EngineState, orquestrador: &str) -> Vec<(String, Vec<String>)> {
    ler_registro(state).orquestrador(orquestrador).map(|o| o.agentes.iter()
        .filter(|a| a.worktree.is_none() && a.arquivos().next().is_some() && estado_de(state, &a.conversation_key).0.vivo())
        .map(|a| (nome_do_agente(a), a.arquivos().cloned().collect())).collect()).unwrap_or_default()
}

fn vivos(state: &EngineState, orquestrador: &str) -> usize {
    ler_registro(state).orquestrador(orquestrador).map(|o| o.agentes.iter()
        .filter(|a| estado_de(state, &a.conversation_key).0.vivo()).count()).unwrap_or(0)
}

fn criar(state: &Arc<EngineState>, orquestrador: &str, args: &Value) -> Result<String, String> {
    let plataforma = obrigatorio(args, "plataforma")?.to_owned();
    let objetivo = obrigatorio(args, "tarefa")?.to_owned();
    let modelo = texto_do_campo(args, "modelo").map(str::to_owned);
    let esforco = texto_do_campo(args, "esforco").map(str::to_owned);
    let extras = regra::argumentos_de_modelo(&plataforma, modelo.as_deref(), esforco.as_deref())?;
    let titulo = texto_do_campo(args, "titulo").map(str::to_owned);
    // Fork: `worktree` = true (nome da tarefa) ou um nome.
    let nome_da_worktree = match args.get("worktree") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(worktree_isolada::nome_a_partir_de(titulo.as_deref().unwrap_or(&objetivo))),
        Some(Value::String(nome)) if nome.trim().is_empty() => None,
        Some(Value::String(nome)) => Some(worktree_isolada::nome_valido(nome)?),
        Some(_) => return Err("`worktree` é true/false ou um nome".into()),
    };
    let (mae, _) = current(state, orquestrador).map_err(mensagem_do_erro)?;
    // Harness: o que ele pode mexer, relativo à pasta do orquestrador.
    let editar = briefing::normalizar_lista(&textos(args, "editar"), &mae.cwd)?;
    let criar_ = briefing::normalizar_lista(&textos(args, "criar"), &mae.cwd)?;
    let ler = briefing::normalizar_lista(&textos(args, "ler"), &mae.cwd)?;

    let publicados = state.mobile.workspace.lock().expect("workspace poisoned").agents.clone();
    let Some(publicado) = publicados.iter().find(|a| a.id == plataforma) else {
        let nomes: Vec<&str> = publicados.iter().map(|a| a.id.as_str()).collect();
        return Err(format!("Plataforma desconhecida: {plataforma}. Disponíveis: {}", nomes.join(", ")));
    };
    let perfis = omni_core::profiles(dir(state));
    let conta = match texto_do_campo(args, "conta") {
        Some(pedida) => Some(perfis.iter().filter(|p| p.provider == plataforma)
            .find(|p| p.id == pedida || p.name.eq_ignore_ascii_case(pedida)).map(|p| p.id.clone())
            .ok_or_else(|| format!("Conta desconhecida para {plataforma}: {pedida}. Use listar_plataformas_e_contas."))?),
        None => None,
    };

    // Conferir o limite e abrir sob a mesma trava: duas chamadas juntas não passam as duas.
    static CRIANDO: Mutex<()> = Mutex::new(());
    let _criando = CRIANDO.lock().expect("criando poisoned");
    regra::pode_criar(vivos(state, orquestrador), ler_registro(state).limite())?;
    // Dois subagentes vivos nunca dividem um caminho; o que é dos outros entra no briefing como "não mexa".
    let outros = arquivos_dos_vivos(state, orquestrador);
    let pedidos: Vec<String> = editar.iter().chain(criar_.iter()).cloned().collect();
    // Em worktree própria ele não toca a pasta dos outros: a disputa vira merge, não arquivo pisado.
    let choques = if nome_da_worktree.is_some() { Vec::new() } else { briefing::conflitos(&pedidos, &outros) };
    if !choques.is_empty() {
        let lista: Vec<String> = choques.iter().map(|(pedido, dele, dono)| format!("{pedido} (já é de {dono}: {dele})")).collect();
        return Err(format!("Arquivos de outro subagente vivo: {}. Divida o trabalho em arquivos disjuntos, espere o outro terminar \
            (parar_agente) ou faça essa parte você.", lista.join("; ")));
    }
    let tarefa = briefing::montar(&briefing::Pedido {
        objetivo: objetivo.clone(), contexto: texto_do_campo(args, "contexto").map(str::to_owned), ler, editar: editar.clone(),
        criar: criar_.clone(), criterios: textos(args, "criterios"),
        somente_leitura: args.get("somente_leitura").and_then(Value::as_bool).unwrap_or(false),
        de_outros: outros.iter().flat_map(|(dono, caminhos)| caminhos.iter().map(move |c| (c.clone(), dono.clone()))).collect(),
    })?;

    let corpo = SpawnBody { project_id: mae.project_id.clone(), provider: plataforma.clone(), profile_id: conta,
        titulo: Some(titulo.clone().unwrap_or_else(|| format!("{} · subagente", publicado.label))) };
    let mut plano = resolve_spawn(state, &corpo).map_err(mensagem_do_erro)?;
    plano.titulo_do_usuario = titulo.is_some();
    // Pasta = a do orquestrador (pode ser uma worktree, não a raiz do projeto).
    if Path::new(&mae.cwd).is_dir() && mae.cwd != plano.cwd {
        plano.cwd = mae.cwd.clone();
        if let (Some(sessao), Some(perfil)) = (&plano.external_session_id, perfis.iter().find(|p| Some(&p.id) == plano.profile_id.as_ref())) {
            plano.transcript_path = Some(omni_core::conversations::claude_transcript_path(Path::new(&perfil.config_dir), &plano.cwd, sessao)
                .to_string_lossy().into_owned());
        }
    }
    // Fork: worktree isolada. Criada só agora (limite e posse de arquivos já conferidos) e apagada se o
    // subagente não abrir.
    let worktree = match &nome_da_worktree {
        Some(nome) => Some(worktree_isolada::criar(Path::new(&plano.cwd), nome).map_err(|e| format!("Não consegui criar a worktree: {e}"))?),
        None => None,
    };
    if let Some(wt) = &worktree {
        plano.cwd = wt.pasta_de_trabalho.clone();
        if let (Some(sessao), Some(perfil)) = (&plano.external_session_id, perfis.iter().find(|p| Some(&p.id) == plano.profile_id.as_ref())) {
            plano.transcript_path = Some(omni_core::conversations::claude_transcript_path(Path::new(&perfil.config_dir), &plano.cwd, sessao)
                .to_string_lossy().into_owned());
        }
    }
    let tarefa = match &worktree { Some(wt) => tarefa + &secao_da_worktree(wt), None => tarefa };
    plano.command += &extras;
    let conversa = format!("conv-{}", uuid::Uuid::new_v4());
    plano.conversation_id = Some(conversa.clone());
    let acao = Action { id: state.next_id(), conversation_id: String::new(), session_id: String::new(), state: "executing".into(),
        created_at_ms: now_ms(), error: None, key: String::new(), revision: String::new(), kind: ActionKind::Spawn(plano.clone()) };
    if let Err(e) = spawn_from_mobile(state, &acao, &plano) {
        if let Some(wt) = &worktree { let _ = worktree_isolada::remover(Path::new(&wt.caminho), true); }
        return Err(format!("Não consegui abrir o subagente: {e}"));
    }

    let agente = regra::Agente { conversation_key: conversa.clone(), plataforma: plataforma.clone(), conta: plano.profile_id.clone(),
        modelo: modelo.clone(), esforco: esforco.clone(), tarefa: objetivo.clone(), criado_em: now_ms(), editar, criar: criar_,
        worktree: worktree.as_ref().map(|wt| wt.caminho.clone()), branch: worktree.as_ref().map(|wt| wt.branch.clone()) };
    mudar_registro(state, |r| r.adicionar(orquestrador, agente)).map_err(|_| "Abri o subagente, mas não consegui gravar o registro".to_owned())?;
    drop(_criando);
    registro::info("orquestracao_agente_criado", &[("orquestrador", orquestrador), ("agente", &conversa), ("provider", &plataforma),
        ("perfil", plano.profile_id.as_deref().unwrap_or("-")), ("modelo", modelo.as_deref().unwrap_or("-"))]);

    state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").insert(conversa.clone(), "esperando a CLI ficar pronta".into());
    let (estado, id) = (state.clone(), conversa.clone());
    std::thread::spawn(move || mandar_tarefa_quando_pronta(&estado, &id, &tarefa, std::time::Duration::from_secs(180)));
    Ok(em_json(json!({
        "agente": conversa, "plataforma": plataforma, "conta": nome_da_conta(&perfis, plano.profile_id.as_deref()),
        "modelo": modelo, "esforco": esforco, "estado": "abrindo",
        "worktree": worktree.as_ref().map(|wt| wt.caminho.clone()), "branch": worktree.as_ref().map(|wt| wt.branch.clone()),
        "nota": "A tarefa vai assim que a CLI ficar pronta. Chame esperar(agente) e depois ler_conversa(agente).",
    })))
}

/// Fork: o que o subagente numa worktree isolada precisa saber, no fim do briefing.
fn secao_da_worktree(wt: &worktree_isolada::WorktreeIsolada) -> String {
    format!("\n## Sua pasta (worktree isolada)\nVocê trabalha em `{}`, na branch `{}`: uma cópia do repositório só sua. Os outros \
        subagentes não veem suas mudanças até o orquestrador mesclar. Ao terminar, faça commit das suas mudanças nesta branch \
        (sem push) e diga o hash do commit.\n", wt.pasta_de_trabalho, wt.branch)
}

/// Põe o prompt na fila de ações do chat (a mesma do celular) e espera ele ser digitado.
fn enviar(state: &Arc<EngineState>, conversa: &str, kind: ActionKind, revisao: &str) -> Result<(), String> {
    let antes = current(state, conversa).ok().and_then(|(_, s)| s).map(|s| turnos(&s)).unwrap_or(0);
    let mut headers = HeaderMap::new();
    headers.insert("idempotency-key", format!("orq-{}", uuid::Uuid::new_v4()).parse().expect("chave"));
    headers.insert("if-match", revisao.parse().map_err(|_| "Revisão inválida".to_owned())?);
    let (_, Json(acao)) = enqueue(state, conversa, &headers, kind).map_err(mensagem_do_erro)?;
    let id = acao["id"].as_str().unwrap_or_default().to_owned();
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let situacao = state.mobile.actions.lock().expect("actions poisoned").iter().find(|a| a.id == id)
            .map(|a| (a.state.clone(), a.error.clone()));
        match situacao {
            Some((estado, _)) if estado == "sent" => {
                state.mobile.orquestracao.enviado_em.lock().expect("enviado poisoned").insert(conversa.to_owned(), (now_ms(), antes));
                return Ok(());
            }
            Some((estado, erro)) if estado == "rejected" => return Err(erro.unwrap_or_else(|| "Recusada".into())),
            None => return Err("A ação sumiu da fila".into()),
            _ if std::time::Instant::now() >= limite => return Err("A CLI não recebeu a mensagem a tempo".into()),
            _ => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
}

/// A tarefa inicial: espera a CLI chegar no campo vazio (passando pela confiança na pasta do Claude,
/// ver `passar_pela_confianca`). Menu ou permissão na tela fica para o usuário responder no fluxo.
pub(super) fn mandar_tarefa_quando_pronta(state: &Arc<EngineState>, conversa: &str, tarefa: &str, prazo: std::time::Duration) {
    let limite = std::time::Instant::now() + prazo;
    let resultado = loop {
        if std::time::Instant::now() >= limite { break Err("a CLI não ficou pronta a tempo".to_owned()); }
        if !state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").contains_key(conversa) { return; } // parado
        match current(state, conversa) {
            Ok((_, Some(sessao))) => {
                let cap = interaction::capabilities(&sessao);
                if cap.prompt {
                    // A tela mudou entre ler e digitar: tenta de novo. Digitado mas sem turno (a CLI ainda
                    // subindo engoliu a colagem — visto no Codex 0.163 real): também.
                    if enviar(state, conversa, ActionKind::Prompt(tarefa.to_owned()), &cap.revision).is_ok() && comecou_turno(state, conversa) {
                        break Ok(());
                    }
                }
            }
            Ok((_, None)) => break Err("a sessão do subagente fechou".to_owned()),
            Err(_) => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    };
    let mut pendentes = state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned");
    match resultado {
        Ok(()) => { pendentes.remove(conversa); registro::info("orquestracao_tarefa_enviada", &[("agente", conversa)]); }
        Err(motivo) => {
            registro::aviso("orquestracao_tarefa_nao_enviada", &[("agente", conversa), ("motivo", &motivo)]);
            pendentes.insert(conversa.to_owned(), format!("tarefa não enviada: {motivo}; mande com mandar_prompt"));
        }
    }
}

/// Depois de digitar: a CLI começou um turno (ou pediu algo ao usuário) em até 10 s?
fn comecou_turno(state: &EngineState, conversa: &str) -> bool {
    let antes = state.mobile.orquestracao.enviado_em.lock().expect("enviado poisoned").get(conversa).map(|(_, t)| *t).unwrap_or(0);
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < limite {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let Ok((_, Some(sessao))) = current(state, conversa) else { return false };
        // Só sinal de turno de verdade: campo com texto pode ser a colagem parada, sem o Enter.
        let cap = interaction::capabilities(&sessao);
        if turnos(&sessao) > antes || cap.approve || cap.escolha.is_some() { return true; }
    }
    false
}

/* ── mandar_prompt, ler_conversa, esperar, listar, parar ─────────────────────────────────────── */

fn mandar(state: &Arc<EngineState>, orquestrador: &str, agente: &str, texto: &str) -> Result<String, String> {
    do_orquestrador(state, orquestrador, agente)?;
    regra::texto_valido(texto)?;
    if let Some(motivo) = state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").get(agente) {
        if motivo.starts_with("esperando") { return Err("A tarefa inicial ainda não foi enviada (a CLI está abrindo). Use esperar antes.".into()); }
    }
    let (estado, sessao, cap) = estado_de(state, agente);
    let (Some(_), Some(cap)) = (sessao, cap) else { return Err("O agente está parado. Crie outro com criar_agente.".into()) };
    if estado == regra::Estado::EsperandoVoce {
        return Err("O agente está esperando o usuário (permissão ou pergunta na tela). Peça ao usuário para responder no fluxo.".into());
    }
    let (kind, como) = if cap.prompt { (ActionKind::Prompt(texto.to_owned()), "enviada") }
        else if cap.enfileirar { (ActionKind::Fila(texto.to_owned()), "na fila da CLI (o agente está trabalhando)") }
        else { return Err(format!("O agente não pode receber mensagem agora: {}", cap.reason.unwrap_or_else(|| "tela não reconhecida".into()))) };
    let digitada = matches!(kind, ActionKind::Prompt(_));
    enviar(state, agente, kind, &cap.revision)?;
    state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").remove(agente);
    if digitada && !comecou_turno(state, agente) {
        return Ok("Mensagem digitada, mas a CLI ainda não começou o turno. Confira com ler_conversa antes de mandar de novo.".into());
    }
    Ok(format!("Mensagem {como}. Use esperar(agente) para a resposta."))
}

/// Últimas linhas com texto da tela: para a plataforma sem transcript que o app leia.
fn tela(sessao: &Arc<LiveSession>) -> String {
    let texto = sessao.interaction.lock().map(|c| c.parser.screen().contents()).unwrap_or_default();
    let linhas: Vec<&str> = texto.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect();
    linhas[linhas.len().saturating_sub(15)..].join("\n")
}

fn ler(state: &Arc<EngineState>, orquestrador: &str, args: &Value) -> Result<String, String> {
    let agente = obrigatorio(args, "agente")?;
    do_orquestrador(state, orquestrador, agente)?;
    let quantas = regra::ultimas(args.get("ultimas").and_then(Value::as_u64).map(|n| n as usize));
    let todas = mensagens(state, agente);
    let (estado, sessao, cap) = estado_de(state, agente);
    let ultimas: Vec<Value> = todas[todas.len().saturating_sub(quantas)..].iter().map(|(papel, texto)| json!({
        "de": if papel == "assistant" { "agente" } else { "orquestrador" }, "texto": regra::resumir(texto, 4_000),
    })).collect();
    let mut saida = json!({"agente": agente, "estado": estado.texto(), "mensagens": ultimas});
    if let Some(pedido) = cap.as_ref().and_then(pedido_de) { saida["pedido"] = pedido; }
    if let Some(motivo) = state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").get(agente) { saida["tarefa"] = json!(motivo); }
    if todas.is_empty() { if let Some(sessao) = sessao { saida["tela"] = json!(tela(&sessao)); } }
    Ok(em_json(saida))
}

/// Espera sem segurar thread: um `estado_de` por segundo. "Pronto" logo depois de um envio não vale
/// (a CLI ainda não começou o turno) até 5 s.
async fn esperar(state: &Arc<EngineState>, orquestrador: &str, args: &Value) -> Result<String, String> {
    let agente = obrigatorio(args, "agente")?.to_owned();
    { let (s, o, a) = (state.clone(), orquestrador.to_owned(), agente.clone());
      bloqueante(move || do_orquestrador(&s, &o, &a).map(|_| String::new())).await?; }
    let segundos = regra::segundos_de_espera(args.get("segundos").and_then(Value::as_u64));
    let inicio = std::time::Instant::now();
    let mut viu_trabalhando = false;
    loop {
        let (s, a) = (state.clone(), agente.clone());
        let (estado, pendente, pedido) = tokio::task::spawn_blocking(move || {
            let pendente = s.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").get(&a).cloned();
            let (estado, _, cap) = estado_de(&s, &a);
            (estado, pendente, cap.as_ref().and_then(pedido_de))
        }).await.map_err(|_| "Falha interna".to_owned())?;
        // "Pronto" de antes do turno que a última mensagem abriu não vale (até 20 s depois do envio).
        let envio = state.mobile.orquestracao.enviado_em.lock().expect("enviado poisoned").get(&agente).copied();
        let turnos_agora = { let (s, a) = (state.clone(), agente.clone());
            tokio::task::spawn_blocking(move || current(&s, &a).ok().and_then(|(_, s)| s).map(|s| turnos(&s)).unwrap_or(0)).await.unwrap_or(0) };
        let recem_enviado = envio.is_some_and(|(quando, antes)| turnos_agora <= antes && now_ms().saturating_sub(quando) < 20_000);
        viu_trabalhando |= estado == regra::Estado::Trabalhando;
        let esperando_tarefa = pendente.as_deref().is_some_and(|m| m.starts_with("esperando"));
        let terminou = match estado {
            regra::Estado::Trabalhando | regra::Estado::Abrindo => false,
            regra::Estado::Pronto => !esperando_tarefa && (viu_trabalhando || !recem_enviado),
            _ => true,
        };
        if terminou {
            let (s, a) = (state.clone(), agente.clone());
            let ultima = tokio::task::spawn_blocking(move || ultima_do_agente(&s, &a)).await.ok().flatten();
            return Ok(em_json(json!({"agente": agente, "estado": estado.texto(),
                "ultima_mensagem": ultima.map(|t| regra::resumir(&t, 4_000)), "tarefa": pendente, "pedido": pedido})));
        }
        if inicio.elapsed().as_secs() >= segundos {
            return Ok(format!("ainda trabalhando (estado: {}). Chame esperar de novo.", estado.texto()));
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

pub(super) fn listar_agentes(state: &EngineState, orquestrador: &str) -> Value {
    let registro = ler_registro(state);
    let perfis = omni_core::profiles(dir(state));
    let agentes: Vec<Value> = registro.orquestrador(orquestrador).map(|o| o.agentes.iter().map(|a| {
        let (estado, _, cap) = estado_de(state, &a.conversation_key);
        json!({"agente": a.conversation_key, "pedido": cap.as_ref().and_then(pedido_de), "plataforma": a.plataforma, "conta": nome_da_conta(&perfis, a.conta.as_deref()),
            "modelo": a.modelo, "tarefa": regra::resumir(&a.tarefa, 200), "estado": estado.texto(),
            "editar": a.editar, "criar": a.criar, "worktree": a.worktree, "branch": a.branch,
            "ultima_mensagem": ultima_do_agente(state, &a.conversation_key).map(|t| regra::resumir(&t, 300))})
    }).collect()).unwrap_or_default();
    let vivos = agentes.iter().filter(|a| !matches!(a["estado"].as_str(), Some("parado" | "erro"))).count();
    json!({"agentes": agentes, "vivos": vivos, "limite": registro.limite()})
}

/// `git status` da pasta do orquestrador cruzado com a posse de cada subagente (vivos ou não).
fn conferir(state: &EngineState, orquestrador: &str) -> Result<String, String> {
    let (mae, _) = current(state, orquestrador).map_err(mensagem_do_erro)?;
    let saida = std::process::Command::new("git").args(["status", "--porcelain=v1", "-uall"]).current_dir(&mae.cwd)
        .stdin(std::process::Stdio::null()).output().map_err(|_| "Não consegui rodar o git nesta pasta".to_owned())?;
    if !saida.status.success() { return Err("A pasta do orquestrador não é um repositório git: não dá para conferir".into()); }
    let mudados = briefing::caminhos_do_git_status(&String::from_utf8_lossy(&saida.stdout));
    let registro = ler_registro(state);
    let todos = registro.orquestrador(orquestrador).map(|o| o.agentes.as_slice()).unwrap_or_default();
    // Fork: subagente em worktree isolada mexe na pasta dele, não nesta: entra à parte, com a branch.
    let agentes: Vec<(String, Vec<String>)> = todos.iter().filter(|a| a.worktree.is_none())
        .map(|a| (format!("{} ({})", nome_do_agente(a), a.conversation_key), a.arquivos().cloned().collect())).collect();
    let em_worktrees: Vec<Value> = todos.iter().filter_map(|a| {
        let caminho = a.worktree.as_deref()?;
        let estado = worktree_isolada::estado(Path::new(caminho)).ok();
        Some(json!({"agente": a.conversation_key, "branch": a.branch, "worktree": caminho, "existe": estado.is_some(),
            "nao_commitadas": estado.map(|e| e.mudancas)}))
    }).collect();
    let mut por_agente: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let (mut sem_dono, mut em_disputa) = (Vec::new(), Vec::new());
    for arquivo in mudados.iter().take(500) {
        match briefing::donos(arquivo, &agentes).as_slice() {
            [] => sem_dono.push(arquivo.clone()),
            [dono] => por_agente.entry((*dono).to_owned()).or_default().push(arquivo.clone()),
            varios => em_disputa.push(json!({"arquivo": arquivo, "donos": varios})),
        }
    }
    Ok(em_json(json!({"alterados": mudados.len(), "por_agente": por_agente, "sem_dono": sem_dono, "em_disputa": em_disputa,
        "em_worktrees": em_worktrees,
        "nota": "sem_dono inclui o que você ou o usuário mudaram, e o que já estava alterado antes dos subagentes."})))
}

/// O que o subagente está pedindo na tela, no formato que o orquestrador lê e responde.
fn pedido_de(cap: &interaction::Capabilities) -> Option<Value> {
    if cap.approve {
        let texto = cap.approval_text.clone().unwrap_or_default();
        return Some(json!({"tipo": "permissao", "texto": regra::resumir(&texto, 3_000),
            // Fork (Onda 2): pedido do gancho traz a ferramenta e o prazo (depois dele a CLI pergunta no terminal).
            "ferramenta": cap.pedido.as_ref().map(|p| p.ferramenta.clone()), "expira_em_ms": cap.pedido.as_ref().map(|p| p.expira_em_ms),
            "arriscado": briefing::pedido_arriscado(&texto),
            "como_responder": "responder_pedido(agente, permitir: true|false)"}));
    }
    cap.escolha.as_ref().map(|menu| json!({"tipo": "pergunta", "pergunta": menu.pergunta, "detalhe": menu.detalhe,
        "opcoes": menu.opcoes.iter().map(|o| json!({"numero": o.numero, "rotulo": o.rotulo, "descricao": o.descricao})).collect::<Vec<_>>(),
        "como_responder": "responder_pedido(agente, opcao: <número>)"}))
}

/// Responde o pedido na tela do subagente pela mesma fila do chat (com a versão da tela que foi lida).
/// Aprovar algo arriscado é recusado: fica para o usuário. Opção que guarda permissão para sempre nunca
/// é mandada (`interaction::approval` só usa a de uma vez).
fn responder(state: &Arc<EngineState>, orquestrador: &str, args: &Value) -> Result<String, String> {
    let agente = obrigatorio(args, "agente")?;
    do_orquestrador(state, orquestrador, agente)?;
    let (_, sessao, cap) = estado_de(state, agente);
    let (Some(sessao), Some(cap)) = (sessao, cap) else { return Err("O agente está parado.".into()) };
    // Fork (Onda 2): pedido segurado pelo gancho do Claude responde direto, com o id do que foi lido.
    if let (Some(pedido), Some(permitir)) = (cap.pedido.as_ref(), args.get("permitir").and_then(Value::as_bool)) {
        if let Some(motivo) = pedido.arriscado.filter(|_| permitir) {
            registro::info("orquestracao_pedido_para_o_usuario", &[("orquestrador", orquestrador), ("agente", agente)]);
            return Err(format!("Este pedido {motivo}: só o usuário aprova. Avise-o (ele responde no fluxo) ou negue com permitir: false."));
        }
        let terminal = sessao.meta.lock().map(|m| m.id.clone()).map_err(|_| "Sessão indisponível".to_owned())?;
        let decisao = if permitir { crate::ganchos::Decisao::Permitir } else { crate::ganchos::Decisao::Negar(texto_do_campo(args, "mensagem").map(str::to_owned)) };
        crate::ganchos::responder(&terminal, Some(&pedido.id), decisao).map_err(|_| "O pedido já foi respondido ou mudou. Veja com ler_conversa.".to_owned())?;
        let resumo = if permitir { "permitido" } else { "negado" };
        registro::info("orquestracao_pedido_respondido", &[("orquestrador", orquestrador), ("agente", agente), ("resposta", resumo)]);
        return Ok(format!("Pedido {resumo}. Use esperar(agente) para acompanhar."));
    }
    let (kind, resumo) = match (args.get("permitir").and_then(Value::as_bool), args.get("opcao").and_then(Value::as_u64)) {
        (Some(permitir), _) if cap.approve => {
            let texto = cap.approval_text.clone().unwrap_or_default();
            if let Some(motivo) = briefing::pedido_arriscado(&texto).filter(|_| permitir) {
                registro::info("orquestracao_pedido_para_o_usuario", &[("orquestrador", orquestrador), ("agente", agente)]);
                return Err(format!("Este pedido {motivo}: só o usuário aprova. Avise-o (ele responde no fluxo) ou negue com permitir: false."));
            }
            (ActionKind::Approval(permitir), if permitir { "permitido" } else { "negado" })
        }
        (_, Some(n)) if cap.escolha.as_ref().is_some_and(|m| m.opcoes.iter().any(|o| u64::from(o.numero) == n)) => {
            (ActionKind::Escolha(Resposta::Opcao(n as u8)), "opção escolhida")
        }
        (_, Some(_)) if cap.escolha.is_some() => return Err("Essa opção não existe no menu. Veja as opções com ler_conversa.".into()),
        _ if cap.approve => return Err("O pedido é de permissão: responda com permitir: true ou false.".into()),
        _ if cap.escolha.is_some() => return Err("O pedido é uma pergunta: responda com opcao: <número>.".into()),
        _ => return Err("Não há pedido na tela deste agente agora.".into()),
    };
    enviar(state, agente, kind, &cap.revision)?;
    registro::info("orquestracao_pedido_respondido", &[("orquestrador", orquestrador), ("agente", agente), ("resposta", resumo)]);
    Ok(format!("Pedido {resumo}. Use esperar(agente) para acompanhar."))
}

/// Encerra a CLI do subagente (a mesma parada do botão do terminal). Também usado pela tela.
pub(super) fn parar(state: &EngineState, orquestrador: &str, agente: &str) -> Result<String, String> {
    do_orquestrador(state, orquestrador, agente)?;
    state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").remove(agente);
    let Ok((_, Some(sessao))) = current(state, agente) else { return Ok("O agente já estava parado.".into()) };
    let id = sessao.meta.lock().map(|m| m.id.clone()).map_err(|_| "Sessão indisponível".to_owned())?;
    stop_session(state, &id).map_err(|e| e.to_string())?;
    registro::info("orquestracao_agente_parado", &[("orquestrador", orquestrador), ("agente", agente)]);
    Ok("Agente parado.".into())
}
