//! Tela do print enviado pelo Enzo. É uma CLI parada, sem turno a interromper.
pub(crate) fn reconhece(tela: &str) -> bool {
    tela.contains("This conversation is open in another app")
        && tela.contains("Close it there and press")
        && tela.contains("r retry")
        && tela.contains("f fork")
        && tela.contains("transcript")
        && !tela.to_lowercase().contains("esc to interrupt")
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn so_a_tela_completa_da_cli() {
        assert!(reconhece("🔒 This conversation is open in another app\nClose it there and press R to continue here.\nr retry   f fork   esc/^c/q exit   ^t transcript"));
        assert!(!reconhece(
            "O erro This conversation is open in another app foi corrigido."
        ));
    }
}
