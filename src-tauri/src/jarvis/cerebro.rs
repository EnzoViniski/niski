//! Fork: Jarvis. O "cérebro" é uma sessão do Claude Code oficial que fica aberta enquanto o Jarvis
//! está aberto (`claude -p --input-format stream-json`): cada fala do Enzo entra como uma linha, e a
//! resposta volta aos pedaços (`--include-partial-messages`), o que deixa a voz começar na primeira
//! frase. Sem ferramentas (fase 1), sem MCP, sem as configurações do usuário (hooks deixariam lento) e
//! sem gravar sessão: a conversa do Jarvis não aparece no Histórico nem em "Anteriores".
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

pub const SISTEMA: &str = "Você é o Jarvis do Medza Agents, o copiloto por voz do Enzo. Tudo o que você \
escreve é lido em voz alta por um sintetizador de voz em português do Brasil. Responda curto: uma a três \
frases, como numa conversa falada. Nada de markdown, listas, títulos, emojis, links ou blocos de código. \
Escreva números e siglas do jeito que se falam quando ajudar a leitura. O reconhecimento de fala erra \
nomes: \"Medes\" ou \"Med\" é Medza; \"Cláudio\" ou \"Cloud\" é Claude; \"Códex\" é Codex. Nesta versão você \
ainda não abre conversas nem mexe no computador; se o Enzo pedir isso, diga que essa parte chega na próxima \
versão do Jarvis e ofereça ajuda com o que der para responder conversando.";

/// O que sai da sessão do Claude, já interpretado.
#[derive(Debug, PartialEq)]
pub enum Saida {
    Pronto,
    Pedaco(String),
    /// Mensagem inteira do assistente (vem também com os pedaços; serve de reserva sem eles).
    Mensagem(String),
    FimDoTurno { erro: Option<String> },
}

/// Uma linha do `--output-format stream-json`.
pub fn interpretar(linha: &str) -> Option<Saida> {
    let valor: Value = serde_json::from_str(linha).ok()?;
    match valor["type"].as_str()? {
        "system" if valor["subtype"] == "init" => Some(Saida::Pronto),
        "stream_event" => {
            let evento = &valor["event"];
            (evento["type"] == "content_block_delta" && evento["delta"]["type"] == "text_delta")
                .then(|| evento["delta"]["text"].as_str().map(|t| Saida::Pedaco(t.to_owned())))?
        }
        "assistant" => {
            let texto: String = valor["message"]["content"].as_array()?.iter()
                .filter(|parte| parte["type"] == "text").filter_map(|parte| parte["text"].as_str()).collect();
            (!texto.is_empty()).then_some(Saida::Mensagem(texto))
        }
        "result" => {
            let erro = valor["is_error"].as_bool().unwrap_or(false)
                .then(|| valor["result"].as_str().unwrap_or("o Claude não respondeu").to_owned());
            Some(Saida::FimDoTurno { erro })
        }
        _ => None,
    }
}

/// A linha que leva uma fala do Enzo para a sessão.
pub fn linha_do_usuario(texto: &str) -> String {
    json!({ "type": "user", "message": { "role": "user", "content": [{ "type": "text", "text": texto }] } }).to_string()
}

pub fn argumentos(modelo: &str) -> Vec<String> {
    [
        "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose",
        "--include-partial-messages", "--model", modelo, "--tools", "", "--strict-mcp-config",
        "--setting-sources", "project", "--no-session-persistence", "--system-prompt", SISTEMA,
    ].into_iter().map(str::to_owned).collect()
}

pub struct Cerebro {
    filho: Child,
    entrada: ChildStdin,
}

impl Cerebro {
    /// Abre a sessão. `ao_sair` é chamada a cada linha interpretada, numa thread própria; `None` no fim.
    pub fn abrir(executavel: &Path, ambiente: Vec<(String, String)>, pasta: PathBuf, modelo: &str,
        mut ao_sair: impl FnMut(Option<Saida>) + Send + 'static) -> Result<Self, String> {
        std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
        let mut comando = Command::new(executavel);
        comando.args(argumentos(modelo)).envs(ambiente).current_dir(&pasta)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut filho = comando.spawn().map_err(|e| format!("Não consegui abrir o Claude: {e}"))?;
        let entrada = filho.stdin.take().ok_or("sem stdin")?;
        let saida = filho.stdout.take().ok_or("sem stdout")?;
        std::thread::spawn(move || {
            for linha in BufReader::new(saida).lines().map_while(Result::ok) {
                if let Some(saida) = interpretar(&linha) { ao_sair(Some(saida)); }
            }
            ao_sair(None);
        });
        Ok(Self { filho, entrada })
    }

    pub fn perguntar(&mut self, texto: &str) -> Result<(), String> {
        writeln!(self.entrada, "{}", linha_do_usuario(texto)).and_then(|_| self.entrada.flush())
            .map_err(|_| "A sessão do Claude fechou".to_owned())
    }
}

impl Drop for Cerebro {
    fn drop(&mut self) {
        let _ = self.filho.kill();
        let _ = self.filho.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_as_linhas_do_stream_json() {
        assert_eq!(interpretar(r#"{"type":"system","subtype":"init","model":"x"}"#), Some(Saida::Pronto));
        assert_eq!(interpretar(r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Oi"}}}"#),
            Some(Saida::Pedaco("Oi".into())));
        assert_eq!(interpretar(r#"{"type":"stream_event","event":{"type":"message_start"}}"#), None);
        assert_eq!(interpretar(r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Oi Enzo."}]}}"#),
            Some(Saida::Mensagem("Oi Enzo.".into())));
        assert_eq!(interpretar(r#"{"type":"result","subtype":"success","is_error":false,"result":"Oi"}"#),
            Some(Saida::FimDoTurno { erro: None }));
        assert_eq!(interpretar(r#"{"type":"result","is_error":true,"result":"limite"}"#),
            Some(Saida::FimDoTurno { erro: Some("limite".into()) }));
        assert_eq!(interpretar("lixo"), None);
    }

    #[test]
    fn fala_vira_mensagem_de_usuario() {
        let v: Value = serde_json::from_str(&linha_do_usuario("abre \"isso\"")).unwrap();
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["content"][0]["text"], "abre \"isso\"");
    }

    #[test]
    fn sessao_sem_ferramentas_e_sem_historico() {
        let a = argumentos("haiku");
        let depois = |flag: &str| a.iter().position(|x| x == flag).map(|i| a[i + 1].as_str());
        assert_eq!(depois("--tools"), Some(""));
        assert_eq!(depois("--model"), Some("haiku"));
        assert!(a.contains(&"--no-session-persistence".to_owned()));
        assert!(a.contains(&"--strict-mcp-config".to_owned()));
    }

    /// CLI falsa: responde cada linha recebida com pedaços e fim de turno.
    #[cfg(unix)]
    #[test]
    fn conversa_com_cli_falsa() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::mpsc;
        let pasta = tempfile::tempdir().unwrap();
        let falsa = pasta.path().join("claude");
        std::fs::write(&falsa, "#!/bin/sh\necho '{\"type\":\"system\",\"subtype\":\"init\"}'\nwhile read linha; do\n\
            echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Oi \"}}}'\n\
            echo '{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Enzo.\"}}}'\n\
            echo '{\"type\":\"result\",\"is_error\":false,\"result\":\"Oi Enzo.\"}'\ndone\n").unwrap();
        std::fs::set_permissions(&falsa, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (tx, rx) = mpsc::channel();
        let mut cerebro = Cerebro::abrir(&falsa, vec![], pasta.path().join("jarvis"), "haiku", move |s| { let _ = tx.send(s); }).unwrap();
        let espera = std::time::Duration::from_secs(5);
        assert_eq!(rx.recv_timeout(espera).unwrap(), Some(Saida::Pronto));
        cerebro.perguntar("oi").unwrap();
        assert_eq!(rx.recv_timeout(espera).unwrap(), Some(Saida::Pedaco("Oi ".into())));
        assert_eq!(rx.recv_timeout(espera).unwrap(), Some(Saida::Pedaco("Enzo.".into())));
        assert_eq!(rx.recv_timeout(espera).unwrap(), Some(Saida::FimDoTurno { erro: None }));
        drop(cerebro);
        assert_eq!(rx.recv_timeout(espera).unwrap(), None);
    }
}
