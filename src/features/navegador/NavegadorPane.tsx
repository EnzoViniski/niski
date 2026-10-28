import { useEffect, useRef, useState } from "react";
import { GlobeIcon, ReloadIcon } from "../../components/ui/PixelIcon";
import type { WorkspaceTab } from "../../types/workspace";
import { TAB_DRAG_EVENT, type TabDragState } from "../workspace/tabPointerDrag";
import { marcarDesenhada } from "./registroDoNavegador";

/**
 * Aba "Navegador". A página é um webview nativo filho da janela (`src-tauri/src/navegador.rs`),
 * desenhado por cima do retângulo `area` deste componente. Aqui só ficam a barra (voltar, avançar,
 * recarregar, endereço, abrir fora) e a sincronia de posição:
 *
 * - O webview fica **por cima** do HTML. Por isso some quando a aba sai da tela (desmonta), durante
 *   um arraste (cobriria a prévia do layout) e quando a área fica sem tamanho.
 * - A posição é relida a cada 200 ms e em cada redimensionamento: painel que muda de lugar sem mudar
 *   de tamanho (outro painel fechou) não dispara `ResizeObserver`.
 */
const NO_PC = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function nativo<T = void>(comando: string, args: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(comando, args);
}

export const rotuloDoNavegador = (tabId: string) => `nav-${tabId.replace(/[^\w-]/g, "_")}`;

export function NavegadorPane({ tab, onTitulo }: { tab: WorkspaceTab; onTitulo?: (titulo: string) => void }) {
  const area = useRef<HTMLDivElement>(null);
  const rotulo = rotuloDoNavegador(tab.id);
  const inicial = tab.resourceId ?? "about:blank";
  const [endereco, setEndereco] = useState(inicial);
  const [digitando, setDigitando] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const tituloRef = useRef(onTitulo);
  tituloRef.current = onTitulo;

  useEffect(() => {
    if (!NO_PC) return;
    // A conferência do `useLinksEmAba` só deixa à vista a página de quem está montado.
    const desmarcar = marcarDesenhada(rotulo);
    let vivo = true;
    let aberto = false;
    let arrastando = false;
    let ultimo = "";
    const sincronizar = () => {
      const caixa = area.current?.getBoundingClientRect();
      // Área escondida por CSS (`visibility`/`display` de quem está em volta) também tira a página da tela.
      const aVista = !area.current?.checkVisibility || area.current.checkVisibility({ visibilityProperty: true });
      const visivel = Boolean(caixa && caixa.width > 4 && caixa.height > 4 && aVista && !arrastando && !document.body.classList.contains("lateral-arrastando"));
      const chave = caixa ? `${visivel}|${Math.round(caixa.left)}|${Math.round(caixa.top)}|${Math.round(caixa.width)}|${Math.round(caixa.height)}` : "oculto";
      if (chave === ultimo || !caixa) return;
      ultimo = chave;
      const pos = { rotulo, x: caixa.left, y: caixa.top, largura: caixa.width, altura: caixa.height };
      if (!aberto && visivel) {
        aberto = true;
        nativo("navegador_abrir", { ...pos, url: inicial }).catch((motivo) => { if (vivo) setErro(String(motivo)); });
      } else if (aberto) {
        void nativo("navegador_posicionar", { ...pos, visivel }).catch(() => undefined);
      }
    };
    const aoArrastar = (evento: Event) => { arrastando = Boolean((evento as CustomEvent<TabDragState>).detail); sincronizar(); };
    const observador = new ResizeObserver(sincronizar);
    if (area.current) observador.observe(area.current);
    const timer = window.setInterval(sincronizar, 200);
    window.addEventListener("resize", sincronizar);
    window.addEventListener(TAB_DRAG_EVENT, aoArrastar);
    sincronizar();

    let desligar: Array<() => void> = [];
    void import("@tauri-apps/api/event").then(async ({ listen }) => {
      const a = await listen<{ rotulo: string; url: string }>("navegador-endereco", (e) => { if (e.payload.rotulo === rotulo) setEndereco(e.payload.url); });
      const b = await listen<{ rotulo: string; titulo: string }>("navegador-titulo", (e) => { if (e.payload.rotulo === rotulo && e.payload.titulo) tituloRef.current?.(e.payload.titulo); });
      if (vivo) desligar = [a, b]; else { a(); b(); }
    }).catch(() => undefined);

    return () => {
      vivo = false;
      desmarcar();
      observador.disconnect();
      window.clearInterval(timer);
      window.removeEventListener("resize", sincronizar);
      window.removeEventListener(TAB_DRAG_EVENT, aoArrastar);
      desligar.forEach((parar) => parar());
      // Aba trocada ou fora da tela: esconde (o fechar de verdade é do App, quando a aba some).
      if (aberto) void nativo("navegador_posicionar", { rotulo, x: 0, y: 0, largura: 1, altura: 1, visivel: false }).catch(() => undefined);
    };
  }, [rotulo, inicial]);

  const acao = (nome: string, url?: string) => { setErro(null); void nativo("navegador_acao", { rotulo, acao: nome, url }).catch((m) => setErro(String(m))); };

  function ir(texto: string) {
    const limpo = texto.trim();
    if (!limpo) return;
    const url = /^https?:\/\//i.test(limpo) ? limpo : `https://${limpo}`;
    setDigitando(null);
    setEndereco(url);
    acao("ir", url);
  }

  return (
    <div className="flex h-full flex-col">
      <div className="navegador-barra">
        <button type="button" className="navegador-botao" aria-label="Voltar" title="Voltar" onClick={() => acao("voltar")}>←</button>
        <button type="button" className="navegador-botao" aria-label="Avançar" title="Avançar" onClick={() => acao("avancar")}>→</button>
        <button type="button" className="navegador-botao" aria-label="Recarregar" title="Recarregar" onClick={() => acao("recarregar")}><ReloadIcon aria-hidden className="h-3 w-3" /></button>
        <form className="min-w-0 flex-1" onSubmit={(e) => { e.preventDefault(); ir(digitando ?? endereco); }}>
          <input className="navegador-endereco" aria-label="Endereço" value={digitando ?? endereco} spellCheck={false}
            onFocus={(e) => e.currentTarget.select()} onChange={(e) => setDigitando(e.target.value)} onBlur={() => setDigitando(null)} />
        </form>
        <button type="button" className="navegador-botao" title="Abrir no navegador do Mac" aria-label="Abrir no navegador do Mac"
          onClick={() => void nativo("navegador_abrir_fora", { url: endereco }).catch((m) => setErro(String(m)))}>
          <GlobeIcon aria-hidden className="h-3.5 w-3.5" />
        </button>
      </div>
      {erro && <p role="alert" className="px-3 py-1 text-[11px] text-danger">{erro}</p>}
      <div ref={area} className="relative min-h-0 flex-1 bg-white/5">
        {!NO_PC && <p className="p-4 text-xs text-text-muted">O navegador só funciona no app do computador.</p>}
      </div>
    </div>
  );
}
