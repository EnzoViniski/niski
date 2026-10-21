//! Fork: "Atualizar o PC" pelo celular. Quem baixa a versão nova e confere a assinatura é a casca
//! (updater do Tauri), não o engine. Aqui só fica um pedido na pasta de dados; o app do PC confere
//! a cada poucos segundos (`src-tauri/src/pedido_de_atualizacao.rs`) e roda o mesmo "Atualizar
//! agora" da barra (cópia do estado, download assinado, troca do engine e retomada das abas).
use super::*;

pub const ARQUIVO_DO_PEDIDO: &str = "pedido-de-atualizacao.json";

pub fn gravar_pedido(dir: &std::path::Path, agora_ms: u64) -> std::io::Result<()> {
    std::fs::write(dir.join(ARQUIVO_DO_PEDIDO), json!({ "pedido_em_ms": agora_ms }).to_string())
}

pub(super) async fn pedir_atualizacao(State(state): State<Arc<EngineState>>) -> Json<Value> {
    let dir = state.state_file.parent().unwrap().to_path_buf();
    match gravar_pedido(&dir, now_ms()) {
        Ok(()) => {
            omni_core::registro::info("pedido_de_atualizacao", &[("origem", "celular")]);
            Json(json!({ "ok": true }))
        }
        Err(erro) => Json(json!({ "ok": false, "erro": erro.kind().to_string() })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pedido_fica_na_pasta_de_dados_com_a_hora() {
        let dir = tempfile::tempdir().unwrap();
        gravar_pedido(dir.path(), 1234).unwrap();
        let texto = std::fs::read_to_string(dir.path().join(ARQUIVO_DO_PEDIDO)).unwrap();
        let valor: Value = serde_json::from_str(&texto).unwrap();
        assert_eq!(valor["pedido_em_ms"], 1234);
    }
}
