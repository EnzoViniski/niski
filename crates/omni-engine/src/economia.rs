//! Fork: economia de energia no engine (2026-10-19).
//!
//! Duas coisas que rodavam a cada leitura da interface e não precisam:
//!
//! - `ps -A` (a lista de **todos** os processos da máquina). `interaction::capabilities` chama
//!   `provider_running`, que roda o `ps` duas vezes; ele entra em cada `resumo` de conversa (a
//!   Conversa relê a timeline a cada 1,2 s com o agente trabalhando, a faixa do terminal a cada 3 s, o
//!   celular também). `has_children` roda o `ps` a cada `settle` de um shell calado com processo
//!   filho (dev server), e `settle` entra em todo snapshot do terminal (10x por segundo por pane).
//!   Aqui a saída fica guardada por [`VALIDADE_DO_PS`]: um processo que acabou de sair aparece no
//!   máximo esse tanto depois, o que para estado de tela não muda nada.
//! - A busca por rollouts do Codex sem caminho em `list_sessions` (abre o começo de cada rollout
//!   da conta). A lista de sessões é lida a cada 1 s; a busca passa a rodar no máximo a cada
//!   [`INTERVALO_DA_DESCOBERTA`] por pasta de dados — o mesmo ritmo que `mobile::summaries` já usa.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Quanto tempo uma leitura do `ps` vale. Nos testes, nada: eles sobem e matam processos e conferem
/// na hora.
#[cfg(not(test))]
const VALIDADE_DO_PS: Duration = Duration::from_millis(1_000);
#[cfg(test)]
const VALIDADE_DO_PS: Duration = Duration::ZERO;

const INTERVALO_DA_DESCOBERTA: Duration = Duration::from_secs(15);

type Leituras = HashMap<String, (Instant, Arc<String>)>;

/// Saída de `ps -A -o <colunas...>` (stdout), reaproveitada por [`VALIDADE_DO_PS`].
/// `None` quando o `ps` não rodou — o chamador decide o que isso significa, como antes.
pub fn ps(colunas: &[&str]) -> Option<Arc<String>> {
    static CACHE: OnceLock<Mutex<Leituras>> = OnceLock::new();
    let chave = colunas.join(" ");
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if !VALIDADE_DO_PS.is_zero() {
        if let Some((quando, saida)) = cache.lock().ok()?.get(&chave) {
            if quando.elapsed() < VALIDADE_DO_PS { return Some(saida.clone()); }
        }
    }
    let mut args = vec!["-A"];
    for coluna in colunas { args.push("-o"); args.push(coluna); }
    let saida = std::process::Command::new("ps").args(&args).output().ok()?;
    let texto = Arc::new(String::from_utf8_lossy(&saida.stdout).into_owned());
    if let Ok(mut cache) = cache.lock() { cache.insert(chave, (Instant::now(), texto.clone())); }
    Some(texto)
}

/// `true` quando já passou [`INTERVALO_DA_DESCOBERTA`] desde a última busca nesta pasta de dados
/// (e marca agora). Por pasta, e não global, para cada engine de teste ter a sua.
pub fn pode_descobrir_codex(pasta: &Path) -> bool {
    static ULTIMA: OnceLock<Mutex<HashMap<PathBuf, Instant>>> = OnceLock::new();
    let Ok(mut ultima) = ULTIMA.get_or_init(|| Mutex::new(HashMap::new())).lock() else { return true };
    let vencida = ultima.get(pasta).is_none_or(|quando| quando.elapsed() >= INTERVALO_DA_DESCOBERTA);
    if vencida { ultima.insert(pasta.to_path_buf(), Instant::now()); }
    vencida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descoberta_roda_na_primeira_e_espera_na_seguinte() {
        let pasta = std::env::temp_dir().join(format!("omni-economia-{}", std::process::id()));
        assert!(pode_descobrir_codex(&pasta));
        assert!(!pode_descobrir_codex(&pasta));
        assert!(pode_descobrir_codex(&pasta.join("outra")));
    }

    #[cfg(not(windows))]
    #[test]
    fn ps_lista_este_processo() {
        let saida = ps(&["pid="]).expect("ps");
        let eu = std::process::id();
        assert!(saida.lines().any(|linha| linha.trim().parse::<u32>() == Ok(eu)));
    }
}
