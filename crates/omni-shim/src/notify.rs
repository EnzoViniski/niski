//! Fork (harness, A2): modo notify do `omni-shim` para o Codex (`omni_core::notify_do_codex`).
//!
//! O Codex roda `<omni-shim> --medza-notify <porta> '<json>'` no fim de cada turno. Aqui:
//! 1. com o token da PTY no ambiente, `POST /ganchos/codex` com **só** o tipo e os ids (no máximo 300 ms);
//! 2. chama o `notify` que a conta já tinha (`MEDZA_NOTIFY_DO_USUARIO`) com o mesmo JSON, como o Codex faria.
//!
//! Sem token ou sem engine, só o passo 2. Nada é gravado nem logado aqui.

use omni_core::notify_do_codex as regra;
use std::process::{Command, Stdio};

/// `argumentos`: o que vem depois de `--medza-notify` (a porta e o JSON do Codex, que vem por último).
pub fn rodar(argumentos: &[String], token: Option<&str>) -> i32 {
    let json = argumentos.last().map(String::as_str).unwrap_or("");
    if let (Some(token), Some(porta), Some(corpo)) = (token, argumentos.first().and_then(|p| p.parse::<u16>().ok()), regra::corpo_enxuto(json)) {
        let _ = crate::statusline::enviar_para(porta, token, regra::ROTA, corpo.to_string().as_bytes(), crate::statusline::TEMPO_MAXIMO);
    }
    encadear(std::env::var(regra::VARIAVEL_DO_USUARIO).ok().as_deref(), json)
}

/// Roda o `notify` do usuário (array JSON: programa e argumentos) com o JSON no fim. Sem ele: 0.
pub fn encadear(do_usuario: Option<&str>, json: &str) -> i32 {
    let Some(lista) = do_usuario.and_then(|t| serde_json::from_str::<Vec<String>>(t).ok()).filter(|l| !l.is_empty()) else { return 0 };
    Command::new(&lista[0]).args(&lista[1..]).arg(json).stdin(Stdio::null())
        .status().map(|s| s.code().unwrap_or(1)).unwrap_or(1)
}

#[cfg(all(test, unix))]
mod testes {
    use super::*;
    use std::{io::{Read, Write}, net::TcpListener, time::Duration};

    #[test]
    fn manda_so_os_ids_e_encadeia_o_notify_do_usuario() {
        let servidor = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = servidor.local_addr().unwrap().port();
        let recebido = std::thread::spawn(move || {
            let (mut conexao, _) = servidor.accept().unwrap();
            conexao.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut tudo = Vec::new();
            let mut pedaco = [0_u8; 1024];
            while let Ok(n) = conexao.read(&mut pedaco) { if n == 0 { break; } tudo.extend_from_slice(&pedaco[..n]); if tudo.ends_with(b"}") { break; } }
            conexao.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            String::from_utf8(tudo).unwrap()
        });
        let json = r#"{"type":"agent-turn-complete","thread-id":"th-1","turn-id":"t-2","last-assistant-message":"conteudo"}"#;
        let dir = tempfile::tempdir().unwrap();
        let marca = dir.path().join("usuario.txt");
        let script = dir.path().join("n.sh");
        std::fs::write(&script, format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", marca.display())).unwrap();
        let usuario = serde_json::json!(["/bin/sh", script.to_string_lossy()]).to_string();
        let pedido_ok = rodar(&[porta.to_string(), json.into()], Some("tok-9"));
        assert_eq!(pedido_ok, 0);
        let pedido = recebido.join().unwrap();
        assert!(pedido.starts_with("POST /ganchos/codex HTTP/1.1\r\n") && pedido.contains("Authorization: Bearer tok-9\r\n"), "{pedido}");
        assert!(pedido.contains("\"thread-id\":\"th-1\"") && !pedido.contains("conteudo"), "{pedido}");
        assert_eq!(encadear(Some(&usuario), json), 0);
        assert_eq!(std::fs::read_to_string(&marca).unwrap(), json, "o notify do usuário recebe o JSON inteiro");
        assert_eq!(encadear(None, json), 0);
        assert_eq!(encadear(Some("não é json"), json), 0);
    }

    #[test]
    fn sem_token_nem_engine_nao_trava() {
        let livre = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let inicio = std::time::Instant::now();
        assert_eq!(rodar(&[livre.to_string(), r#"{"type":"agent-turn-complete"}"#.into()], Some("t")), 0);
        assert_eq!(rodar(&["x".into()], None), 0);
        assert!(inicio.elapsed() < Duration::from_secs(2));
    }
}
