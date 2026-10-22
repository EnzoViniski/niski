//! Fork (Copiloto): ferramentas MCP de tarefas para orquestradores, o copiloto e o Jarvis.
//!
//! Gravam direto nos arquivos de tarefas (`omni_core::tarefas`), na mesma pasta que o app usa:
//! a padrão da pasta de dados ou a compartilhada escolhida em Configurações → Tarefas (lida do
//! espelho da interface). A tela relê a pasta sozinha e mostra a mudança em segundos.

use super::*;
use omni_core::tarefas as armazenamento;

const QUANDO: &[&str] = &["abertas", "hoje", "atrasadas", "semana", "todas"];

pub(super) fn lista() -> Vec<Value> {
    let campos = json!({
        "titulo": {"type": "string"},
        "descricao": {"type": "string"},
        "data": {"type": "string", "description": "AAAA-MM-DD (vazio = sem data)"},
        "hora": {"type": "string", "description": "HH:MM (vazio = dia todo)"},
        "duracaoMin": {"type": "integer", "minimum": 0},
        "status": {"type": "string", "enum": armazenamento::STATUS},
        "prioridade": {"type": "string", "enum": armazenamento::PRIORIDADES},
        "categoria": {"type": "string", "enum": armazenamento::CATEGORIAS, "description": "saude = saúde e médicos"},
        "recorrencia": {"type": "string", "enum": armazenamento::RECORRENCIAS},
        "responsavel": {"type": "string", "description": "Id da pessoa (veja `pessoas` em listar_tarefas); vazio = ninguém"},
        "local": {"type": "string"},
        "etiquetas": {"type": "array", "items": {"type": "string"}},
        "checklist": {"type": "array", "items": {"type": "string"}, "description": "Passos da tarefa"},
    });
    let id = json!({"type": "string", "description": "Id da tarefa (de listar_tarefas)"});
    vec![
        json!({"name": "listar_tarefas",
            "description": "Lista as tarefas do usuário no app (as mesmas da tela Tarefas), com a data de hoje e as pessoas que dividem a lista. Use antes de editar ou concluir, para pegar o id.",
            "inputSchema": {"type": "object", "properties": {
                "quando": {"type": "string", "enum": QUANDO, "description": "abertas (padrão), hoje (inclui atrasadas), atrasadas, semana (próximos 7 dias) ou todas"},
                "busca": {"type": "string", "description": "Texto no título ou na descrição"}}}}),
        json!({"name": "criar_tarefa",
            "description": "Cria uma tarefa no app. Aparece na tela Tarefas em poucos segundos. Datas relativas (amanhã, sexta) você converte para AAAA-MM-DD.",
            "inputSchema": {"type": "object", "required": ["titulo"], "properties": campos}}),
        json!({"name": "editar_tarefa",
            "description": "Muda campos de uma tarefa (só os enviados). Para reagendar, mude `data`/`hora`; para mover no quadro, `status`.",
            "inputSchema": {"type": "object", "required": ["id", "campos"], "properties": {"id": id, "campos": {"type": "object", "properties": campos}}}}),
        json!({"name": "concluir_tarefa",
            "description": "Marca a tarefa como feita (recorrente volta na próxima data). Com `feita: false`, reabre.",
            "inputSchema": {"type": "object", "required": ["id"], "properties": {"id": id, "feita": {"type": "boolean"}}}}),
        json!({"name": "apagar_tarefa",
            "description": "Apaga uma tarefa. Só quando o usuário pedir claramente.",
            "inputSchema": {"type": "object", "required": ["id"], "properties": {"id": id}}}),
    ]
}

fn agora_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn aleatorio() -> u32 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    nanos ^ std::process::id().rotate_left(16)
}

fn resumo(t: &Value) -> Value {
    let checklist = t.get("checklist").and_then(Value::as_array).map(|i| {
        let feitos = i.iter().filter(|x| x.get("feito") == Some(&Value::Bool(true))).count();
        format!("{feitos}/{}", i.len())
    });
    let descricao: String = t.get("descricao").and_then(Value::as_str).unwrap_or("").chars().take(200).collect();
    json!({
        "id": t["id"], "titulo": t["titulo"], "status": t["status"], "data": t["data"], "hora": t["hora"],
        "categoria": t["categoria"], "prioridade": t["prioridade"], "responsavel": t["responsavel"],
        "recorrencia": t["recorrencia"], "descricao": descricao, "checklist": checklist,
    })
}

fn listar(contexto: &armazenamento::Contexto, args: &Value, hoje: chrono::NaiveDate) -> Value {
    let quando = args.get("quando").and_then(Value::as_str).filter(|q| QUANDO.contains(q)).unwrap_or("abertas");
    let busca = args.get("busca").and_then(Value::as_str).unwrap_or("").to_lowercase();
    let dia = hoje.format("%Y-%m-%d").to_string();
    let semana = (hoje + chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let mut tarefas: Vec<Value> = armazenamento::todas(&contexto.raiz).into_iter().filter(|t| {
        let data = t.get("data").and_then(Value::as_str).unwrap_or("");
        let aberta = t.get("status").and_then(Value::as_str) != Some("feito");
        let combina = busca.is_empty() || format!("{} {}", t["titulo"].as_str().unwrap_or(""), t["descricao"].as_str().unwrap_or("")).to_lowercase().contains(&busca);
        combina && match quando {
            "hoje" => aberta && !data.is_empty() && data <= dia.as_str(),
            "atrasadas" => aberta && !data.is_empty() && data < dia.as_str(),
            "semana" => aberta && !data.is_empty() && data <= semana.as_str(),
            "todas" => true,
            _ => aberta,
        }
    }).collect();
    tarefas.sort_by(|a, b| {
        let chave = |t: &Value| (t["data"].as_str().filter(|d| !d.is_empty()).unwrap_or("9999").to_owned(), t["hora"].as_str().unwrap_or("").to_owned());
        chave(a).cmp(&chave(b))
    });
    let pessoas: Vec<Value> = armazenamento::listar(&contexto.raiz, "pessoas").unwrap_or_default().iter()
        .filter_map(|p| serde_json::from_str::<Value>(p).ok())
        .map(|p| json!({"id": p["id"], "nome": p["nome"]})).collect();
    json!({"hoje": dia, "eu": contexto.eu, "pessoas": pessoas, "total": tarefas.len(), "tarefas": tarefas.iter().take(150).map(resumo).collect::<Vec<_>>()})
}

fn obrigatorio<'a>(args: &'a Value, campo: &str) -> Result<&'a str, String> {
    args.get(campo).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).ok_or_else(|| format!("Falta o campo `{campo}`"))
}

fn executar(pasta_de_dados: &Path, nome: &str, args: &Value) -> Result<Value, String> {
    let contexto = armazenamento::contexto(pasta_de_dados).map_err(|e| e.to_string())?;
    let hoje = chrono::Local::now().date_naive();
    let erro = |e: std::io::Error| e.to_string();
    let (acao, tarefa) = match nome {
        "listar_tarefas" => return Ok(listar(&contexto, args, hoje)),
        "criar_tarefa" => ("criada", armazenamento::criar(&contexto.raiz, args, &contexto.eu, agora_ms(), aleatorio()).map_err(erro)?),
        "editar_tarefa" => ("alterada", armazenamento::editar(&contexto.raiz, obrigatorio(args, "id")?, args.get("campos").unwrap_or(&Value::Null), agora_ms()).map_err(erro)?),
        "concluir_tarefa" => {
            let feita = args.get("feita") != Some(&Value::Bool(false));
            (if feita { "concluída" } else { "reaberta" }, armazenamento::concluir(&contexto.raiz, obrigatorio(args, "id")?, feita, agora_ms(), hoje).map_err(erro)?)
        }
        "apagar_tarefa" => ("apagada", armazenamento::apagar(&contexto.raiz, obrigatorio(args, "id")?, agora_ms()).map_err(erro)?),
        _ => return Err(format!("Ferramenta desconhecida: {nome}")),
    };
    registro::info("copiloto_tarefa", &[("acao", acao)]);
    Ok(json!({"ok": true, "acao": acao, "tarefa": resumo(&tarefa)}))
}

/// `Some` quando a ferramenta é de tarefas.
pub(super) fn e_de_tarefas(nome: &str) -> bool {
    matches!(nome, "listar_tarefas" | "criar_tarefa" | "editar_tarefa" | "concluir_tarefa" | "apagar_tarefa")
}

pub(super) fn chamar(pasta_de_dados: &Path, nome: &str, args: &Value) -> Result<String, String> {
    executar(pasta_de_dados, nome, args).map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cria_lista_conclui_pela_ferramenta() {
        let dir = tempfile::tempdir().unwrap();
        let criada: Value = serde_json::from_str(&chamar(dir.path(), "criar_tarefa", &json!({"titulo": "Cardiologista", "categoria": "saude", "data": chrono::Local::now().date_naive().format("%Y-%m-%d").to_string()})).unwrap()).unwrap();
        assert_eq!(criada["acao"], "criada");
        let id = criada["tarefa"]["id"].as_str().unwrap().to_owned();
        let hoje: Value = serde_json::from_str(&chamar(dir.path(), "listar_tarefas", &json!({"quando": "hoje"})).unwrap()).unwrap();
        assert_eq!(hoje["total"], 1);
        assert!(chamar(dir.path(), "editar_tarefa", &json!({"id": id, "campos": {"categoria": "inexistente"}})).is_err());
        chamar(dir.path(), "concluir_tarefa", &json!({"id": id})).unwrap();
        let abertas: Value = serde_json::from_str(&chamar(dir.path(), "listar_tarefas", &json!({})).unwrap()).unwrap();
        assert_eq!(abertas["total"], 0);
        assert!(e_de_tarefas("criar_tarefa") && !e_de_tarefas("criar_agente"));
        // O arquivo fica onde o app lê: <dados>/tarefas/tarefas/<id>.json.
        assert!(dir.path().join("tarefas/tarefas").join(format!("{id}.json")).exists());
    }
}
