//! Fork: o que a conversa ganha além de texto e ação — lido do mesmo transcript da CLI.
//!
//! - **Lista de tarefas** do agente: `TodoWrite` (lista inteira a cada chamada) e
//!   `TaskCreate`/`TaskUpdate` (uma tarefa por chamada; o id sai no resultado do `TaskCreate`) do
//!   Claude; `update_plan` do Codex (direto ou de dentro do `exec`). Cada chamada leva a lista como
//!   ficou depois dela (`tarefas`) e a timeline leva a lista atual, para o cartão fixo acima do
//!   campo de mensagem.
//! - **Plano do plan mode**: o `plan` do `ExitPlanMode` do Claude, em markdown.
//! - **Modo de permissão**: a última linha `permission-mode` do Claude; no Codex, o `turn_context`
//!   (`approval_policy`, `sandbox_policy`, `collaboration_mode`).
//! - **Diffs** das edições: ver `diff_do_agente`.
//!
//! Tudo é campo novo e opcional no JSON da timeline: o celular com a tela antiga ignora.

use crate::diff_do_agente::{self, DiffDeArquivo, TipoDeMudanca};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

/// Maior plano (plan mode) que vai na timeline.
const PLANO_MAX: usize = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoDaTarefa {
    Pendente,
    Andamento,
    Feita,
}

impl EstadoDaTarefa {
    fn da_cli(status: &str) -> Option<Self> {
        match status {
            "pending" => Some(Self::Pendente),
            "in_progress" => Some(Self::Andamento),
            "completed" => Some(Self::Feita),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tarefa {
    pub texto: String,
    pub estado: EstadoDaTarefa,
    /// Como a CLI descreve a tarefa enquanto a faz ("Rodando os testes", o `activeForm` do Claude).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fazendo: Option<String>,
}

/// O que uma chamada muda na lista. Fica fora do JSON: quem aplica é o [`EstadoDaConversa`], que
/// lembra a lista entre uma linha do transcript e outra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MudancaDeTarefas {
    Todas(Vec<Tarefa>),
    Criar { chamada: Option<String>, tarefa: Tarefa },
    Atualizar { id: String, estado: Option<EstadoDaTarefa>, texto: Option<String>, apagar: bool },
}

/// Campos novos de um item da timeline (achatados no JSON do item).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Extras {
    /// Edição do agente: o mini-diff de cada arquivo.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diffs: Vec<DiffDeArquivo>,
    /// Chamada que mexeu na lista de tarefas: a lista como ficou depois dela.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tarefas: Option<Vec<Tarefa>>,
    /// `ExitPlanMode`: o plano em markdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plano: Option<String>,
    #[serde(skip)]
    pub mudanca: Option<MudancaDeTarefas>,
}

fn texto<'a>(valor: &'a Value, chave: &str) -> Option<&'a str> {
    valor.get(chave)?.as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn cortar(texto: &str, max: usize) -> String {
    if texto.len() <= max { return texto.to_owned(); }
    let mut fim = max;
    while !texto.is_char_boundary(fim) { fim -= 1; }
    format!("{}\n\n…", &texto[..fim])
}

/// Primeira linha do plano, sem os `#` do título: vira o texto curto da ação.
pub fn titulo_do_plano(plano: &str) -> String {
    plano.lines().map(|l| l.trim().trim_start_matches('#').trim()).find(|l| !l.is_empty()).unwrap_or("Plano").to_owned()
}

fn tarefas_do_todo(lista: &Value, chave_texto: &str) -> Option<Vec<Tarefa>> {
    Some(lista.as_array()?.iter().filter_map(|t| Some(Tarefa {
        texto: texto(t, chave_texto)?.to_owned(),
        estado: t["status"].as_str().and_then(EstadoDaTarefa::da_cli).unwrap_or(EstadoDaTarefa::Pendente),
        fazendo: texto(t, "activeForm").map(str::to_owned),
    })).collect())
}

/// Extras de uma chamada de ferramenta do Claude (`tool_use`). `chamada` é o `id` do bloco.
pub fn extras_claude(nome: &str, input: &Value, chamada: Option<&str>) -> Extras {
    let mut extras = Extras::default();
    let arquivo = texto(input, "file_path").unwrap_or("");
    match nome {
        "Edit" => extras.diffs.push(diff_do_agente::de_edicao(arquivo,
            input["old_string"].as_str().unwrap_or(""), input["new_string"].as_str().unwrap_or(""))),
        "MultiEdit" => {
            let edicoes: Vec<(&str, &str)> = input["edits"].as_array().into_iter().flatten()
                .map(|e| (e["old_string"].as_str().unwrap_or(""), e["new_string"].as_str().unwrap_or(""))).collect();
            if !edicoes.is_empty() { extras.diffs.push(diff_do_agente::de_edicoes(arquivo, &edicoes)); }
        }
        "Write" => if let Some(conteudo) = input["content"].as_str() { extras.diffs.push(diff_do_agente::de_escrita(arquivo, conteudo)); },
        "TodoWrite" => if let Some(lista) = tarefas_do_todo(&input["todos"], "content") {
            extras.tarefas = Some(lista.clone());
            extras.mudanca = Some(MudancaDeTarefas::Todas(lista));
        },
        "TaskCreate" => if let Some(assunto) = texto(input, "subject") {
            extras.mudanca = Some(MudancaDeTarefas::Criar { chamada: chamada.map(str::to_owned), tarefa: Tarefa {
                texto: assunto.to_owned(),
                estado: input["status"].as_str().and_then(EstadoDaTarefa::da_cli).unwrap_or(EstadoDaTarefa::Pendente),
                fazendo: texto(input, "activeForm").map(str::to_owned),
            } });
        },
        "TaskUpdate" => if let Some(id) = input["taskId"].as_str().map(str::to_owned).or_else(|| input["taskId"].as_u64().map(|n| n.to_string())) {
            let status = input["status"].as_str();
            extras.mudanca = Some(MudancaDeTarefas::Atualizar {
                id,
                estado: status.and_then(EstadoDaTarefa::da_cli),
                texto: texto(input, "subject").map(str::to_owned),
                apagar: status == Some("deleted"),
            });
        },
        "ExitPlanMode" => extras.plano = texto(input, "plan").map(|p| cortar(p, PLANO_MAX)),
        _ => {}
    }
    diff_do_agente::limitar(&mut extras.diffs);
    extras
}

/// Argumento de `tools.<ferramenta>(…)` dentro do JavaScript do `exec` do Codex, quando é JSON
/// (string ou objeto literal). Variável ou template string ficam de fora.
fn argumento_do_exec(codigo: &str, ferramenta: &str) -> Option<Value> {
    let marca = format!("tools.{ferramenta}(");
    let inicio = codigo.find(&marca)? + marca.len();
    serde_json::Deserializer::from_str(codigo[inicio..].trim_start()).into_iter::<Value>().next()?.ok()
}

fn arquivos_dos_diffs(diffs: &[DiffDeArquivo]) -> String {
    diffs.iter().map(|d| d.arquivo.as_str()).collect::<Vec<_>>().join(", ")
}

/// O `exec` do Codex que só chama `apply_patch` ou `update_plan`: a linha da ação (rótulo, detalhe).
/// Sem isto o `exec` sem comando sumia da conversa (o patch só aparecia como nada).
pub fn ferramenta_do_exec(codigo: &str) -> Option<(String, String)> {
    if let Some(patch) = argumento_do_exec(codigo, "apply_patch") {
        let diffs = diff_do_agente::do_patch(patch.as_str()?);
        return (!diffs.is_empty()).then(|| ("Edit".into(), crate::feed::curto(&arquivos_dos_diffs(&diffs))));
    }
    argumento_do_exec(codigo, "update_plan").filter(|a| a["plan"].is_array()).map(|_| ("update_plan".into(), "Atualizou o plano".into()))
}

/// Extras de uma chamada do Codex (`response_item` de `function_call`/`custom_tool_call`).
pub fn extras_codex(payload: &Value) -> Extras {
    let mut extras = Extras::default();
    let nome = payload["name"].as_str().unwrap_or("");
    let plano = |args: &Value| tarefas_do_todo(&args["plan"], "step");
    match payload["type"].as_str() {
        Some("custom_tool_call") => {
            let input = payload["input"].as_str().unwrap_or("");
            if nome == "exec" {
                if let Some(patch) = argumento_do_exec(input, "apply_patch") {
                    extras.diffs = diff_do_agente::do_patch(patch.as_str().unwrap_or(""));
                } else if let Some(lista) = argumento_do_exec(input, "update_plan").as_ref().and_then(plano) {
                    extras.tarefas = Some(lista.clone());
                    extras.mudanca = Some(MudancaDeTarefas::Todas(lista));
                }
            } else if nome == "apply_patch" || input.trim_start().starts_with("*** Begin Patch") {
                extras.diffs = diff_do_agente::do_patch(input);
            }
        }
        Some("function_call") if nome == "update_plan" => {
            let args: Value = serde_json::from_str(payload["arguments"].as_str().unwrap_or("{}")).unwrap_or(Value::Null);
            if let Some(lista) = plano(&args) {
                extras.tarefas = Some(lista.clone());
                extras.mudanca = Some(MudancaDeTarefas::Todas(lista));
            }
        }
        _ => {}
    }
    diff_do_agente::limitar(&mut extras.diffs);
    extras
}

/// Caminho relativo à pasta da conversa, quando está dentro dela: na linha da ação o caminho
/// absoluto inteiro empurrava o nome do arquivo para fora.
fn relativo(caminho: &str, cwd: Option<&Path>) -> String {
    cwd.and_then(|pasta| Path::new(caminho).strip_prefix(pasta).ok())
        .map(|r| r.to_string_lossy().into_owned()).filter(|r| !r.is_empty())
        .unwrap_or_else(|| caminho.to_owned())
}

/// Item `FileChange` concluído do Codex (`event_msg` → `item_completed`): (detalhe, diffs).
pub fn file_change_do_codex(item: &Value, cwd: Option<&Path>) -> Option<(String, Vec<DiffDeArquivo>)> {
    if item["type"] != "FileChange" { return None; }
    // Recusado ou falhou: o arquivo não mudou.
    if item["status"].as_str().is_some_and(|s| s != "completed") { return None; }
    let mut diffs: Vec<DiffDeArquivo> = item["changes"].as_object()?.iter().map(|(caminho, mudanca)| {
        let tipo = match mudanca["type"].as_str() { Some("add") => TipoDeMudanca::Novo, Some("delete") => TipoDeMudanca::Apagado, _ => TipoDeMudanca::Editado };
        let mut nome = relativo(caminho, cwd);
        if let Some(destino) = mudanca["move_path"].as_str() { nome = format!("{nome} → {}", relativo(destino, cwd)); }
        diff_do_agente::do_unified(&nome, tipo, mudanca["unified_diff"].as_str().unwrap_or(""))
    }).collect();
    if diffs.is_empty() { return None; }
    diff_do_agente::limitar(&mut diffs);
    Some((crate::feed::curto(&arquivos_dos_diffs(&diffs)), diffs))
}

/// Modo de permissão do Codex num vocabulário só (`plan`, `acessoTotal`, `somenteLeitura`,
/// `semPerguntar`, `default`), a partir do `turn_context`.
fn modo_do_codex(contexto: &Value) -> Option<String> {
    if contexto["collaboration_mode"]["mode"] == "plan" { return Some("plan".into()); }
    let sandbox = contexto["sandbox_policy"]["type"].as_str().or_else(|| contexto["sandbox_policy"].as_str());
    let aprovacao = contexto["approval_policy"].as_str();
    if sandbox.is_none() && contexto["approval_policy"].is_null() { return None; }
    Some(match (sandbox, aprovacao) {
        (Some("danger-full-access"), _) => "acessoTotal",
        (Some("read-only"), _) => "somenteLeitura",
        (_, Some("never")) => "semPerguntar",
        _ => "default",
    }.into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TarefaViva {
    /// Id que a CLI deu (`TaskCreate` → resultado). `TodoWrite`/`update_plan` não têm.
    id: Option<String>,
    /// Id da chamada que criou, para casar com o resultado que traz o id.
    chamada: Option<String>,
    tarefa: Tarefa,
}

/// O que se sabe de uma conversa ao longo do transcript: a lista de tarefas, o modo de permissão e
/// o último patch mostrado. A leitura é incremental, então isto anda junto com ela (ver `feed`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EstadoDaConversa {
    tarefas: Vec<TarefaViva>,
    /// Alguma chamada de lista já apareceu (lista vazia depois disso é "limpou", não "nunca teve").
    com_lista: bool,
    pub modo: Option<String>,
    /// Arquivos do último patch do Codex mostrado com diff: o `FileChange` que vem logo depois é o
    /// mesmo patch aplicado e não repete a linha.
    ultimo_patch: Vec<String>,
}

impl EstadoDaConversa {
    /// Linhas que não viram item mas mudam o estado: modo de permissão e id de tarefa criada.
    pub fn observar(&mut self, valor: &Value, provider: &str) {
        match provider {
            "claude" if valor["type"] == "permission-mode" => {
                if let Some(modo) = texto(valor, "permissionMode") { self.modo = Some(modo.to_owned()); }
            }
            "claude" if valor["type"] == "user" => {
                let Some(id) = valor["toolUseResult"]["task"]["id"].as_str().map(str::to_owned)
                    .or_else(|| valor["toolUseResult"]["task"]["id"].as_u64().map(|n| n.to_string())) else { return };
                let chamadas: Vec<&str> = valor["message"]["content"].as_array().into_iter().flatten()
                    .filter(|b| b["type"] == "tool_result").filter_map(|b| b["tool_use_id"].as_str()).collect();
                if let Some(viva) = self.tarefas.iter_mut().find(|t| t.id.is_none() && t.chamada.as_deref().is_some_and(|c| chamadas.contains(&c))) {
                    viva.id = Some(id);
                }
            }
            "codex" if valor["type"] == "turn_context" => {
                if let Some(modo) = modo_do_codex(&valor["payload"]) { self.modo = Some(modo); }
            }
            _ => {}
        }
    }

    /// Posição da tarefa `id`. Sem o resultado do `TaskCreate` (linha perdida), o id numérico do
    /// Claude é a ordem de criação: a n-ésima criada ainda sem id.
    fn achar(&self, id: &str) -> Option<usize> {
        self.tarefas.iter().position(|t| t.id.as_deref() == Some(id)).or_else(|| {
            let n: usize = id.parse().ok()?;
            let k = self.tarefas.iter().enumerate().filter(|(_, t)| t.chamada.is_some()).nth(n.checked_sub(1)?)?.0;
            self.tarefas[k].id.is_none().then_some(k)
        })
    }

    /// Aplica a mudança de lista da chamada e devolve o texto curto dela ("3/7 · Rodando testes").
    /// `None` quando a chamada não mexe na lista.
    pub fn aplicar(&mut self, extras: &mut Extras) -> Option<String> {
        let mudanca = extras.mudanca.take()?;
        let resumo = match mudanca {
            MudancaDeTarefas::Todas(lista) => {
                self.tarefas = lista.into_iter().map(|tarefa| TarefaViva { id: None, chamada: None, tarefa }).collect();
                None
            }
            MudancaDeTarefas::Criar { chamada, tarefa } => {
                let resumo = format!("Nova: {}", tarefa.texto);
                self.tarefas.push(TarefaViva { id: None, chamada, tarefa });
                Some(resumo)
            }
            MudancaDeTarefas::Atualizar { id, estado, texto, apagar } => {
                let k = self.achar(&id)?;
                if self.tarefas[k].id.is_none() { self.tarefas[k].id = Some(id); }
                if apagar {
                    Some(format!("Removida: {}", self.tarefas.remove(k).tarefa.texto))
                } else {
                    let tarefa = &mut self.tarefas[k].tarefa;
                    let renomeada = texto.is_some();
                    if let Some(novo) = texto { tarefa.texto = novo; }
                    if let Some(estado) = estado { tarefa.estado = estado; }
                    Some(match estado {
                        Some(EstadoDaTarefa::Feita) => format!("Feita: {}", tarefa.texto),
                        Some(EstadoDaTarefa::Andamento) => format!("Começou: {}", tarefa.fazendo.as_deref().unwrap_or(&tarefa.texto)),
                        Some(EstadoDaTarefa::Pendente) => format!("Reaberta: {}", tarefa.texto),
                        None if renomeada => format!("Renomeada: {}", tarefa.texto),
                        None => format!("Atualizou: {}", tarefa.texto),
                    })
                }
            }
        };
        self.com_lista = true;
        let lista = self.lista();
        extras.tarefas = Some(lista.clone());
        Some(resumo.unwrap_or_else(|| resumo_da_lista(&lista)))
    }

    fn lista(&self) -> Vec<Tarefa> { self.tarefas.iter().map(|t| t.tarefa.clone()).collect() }

    /// A lista atual, se a conversa já teve uma.
    pub fn tarefas(&self) -> Option<Vec<Tarefa>> { self.com_lista.then(|| self.lista()) }

    /// Diz se o `FileChange` com estes arquivos é o patch que acabou de ser mostrado (e esquece
    /// esse patch: o próximo `FileChange` é outro).
    pub fn patch_repetido(&mut self, diffs: &[DiffDeArquivo]) -> bool {
        let anteriores = std::mem::take(&mut self.ultimo_patch);
        !anteriores.is_empty() && diffs.iter().all(|d| anteriores.iter().any(|a|
            d.arquivo == *a || d.arquivo.ends_with(&format!("/{a}")) || a.ends_with(&format!("/{}", d.arquivo))))
    }

    pub fn lembrar_patch(&mut self, diffs: &[DiffDeArquivo]) {
        self.ultimo_patch = diffs.iter().map(|d| d.arquivo.clone()).collect();
    }
}

/// "3/7 · Rodando os testes": quantas feitas e a que está em andamento.
pub fn resumo_da_lista(lista: &[Tarefa]) -> String {
    if lista.is_empty() { return "Limpou a lista de tarefas".into(); }
    let feitas = lista.iter().filter(|t| t.estado == EstadoDaTarefa::Feita).count();
    match lista.iter().find(|t| t.estado == EstadoDaTarefa::Andamento) {
        Some(atual) => format!("{feitas}/{} · {}", lista.len(), atual.fazendo.as_deref().unwrap_or(&atual.texto)),
        None => format!("{feitas}/{}", lista.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn todo_write_vira_lista_e_resumo() {
        let mut extras = extras_claude("TodoWrite", &json!({"todos": [
            {"content": "Ler o código", "status": "completed", "activeForm": "Lendo o código"},
            {"content": "Rodar testes", "status": "in_progress", "activeForm": "Rodando testes"},
            {"content": "Abrir PR", "status": "pending", "activeForm": "Abrindo PR"},
        ]}), None);
        assert_eq!(extras.tarefas.as_ref().map(Vec::len), Some(3));
        let mut estado = EstadoDaConversa::default();
        assert_eq!(estado.tarefas(), None);
        assert_eq!(estado.aplicar(&mut extras).as_deref(), Some("1/3 · Rodando testes"));
        let atual = estado.tarefas().unwrap();
        assert_eq!(atual.iter().map(|t| t.estado).collect::<Vec<_>>(),
            vec![EstadoDaTarefa::Feita, EstadoDaTarefa::Andamento, EstadoDaTarefa::Pendente]);
        // Lista vazia: a conversa teve lista e limpou.
        let mut vazia = extras_claude("TodoWrite", &json!({"todos": []}), None);
        assert_eq!(estado.aplicar(&mut vazia).as_deref(), Some("Limpou a lista de tarefas"));
        assert_eq!(estado.tarefas(), Some(vec![]));
    }

    #[test]
    fn task_create_e_update_casam_pelo_id_do_resultado() {
        let mut estado = EstadoDaConversa::default();
        let mut a = extras_claude("TaskCreate", &json!({"subject": "Escrever teste", "description": "x", "activeForm": "Escrevendo teste"}), Some("call_a"));
        assert_eq!(estado.aplicar(&mut a).as_deref(), Some("Nova: Escrever teste"));
        let mut b = extras_claude("TaskCreate", &json!({"subject": "Corrigir bug", "description": "y", "activeForm": "Corrigindo bug"}), Some("call_b"));
        estado.aplicar(&mut b);
        // O id chega no resultado, fora de ordem (o da segunda primeiro).
        estado.observar(&json!({"type": "user", "toolUseResult": {"task": {"id": "7", "subject": "Corrigir bug"}},
            "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "call_b"}]}}), "claude");
        let mut feito = extras_claude("TaskUpdate", &json!({"taskId": "7", "status": "completed"}), None);
        assert_eq!(estado.aplicar(&mut feito).as_deref(), Some("Feita: Corrigir bug"));
        assert_eq!(feito.tarefas.as_ref().unwrap()[1].estado, EstadoDaTarefa::Feita);
        let mut comecou = extras_claude("TaskUpdate", &json!({"taskId": "1", "status": "in_progress"}), None);
        // Sem resultado para a primeira: o id numérico é a ordem de criação entre as sem id.
        assert_eq!(estado.aplicar(&mut comecou).as_deref(), Some("Começou: Escrevendo teste"));
        let mut some = extras_claude("TaskUpdate", &json!({"taskId": "7", "status": "deleted"}), None);
        assert_eq!(estado.aplicar(&mut some).as_deref(), Some("Removida: Corrigir bug"));
        assert_eq!(estado.tarefas().unwrap().len(), 1);
        // Tarefa desconhecida não mexe em nada.
        let mut outra = extras_claude("TaskUpdate", &json!({"taskId": "99", "status": "completed"}), None);
        assert_eq!(estado.aplicar(&mut outra), None);
    }

    #[test]
    fn update_plan_do_codex_direto_e_pelo_exec() {
        let direto = json!({"type": "function_call", "name": "update_plan",
            "arguments": "{\"explanation\":\"x\",\"plan\":[{\"step\":\"Ler\",\"status\":\"completed\"},{\"step\":\"Mudar\",\"status\":\"in_progress\"}]}"});
        let mut extras = extras_codex(&direto);
        let mut estado = EstadoDaConversa::default();
        assert_eq!(estado.aplicar(&mut extras).as_deref(), Some("1/2 · Mudar"));

        let codigo = "await tools.update_plan({\"plan\":[{\"step\":\"Só um\",\"status\":\"pending\"}]});";
        let exec = json!({"type": "custom_tool_call", "name": "exec", "input": codigo});
        assert_eq!(ferramenta_do_exec(codigo), Some(("update_plan".into(), "Atualizou o plano".into())));
        assert_eq!(extras_codex(&exec).tarefas.unwrap()[0].texto, "Só um");
    }

    #[test]
    fn patch_do_codex_pelo_exec_e_file_change() {
        let patch = "*** Begin Patch\n*** Update File: src/a.ts\n@@\n-x\n+y\n*** End Patch\n";
        let codigo = format!("const r = await tools.apply_patch({});\nconsole.log(r);", serde_json::to_string(patch).unwrap());
        assert_eq!(ferramenta_do_exec(&codigo), Some(("Edit".into(), "src/a.ts".into())));
        let extras = extras_codex(&json!({"type": "custom_tool_call", "name": "exec", "input": codigo}));
        assert_eq!(extras.diffs[0].linhas, vec!["-x", "+y"]);
        // Patch em variável: não dá para ler aqui (o `FileChange` cobre).
        assert_eq!(ferramenta_do_exec("await tools.apply_patch(p)"), None);

        let item = json!({"type": "FileChange", "status": "completed", "changes": {
            "/proj/src/a.ts": {"type": "update", "unified_diff": "@@ -1 +1 @@\n-x\n+y\n", "move_path": null}}});
        let (detalhe, diffs) = file_change_do_codex(&item, Some(Path::new("/proj"))).unwrap();
        assert_eq!((detalhe.as_str(), diffs[0].adicionadas), ("src/a.ts", 1));
        let mut estado = EstadoDaConversa::default();
        estado.lembrar_patch(&extras.diffs);
        assert!(estado.patch_repetido(&diffs));
        assert!(!estado.patch_repetido(&diffs), "o mesmo FileChange de novo já é outro patch");
        let recusado = json!({"type": "FileChange", "status": "declined", "changes": {"/a": {"type": "add", "unified_diff": ""}}});
        assert!(file_change_do_codex(&recusado, None).is_none());
    }

    #[test]
    fn modo_de_permissao_do_claude_e_do_codex() {
        let mut estado = EstadoDaConversa::default();
        estado.observar(&json!({"type": "permission-mode", "permissionMode": "acceptEdits", "sessionId": "s"}), "claude");
        assert_eq!(estado.modo.as_deref(), Some("acceptEdits"));
        let contexto = |aprovacao: &str, sandbox: &str, modo: &str| json!({"type": "turn_context", "payload": {
            "approval_policy": aprovacao, "sandbox_policy": {"type": sandbox}, "collaboration_mode": {"mode": modo}}});
        let mut codex = EstadoDaConversa::default();
        for (aprovacao, sandbox, modo, esperado) in [
            ("on-request", "workspace-write", "default", "default"),
            ("never", "workspace-write", "default", "semPerguntar"),
            ("never", "danger-full-access", "default", "acessoTotal"),
            ("on-request", "read-only", "default", "somenteLeitura"),
            ("on-request", "workspace-write", "plan", "plan"),
        ] {
            codex.observar(&contexto(aprovacao, sandbox, modo), "codex");
            assert_eq!(codex.modo.as_deref(), Some(esperado));
        }
    }

    fn conversa(dir: &Path, provider: &str, linhas: &[Value]) -> crate::conversations::Conversation {
        let transcript = dir.join("t.jsonl");
        std::fs::write(&transcript, linhas.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        serde_json::from_value(json!({"id": "c", "project_id": "p", "cwd": "/proj", "title": "t", "created_at_ms": 0,
            "segments": [{"provider": provider, "profile_id": "conta", "external_session_id": null,
                "transcript_path": transcript.to_string_lossy(), "terminal_session_id": null, "started_at_ms": 0, "ended_at_ms": null}]})).unwrap()
    }

    /// Conta cuja pasta é `dir`: o transcript só é lido de dentro da pasta da conta.
    fn conta(dir: &Path, provider: &str) -> Vec<crate::Profile> {
        serde_json::from_value(json!([{"id": "conta", "provider": provider, "name": "Conta",
            "config_dir": dir.to_string_lossy(), "created_at_ms": 0}])).unwrap()
    }

    /// Transcript sintético do Claude de ponta a ponta: o JSON da timeline leva a lista atual, o modo,
    /// o diff da edição e o plano — tudo em campos novos, sem mexer nos antigos.
    #[test]
    fn timeline_do_claude_leva_lista_modo_diff_e_plano() {
        let dir = tempfile::tempdir().unwrap();
        let ferramenta = |uuid: &str, id: &str, nome: &str, input: Value| json!({"type": "assistant", "uuid": uuid,
            "message": {"role": "assistant", "content": [{"type": "tool_use", "id": id, "name": nome, "input": input}]}});
        let resultado = |uuid: &str, id: &str, extra: Value| json!({"type": "user", "uuid": uuid, "toolUseResult": extra,
            "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": id, "content": "ok"}]}});
        let c = conversa(dir.path(), "claude", &[
            json!({"type": "permission-mode", "permissionMode": "default", "sessionId": "s"}),
            json!({"type": "user", "uuid": "u0", "message": {"role": "user", "content": "faça"}}),
            ferramenta("a1", "t1", "TaskCreate", json!({"subject": "Ler", "description": "d", "activeForm": "Lendo"})),
            resultado("r1", "t1", json!({"task": {"id": "1", "subject": "Ler"}})),
            ferramenta("a2", "t2", "TaskCreate", json!({"subject": "Editar", "description": "d", "activeForm": "Editando"})),
            resultado("r2", "t2", json!({"task": {"id": "2", "subject": "Editar"}})),
            ferramenta("a3", "t3", "TaskUpdate", json!({"taskId": "1", "status": "completed"})),
            ferramenta("a4", "t4", "Edit", json!({"file_path": "/proj/a.ts", "old_string": "x\ny", "new_string": "x\nz"})),
            ferramenta("a5", "t5", "TaskList", json!({})),
            json!({"type": "permission-mode", "permissionMode": "plan", "sessionId": "s"}),
            ferramenta("a6", "t6", "ExitPlanMode", json!({"plan": "# Próximo passo\n\n- testar", "planFilePath": "/p.md"})),
        ]);
        let corpo = serde_json::to_value(crate::conversations::timeline(&c, &conta(dir.path(), &c.segments[0].provider), None, 50)).unwrap();
        assert_eq!(corpo["modo_de_permissao"], "plan");
        assert_eq!(corpo["tarefas"], json!([{"texto": "Ler", "estado": "feita", "fazendo": "Lendo"}, {"texto": "Editar", "estado": "pendente", "fazendo": "Editando"}]));
        let msgs = corpo["messages"].as_array().unwrap();
        let textos: Vec<_> = msgs.iter().map(|m| (m["tool"].as_str().unwrap_or("-"), m["text"].as_str().unwrap())).collect();
        assert_eq!(textos, vec![("-", "faça"), ("TaskCreate", "Nova: Ler"), ("TaskCreate", "Nova: Editar"),
            ("TaskUpdate", "Feita: Ler"), ("Edit", "/proj/a.ts"), ("ExitPlanMode", "Próximo passo")]);
        assert_eq!(msgs[3]["tarefas"][0]["estado"], "feita");
        assert_eq!(msgs[4]["diffs"], json!([{"arquivo": "/proj/a.ts", "tipo": "editado", "adicionadas": 1, "removidas": 1, "linhas": [" x", "-y", "+z"]}]));
        assert_eq!(msgs[5]["plano"], "# Próximo passo\n\n- testar");
        // Item comum continua sem os campos novos.
        assert!(msgs[0].get("diffs").is_none() && msgs[0].get("tarefas").is_none() && msgs[0].get("plano").is_none());
    }

    #[test]
    fn timeline_do_codex_mostra_o_patch_uma_vez_so() {
        let dir = tempfile::tempdir().unwrap();
        let patch = "*** Begin Patch\n*** Add File: novo.md\n+oi\n*** End Patch";
        // `cwd` só vale se for absoluto, e `/proj` não é no Windows.
        let proj = if cfg!(windows) { r"C:\proj" } else { "/proj" };
        let arquivo = |nome: &str| Path::new(proj).join(nome).to_string_lossy().into_owned();
        let c = conversa(dir.path(), "codex", &[
            json!({"type": "turn_context", "payload": {"cwd": proj, "approval_policy": "never", "sandbox_policy": {"type": "danger-full-access"}}}),
            json!({"type": "response_item", "payload": {"type": "custom_tool_call", "name": "exec",
                "input": format!("await tools.apply_patch({});", serde_json::to_string(patch).unwrap())}}),
            json!({"type": "event_msg", "payload": {"type": "item_completed", "item": {"type": "FileChange", "status": "completed",
                "changes": {arquivo("novo.md"): {"type": "add", "unified_diff": "", "move_path": null}}}}}),
            // Outro patch, por variável: só o `FileChange` mostra.
            json!({"type": "response_item", "payload": {"type": "custom_tool_call", "name": "exec", "input": "await tools.apply_patch(p);"}}),
            json!({"type": "event_msg", "payload": {"type": "item_completed", "item": {"type": "FileChange", "status": "completed",
                "changes": {arquivo("b.ts"): {"type": "update", "unified_diff": "@@ -1 +1 @@\n-a\n+b\n", "move_path": null}}}}}),
        ]);
        let corpo = serde_json::to_value(crate::conversations::timeline(&c, &conta(dir.path(), &c.segments[0].provider), None, 50)).unwrap();
        assert_eq!(corpo["modo_de_permissao"], "acessoTotal");
        assert!(corpo.get("tarefas").is_none());
        let msgs = corpo["messages"].as_array().unwrap();
        assert_eq!(msgs.iter().map(|m| m["text"].as_str().unwrap()).collect::<Vec<_>>(), vec!["novo.md", "b.ts"]);
        assert_eq!(msgs[0]["diffs"][0]["tipo"], "novo");
        assert_eq!(msgs[1]["diffs"][0]["linhas"], json!(["-a", "+b"]));
    }

    #[test]
    fn plano_do_plan_mode() {
        let extras = extras_claude("ExitPlanMode", &json!({"plan": "# Plano: login\n\n1. Fazer x", "planFilePath": "/x.md"}), None);
        assert_eq!(extras.plano.as_deref(), Some("# Plano: login\n\n1. Fazer x"));
        assert_eq!(titulo_do_plano("\n## Plano: login\nresto"), "Plano: login");
        let enorme = extras_claude("ExitPlanMode", &json!({"plan": "é".repeat(PLANO_MAX)}), None);
        assert!(enorme.plano.unwrap().ends_with('…'));
    }
}
