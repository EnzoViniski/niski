//! Fork (energia): long-poll do snapshot do terminal.
//!
//! Antes a UI perguntava "tem saída nova?" de 16 em 16 ms (digitando) a 500 ms (parada), por aba
//! visível: dezenas de viagens de IPC por segundo com o app parado. Agora o pedido fica aqui até a
//! PTY escrever depois de `since`, até o estado da sessão mudar (conferido a cada
//! [`RITMO_DO_ESTADO`], dentro do engine, sem IPC) ou até o prazo. Parado não custa nada para a UI;
//! a saída nova chega no instante em que a PTY escreve, sem esperar o próximo poll.
//!
//! O aviso é um só para todas as sessões (geração + `Condvar`): quem acorda confere só a própria
//! sessão, que é barato (comparar `next_seq`). Assim nada muda nas structs das sessões.
use super::{EngineState, SessionEntry};
use omni_protocol::TerminalSession;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// O estado da sessão (trabalhando → respondeu, aviso de limite...) muda também com o tempo, sem
/// saída nova. Este é o atraso máximo para a UI ver essa mudança, igual ao poll calado de antes.
const RITMO_DO_ESTADO: Duration = Duration::from_millis(250);
/// Teto do prazo pedido pela UI. Acima disso o cliente desistiria antes (timeout de leitura).
const PRAZO_MAXIMO: Duration = Duration::from_secs(30);
/// Saída chega em rajadas (redesenho da TUI em vários `write`). Junta o que vier logo em seguida
/// numa resposta só, em vez de uma viagem por pedaço.
const JUNTAR_RAJADA: Duration = Duration::from_millis(6);

static GERACAO: Mutex<u64> = Mutex::new(0);
static MUDOU: Condvar = Condvar::new();

/// Alguma PTY escreveu. Chamado pela thread leitora depois de soltar os locks da sessão.
pub(super) fn avisar() {
    let mut geracao = GERACAO.lock().unwrap_or_else(|erro| erro.into_inner());
    *geracao = geracao.wrapping_add(1);
    MUDOU.notify_all();
}

/// Impressão digital do estado que a UI já conhece. Texto hexadecimal: um `u64` em JSON perde
/// precisão no JavaScript.
pub(super) fn impressao(sessao: &TerminalSession) -> String {
    let mut hasher = DefaultHasher::new();
    serde_json::to_vec(sessao).unwrap_or_default().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Volta quando `pronto(conferir_estado)` disser que sim ou no prazo. `conferir_estado` é `true`
/// no máximo uma vez a cada `ritmo`; entre uma e outra, só os avisos de saída acordam.
fn esperar(prazo: Duration, ritmo: Duration, mut pronto: impl FnMut(bool) -> bool) {
    let fim = Instant::now() + prazo;
    let mut conferir_em = Instant::now();
    loop {
        // Lê a geração **antes** de conferir: um aviso entre a conferência e o `wait` não se perde.
        let vista = *GERACAO.lock().unwrap_or_else(|erro| erro.into_inner());
        let agora = Instant::now();
        let conferir_estado = agora >= conferir_em;
        if conferir_estado {
            conferir_em = agora + ritmo;
        }
        if pronto(conferir_estado) || agora >= fim {
            return;
        }
        let ate = fim.min(conferir_em).saturating_duration_since(Instant::now());
        let guarda = GERACAO.lock().unwrap_or_else(|erro| erro.into_inner());
        let _ = MUDOU.wait_timeout_while(guarda, ate, |geracao| *geracao == vista);
    }
}

/// Segura o pedido de snapshot até haver o que mostrar. Sessão inexistente, histórica ou trocada
/// (reiniciar/fechar) volta na hora: quem responde é o `snapshot` de sempre.
pub(super) fn esperar_snapshot(state: &EngineState, session_id: &str, since: u64, espera_ms: u64, conhecido: Option<&str>) {
    if espera_ms == 0 {
        return;
    }
    let Some(sessao) = viva(state, session_id) else { return };
    // Sem estado conhecido (primeira leitura desta aba) não há o que comparar: responde já.
    let Some(conhecido) = conhecido else { return };
    let mut veio_saida = false;
    esperar(Duration::from_millis(espera_ms).min(PRAZO_MAXIMO), RITMO_DO_ESTADO, |conferir_estado| {
        veio_saida = sessao.output.lock().map(|saida| saida.next_seq > since).unwrap_or(true);
        if veio_saida {
            return true;
        }
        if !conferir_estado {
            return false;
        }
        // A sessão foi reiniciada ou fechada: o `Arc` guardado não recebe mais saída.
        let Some(atual) = viva(state, session_id) else { return true };
        if !Arc::ptr_eq(&atual, &sessao) {
            return true;
        }
        let mut meta = super::settle(&sessao);
        meta.input_locked = sessao.reserved.load(std::sync::atomic::Ordering::SeqCst);
        impressao(&meta) != conhecido
    });
    if veio_saida {
        std::thread::sleep(JUNTAR_RAJADA);
    }
}

fn viva(state: &EngineState, session_id: &str) -> Option<Arc<super::LiveSession>> {
    let sessoes = state.sessions.lock().ok()?;
    match sessoes.get(session_id)? {
        SessionEntry::Live(sessao) => Some(sessao.clone()),
        SessionEntry::Historical(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volta_no_prazo_quando_nada_acontece() {
        let inicio = Instant::now();
        let mut conferencias = 0;
        esperar(Duration::from_millis(120), Duration::from_millis(40), |conferir| {
            if conferir { conferencias += 1; }
            false
        });
        let passou = inicio.elapsed();
        assert!(passou >= Duration::from_millis(120), "voltou cedo: {passou:?}");
        assert!(passou < Duration::from_millis(400), "demorou demais: {passou:?}");
        // Confere o estado no ritmo (na entrada e a cada 40 ms), não em laço apertado.
        assert!((3..=5).contains(&conferencias), "conferências: {conferencias}");
    }

    #[test]
    fn acorda_logo_com_saida_nova() {
        let pronto = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sinal = pronto.clone();
        let escritor = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            sinal.store(true, std::sync::atomic::Ordering::SeqCst);
            avisar();
        });
        let inicio = Instant::now();
        esperar(Duration::from_secs(5), Duration::from_secs(5), |_| pronto.load(std::sync::atomic::Ordering::SeqCst));
        escritor.join().unwrap();
        assert!(inicio.elapsed() < Duration::from_secs(1), "não acordou com o aviso: {:?}", inicio.elapsed());
    }

    #[test]
    fn impressao_muda_com_o_estado_e_e_estavel_sem_mudanca() {
        let sessao: TerminalSession = serde_json::from_value(serde_json::json!({
            "id": "t1", "project_id": "p", "name": "n", "cwd": "/", "shell": "zsh", "state": "working",
            "pid": null, "created_at_ms": 1, "last_activity_at_ms": 2, "output_seq": 3, "rows": 24, "cols": 80
        })).unwrap();
        let mut outra = sessao.clone();
        assert_eq!(impressao(&sessao), impressao(&outra));
        outra.input_locked = true;
        assert_ne!(impressao(&sessao), impressao(&outra));
        assert_eq!(impressao(&sessao).len(), 16);
    }
}

#[cfg(test)]
#[path = "espera_de_saida_testes.rs"]
mod testes_com_sessao;
