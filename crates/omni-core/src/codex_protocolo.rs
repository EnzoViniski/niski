//! Fork: JSON-RPC sem cabeçalho `jsonrpc`, conforme app-server 0.163.0.
//! Fontes e limites de compatibilidade em docs/fork/codex-app-server.md.
use serde::Deserialize;
use serde_json::{json, Value};

pub const VERSAO_CONFERIDA: &str = "0.163.0";
pub const MENSAGEM_MAX: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum Id {
    Numero(i64),
    Texto(String),
}

#[derive(Debug, Deserialize)]
pub struct Mensagem {
    pub id: Option<Id>,
    pub method: Option<String>,
    #[serde(default)]
    pub params: Value,
    #[serde(default, deserialize_with = "resultado")]
    pub result: Option<Value>,
    pub error: Option<Value>,
}
fn resultado<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

pub fn ler(linha: &[u8]) -> Result<Mensagem, &'static str> {
    if linha.len() > MENSAGEM_MAX {
        return Err("Mensagem grande demais");
    }
    let m: Mensagem = serde_json::from_slice(linha).map_err(|_| "JSON-RPC inválido")?;
    if m.method.is_none() && (m.id.is_none() || (m.result.is_some() == m.error.is_some())) {
        return Err("Envelope JSON-RPC inválido");
    }
    Ok(m)
}

pub fn pedido(id: u64, metodo: &str, params: Value) -> Value {
    json!({"id":id,"method":metodo,"params":params})
}
pub fn inicializar(id: u64, versao: &str) -> Value {
    pedido(
        id,
        "initialize",
        json!({"clientInfo":{"name":"medza_agents","title":"Niski","version":versao}}),
    )
}
pub fn entrada(texto: &str) -> Value {
    json!([{"type":"text","text":texto,"text_elements":[]}])
}
pub fn decisao(id: &Id, permitir: bool) -> Value {
    json!({"id":id,"result":{"decision":if permitir {"accept"} else {"decline"}}})
}

#[derive(Debug, Deserialize)]
pub struct Thread {
    pub id: String,
    pub path: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct ThreadResposta {
    pub thread: Thread,
}

/// Aprovações de comando/arquivo somente; outros pedidos nunca recebem `accept` por engano.
pub fn aprovacao(metodo: &str) -> Option<&'static str> {
    match metodo {
        "item/commandExecution/requestApproval" => Some("Bash"),
        "item/fileChange/requestApproval" => Some("Edit"),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn envelope_tolerante_preserva_ids_e_rejeita_lixo() {
        let m = ler(br#"{"id":"pedido-1","method":"novo/evento","params":{},"futuro":1}"#).unwrap();
        assert_eq!(m.id, Some(Id::Texto("pedido-1".into())));
        assert!(ler(br#"{"result":{}}"#).is_err());
        assert!(ler(b"{}").is_err());
        assert_eq!(
            ler(br#"{"id":1,"result":null}"#).unwrap().result,
            Some(Value::Null)
        );
        assert!(ler(&vec![b' '; MENSAGEM_MAX + 1]).is_err());
    }
    #[test]
    fn decisoes_sao_so_deste_pedido() {
        assert_eq!(
            decisao(&Id::Numero(9), true),
            json!({"id":9,"result":{"decision":"accept"}})
        );
        assert_eq!(
            decisao(&Id::Texto("x".into()), false)["result"]["decision"],
            "decline"
        );
        assert!(aprovacao("item/permissions/requestApproval").is_none());
        assert!(inicializar(1, "test").get("jsonrpc").is_none());
    }
}
