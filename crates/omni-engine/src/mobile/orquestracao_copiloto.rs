//! Fork (Copiloto abre conversa na tela, 2026-10-22): o que o copiloto tem a mais que um orquestrador
//! comum, e de que projeto sai o subagente de qualquer orquestrador.
//!
//! O copiloto vive no projeto especial `copiloto`, que não é projeto da lateral. Antes o subagente
//! herdava esse id, e `resolve_spawn` só aceita projeto de `known_projects`: com a pasta do copiloto
//! já ligada a um projeto publicado (a pasta do usuário, no PC do Enzo), `known_projects` deduplica
//! por caminho, o id `copiloto` some e a criação falhava com "Projeto desconhecido; abra o app no PC…"
//! (o agente então tentava `open -a Niski`). Agora o projeto é resolvido aqui, sempre no servidor:
//! um id de `known_projects` (lista fechada) ou a pasta da própria sessão do orquestrador, que o
//! usuário escolheu no app. Nada vem de caminho mandado pelo agente.
use super::*;

/// A conversa orquestradora é a do copiloto (projeto especial `copiloto`)?
pub(super) fn e_do_copiloto(state: &EngineState, orquestrador: &str) -> bool {
    current(state, orquestrador).is_ok_and(|(c, _)| c.project_id == PROJETO_DO_COPILOTO)
}

const NAO_ABRA_O_APP: &str = "O app já está aberto: não rode `open -a`, nem peça ao usuário para cadastrar projeto.";

/// Projeto (id, pasta) em que o subagente abre. `pedido` = id de `listar_projetos`; sem ele, o
/// projeto da conversa orquestradora, o projeto conhecido da mesma pasta ou, se a pasta não é de
/// projeto nenhum, ela mesma com o id estável de pasta (o mesmo que o histórico usa).
pub(super) fn projeto_do_subagente(state: &EngineState, mae: &omni_core::conversations::Conversation, pedido: Option<&str>)
    -> Result<PublishedProject, String> {
    // O projeto especial nunca é escolhível: um subagente nele nasceria outro copiloto (`marcar_copiloto`).
    let conhecidos: Vec<PublishedProject> = known_projects(state).into_iter().filter(|p| p.id != PROJETO_DO_COPILOTO).collect();
    let projeto = match pedido {
        Some(id) => conhecidos.into_iter().find(|p| p.id == id).ok_or_else(|| format!(
            "Projeto desconhecido: {id}. Use listar_projetos e passe um id da lista, ou crie sem projeto_id (abre na pasta do copiloto). {NAO_ABRA_O_APP}"))?,
        None => conhecidos.iter().find(|p| p.id == mae.project_id)
            .or_else(|| conhecidos.iter().find(|p| same_path(&p.path, &mae.cwd))).cloned()
            .unwrap_or_else(|| PublishedProject {
                id: history_project_id(&mae.cwd),
                name: Path::new(&mae.cwd).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| mae.cwd.clone()),
                path: mae.cwd.clone(),
            }),
    };
    if !Path::new(&projeto.path).is_dir() {
        return Err(format!("A pasta do projeto {} não existe mais neste PC. Use listar_projetos e escolha outro.", projeto.name));
    }
    Ok(projeto)
}

/// As ferramentas do copiloto: as do orquestrador, com `projeto_id` e `na_tela` no `criar_agente`, e
/// `listar_projetos`.
pub(super) fn lista() -> Value {
    let mut lista = ferramentas::lista();
    let itens = lista.as_array_mut().expect("lista de ferramentas");
    if let Some(criar) = itens.iter_mut().find(|f| f["name"] == "criar_agente") {
        let descricao = criar["description"].as_str().unwrap_or_default().to_owned();
        criar["description"] = json!(format!("{descricao} No copiloto: abre na pasta do copiloto (ou no projeto_id de \
            listar_projetos). Quando o usuário pedir para abrir, mostrar ou começar uma conversa (\"abre uma conversa na tela com \
            o Codex no gpt-6.1-sol médio\"), use na_tela: true com plataforma, conta, modelo e esforço pedidos: a conversa abre \
            numa aba do workspace atual e tarefa vira opcional (sem ela, a conversa espera o usuário). {NAO_ABRA_O_APP}"));
        let schema = &mut criar["inputSchema"];
        schema["properties"]["projeto_id"] = json!({"type": "string",
            "description": "Id exato de listar_projetos. Sem ele, a pasta do copiloto. Nunca invente um caminho"});
        schema["properties"]["na_tela"] = json!({"type": "boolean",
            "description": "true: o usuário quer ver/usar a conversa. Abre numa aba do workspace atual; tarefa fica opcional"});
        schema["properties"]["tarefa"]["description"] = json!("O objetivo do subagente, claro e único (opcional com na_tela)");
        schema["required"] = json!(["plataforma"]);
    }
    itens.push(json!({"name": "listar_projetos",
        "description": "Projetos da lateral do app (id, nome, pasta) e o padrão (a pasta do copiloto). Use o id em criar_agente(projeto_id) quando o usuário pedir outra pasta.",
        "inputSchema": {"type": "object", "properties": {}}}));
    lista
}

pub(super) async fn chamar(state: &Arc<EngineState>, orquestrador: &str, nome: &str, args: &Value) -> Result<String, String> {
    if nome != "listar_projetos" { return ferramentas::chamar(state, orquestrador, nome, args).await; }
    let (state, orquestrador) = (state.clone(), orquestrador.to_owned());
    tokio::task::spawn_blocking(move || listar_projetos(&state, &orquestrador)).await.map_err(|_| "Falha interna".to_owned())?
}

fn listar_projetos(state: &EngineState, orquestrador: &str) -> Result<String, String> {
    let (mae, _) = current(state, orquestrador).map_err(|_| "Conversa do copiloto não encontrada".to_owned())?;
    let padrao = projeto_do_subagente(state, &mae, None).ok();
    let publicados = state.mobile.workspace.lock().expect("workspace poisoned").projects.clone();
    let item = |p: &PublishedProject| json!({"id": p.id, "nome": p.name, "pasta": p.path});
    Ok(serde_json::to_string_pretty(&json!({
        "padrao": padrao.as_ref().map(item),
        "projetos": publicados.iter().filter(|p| p.id != PROJETO_DO_COPILOTO).map(item).collect::<Vec<_>>(),
        "nota": "Sem projeto_id, criar_agente abre no padrão. Com projeto_id, use um id desta lista.",
    })).unwrap_or_default())
}
