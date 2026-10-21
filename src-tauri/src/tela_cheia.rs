//! Fork: tela cheia no Mac como no Chrome.
//!
//! A janela usa `titleBarStyle: Overlay` (conteúdo até o topo). Em tela cheia, levar o mouse ao
//! topo faz o macOS revelar a barra de menus E uma faixa de barra de título nativa (a janela
//! privada `NSToolbarFullScreenWindow`, com os semáforos), as duas por cima da nossa TitleBar.
//!
//! O que fazemos, só enquanto a janela está em tela cheia:
//! 1. Penduramos um `NSTitlebarAccessoryViewController` de 1 pt. O AppKit chama o
//!    `setRevealAmount:` (privado, 0 a 1) dele durante a animação — o mesmo gancho do Chromium
//!    (`immersive_mode_controller_cocoa.mm`).
//! 2. A faixa de título nativa fica transparente (sem fundo nem material); semáforos criados pela
//!    fábrica pública AppKit ficam permanentemente no conteúdo (semaforos_nativos.rs).
//! 3. A cada mudança, `--medza-barra-revelada` = quanto a barra de menus entra na nossa janela ×
//!    `revealAmount`, preservando subpixels. Duração/curva do NSAnimationContext acompanham a barra.
//!
//! Fora da tela cheia nada disto existe. Sem o seletor privado, não pendura nada (barra por cima,
//! como antes).

use std::cell::{Cell, OnceCell, RefCell};
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSLayoutAttribute, NSTitlebarAccessoryViewController, NSView, NSWindow,
    NSWindowDidEnterFullScreenNotification, NSWindowDidExitFullScreenNotification,
    NSWindowWillEnterFullScreenNotification, NSWindowWillExitFullScreenNotification,
    NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification, NSWindowDidResizeNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSPoint, NSRect, NSSize};
use tauri::{Manager, WebviewWindow};

// Fork: controles permanentes AppKit e ponte de animação em arquivos próprios.
#[path = "semaforos_nativos.rs"]
mod semaforos;
#[path = "animacao_do_topo.rs"]
mod animacao;

/// Barra de menus sem notch, quando o AppKit não informa a altura.
const MENU_PADRAO_PT: f64 = 24.0;
/// Acima disto a medida está errada (outra tela, transição): não desloca.
const MAXIMO_PX: f64 = 60.0;

struct Janela {
    webview: WebviewWindow,
    ns: Retained<NSWindow>,
}

thread_local! {
    static JANELA: OnceCell<Janela> = const { OnceCell::new() };
    static ACESSORIO: RefCell<Option<Retained<ControleDeRevelacao>>> = const { RefCell::new(None) };
    /// Saindo da tela cheia: ignora o que o AppKit ainda mandar até o acessório sair.
    static SAINDO: Cell<bool> = const { Cell::new(false) };
    static ULTIMO_PX: Cell<f64> = const { Cell::new(0.0) };
    static ULTIMA_REVELACAO: Cell<f64> = const { Cell::new(0.0) };
    static ULTIMO_REGISTRADO: Cell<f64> = const { Cell::new(0.0) };
    /// Endereço da janela de faixa já tratada (0 = nenhuma).
    static FAIXA_TRATADA: Cell<usize> = const { Cell::new(0) };
    static FUNDOS_ESCONDIDOS: RefCell<Vec<Retained<NSView>>> = const { RefCell::new(Vec::new()) };
}

define_class!(
    #[unsafe(super(NSTitlebarAccessoryViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "MedzaControleDeRevelacao"]
    struct ControleDeRevelacao;

    impl ControleDeRevelacao {
        /// Privado do AppKit: chamado durante a animação da barra (0 = escondida, 1 = toda).
        #[unsafe(method(setRevealAmount:))]
        fn definir_revelacao(&self, quanto: f64) {
            unsafe {
                let _: () = msg_send![super(self), setRevealAmount: quanto];
            }
            deixar_faixa_transparente(self);
            let anterior = ULTIMA_REVELACAO.with(|c| c.replace(quanto));
            let movimento = if anterior > 0.0 && anterior < 1.0 {
                animacao::Movimento::imediato()
            } else { animacao::Movimento::atual(quanto) };
            aplicar_com_movimento(medir(quanto), movimento);
            // Ao fim da animação registra no log quanto desceu (uma linha por revelar/esconder).
            unsafe {
                let _: () = msg_send![
                    NSObject::class(),
                    cancelPreviousPerformRequestsWithTarget: self,
                    selector: sel!(medzaRegistrar),
                    object: None::<&AnyObject>
                ];
                let _: () = msg_send![
                    self,
                    performSelector: sel!(medzaRegistrar),
                    withObject: None::<&AnyObject>,
                    afterDelay: 0.3f64
                ];
            }
        }

        #[unsafe(method(medzaRegistrar))]
        fn registrar(&self) {
            let px = ULTIMO_PX.with(Cell::get);
            let anterior = ULTIMO_REGISTRADO.with(|c| c.replace(px));
            if (anterior - px).abs() >= 1.0 {
                omni_core::registro::info("tela_cheia_barra", &[("px", &format!("{px:.0}"))]);
            }
        }
    }
);

impl ControleDeRevelacao {
    fn novo(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        let vista = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
        );
        this.setView(&vista);
        this.setLayoutAttribute(NSLayoutAttribute::Bottom);
        this.setFullScreenMinHeight(0.0);
        this
    }
}

/// A janela que hospeda o acessório na tela cheia é a `NSToolbarFullScreenWindow` (a faixa com
/// os semáforos). Esconder a janela inteira (`alphaValue` 0) não pegou no macOS 26 — o AppKit
/// anima o alfa dela na revelação. Então a faixa fica TRANSPARENTE: janela sem fundo e sem os
/// fundos/materiais da barra de título; os semáforos nativos continuam por cima da nossa TitleBar
/// (que reserva 78 px à esquerda) e o clique nas áreas transparentes passa para a webview.
/// Feito uma vez por janela de faixa; na primeira, registra no log o que achou (só nomes de classe).
fn deixar_faixa_transparente(controle: &ControleDeRevelacao) {
    if SAINDO.with(Cell::get) {
        return;
    }
    let Some(janela) = controle.view().window() else { return };
    let eh_a_principal = JANELA.with(|j| j.get().is_some_and(|j| std::ptr::eq(&*j.ns, &*janela)));
    if eh_a_principal {
        return;
    }
    let endereco = &*janela as *const NSWindow as usize;
    semaforos::ocultar_originais(&janela);
    if FAIXA_TRATADA.with(|c| c.replace(endereco)) == endereco {
        return;
    }
    let classe_da_janela = nome_da_classe(&janela);
    unsafe {
        janela.setOpaque(false);
        let transparente: Option<Retained<AnyObject>> = msg_send![objc2::class!(NSColor), clearColor];
        let _: () = msg_send![&*janela, setBackgroundColor: transparente.as_deref()];
    }
    // Fundo da barra de título fica na moldura (superview do contentView), não só no contentView.
    let raiz = janela.contentView().map(|v| unsafe { v.superview() }.unwrap_or(v));
    let mut achados: Vec<String> = Vec::new();
    if let Some(raiz) = raiz {
        varrer(&raiz, 0, &mut achados);
    }
    omni_core::registro::info("tela_cheia_faixa", &[
        ("janela", &classe_da_janela),
        ("fundos", &achados.join(",")),
    ]);
}

fn nome_da_classe(objeto: &AnyObject) -> String {
    objeto.class().name().to_string_lossy().into_owned()
}

fn tem_botao(vista: &NSView) -> bool {
    let classe = objc2::class!(NSButton);
    vista.subviews().iter().any(|filha| {
        let eh: bool = unsafe { msg_send![&*filha, isKindOfClass: classe] };
        !filha.isHidden() && (eh || tem_botao(&filha))
    })
}

/// Esconde os fundos (efeito visual / "BackgroundView") que não carregam os semáforos. Fundo com
/// botão dentro fica (escondê-lo sumiria com os semáforos) e vai para o log como `mantido`.
fn varrer(vista: &NSView, nivel: usize, achados: &mut Vec<String>) {
    if nivel > 8 {
        return;
    }
    for filha in vista.subviews().iter() {
        let nome = nome_da_classe(&filha);
        let eh_fundo = nome.contains("VisualEffect") || nome.contains("BackgroundView");
        if eh_fundo {
            if tem_botao(&filha) {
                achados.push(format!("{nome}:mantido"));
            } else {
                filha.setHidden(true);
                FUNDOS_ESCONDIDOS.with(|f| f.borrow_mut().push(filha.clone()));
                achados.push(format!("{nome}:escondido"));
                continue;
            }
        }
        varrer(&filha, nivel + 1, achados);
    }
}

/// Saindo da tela cheia: devolve os fundos escondidos (a janela da faixa pode ser reaproveitada).
fn restaurar_fundos() {
    for vista in FUNDOS_ESCONDIDOS.with(|f| std::mem::take(&mut *f.borrow_mut())) {
        vista.setHidden(false);
    }
    FAIXA_TRATADA.with(|c| c.set(0));
}

/// Quanto (em pt = px CSS) a barra de menus entra na nossa janela quando toda revelada. Sem notch
/// a janela vai até o topo da tela (≈24 pt); com notch o macOS costuma deixar a janela abaixo da
/// faixa do notch, e aí a barra nem encosta (0).
fn altura_da_barra_de_menus(ns: &NSWindow) -> f64 {
    let mut altura: f64 = unsafe {
        let app: Option<Retained<AnyObject>> = msg_send![objc2::class!(NSApplication), sharedApplication];
        let menu: Option<Retained<AnyObject>> = app.and_then(|app| msg_send![&*app, mainMenu]);
        menu.map(|menu| msg_send![&*menu, menuBarHeight]).unwrap_or(0.0)
    };
    let tela: Option<Retained<AnyObject>> = unsafe { msg_send![ns, screen] };
    let Some(tela) = tela else { return 0.0 };
    // `safeAreaInsets` é do macOS 12+; o mínimo do app é 11.
    let tem_inset: bool = unsafe { msg_send![&*tela, respondsToSelector: sel!(safeAreaInsets)] };
    if tem_inset {
        let inset: objc2_foundation::NSEdgeInsets = unsafe { msg_send![&*tela, safeAreaInsets] };
        altura = altura.max(inset.top);
    }
    if !(altura > 0.0) {
        altura = MENU_PADRAO_PT;
    }
    let quadro_da_tela: NSRect = unsafe { msg_send![&*tela, frame] };
    let topo_da_tela = quadro_da_tela.origin.y + quadro_da_tela.size.height;
    let quadro = ns.frame();
    let folga = topo_da_tela - (quadro.origin.y + quadro.size.height);
    (altura - folga.max(0.0)).max(0.0)
}

fn medir(quanto: f64) -> f64 {
    if SAINDO.with(Cell::get) || !(quanto > 0.0) {
        return 0.0;
    }
    let altura = JANELA.with(|j| j.get().map(|j| altura_da_barra_de_menus(&j.ns))).unwrap_or(0.0);
    let px = quanto.min(1.0) * altura;
    if px > 0.0 && px <= MAXIMO_PX { px } else { 0.0 }
}

fn rodar_js(js: &str) {
    JANELA.with(|j| {
        if let Some(j) = j.get() {
            let _ = j.webview.eval(js);
        }
    });
}

fn aplicar(px: f64) {
    aplicar_com_movimento(px, animacao::Movimento::imediato());
}

fn aplicar_com_movimento(px: f64, movimento: animacao::Movimento) {
    JANELA.with(|j| {
        if let Some(j) = j.get() { semaforos::atualizar(&j.ns, px, movimento.duracao > 0.0); }
    });
    let anterior = ULTIMO_PX.with(|c| c.replace(px));
    if (anterior - px).abs() < 0.01 {
        return;
    }
    rodar_js(&movimento.javascript(px));
}

fn marcar_tela_cheia(ligada: bool) {
    rodar_js(if ligada {
        "document.documentElement.classList.add('medza-tela-cheia')"
    } else {
        "document.documentElement.classList.remove('medza-tela-cheia')"
    });
}

fn pendurar_acessorio(mtm: MainThreadMarker) {
    SAINDO.with(|c| c.set(false));
    if ACESSORIO.with(|a| a.borrow().is_some()) {
        return;
    }
    // Seletor privado: sem ele, não pendura nada (chamar `super` sem ele derrubaria o app).
    let classe = NSTitlebarAccessoryViewController::class();
    if classe.instance_method(sel!(setRevealAmount:)).is_none() || classe.instance_method(sel!(revealAmount)).is_none() {
        omni_core::registro::aviso_sem_repetir("tela_cheia_sem_reveal", "tela_cheia_sem_reveal", &[]);
        return;
    }
    let controle = ControleDeRevelacao::novo(mtm);
    JANELA.with(|j| {
        if let Some(j) = j.get() {
            j.ns.addTitlebarAccessoryViewController(&controle);
        }
    });
    ACESSORIO.with(|a| *a.borrow_mut() = Some(controle));
    marcar_tela_cheia(true);
}

fn com_acessorio(f: impl FnOnce(&ControleDeRevelacao)) {
    if let Some(controle) = ACESSORIO.with(|a| a.borrow().clone()) {
        f(&controle);
    }
}

fn tirar_acessorio() {
    if let Some(controle) = ACESSORIO.with(|a| a.borrow_mut().take()) {
        unsafe {
            let _: () = msg_send![
                NSObject::class(),
                cancelPreviousPerformRequestsWithTarget: &*controle,
                selector: sel!(medzaRegistrar),
                object: None::<&AnyObject>
            ];
        }
        controle.removeFromParentViewController();
    }
    aplicar(0.0);
    ULTIMA_REVELACAO.with(|c| c.set(0.0));
    marcar_tela_cheia(false);
}

/// Liga a observação da tela cheia na janela principal. Chamado no `setup` (thread principal).
pub fn instalar(app: &tauri::App) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(webview) = app.get_webview_window("main") else { return };
    let Ok(ponteiro) = webview.ns_window() else { return };
    let Some(ns) = NonNull::new(ponteiro.cast::<NSWindow>()) else { return };
    // SAFETY: ponteiro vivo do NSWindow da janela principal; retemos uma referência própria.
    let Some(ns) = (unsafe { Retained::retain(ns.as_ptr()) }) else { return };
    semaforos::instalar(&ns, mtm);

    let centro = NSNotificationCenter::defaultCenter();
    let objeto: &AnyObject = &ns;
    let vai_entrar = RcBlock::new(move |_: NonNull<NSNotification>| pendurar_acessorio(mtm));
    // Já na janela da faixa: esconde antes da primeira revelação.
    let entrou = RcBlock::new(|_: NonNull<NSNotification>| com_acessorio(deixar_faixa_transparente));
    let vai_sair = RcBlock::new(|_: NonNull<NSNotification>| {
        SAINDO.with(|c| c.set(true));
        restaurar_fundos();
        aplicar(0.0);
        marcar_tela_cheia(false);
    });
    let saiu = RcBlock::new(|_: NonNull<NSNotification>| tirar_acessorio());
    let atualizou_janela = RcBlock::new(|_: NonNull<NSNotification>| {
        JANELA.with(|j| {
            if let Some(j) = j.get() { semaforos::atualizar(&j.ns, ULTIMO_PX.with(Cell::get), false); }
        });
    });
    // Os observadores vivem enquanto o app viver (a janela principal não é recriada).
    unsafe {
        let a = centro.addObserverForName_object_queue_usingBlock(
            Some(NSWindowWillEnterFullScreenNotification), Some(objeto), None, &vai_entrar);
        let b = centro.addObserverForName_object_queue_usingBlock(
            Some(NSWindowDidEnterFullScreenNotification), Some(objeto), None, &entrou);
        let c = centro.addObserverForName_object_queue_usingBlock(
            Some(NSWindowWillExitFullScreenNotification), Some(objeto), None, &vai_sair);
        let d = centro.addObserverForName_object_queue_usingBlock(
            Some(NSWindowDidExitFullScreenNotification), Some(objeto), None, &saiu);
        let e = centro.addObserverForName_object_queue_usingBlock(
            Some(NSWindowDidBecomeKeyNotification), Some(objeto), None, &atualizou_janela);
        let f = centro.addObserverForName_object_queue_usingBlock(
            Some(NSWindowDidResignKeyNotification), Some(objeto), None, &atualizou_janela);
        let g = centro.addObserverForName_object_queue_usingBlock(
            Some(NSWindowDidResizeNotification), Some(objeto), None, &atualizou_janela);
        std::mem::forget((a, b, c, d, e, f, g));
    }
    JANELA.with(|j| {
        let _ = j.set(Janela { webview, ns });
    });
}
