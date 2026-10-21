//! Fork: transfere duração e curva da animação AppKit para a barra desenhada na WKWebView.
//!
//! Até a 0.12.0 cada amostra intermediária do AppKit virava um `eval` com duração zero: a barra
//! pulava de posição em posição no ritmo em que os `eval` chegavam (irregular), e o Enzo via o topo
//! descer "duro". Agora a barra recebe só o destino (toda revelada ou escondida) quando o movimento
//! começa ou muda de sentido, e o CSS faz um deslizar contínuo até lá (`planejar`).
use objc2::{class, msg_send};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;

#[derive(Clone)]
pub(super) struct Movimento {
    pub duracao: f64,
    pub curva: [f32; 4],
}

impl Movimento {
    pub fn imediato() -> Self { Self { duracao: 0.0, curva: [0.25, 0.1, 0.25, 1.0] } }

    /// Duração e curva do AppKit (ou o padrão do macOS) para percorrer `distancia` (0 a 1) da barra.
    pub fn atual(distancia: f64) -> Self {
        let contexto: Retained<AnyObject> = unsafe { msg_send![class!(NSAnimationContext), currentContext] };
        let duracao: f64 = unsafe { msg_send![&*contexto, duration] };
        let workspace: Retained<AnyObject> = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
        let reduzir: bool = unsafe { msg_send![&*workspace, accessibilityDisplayShouldReduceMotion] };
        let mut movimento = Self { duracao: duracao_do_trecho(duracao, distancia, reduzir), ..Self::imediato() };
        let funcao: Option<Retained<AnyObject>> = unsafe { msg_send![&*contexto, timingFunction] };
        if let Some(funcao) = funcao {
            let mut inicio = [0.0f32; 2];
            let mut fim = [0.0f32; 2];
            unsafe {
                let _: () = msg_send![&*funcao, getControlPointAtIndex: 1u32, values: inicio.as_mut_ptr()];
                let _: () = msg_send![&*funcao, getControlPointAtIndex: 2u32, values: fim.as_mut_ptr()];
            }
            let curva = [inicio[0], inicio[1], fim[0], fim[1]];
            if curva.iter().all(|n| n.is_finite()) && (0.0..=1.0).contains(&curva[0]) && (0.0..=1.0).contains(&curva[2]) {
                movimento.curva = curva;
            }
        }
        movimento
    }

    pub fn javascript(&self, px: f64) -> String {
        let [x1,y1,x2,y2] = self.curva;
        format!("(()=>{{const s=document.documentElement.style;s.setProperty('--medza-barra-duracao','{:.3}s');s.setProperty('--medza-barra-curva','cubic-bezier({x1:.6},{y1:.6},{x2:.6},{y2:.6})');s.setProperty('--medza-barra-revelada','{px:.3}px')}})()",self.duracao)
    }
}

fn limitar_duracao(duracao: f64, reduzir: bool) -> f64 {
    if reduzir || !duracao.is_finite() { 0.0 } else { duracao.clamp(0.0, 1.0) }
}

/// Duração da barra de menus do macOS quando o AppKit não informa (contexto sem animação).
const DURACAO_PADRAO: f64 = 0.25;

/// Tempo para percorrer o que falta: proporcional à distância, com piso para não virar um salto.
fn duracao_do_trecho(nativa: f64, distancia: f64, reduzir: bool) -> f64 {
    let base = limitar_duracao(nativa, reduzir);
    if reduzir { return 0.0; }
    let base = if base >= 0.05 { base } else { DURACAO_PADRAO };
    let distancia = if distancia.is_finite() { distancia.clamp(0.0, 1.0) } else { 1.0 };
    (base * distancia).max(0.08)
}

/// Para onde a barra deve ir depois desta amostra do AppKit (0 = escondida, 1 = toda revelada), ou
/// `None` quando continua indo para o mesmo destino e não há nada a mandar.
pub(super) fn planejar(anterior: f64, atual: f64, destino: Option<f64>) -> Option<f64> {
    let novo = if !(atual > 0.0) { 0.0 }
        else if atual >= 1.0 { 1.0 }
        else if atual > anterior { 1.0 }
        else if atual < anterior { 0.0 }
        else { return None };
    (destino != Some(novo)).then_some(novo)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserva_tempo_nativo_sem_numeros_invalidos_nem_animacao_em_reduzir_movimento() {
        assert_eq!(limitar_duracao(0.18,false),0.18);
        assert_eq!(limitar_duracao(0.18,true),0.0);
        assert_eq!(limitar_duracao(f64::NAN,false),0.0);
        assert_eq!(limitar_duracao(-1.0,false),0.0);
        assert_eq!(limitar_duracao(30.0,false),1.0);
    }
    #[test]
    fn manda_so_o_destino_quando_o_movimento_comeca_ou_vira() {
        // Revelar: a primeira amostra já manda "toda revelada"; as seguintes não mandam nada.
        assert_eq!(planejar(0.0, 0.08, None), Some(1.0));
        assert_eq!(planejar(0.08, 0.4, Some(1.0)), None);
        assert_eq!(planejar(0.4, 1.0, Some(1.0)), None);
        // Esconder no meio do caminho: vira uma vez e segue.
        assert_eq!(planejar(0.6, 0.5, Some(1.0)), Some(0.0));
        assert_eq!(planejar(0.5, 0.2, Some(0.0)), None);
        assert_eq!(planejar(0.2, 0.0, Some(0.0)), None);
        // Valor inválido do AppKit esconde.
        assert_eq!(planejar(0.3, f64::NAN, Some(1.0)), Some(0.0));
        assert_eq!(planejar(0.5, 0.5, Some(1.0)), None);
    }
    #[test]
    fn trecho_proporcional_com_piso_e_sem_movimento_em_reduzir() {
        assert!((duracao_do_trecho(0.0, 1.0, false) - DURACAO_PADRAO).abs() < 1e-9);
        assert!((duracao_do_trecho(0.3, 0.5, false) - 0.15).abs() < 1e-9);
        assert_eq!(duracao_do_trecho(0.3, 0.01, false), 0.08);
        assert_eq!(duracao_do_trecho(0.3, 1.0, true), 0.0);
        assert_eq!(duracao_do_trecho(f64::NAN, 1.0, false), DURACAO_PADRAO);
    }
    #[test]
    fn ponte_css_preserva_subpixel_e_ordem_das_propriedades() {
        let js=Movimento::imediato().javascript(12.375);
        assert!(js.contains("12.375px"));
        assert!(js.find("--medza-barra-duracao").unwrap()<js.find("--medza-barra-revelada").unwrap());
    }
}
