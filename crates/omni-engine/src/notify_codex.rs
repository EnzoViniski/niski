//! Fork (harness, A2): a PTY do Codex ganha o `notify` oficial apontando para o `omni-shim`, que avisa o
//! servidor dos ganchos no fim de cada turno (`omni_core::notify_do_codex`, rota `/ganchos/codex`).
//!
//! Só na linha **digitada** (o comando persistido não muda, e é ele que a retomada lê) e só em shells
//! POSIX: `-c "$MEDZA_CODEX_NOTIFY"`, com o valor no ambiente da PTY (sem aspas na linha, como pede o
//! reinício do Codex). O `notify` que a conta já tinha continua: o shim o chama depois. Sem servidor,
//! sem shim ou com os eventos desligados, nada muda. Codex pelo app-server (`--remote`) já tem eventos.
use super::*;
use omni_core::notify_do_codex as regra;

pub(crate) fn preparar(state: &EngineState, terminal: &str, origem: &SessionOrigin, comando: Option<String>)
    -> (Option<String>, Vec<(String, String)>) {
    preparar_com(state, terminal, origem, comando, caminho_do_shim())
}

pub(crate) fn preparar_com(state: &EngineState, terminal: &str, origem: &SessionOrigin, comando: Option<String>, shim: Option<PathBuf>)
    -> (Option<String>, Vec<(String, String)>) {
    let Some(mut comando) = comando else { return (None, Vec::new()) };
    let cabe = cfg!(unix) && origem.provider.as_deref() == Some("codex") && origem.conversation_id.is_some()
        && !comando.contains("--remote") && !regra::ja_tem(&comando);
    let Some(shim) = shim.filter(|_| cabe) else { return (Some(comando), Vec::new()) };
    let Some((porta, token)) = ganchos::registrar_codex(state, terminal, origem.profile_id.clone()) else { return (Some(comando), Vec::new()) };
    let mut ambiente = vec![
        (omni_core::ganchos::VARIAVEL_DO_TOKEN.to_owned(), token),
        (regra::VARIAVEL_DO_VALOR.to_owned(), regra::valor(&shim.to_string_lossy(), porta)),
    ];
    let codex_home = origem.env.iter().find(|(k, _)| k == "CODEX_HOME").map(|(_, v)| PathBuf::from(v))
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")));
    if let Some(dele) = codex_home.as_deref().and_then(regra::do_usuario) {
        ambiente.push((regra::VARIAVEL_DO_USUARIO.to_owned(), serde_json::to_string(&dele).unwrap_or_default()));
    }
    comando.push_str(&format!(" -c \"${}\"", regra::VARIAVEL_DO_VALOR));
    (Some(comando), ambiente)
}

#[cfg(all(test, unix))]
#[path = "notify_codex_testes.rs"]
mod testes;
