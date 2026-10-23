//! Fork (Rotina): o agente da rotina — uma conversa normal do app (Claude ou Codex na PTY, no
//! Histórico, retomada com `--resume`) no projeto especial `rotina`, com as ferramentas MCP da rotina
//! e as de tarefas, servidas pelo mesmo servidor MCP dos orquestradores.
//!
//! Decisão: **uma conversa por semana** (padrão; `modo_conversa = continua` nos Ajustes troca). O
//! domingo abre a conversa da semana seguinte e ela segue até o próximo planejamento. A continuidade
//! entre semanas não depende do contexto da CLI: o plano, o histórico de imprevistos, os treinos, a
//! alimentação e a **memória da rotina** (`anotar_memoria`) ficam nos arquivos e o agente lê tudo pelas
//! ferramentas. Uma conversa sem fim ficaria cada vez mais lenta e cara e seria compactada sem
//! controle; uma por semana começa limpa e sabe tudo o que importa.
//!
//! O briefing vai em três lugares: `CLAUDE.md`/`AGENTS.md` na pasta da conversa (a CLI relê a cada
//! início e retomada), as instruções do `initialize` do MCP e a primeira mensagem.

use super::*;
use omni_core::rotina::{self as rotina_core, ajustes::Ajustes as AjustesDaRotina};

/// Projeto especial das conversas do agente da rotina (não é projeto da lateral).
pub(crate) const PROJETO_DA_ROTINA: &str = "rotina";
/// Pasta da conversa, dentro de `<dados>/rotina/`.
const PASTA_DA_CONVERSA: &str = "conversa";

fn mensagem_do_erro(erro: ApiError) -> String {
    erro.1 .0["error"].as_str().unwrap_or("Falha").to_owned()
}

pub(super) fn e_da_rotina(state: &EngineState, conversa: &str) -> bool {
    current(state, conversa).is_ok_and(|(c, _)| c.project_id == PROJETO_DA_ROTINA)
}

pub(super) const INSTRUCOES: &str = "Ferramentas do Niski para organizar a rotina do usuário: plano da semana (blocos fixos, metas, \
foco), o dia (linha do tempo), treino da academia, alimentação e água, lembretes, e as Tarefas do app (compromissos e afazeres com \
data e hora). Leia antes de mudar (ler_dia, ler_semana, ler_treino, ler_alimentacao, listar_tarefas) e confirme com o usuário \
antes de gravar um plano inteiro. Compromisso com data/hora é tarefa (criar_tarefa), nunca bloco: blocos são o que se repete \
toda semana (aulas, trabalho, sono, academia).";

/// O briefing completo (vai para `CLAUDE.md`/`AGENTS.md` da pasta da conversa).
pub(super) fn briefing() -> String {
    format!(r#"# Agente da rotina do Niski

Você é o agente da rotina do usuário (o Enzo) dentro do Niski. Seu trabalho: organizar a semana dele (faculdade, trabalho na
Medza, trabalho na Aircook, compromissos, treino da academia, alimentação, água, metas) e ajustar quando aparece um imprevisto.
Fale em português do Brasil, curto e direto. Nada de listas enormes: o plano completo ele vê na tela Rotina do app.

## Onde as coisas moram (use as ferramentas MCP `medza`)
- **Blocos fixos** (`salvar_bloco`, `planejar_semana.blocos_fixos`): o que se repete toda semana — aulas, expediente, sono,
  academia, refeições fixas. Dias: 0 = segunda … 6 = domingo. Bloco de um dia só: `data`.
- **Tarefas do app** (`criar_tarefa`, `editar_tarefa`, `listar_tarefas`, `concluir_tarefa`): todo compromisso ou afazer com
  data (e hora, se tiver). Área: Faculdade = `categoria: faculdade`; Medza = `categoria: trabalho` + `etiquetas: ["medza"]`;
  Aircook = `categoria: trabalho` + `etiquetas: ["aircook"]`; saúde/médico = `saude`; resto = `pessoal`. Nunca crie bloco
  para compromisso: vira tarefa.
- **Metas e foco da semana** (`planejar_semana.metas`/`foco`, `salvar_meta` com `alvo` e `somar` para progresso).
- **Treino** (`ler_treino`, `salvar_ficha`, `registrar_treino`, `registrar_serie`, `concluir_treino`, `ajustar_treino`):
  fichas A/B/C com exercícios; sessões com séries (carga × repetições). Para evolução, `ler_treino` com `exercicio`.
- **Alimentação** (`ler_alimentacao`, `registrar_refeicao`, `registrar_agua`, `salvar_plano_alimentar`): refeições em texto
  livre, kcal/proteína só se ele disser (não invente números), água em copos ou ml.
- **Lembretes** (`ler_lembretes`, `salvar_lembretes`, `salvar_lembrete`): avisos no app e no Telegram da rotina. Metas e
  remédios viram `salvar_lembrete` (tipo meta/remedio).
- **Memória da rotina** (`anotar_memoria`, lida em `ler_semana.memoria`): o que você aprendeu e deve lembrar nas próximas
  semanas (horários que funcionam, preferências, o que costuma dar errado). Atualize ao fim de cada planejamento.

## Planejamento de domingo (quando a conversa abre com "Vamos planejar a semana")
1. Leia `ler_semana` (da semana que vai planejar, e a atual para ver o que ficou para trás), `listar_tarefas` (semana e
   atrasadas), `ler_treino` e a memória.
2. Pergunte, uma área por vez e sem enrolar: compromissos com data/hora; faculdade (aulas, provas, trabalhos, entregas);
   Medza; Aircook; treino (quantos dias, quais fichas); alimentação (plano do dia, meta de água); metas da semana.
   Aproveite o que já existe: pergunte só o que mudou em relação à semana passada.
3. Monte a proposta (resumo curto por dia) e peça confirmação.
4. Confirmado: grave com `planejar_semana` (blocos fixos, foco, metas, `concluir_planejamento: true`), crie as tarefas, os
   lembretes de metas, ajuste o plano alimentar e as fichas se mudaram, e anote na memória o que aprendeu.

## Durante a semana
- Imprevisto ("a aula foi cancelada", "vou chegar 2 h atrasado", "fiquei doente"): `ler_dia`, proponha o novo dia e grave
  com `ajustar_dia` (cancelar/mover blocos só naquela data, blocos novos, `nota` com o motivo) e `editar_tarefa` para
  remarcar tarefas. Diga em uma ou duas frases o que mudou.
- Registro falado ("fiz supino 4x8 com 60 kg", "almocei arroz e frango", "bebi 2 copos"): grave direto e confirme curto.
  Recorde de carga: comemore em uma frase.
- Pergunta ("o que tenho agora?", "como está a semana?"): leia e responda curto.
- Mensagem que começa com "[Pelo Telegram]" chegou pelo bot da rotina: responda em poucas linhas, sem tabelas.

## Regras
- Não invente compromisso, carga, kcal nem horário: pergunte.
- Não apague tarefa nem bloco sem o usuário pedir.
- Data relativa (amanhã, sexta) você converte para AAAA-MM-DD; hoje está em `ler_dia.data`.
- {instrucoes}
"#, instrucoes = INSTRUCOES)
}

/// Ferramentas da rotina expostas ao agente (o resto das operações é da tela).
const FERRAMENTAS: &[(&str, &str)] = &[
    ("ler_dia", "O dia como linha do tempo: blocos, tarefas com hora, refeições, treino, água; agora/próximo; tarefas sem hora e atrasadas. `data` opcional (AAAA-MM-DD; padrão hoje)."),
    ("ler_semana", "A semana (segunda a domingo) de `semana` (qualquer data dela; padrão a atual): dias, foco, metas, notas, imprevistos, progresso, áreas, blocos_fixos (com ids) e a memória da rotina."),
    ("planejar_semana", "Grava o planejamento da semana de `semana`: `blocos_fixos` (troca TODOS os fixos pela lista; mande os que continuam com o id), `blocos_da_semana` (avulsos com `data`), `foco`, `notas`, `metas` (troca a lista) e `concluir_planejamento` (padrão true)."),
    ("salvar_bloco", "Cria ou muda (com `id`) um bloco: titulo, area (faculdade, medza, aircook, saude, pessoal), tipo, dias (0=seg…6=dom) ou data, inicio, fim, local, lembrar."),
    ("remover_bloco", "Remove um bloco pelo `id`. Só quando o usuário pedir."),
    ("ajustar_dia", "Imprevisto num dia (`data`, padrão hoje): `mudancas` [{bloco, acao: cancelar|mover|restaurar, inicio, fim}] só naquela data, `novos_blocos` [...] e `nota` (o motivo, vai para o histórico de imprevistos)."),
    ("marcar", "Marca um item do dia (`item` = id de ler_dia, ex.: bloco:<id>, refeicao:<id>, tarefa:<id>) como `estado` feito, pulado ou nada."),
    ("salvar_meta", "Cria ou muda (com `id`) uma meta da semana: texto, area, alvo, unidade, progresso, somar (soma ao progresso), concluida."),
    ("remover_meta", "Remove uma meta da semana pelo `id`."),
    ("anotar_memoria", "Memória da rotina entre semanas: `texto` e `modo` (substituir, padrão, ou acrescentar)."),
    ("salvar_area", "Cria ou muda uma área (id, nome, cor #rrggbb, categoria das tarefas, etiqueta das tarefas)."),
    ("ler_treino", "Fichas, próxima ficha, sessões recentes, frequência por semana, exercícios com recordes; com `exercicio`, a evolução dele."),
    ("salvar_ficha", "Cria ou muda (com `id`) uma ficha: nome (A, B…), titulo, dias, exercicios [{nome, series, repeticoes, carga_kg, descanso_s, obs}]."),
    ("registrar_treino", "Registra várias séries de uma vez (`series` [{exercicio, carga_kg, repeticoes}]) na sessão do dia (`data`, `ficha` opcionais); `concluir`, `obs`, `inicio`, `fim`."),
    ("registrar_serie", "Registra uma série: exercicio, carga_kg, repeticoes (e `ficha`, `data`). Diz se foi recorde."),
    ("concluir_treino", "Conclui a sessão do dia (ou `sessao`), com `obs`."),
    ("ajustar_treino", "Muda a meta de treinos por semana (`meta_semanal`)."),
    ("ler_alimentacao", "Plano de refeições, metas, o dia (`data`) e o histórico dos últimos `dias`."),
    ("registrar_refeicao", "Registra o que comeu: texto, nome (Almoço…), hora, kcal e proteina_g (só se o usuário disse)."),
    ("registrar_agua", "Soma água: `copos` (padrão 1) ou `ml` (negativo corrige)."),
    ("salvar_plano_alimentar", "Plano do dia: `refeicoes` [{nome, hora, sugestao, kcal, proteina_g}] (troca a lista), meta_agua_ml, copo_ml, meta_kcal, meta_proteina_g."),
    ("ler_lembretes", "Ajustes dos lembretes e os lembretes personalizados."),
    ("salvar_lembretes", "Muda os ajustes: ativo, antecedencia_min, tarefas, blocos, refeicoes, treino, agua {ativo, intervalo_min}, acordar, dormir, planejar {ativo, dia, hora}."),
    ("salvar_lembrete", "Cria ou muda (com `id`) um lembrete: texto, hora, dias (vazio = todo dia) ou data, tipo (meta, remedio, livre), ativo."),
    ("remover_lembrete", "Remove um lembrete personalizado pelo `id`."),
];

pub(super) fn lista() -> Value {
    let mut itens: Vec<Value> = FERRAMENTAS.iter().map(|(nome, descricao)| json!({
        "name": nome, "description": descricao,
        // Esquema aberto: as operações validam campo a campo e dizem o que falta.
        "inputSchema": {"type": "object", "additionalProperties": true, "properties": {}},
    })).collect();
    itens.extend(super::tarefas_mcp::lista());
    Value::Array(itens)
}

pub(super) async fn chamar(state: &Arc<EngineState>, nome: &str, args: &Value) -> Result<String, String> {
    let pasta = dir(state).to_path_buf();
    let (nome, args) = (nome.to_owned(), if args.is_object() { args.clone() } else { json!({}) });
    tokio::task::spawn_blocking(move || {
        if super::tarefas_mcp::e_de_tarefas(&nome) {
            return super::tarefas_mcp::chamar(&pasta, &nome, &args);
        }
        if !FERRAMENTAS.iter().any(|(n, _)| *n == nome) {
            return Err(format!("Ferramenta desconhecida: {}", rotina_core::limpar(&nome, 40)));
        }
        rotina_core::operacoes::executar(&pasta, &nome, &args, &super::super::rotina_rotas::agora())
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
    }).await.map_err(|_| "Falha interna".to_owned())?
}

/* ── Abrir a conversa ──────────────────────────────────────────────────────────────────────── */

/// Semana em que a conversa vale: o planejamento de sexta a domingo é da semana seguinte, e conversar
/// no fim de semana depois de planejar continua na conversa nova.
pub(super) fn semana_alvo(ajustes: &AjustesDaRotina, hoje: chrono::NaiveDate, planejar: bool) -> chrono::NaiveDate {
    let atual = rotina_core::segunda_de(hoje);
    let proxima = atual + chrono::Duration::days(7);
    let fim_de_semana = rotina_core::dia_da_semana(hoje) >= 4;
    if fim_de_semana && (planejar || ajustes.conversas.contains_key(&rotina_core::texto_da_data(proxima))) { proxima } else { atual }
}

fn primeira_mensagem(planejar: bool, segunda: chrono::NaiveDate, nova: bool) -> Option<String> {
    let domingo = segunda + chrono::Duration::days(6);
    let periodo = format!("{} a {}", segunda.format("%d/%m"), domingo.format("%d/%m"));
    match (planejar, nova) {
        (true, true) => Some(format!("Vamos planejar a semana de {periodo}. Você é o agente da rotina do Niski (siga o CLAUDE.md/AGENTS.md \
            desta pasta): leia o que já existe e me conduza pelo planejamento, uma área por vez.")),
        (true, false) => Some(format!("Vamos planejar a semana de {periodo}. Leia o que já existe e me conduza pelo planejamento, uma área por vez.")),
        (false, true) => Some("Você é o agente da rotina do Niski (siga o CLAUDE.md/AGENTS.md desta pasta). Leia ler_dia e me diga em duas \
            linhas o que tenho agora e a seguir; depois espere o que eu disser.".into()),
        (false, false) => None,
    }
}

fn pasta_da_conversa(state: &EngineState) -> Result<std::path::PathBuf, String> {
    let pasta = rotina_core::pasta(dir(state)).join(PASTA_DA_CONVERSA);
    std::fs::create_dir_all(&pasta).map_err(|_| "Não consegui criar a pasta da conversa da rotina".to_owned())?;
    let texto = briefing();
    for nome in ["CLAUDE.md", "AGENTS.md"] {
        let caminho = pasta.join(nome);
        if std::fs::read_to_string(&caminho).ok().as_deref() != Some(texto.as_str()) {
            let _ = std::fs::write(&caminho, &texto);
        }
    }
    Ok(pasta)
}

fn sessao_de(sessao: &Arc<LiveSession>) -> Option<String> {
    sessao.meta.lock().ok().map(|m| m.id.clone())
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct PedidoDeConversa {
    /// `planejar` ou `conversar`.
    pub modo: String,
    /// Padrão da tela (a do copiloto) quando os Ajustes da Rotina não escolheram.
    pub plataforma_padrao: Option<String>,
    pub conta_padrao: Option<String>,
}

/// Abre (ou reabre, ou só devolve) a conversa da rotina. Devolve `{sessao, conversa, nova}`.
pub(super) fn abrir(state: &Arc<EngineState>, pedido: &PedidoDeConversa) -> Result<Value, String> {
    static ABRINDO: Mutex<()> = Mutex::new(());
    let _abrindo = ABRINDO.lock().unwrap_or_else(|e| e.into_inner());
    let planejar = pedido.modo == "planejar";
    let agora = super::super::rotina_rotas::agora();
    let ajustes: AjustesDaRotina = rotina_core::ler(dir(state));
    let segunda = semana_alvo(&ajustes, agora.hoje, planejar);
    let chave = rotina_core::texto_da_data(segunda);
    let pasta = pasta_da_conversa(state)?;

    if let Some(conversa) = ajustes.conversa_da_semana(&chave) {
        if let Ok((_, viva)) = current(state, &conversa) {
            if let Some(sessao) = viva {
                if let Some(texto) = primeira_mensagem(planejar, segunda, false) { enviar_quando_pronta(state, &conversa, texto); }
                return Ok(json!({"conversa": conversa, "sessao": sessao_de(&sessao), "nova": false}));
            }
            // Fechada: reabre a mesma com `--resume` (o registro mantém o MCP ligado).
            if let Ok(sessao) = retomar_conversa(state, &conversa) {
                if let Some(texto) = primeira_mensagem(planejar, segunda, false) { enviar_quando_pronta(state, &conversa, texto); }
                return Ok(json!({"conversa": conversa, "sessao": sessao, "nova": false}));
            }
            registro::aviso("rotina_conversa_nao_retomou", &[("conversa", &conversa)]);
        }
    }

    // Conversa nova.
    let publicados = state.mobile.workspace.lock().expect("workspace poisoned").agents.clone();
    let plataforma = [Some(ajustes.plataforma.clone()), pedido.plataforma_padrao.clone(), Some("claude".into()), Some("codex".into())]
        .into_iter().flatten()
        .find(|p| !p.is_empty() && regra::pode_orquestrar(p) && publicados.iter().any(|a| &a.id == p))
        .ok_or_else(|| "Nenhum Claude Code ou Codex disponível no app para o agente da rotina. Abra o app no PC uma vez.".to_owned())?;
    let conta = [Some(ajustes.conta.clone()).filter(|_| ajustes.plataforma == plataforma),
        pedido.conta_padrao.clone().filter(|_| pedido.plataforma_padrao.as_deref() == Some(plataforma.as_str()))]
        .into_iter().flatten().find(|c| !c.is_empty());
    let (modelo, esforco) = if ajustes.plataforma == plataforma {
        (Some(ajustes.modelo.clone()).filter(|m| !m.is_empty()), Some(ajustes.esforco.clone()).filter(|e| !e.is_empty()))
    } else { (None, None) };
    let extras = regra::argumentos_de_modelo(&plataforma, modelo.as_deref(), esforco.as_deref())?;
    let projeto = PublishedProject { id: PROJETO_DA_ROTINA.into(), name: "Rotina".into(), path: pasta.to_string_lossy().into_owned() };
    let titulo = format!("Rotina · semana de {}", segunda.format("%d/%m"));
    let corpo = SpawnBody { project_id: PROJETO_DA_ROTINA.into(), provider: plataforma.clone(), profile_id: conta, titulo: Some(titulo) };
    let mut plano = resolve_spawn_no_projeto(state, &corpo, projeto).map_err(mensagem_do_erro)?;
    plano.command += &extras;
    let conversa = format!("conv-{}", uuid::Uuid::new_v4());
    plano.conversation_id = Some(conversa.clone());
    // Marcar antes de abrir: `preparar` põe as flags do MCP já no primeiro comando.
    mudar_registro(state, |r| r.definir_ativo(&conversa, true)).map_err(|_| "Não consegui preparar as ferramentas da rotina".to_owned())?;
    let acao = Action { id: state.next_id(), conversation_id: String::new(), session_id: String::new(), state: "executing".into(),
        created_at_ms: now_ms(), error: None, key: String::new(), revision: String::new(), kind: ActionKind::Spawn(plano.clone()) };
    if let Err(e) = spawn_from_mobile(state, &acao, &plano) {
        let _ = mudar_registro(state, |r| r.definir_ativo(&conversa, false));
        return Err(format!("Não consegui abrir a conversa da rotina: {e}"));
    }
    rotina_core::mudar::<AjustesDaRotina, _>(dir(state), |a| { a.definir_conversa(&chave, &conversa); Ok(()) })?;
    registro::info("rotina_conversa_aberta", &[("provider", &plataforma), ("modo", if planejar { "planejar" } else { "conversar" })]);
    let sessao = current(state, &conversa).ok().and_then(|(_, s)| s).and_then(|s| sessao_de(&s));
    if let Some(texto) = primeira_mensagem(planejar, segunda, true) { enviar_quando_pronta(state, &conversa, texto); }
    Ok(json!({"conversa": conversa, "sessao": sessao, "nova": true}))
}

fn retomar_conversa(state: &Arc<EngineState>, conversa: &str) -> Result<Option<String>, String> {
    limite_de_sessoes::conferir(state).map_err(mensagem_do_erro)?;
    let ajustes = super::super::ajustes::Ajustes { profile_id: None, model: None, effort: None };
    let plano = plano_de_retomada_em(state, conversa, ajustes, None).map_err(mensagem_do_erro)?;
    if plano.session_id.is_empty() { return Err("sem id nativo".into()); }
    let acao = Action { id: state.next_id(), conversation_id: conversa.into(), session_id: String::new(), state: "executing".into(),
        created_at_ms: now_ms(), error: None, key: String::new(), revision: String::new(), kind: ActionKind::Retomar(plano.clone()) };
    retomar(state, &acao, &plano).map_err(|e| e.to_string())?;
    Ok(current(state, conversa).ok().and_then(|(_, s)| s).and_then(|s| sessao_de(&s)))
}

/// Manda a mensagem assim que a CLI chegar ao campo vazio (em outra thread).
fn enviar_quando_pronta(state: &Arc<EngineState>, conversa: &str, texto: String) {
    state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").insert(conversa.to_owned(), "esperando a CLI ficar pronta".into());
    let (estado, id) = (state.clone(), conversa.to_owned());
    std::thread::spawn(move || {
        super::ferramentas::mandar_tarefa_quando_pronta(&estado, &id, &texto, std::time::Duration::from_secs(180));
        estado.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").remove(&id);
    });
}

/* ── Conversa pelo Telegram ────────────────────────────────────────────────────────────────── */

/// Manda `texto` ao agente da rotina (abre/reabre a conversa da semana se precisar), espera o turno
/// terminar e devolve a última resposta dele. Bloqueante (chame fora do runtime).
pub(crate) fn conversar(state: &Arc<EngineState>, texto: &str, prazo: std::time::Duration) -> Result<String, String> {
    regra::texto_valido(texto)?;
    let aberta = abrir(state, &PedidoDeConversa { modo: "conversar".into(), ..Default::default() })?;
    let conversa = aberta["conversa"].as_str().unwrap_or_default().to_owned();
    let limite = std::time::Instant::now() + prazo;
    // Espera a primeira mensagem (conversa nova) sair e a CLI ficar pronta.
    let antes = loop {
        if std::time::Instant::now() >= limite { return Err("O agente da rotina não ficou pronto a tempo.".into()); }
        let pendente = state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").contains_key(&conversa);
        if let (false, Ok((_, Some(sessao)))) = (pendente, current(state, &conversa)) {
            let cap = interaction::capabilities(&sessao);
            if cap.prompt || cap.enfileirar {
                let mut headers = HeaderMap::new();
                headers.insert("idempotency-key", format!("rotina-tg-{}", uuid::Uuid::new_v4()).parse().expect("chave"));
                if let Ok(r) = cap.revision.parse() { headers.insert("if-match", r); }
                let turnos_antes = turnos(&sessao);
                let corpo = format!("[Pelo Telegram] {texto}");
                let kind = if cap.prompt { ActionKind::Prompt(corpo) } else { ActionKind::Fila(corpo) };
                if enqueue(state, &conversa, &headers, kind).is_ok() { break turnos_antes; }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    let mut viu_turno = false;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let (estado, sessao, _) = estado_de(state, &conversa);
        viu_turno |= sessao.as_ref().is_some_and(|s| turnos(s) > antes);
        match estado {
            regra::Estado::Pronto if viu_turno => break,
            regra::Estado::EsperandoVoce => return Ok("O agente da rotina está esperando uma aprovação no app. Abra a Rotina no computador.".into()),
            regra::Estado::Erro | regra::Estado::Parado => return Err("A conversa da rotina parou. Abra a Rotina no computador.".into()),
            _ => {}
        }
        if std::time::Instant::now() >= limite { return Ok("Ainda estou trabalhando nisso; a resposta fica na conversa da Rotina no app.".into()); }
    }
    Ok(ultima_do_agente(state, &conversa).map(|t| regra::resumir(&t, 3500)).unwrap_or_else(|| "Feito.".into()))
}

/* ── Rotas ─────────────────────────────────────────────────────────────────────────────────── */

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/rotina/conversa", get(conversa_atual).post(abrir_rota))
        .route("/rotina/briefing", get(|| async { Json(json!({"briefing": briefing()})) }))
}

async fn abrir_rota(State(state): State<Arc<EngineState>>, Json(pedido): Json<PedidoDeConversa>) -> Result<Json<Value>, ApiError> {
    if !matches!(pedido.modo.as_str(), "planejar" | "conversar") {
        return Err(error(StatusCode::BAD_REQUEST, "`modo` é planejar ou conversar"));
    }
    for valor in [&pedido.plataforma_padrao, &pedido.conta_padrao].into_iter().flatten() {
        if valor.len() > 120 || valor.chars().any(char::is_control) { return Err(error(StatusCode::BAD_REQUEST, "Valor inválido")); }
    }
    tokio::task::spawn_blocking(move || abrir(&state, &pedido).map(Json).map_err(|e| error(StatusCode::CONFLICT, &e)))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao abrir a conversa da rotina"))?
}

/// A conversa da semana (sem abrir nada): id, sessão viva e estado.
async fn conversa_atual(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || {
        let agora = super::super::rotina_rotas::agora();
        let ajustes: AjustesDaRotina = rotina_core::ler(dir(&state));
        let segunda = semana_alvo(&ajustes, agora.hoje, false);
        let Some(conversa) = ajustes.conversa_da_semana(&rotina_core::texto_da_data(segunda)) else {
            return json!({"conversa": null, "sessao": null, "semana": rotina_core::texto_da_data(segunda)});
        };
        let (estado, sessao, _) = estado_de(&state, &conversa);
        json!({"conversa": conversa, "sessao": sessao.as_ref().and_then(sessao_de), "estado": estado.texto(),
            "semana": rotina_core::texto_da_data(segunda)})
    }).await.unwrap_or(json!({"conversa": null, "sessao": null})))
}
