//! Fork (harness, A2): fim de turno do Codex por evento, pelo `notify` oficial.
//!
//! Doc (learn.chatgpt.com/docs/config-file/config-advanced.md, conferida em 23/10/2026): `notify` é um
//! array com o programa e os argumentos; o Codex o chama com **um argumento a mais**, um JSON com
//! `type` (hoje só `agent-turn-complete`), `thread-id`, `turn-id`, `cwd`, `input-messages` e
//! `last-assistant-message`. Não é hook: não pede a confiança por hash do `/hooks`.
//!
//! O engine põe, só na linha digitada da sessão, `-c "$MEDZA_CODEX_NOTIFY"` com o valor
//! `notify=["<omni-shim>","--medza-notify","<porta>"]` no ambiente da PTY. O shim manda ao engine **só**
//! o tipo e os ids ([`corpo_enxuto`]; nada da conversa) e depois chama o `notify` que a conta já tinha
//! (`MEDZA_NOTIFY_DO_USUARIO`, array JSON), com o mesmo JSON, como o Codex chamaria.
//!
//! Aqui: montar o valor, ler o `notify` do usuário no `config.toml` (só essa chave) e traduzir o JSON.

use crate::ganchos::Evento;
use serde_json::{json, Value};
use std::path::Path;

/// Primeiro argumento que põe o `omni-shim` no modo notify.
pub const ARGUMENTO: &str = "--medza-notify";
/// Variável da PTY com o `notify=[…]` (valor do `-c`).
pub const VARIAVEL_DO_VALOR: &str = "MEDZA_CODEX_NOTIFY";
/// Variável da PTY com o `notify` que a conta já tinha (array JSON), para o shim encadear.
pub const VARIAVEL_DO_USUARIO: &str = "MEDZA_NOTIFY_DO_USUARIO";
/// Rota do servidor dos ganchos que recebe o evento.
pub const ROTA: &str = "/ganchos/codex";
pub const TURNO_COMPLETO: &str = "agent-turn-complete";

/// `notify=["<shim>","--medza-notify","<porta>"]`: texto TOML (as strings JSON valem como TOML).
pub fn valor(shim: &str, porta: u16) -> String {
    format!("notify={}", json!([shim, ARGUMENTO, porta.to_string()]))
}

/// O `notify` do `config.toml` da conta, se for um array de textos não vazio. Arquivo ausente, ilegível
/// ou sem a chave: `None`. Não lê mais nada do arquivo.
pub fn do_usuario(codex_home: &Path) -> Option<Vec<String>> {
    let texto = std::fs::read_to_string(codex_home.join("config.toml")).ok()?;
    let tabela: toml::Table = texto.parse().ok()?;
    let lista: Vec<String> = tabela.get("notify")?.as_array()?.iter().map(|v| v.as_str().map(str::to_owned)).collect::<Option<_>>()?;
    (!lista.is_empty() && !lista.iter().any(|a| a == ARGUMENTO)).then_some(lista)
}

/// O que vai ao engine: só tipo e ids. `None` se não for JSON de objeto com `type`.
pub fn corpo_enxuto(json_do_codex: &str) -> Option<Value> {
    let valor: Value = serde_json::from_str(json_do_codex).ok()?;
    let texto = |chave: &str| valor.get(chave).and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= 200);
    let tipo = texto("type")?;
    Some(json!({"type": tipo, "thread-id": texto("thread-id"), "turn-id": texto("turn-id")}))
}

/// O corpo enxuto como evento de gancho: `agent-turn-complete` vira `Stop` (fim de turno), com o
/// `thread-id` no lugar do `session_id`. Outro tipo: `None` (ignorado).
pub fn evento(corpo: &Value) -> Option<Evento> {
    if corpo.get("type").and_then(Value::as_str) != Some(TURNO_COMPLETO) { return None; }
    Some(Evento {
        nome: "Stop".into(),
        session_id: corpo.get("thread-id").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned),
        ..Evento::default()
    })
}

/// Linha já preparada (ou comando que traz o próprio `notify`): não mexer.
pub fn ja_tem(comando: &str) -> bool { comando.contains(VARIAVEL_DO_VALOR) || comando.contains("notify=") }

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn valor_e_toml_valido_com_o_shim_e_a_porta() {
        let v = valor("/Applications/Niski.app/Contents/MacOS/omni-shim", 47425);
        let tabela: toml::Table = v.parse().unwrap();
        let lista: Vec<&str> = tabela["notify"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        assert_eq!(lista, ["/Applications/Niski.app/Contents/MacOS/omni-shim", "--medza-notify", "47425"]);
        let com_aspas = valor("/tmp/a \"b\"/omni-shim", 1);
        assert_eq!(com_aspas.parse::<toml::Table>().unwrap()["notify"][0].as_str(), Some("/tmp/a \"b\"/omni-shim"));
    }

    #[test]
    fn notify_do_usuario_so_quando_e_array_de_texto() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(do_usuario(dir.path()), None);
        std::fs::write(dir.path().join("config.toml"), "model = \"x\"\nnotify = [\"python3\", \"/a/n.py\"]\n").unwrap();
        assert_eq!(do_usuario(dir.path()), Some(vec!["python3".into(), "/a/n.py".into()]));
        std::fs::write(dir.path().join("config.toml"), "notify = []\n").unwrap();
        assert_eq!(do_usuario(dir.path()), None);
        std::fs::write(dir.path().join("config.toml"), "notify = \"x\"\n").unwrap();
        assert_eq!(do_usuario(dir.path()), None);
        std::fs::write(dir.path().join("config.toml"), "notify = [\"/s/omni-shim\", \"--medza-notify\", \"1\"]\n").unwrap();
        assert_eq!(do_usuario(dir.path()), None, "o nosso não se encadeia nele mesmo");
        std::fs::write(dir.path().join("config.toml"), "não é toml [").unwrap();
        assert_eq!(do_usuario(dir.path()), None);
    }

    #[test]
    fn corpo_enxuto_tira_o_conteudo_da_conversa() {
        let doc = r#"{"type":"agent-turn-complete","thread-id":"th-1","turn-id":"t-9","cwd":"/p",
            "input-messages":["segredo do prompt"],"last-assistant-message":"resposta"}"#;
        let corpo = corpo_enxuto(doc).unwrap();
        assert_eq!(corpo, json!({"type": "agent-turn-complete", "thread-id": "th-1", "turn-id": "t-9"}));
        assert!(!corpo.to_string().contains("segredo") && !corpo.to_string().contains("resposta"));
        assert!(corpo_enxuto("[1]").is_none() && corpo_enxuto("{}").is_none() && corpo_enxuto("x").is_none());
        let e = evento(&corpo).unwrap();
        assert_eq!((e.nome.as_str(), e.session_id.as_deref()), ("Stop", Some("th-1")));
        assert!(evento(&json!({"type": "approval-requested"})).is_none());
        assert!(ja_tem("codex -c \"$MEDZA_CODEX_NOTIFY\"") && ja_tem("codex -c notify=[]") && !ja_tem("codex resume x"));
    }
}
