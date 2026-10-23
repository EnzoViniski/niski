//! Fork (harness, A4): testes do vigia do limite com **Claude falso** (`sh`) e duas contas de mentira.
//! Nenhuma CLI real: o comando do agente vem do `workspace.json` da pasta de teste.

use super::*;
use std::sync::atomic::AtomicU64;

struct Cena { dir: tempfile::TempDir, state: Arc<EngineState>, projeto: PathBuf, conta_a: PathBuf, conta_b: PathBuf, cli: PathBuf }

/// `com_b`: a segunda conta existe (sem ela, não há para onde trocar).
fn cena(com_b: bool) -> Cena {
    let dir = tempfile::tempdir().unwrap();
    let (projeto, conta_a, conta_b) = (dir.path().join("projeto"), dir.path().join("conta-a"), dir.path().join("conta-b"));
    for pasta in [&projeto, &conta_a, &conta_b] { fs::create_dir_all(pasta).unwrap(); }
    let cli = dir.path().join("claude-falso");
    // Uma linha por abertura: conta e argumentos. Na conta A, o primeiro turno termina no limite.
    fs::write(&cli, format!(r#"#!/bin/sh
printf '%s|%s\n' "$CLAUDE_CONFIG_DIR" "$*" >> '{}'
printf '\033[?2004h'
while :; do
  printf '\r\n\342\235\257 '
  IFS= read -r linha || exit 0
  [ -z "$linha" ] && continue
  printf '\r\n esc to interrupt'; sleep 1; printf '\r\033[2K'
  case "$CLAUDE_CONFIG_DIR" in *conta-a) printf "\r\n  Claude usage limit reached. Resets at 3pm\r\n";; *) printf '\r\n ok\r\n';; esac
done
"#, dir.path().join("aberturas.txt").display())).unwrap();
    let comando = format!("/bin/sh {}", cli.display());
    atomic_write_json(&dir.path().join("workspace.json"), &json!({"agents": [{"id": "claude", "label": "Claude", "command": comando, "resume": null}]})).unwrap();
    let mut perfis = vec![json!({"id": "conta-a", "provider": "claude", "name": "A", "config_dir": conta_a, "created_at_ms": 1})];
    if com_b { perfis.push(json!({"id": "conta-b", "provider": "claude", "name": "B", "config_dir": conta_b, "created_at_ms": 2})); }
    atomic_write_json(&dir.path().join("profiles.json"), &json!({"profiles": perfis})).unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
        token: "test-only".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    Cena { dir, state, projeto, conta_a, conta_b, cli }
}

impl Cena {
    /// Abre a conversa na conta A, com transcript gravado (a CLI já respondeu antes), e faz um turno que
    /// termina no limite. Devolve o terminal e a conversa.
    async fn no_limite(&self) -> (String, String) {
        let conversa = format!("conv-{}", uuid::Uuid::new_v4());
        let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
        let cwd = self.projeto.to_string_lossy().into_owned();
        let transcript = omni_core::conversations::claude_transcript_path(&self.conta_a, &cwd, "sid-1");
        fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        fs::write(&transcript, "{\"type\":\"user\",\"sessionId\":\"sid-1\"}\n").unwrap();
        atomic_write_json(&self.dir.path().join("conversations.json"), &json!({"version": 1, "conversations": [{"id": conversa, "project_id": "p",
            "cwd": cwd, "title": "conv", "created_at_ms": 1, "segments": [{"provider": "claude", "profile_id": "conta-a",
            "external_session_id": "sid-1", "transcript_path": transcript, "terminal_session_id": terminal, "started_at_ms": 1, "ended_at_ms": null}]}]})).unwrap();
        // zsh com controle de jobs (como o shell de login do Mac): a CLI fica no grupo dela e o reinício a
        // encerra. `ZDOTDIR` vazio: nenhum `.zshrc` do usuário roda no teste.
        spawn_terminal_inner(&self.state, terminal.clone(), "p".into(), "Claude".into(), cwd, Some("/bin/zsh".into()),
            Some(format!("/bin/sh {} --resume sid-1", self.cli.display())), 30, 100, now_ms(), SessionOrigin {
                provider: Some("claude".into()), profile_id: Some("conta-a".into()), conversation_id: Some(conversa.clone()),
                external_session_id: Some("sid-1".into()), env: vec![("CLAUDE_CONFIG_DIR".into(), self.conta_a.to_string_lossy().into_owned()),
                    ("ZDOTDIR".into(), self.dir.path().to_string_lossy().into_owned())],
                ..SessionOrigin::default() }).unwrap();
        let sessao = self.sessao(&terminal);
        assert!(ate(15, || sessao.interaction.lock().unwrap().parser.screen().contents().contains('❯')).await, "a CLI falsa não subiu");
        write_terminal(&self.state, &terminal, "oi\r").unwrap();
        assert!(ate(15, || { settle(&sessao); sessao.meta.lock().unwrap().notice.as_deref() == Some("usage_limit") }).await,
            "o limite não apareceu: {}", sessao.interaction.lock().unwrap().parser.screen().contents());
        (terminal, conversa)
    }

    fn sessao(&self, terminal: &str) -> Arc<LiveSession> {
        match self.state.sessions.lock().unwrap().get(terminal) { Some(SessionEntry::Live(s)) => s.clone(), _ => panic!("sessão viva") }
    }

    fn aberturas(&self) -> Vec<String> {
        fs::read_to_string(self.dir.path().join("aberturas.txt")).unwrap_or_default().lines().map(str::to_owned).collect()
    }
}

async fn ate(segundos: u64, mut condicao: impl FnMut() -> bool) -> bool {
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(segundos);
    while std::time::Instant::now() < limite {
        if condicao() { return true; }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    condicao()
}

#[test]
fn reserva_tem_um_dono_por_vez_mesmo_com_pedidos_juntos() {
    let conversa = format!("conv-{}", uuid::Uuid::new_v4());
    let barreira = Arc::new(std::sync::Barrier::new(16));
    let ganhos: Vec<bool> = (0..16).map(|i| {
        let (b, c) = (barreira.clone(), conversa.clone());
        std::thread::spawn(move || { b.wait(); reservar(&c, &format!("dono-{i}")) })
    }).collect::<Vec<_>>().into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(ganhos.iter().filter(|g| **g).count(), 1, "só um ganha a corrida");
    let dono = format!("dono-{}", ganhos.iter().position(|g| *g).unwrap());
    assert!(reservar(&conversa, &dono), "o dono pode reafirmar");
    soltar(&conversa, "outro");
    assert!(!reservar(&conversa, "outro"), "soltar de quem não é dono não solta");
    soltar(&conversa, &dono);
    assert!(reservar(&conversa, "outro"));
    soltar(&conversa, "outro");
}

#[test]
fn painel_lendo_ou_recente_fica_com_a_troca() {
    let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
    assert!(!painel_ligado(&terminal));
    let leitura = painel_lendo(&terminal);
    assert!(painel_ligado(&terminal), "long-poll em andamento");
    drop(leitura);
    assert!(painel_ligado(&terminal), "acabou de ler");
}

/// Sem painel: o vigia troca para a conta B, leva o transcript e retoma o mesmo id, sem digitar nada.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn vigia_troca_de_conta_sem_painel_e_uma_vez_so() {
    let c = cena(true);
    let (terminal, conversa) = c.no_limite().await;
    let resultado = tokio::task::spawn_blocking({ let s = c.state.clone(); move || uma_volta(&s) }).await.unwrap();
    assert_eq!(resultado, vec![(terminal.clone(), Resultado::Trocou { de: "conta-a".into(), para: "conta-b".into() })]);
    assert!(ate(15, || c.aberturas().len() >= 2).await, "a CLI reabriu: {:?}", c.aberturas());
    let segunda = &c.aberturas()[1];
    assert!(segunda.starts_with(&*c.conta_b.to_string_lossy()) && segunda.contains("--resume sid-1"), "{segunda}");
    let cwd = c.projeto.to_string_lossy().into_owned();
    assert!(omni_core::conversations::claude_transcript_path(&c.conta_b, &cwd, "sid-1").is_file(), "o transcript foi junto");
    let indice = omni_core::conversations::read_index(c.dir.path());
    let segmentos = &indice.iter().find(|x| x.id == conversa).unwrap().segments;
    assert_eq!(segmentos.last().unwrap().profile_id.as_deref(), Some("conta-b"));
    assert_eq!(c.sessao(&terminal).meta.lock().unwrap().notice, None);
    // A volta seguinte não troca de novo (o aviso saiu com o reinício).
    assert!(tokio::task::spawn_blocking({ let s = c.state.clone(); move || uma_volta(&s) }).await.unwrap().is_empty());
    // A retomada reexibe a linha de limite antiga na conta nova: não vira outra troca (sem vaivém).
    c.sessao(&terminal).meta.lock().unwrap().notice = Some("usage_limit".into());
    assert!(tokio::task::spawn_blocking({ let s = c.state.clone(); move || uma_volta(&s) }).await.unwrap().is_empty(), "aviso velho na conta nova");
    // Turno novo na conta nova: a guarda sai e um limite de verdade volta a valer.
    c.sessao(&terminal).meta.lock().unwrap().notice = None;
    c.sessao(&terminal).interaction.lock().unwrap().turn_active = true;
    assert!(uma_volta(&c.state).is_empty(), "trabalhando: sem aviso deste turno ainda");
    assert!(!RECEM_TROCADAS.lock().unwrap().contains_key(&terminal), "a guarda saiu com o turno novo");
    let _ = close_session(&c.state, &terminal);
}

/// Corrida com o painel: quem reservou primeiro troca; o vigia não troca a mesma conversa junto.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn vigia_nao_troca_com_painel_ligado_nem_com_a_troca_reservada() {
    let c = cena(true);
    let (terminal, conversa) = c.no_limite().await;
    let leitura = painel_lendo(&terminal);
    assert!(uma_volta(&c.state).is_empty(), "painel lendo: a troca é dele");
    drop(leitura);
    // O painel some, mas a troca dele está no meio (reserva feita pela rota do painel).
    let EngineResponse::Api { status, .. } = local(c.state.clone(), "POST", &format!("/conversas/{conversa}/troca-de-conta/reservar"), None, None, vec![]).await
        else { panic!("api") };
    assert_eq!(status, 200);
    let EngineResponse::Api { status, .. } = local(c.state.clone(), "POST", &format!("/conversas/{conversa}/troca-de-conta/reservar"), None, None, vec![]).await
        else { panic!("api") };
    assert_eq!(status, 200, "o mesmo painel pode reafirmar");
    // Painel recente ainda conta por alguns segundos; força a leitura antiga para testar só a reserva.
    PAINEIS.lock().unwrap().remove(&terminal);
    assert_eq!(uma_volta(&c.state), vec![(terminal.clone(), Resultado::Reservada)]);
    assert_eq!(c.aberturas().len(), 1, "ninguém reabriu a CLI");
    // Reserva do vigia ativa: o painel recebe 409.
    local(c.state.clone(), "POST", &format!("/conversas/{conversa}/troca-de-conta/soltar"), None, None, vec![]).await;
    assert!(reservar(&conversa, "engine"));
    let EngineResponse::Api { status, .. } = local(c.state.clone(), "POST", &format!("/conversas/{conversa}/troca-de-conta/reservar"), None, None, vec![]).await
        else { panic!("api") };
    assert_eq!(status, 409);
    soltar(&conversa, "engine");
    let _ = close_session(&c.state, &terminal);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sem_outra_conta_adia_e_nao_reabre() {
    let c = cena(false);
    let (terminal, _) = c.no_limite().await;
    assert_eq!(uma_volta(&c.state), vec![(terminal.clone(), Resultado::SemOutraConta)]);
    assert!(uma_volta(&c.state).is_empty(), "adiada: não tenta a cada volta");
    assert_eq!(c.aberturas().len(), 1);
    assert!(!c.conta_b.join("projects").exists());
    let _ = close_session(&c.state, &terminal);
}

/// Fork (harness): o reinício não trava quando a CLI que sai ainda escreve na PTY (no macOS o processo só
/// termina depois que essa saída é lida). Antes: "A sessão anterior ainda está encerrando" em 3 s.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reinicio_com_saida_pendente_nao_trava() {
    let c = cena(true);
    let (terminal, _) = c.no_limite().await;
    let inicio = std::time::Instant::now();
    let r = tokio::task::spawn_blocking({ let s = c.state.clone(); let t = terminal.clone(); move || restart_session(&s, &t).map(|_| ()) }).await.unwrap();
    assert!(r.is_ok(), "{:?} em {:?}", r.err().map(|e| e.to_string()), inicio.elapsed());
    assert!(ate(15, || c.aberturas().len() >= 2).await, "a CLI reabriu");
    let _ = close_session(&c.state, &terminal);
}
