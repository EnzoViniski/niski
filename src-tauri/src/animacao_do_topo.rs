//! Fork: transfere duração e curva da animação AppKit para a barra desenhada na WKWebView.
//! Sem easing adicional nas amostras intermediárias: não deixa a interface perseguir o macOS.
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

    pub fn atual(revelacao: f64) -> Self {
        // Quando o AppKit já entrega progresso por quadro, aplicar diretamente é a sincronização.
        if revelacao > 0.0 && revelacao < 1.0 { return Self::imediato(); }
        let contexto: Retained<AnyObject> = unsafe { msg_send![class!(NSAnimationContext), currentContext] };
        let duracao: f64 = unsafe { msg_send![&*contexto, duration] };
        let workspace: Retained<AnyObject> = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
        let reduzir: bool = unsafe { msg_send![&*workspace, accessibilityDisplayShouldReduceMotion] };
        let mut movimento = Self { duracao: limitar_duracao(duracao, reduzir), ..Self::imediato() };
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
    fn ponte_css_preserva_subpixel_e_ordem_das_propriedades() {
        let js=Movimento::imediato().javascript(12.375);
        assert!(js.contains("12.375px"));
        assert!(js.find("--medza-barra-duracao").unwrap()<js.find("--medza-barra-revelada").unwrap());
    }
}
