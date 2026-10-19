//! O escopo `fs:scope` libera `**`, mas no Unix o plugin de arquivos exige, por padrão, que um `.`
//! no início de um componente apareça literalmente no padrão (`require_literal_leading_dot`
//! cai para `cfg!(unix)` quando não configurado). Sem esta opção, no macOS/Linux toda leitura em
//! `.claude/skills`, `~/.claude` (perfil Padrão), `.env`, `.github` etc. falha com
//! "forbidden path". No Windows o padrão já é `false`, por isso o defeito não aparecia lá.

#[test]
fn escopo_de_arquivos_aceita_pastas_que_comecam_com_ponto() {
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json válido");
    assert_eq!(
        conf["plugins"]["fs"]["requireLiteralLeadingDot"],
        serde_json::Value::Bool(false),
        "sem isto o macOS bloqueia .claude/skills e outros dotfiles"
    );
}
