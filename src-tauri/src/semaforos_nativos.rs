//! Fork: semáforos AppKit permanentes no conteúdo, sem controles HTML ou ações de janela próprias.
//! `standardWindowButton:forStyleMask:` é a fábrica pública da Apple. Os originais permanecem na
//! hierarquia do AppKit, ocultos, e seus estados habilitados continuam sendo a fonte de verdade.
use std::cell::{Cell, RefCell};
use objc2::{class, define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWindow, NSWindowStyleMask};
use objc2_foundation::{ns_string, NSPoint, NSRect, NSSize};

#[link(name = "CoreImage", kind = "framework")]
unsafe extern "C" {}

const ALTURA: f64 = 34.0;
const LARGURA: f64 = 78.0;

struct Grupo {
    vista: Retained<Semaforos>,
    botoes: Vec<Retained<NSView>>,
    originais: Vec<Retained<NSView>>,
}

struct Estado { cinza: Cell<Option<bool>> }

thread_local! {
    static GRUPO: RefCell<Option<Grupo>> = const { RefCell::new(None) };
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "MedzaSemaforosNativos"]
    #[ivars = Estado]
    struct Semaforos;

    impl Semaforos {
        #[unsafe(method(mouseEntered:))]
        fn entrou(&self, _evento: &AnyObject) { self.aplicar_cinza(false); }

        #[unsafe(method(mouseExited:))]
        fn saiu(&self, _evento: &AnyObject) { self.aplicar_cinza(true); }

        // Só os botões interceptam cliques. O espaço entre eles continua arrastando a TitleBar.
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, ponto: NSPoint) -> Option<Retained<NSView>> {
            let alvo: Option<Retained<NSView>> = unsafe { msg_send![super(self), hitTest: ponto] };
            alvo.filter(|vista| !std::ptr::eq(&**vista, &**self))
        }
    }
);

impl Semaforos {
    fn novo(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Estado { cinza: Cell::new(None) });
        let this: Retained<Self> = unsafe {
            msg_send![super(this), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(LARGURA, ALTURA))]
        };
        this.setWantsLayer(true);
        // NSTrackingArea: mouseEnteredAndExited | activeAlways | inVisibleRect. A área acompanha
        // resize, animação e entrada/saída da tela cheia automaticamente (API pública desde 10.5).
        let area: Option<Retained<AnyObject>> = unsafe {
            let alocada: Allocated<AnyObject> = msg_send![class!(NSTrackingArea), alloc];
            msg_send![alocada, initWithRect: this.bounds(), options: (1usize | 128 | 512), owner: &*this, userInfo: None::<&AnyObject>]
        };
        if let Some(area) = area {
            unsafe { let _: () = msg_send![&*this, addTrackingArea: &*area]; }
        }
        this.aplicar_cinza(true);
        this
    }

    fn aplicar_cinza(&self, cinza: bool) {
        if self.ivars().cinza.replace(Some(cinza)) == Some(cinza) { return; }
        // O filtro só muda o desenho, sem desabilitar cliques, acessibilidade ou hover nativo.
        let filtros: Retained<AnyObject> = unsafe {
            let filtro: Option<Retained<AnyObject>> = if cinza {
                msg_send![class!(CIFilter), filterWithName: ns_string!("CIColorControls")]
            } else { None };
            if let Some(filtro) = filtro {
                let _: () = msg_send![&*filtro, setDefaults];
                let zero: Retained<AnyObject> = msg_send![class!(NSNumber), numberWithDouble: 0.0f64];
                let _: () = msg_send![&*filtro, setValue: &*zero, forKey: ns_string!("inputSaturation")];
                msg_send![class!(NSArray), arrayWithObject: &*filtro]
            } else { msg_send![class!(NSArray), array] }
        };
        unsafe { let _: () = msg_send![self, setContentFilters: &*filtros]; }
    }
}

/// Oculta apenas os controles originais; não muda styleMask, fechar/minimizar ou o dono da janela.
pub(super) fn ocultar_originais(ns: &NSWindow) {
    if !GRUPO.with(|grupo| grupo.borrow().is_some()) { return; }
    // O AppKit pode mover os mesmos controles para outra janela durante a tela cheia.
    GRUPO.with(|grupo| {
        if let Some(grupo) = grupo.borrow().as_ref() {
            for botao in &grupo.originais { if !botao.isHidden() { botao.setHidden(true); } }
        }
    });
    for tipo in 0usize..3 {
        let botao: Option<Retained<NSView>> = unsafe { msg_send![ns, standardWindowButton: tipo] };
        if let Some(botao) = botao { if !botao.isHidden() { botao.setHidden(true); } }
    }
}

pub(super) fn instalar(ns: &NSWindow, mtm: MainThreadMarker) {
    if GRUPO.with(|grupo| grupo.borrow().is_some()) { return; }
    let Some(conteudo) = ns.contentView() else { return };
    let vista = Semaforos::novo(mtm);
    let mut botoes = Vec::new();
    let mut originais = Vec::new();
    for tipo in 0usize..3 {
        let original: Option<Retained<NSView>> = unsafe { msg_send![ns, standardWindowButton: tipo] };
        let Some(original) = original else { return };
        let botao: Option<Retained<NSView>> = unsafe {
            msg_send![class!(NSWindow), standardWindowButton: tipo, forStyleMask: ns.styleMask()]
        };
        let Some(botao) = botao else { return };
        let tamanho = botao.frame().size;
        botao.setFrameOrigin(NSPoint::new(14.0 + tipo as f64 * 20.0, (ALTURA - tamanho.height) / 2.0));
        unsafe {
            // A fábrica conserva as ações e tracking nativos, inclusive Option+verde/menus de
            // organização de janelas. Só define o destinatário exigido pela API pública da Apple.
            let _: () = msg_send![&*botao, setTarget: ns];
            let _: () = msg_send![&*botao, setAccessibilityElement: true];
            let _: () = msg_send![&*botao, setAccessibilityRole: ns_string!("AXButton")];
            let rotulo = match tipo { 0 => ns_string!("Fechar"), 1 => ns_string!("Minimizar"), _ => ns_string!("Zoom ou tela cheia") };
            let _: () = msg_send![&*botao, setAccessibilityLabel: rotulo];
        }
        vista.addSubview(&botao);
        botoes.push(botao);
        originais.push(original);
    }
    let margem = if conteudo.isFlipped() { NSAutoresizingMaskOptions::ViewMaxYMargin } else { NSAutoresizingMaskOptions::ViewMinYMargin };
    vista.setAutoresizingMask(margem | NSAutoresizingMaskOptions::ViewMaxXMargin);
    // Acima da WKWebView, sem criar outra janela/Space nem mudar o foco ou a identidade do app.
    unsafe { let _: () = msg_send![&*conteudo, addSubview: &*vista, positioned: 1isize, relativeTo: None::<&NSView>]; }
    GRUPO.with(|grupo| *grupo.borrow_mut() = Some(Grupo { vista, botoes, originais }));
    atualizar(ns, 0.0, false);
}

pub(super) fn atualizar(ns: &NSWindow, deslocamento: f64, animar: bool) {
    ocultar_originais(ns);
    GRUPO.with(|grupo| {
        let grupo = grupo.borrow();
        let Some(grupo) = grupo.as_ref() else { return };
        let Some(pai) = (unsafe { grupo.vista.superview() }) else { return };
        let y = if pai.isFlipped() { deslocamento } else { pai.bounds().size.height - ALTURA - deslocamento };
        let origem = NSPoint::new(0.0, y);
        if animar {
            unsafe {
                let animador: Retained<AnyObject> = msg_send![&*grupo.vista, animator];
                let _: () = msg_send![&*animador, setFrameOrigin: origem];
            }
        } else { grupo.vista.setFrameOrigin(origem); }
        // Recalcula também ao refocar/animar: o grupo pode se mover sob um cursor parado.
        let ponto: NSPoint = unsafe { msg_send![ns, mouseLocationOutsideOfEventStream] };
        let local: NSPoint = unsafe { msg_send![&*grupo.vista, convertPoint: ponto, fromView: None::<&NSView>] };
        let dentro = (0.0..LARGURA).contains(&local.x) && (0.0..ALTURA).contains(&local.y);
        grupo.vista.aplicar_cinza(!dentro);
        for (tipo, botao) in grupo.botoes.iter().enumerate() {
            let original: Option<Retained<AnyObject>> = unsafe { msg_send![ns, standardWindowButton: tipo] };
            let fonte: &AnyObject = original.as_deref().unwrap_or(&grupo.originais[tipo]);
            let permitido: bool = unsafe { msg_send![fonte, isEnabled] };
            // Minimizar em tela cheia é desabilitado pelo próprio macOS; não inventa outra ação.
            let ligado = permitido && !(tipo == 1 && ns.styleMask().contains(NSWindowStyleMask::FullScreen));
            unsafe { let _: () = msg_send![&**botao, setEnabled: ligado]; }
        }
    });
}
