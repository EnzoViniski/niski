import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { PastasDeProjetos } from "../features/projects/PastasDeProjetos";
import { mover, ordenar } from "../features/projects/ordemDaLateral";
import type { Project } from "../types/workspace";
import type { TerminalSession } from "../features/terminal/terminalService";

// O jsdom não tem PointerEvent: sem isto o `clientY` do evento sumia.
if (typeof window.PointerEvent === "undefined") {
  class PointerEventFalso extends MouseEvent { constructor(tipo: string, init?: PointerEventInit) { super(tipo, init); } }
  (window as unknown as { PointerEvent: typeof MouseEvent }).PointerEvent = PointerEventFalso;
}

// Reordenar projetos e conversas da lateral, arrastando ou com ⌥↑/⌥↓.
describe("ordem da lateral", () => {
  it("ordem salva vem primeiro; item novo vai para o fim na ordem natural", () => {
    expect(ordenar(["a", "b", "c", "d"], (x) => x, ["c", "a"])).toEqual(["c", "a", "b", "d"]);
  });
  it("mover para antes de uma posição, para cima, para baixo e para o fim", () => {
    expect(mover(["a", "b", "c", "d"], "d", 0)).toEqual(["d", "a", "b", "c"]);
    expect(mover(["a", "b", "c", "d"], "a", 2)).toEqual(["b", "a", "c", "d"]);
    expect(mover(["a", "b", "c", "d"], "b", 4)).toEqual(["a", "c", "d", "b"]);
    expect(mover(["a", "b"], "a", -1)).toEqual(["a", "b"]);
  });
});

const projeto = (id: string): Project => ({ id, name: id, path: `/p/${id}` } as Project);
const sessao = (id: string, projectId: string): TerminalSession =>
  ({ id, conversation_id: `c-${id}`, project_id: projectId, provider: "claude", name: `Conversa ${id}`, status: "running" } as TerminalSession);

function renderizar() {
  const onAbrirConversa = vi.fn();
  const onSoltarNoLayout = vi.fn();
  render(<PastasDeProjetos projects={[projeto("api"), projeto("web"), projeto("docs")]} activeProjectId="api" activeSessionId={null}
    agentSessions={[sessao("s1", "api"), sessao("s2", "api"), sessao("s3", "api"), sessao("w1", "web")]} naoVistas={new Set()}
    onSelectProject={vi.fn()} onCloseProject={vi.fn()} onAbrirConversa={onAbrirConversa}
    onBifurcar={vi.fn()} onReiniciar={vi.fn()} onFechar={vi.fn()} onSoltarNoLayout={onSoltarNoLayout} />);
  return { onAbrirConversa, onSoltarNoLayout };
}

const nomesDosProjetos = () => within(screen.getByRole("list", { name: "Projetos" })).getAllByRole("button", { expanded: undefined })
  .filter((b) => b.hasAttribute("aria-expanded")).map((b) => b.textContent);
const nomesDasConversas = () => within(screen.getByRole("list", { name: "Conversas de api" })).getAllByRole("listitem")
  // Fork (lateral fluida): a linha tem nome e, embaixo, agente · estado · quando; a ordem é a dos nomes.
  .map((li) => li.querySelector(".conversa-nome")?.textContent ?? li.textContent);

/** Cada linha com 20px de altura, uma embaixo da outra. */
function alturas(lista: HTMLElement) {
  Array.from(lista.children).forEach((li, i) => {
    const caixa = { top: i * 20, height: 20, bottom: i * 20 + 20, left: 0, right: 100, width: 100, x: 0, y: i * 20, toJSON: () => ({}) };
    ((li.firstElementChild ?? li) as HTMLElement).getBoundingClientRect = () => caixa as DOMRect;
  });
}

describe("arrastar na lateral", () => {
  beforeEach(() => { localStorage.clear(); });

  it("arrastar uma conversa para o topo muda a ordem, não abre a conversa e fica salvo", () => {
    const { onAbrirConversa } = renderizar();
    const lista = screen.getByRole("list", { name: "Conversas de api" });
    alturas(lista);
    const terceira = within(lista).getByText("Conversa s3");
    fireEvent.pointerDown(terceira, { button: 0, clientX: 10, clientY: 45 });
    fireEvent.pointerMove(window, { clientX: 10, clientY: 5 });
    fireEvent.pointerUp(window, { clientX: 10, clientY: 5 });
    fireEvent.click(terceira);
    expect(onAbrirConversa).not.toHaveBeenCalled();
    expect(nomesDasConversas()).toEqual(["Conversa s3", "Conversa s1", "Conversa s2"]);
    expect(JSON.parse(localStorage.getItem("omni-ordem-lateral")!).conversas.api).toEqual(["c-s3", "c-s1", "c-s2"]);
  });

  it("clique sem arrastar continua abrindo a conversa", () => {
    const { onAbrirConversa } = renderizar();
    const primeira = screen.getByText("Conversa s1");
    fireEvent.pointerDown(primeira, { button: 0, clientX: 10, clientY: 5 });
    fireEvent.pointerUp(window, { clientX: 11, clientY: 6 });
    fireEvent.click(primeira);
    expect(onAbrirConversa).toHaveBeenCalledTimes(1);
  });

  it("⌥↓ desce um projeto e a ordem sobrevive a abrir de novo", () => {
    renderizar();
    expect(nomesDosProjetos()[0]).toContain("api");
    fireEvent.keyDown(screen.getByTitle(/^\/p\/api/), { key: "ArrowDown", altKey: true });
    expect(nomesDosProjetos().map((n) => n?.replace(/\d+$/, ""))).toEqual(["web", "api", "docs"]);
  });
});

describe("prévia do arraste e soltar no layout", () => {
  // O clique engolido depois de soltar é desarmado num `setTimeout(0)`: espera ele entre os testes.
  beforeEach(async () => { localStorage.clear(); await new Promise((pronto) => setTimeout(pronto, 0)); });

  function arrastarTerceira() {
    const lista = screen.getByRole("list", { name: "Conversas de api" });
    alturas(lista);
    const terceira = within(lista).getByText("Conversa s3");
    fireEvent.pointerDown(terceira, { button: 0, clientX: 10, clientY: 45 });
    return { lista, terceira };
  }

  it("a linha acompanha o mouse e a lista já mostra a ordem nova antes de soltar", () => {
    renderizar();
    const { lista } = arrastarTerceira();
    fireEvent.pointerMove(window, { clientX: 10, clientY: 5 });
    const fantasma = document.querySelector<HTMLElement>(".lateral-fantasma");
    expect(fantasma?.textContent).toContain("Conversa s3");
    expect(fantasma?.getAttribute("aria-hidden")).toBe("true");
    expect(fantasma?.style.top).toBe("-40px"); // segurou 5 px abaixo do topo da linha (45 - 40)
    expect(nomesDasConversas()).toEqual(["Conversa s3", "Conversa s1", "Conversa s2"]);
    expect(within(lista).getAllByRole("listitem")[0].className).toContain("is-arrastando");
    fireEvent.pointerUp(window, { clientX: 10, clientY: 5 });
    expect(document.querySelector(".lateral-fantasma")).toBeNull();
    expect(within(lista).getAllByRole("listitem")[0].className).not.toContain("is-arrastando");
  });

  it("Esc cancela: a ordem volta e nada é salvo", () => {
    renderizar();
    arrastarTerceira();
    fireEvent.pointerMove(window, { clientX: 10, clientY: 5 });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(nomesDasConversas()).toEqual(["Conversa s1", "Conversa s2", "Conversa s3"]);
    expect(localStorage.getItem("omni-ordem-lateral")).toBeNull();
    expect(document.querySelector(".lateral-fantasma")).toBeNull();
  });

  it("solta num painel do layout: zona da borda vira divisão, a ordem da lateral não muda", () => {
    const painel = document.createElement("section");
    painel.dataset.dropPane = "pane-1";
    painel.getBoundingClientRect = () => ({ left: 300, top: 0, width: 400, height: 400, right: 700, bottom: 400, x: 300, y: 0, toJSON: () => ({}) }) as DOMRect;
    document.body.appendChild(painel);
    const original = document.elementFromPoint;
    document.elementFromPoint = (x: number) => (x >= 300 ? painel : null);
    const avisos: unknown[] = [];
    const ouvir = (e: Event) => avisos.push((e as CustomEvent).detail);
    window.addEventListener("omni-tab-drag", ouvir);
    try {
      const { onSoltarNoLayout } = renderizar();
      arrastarTerceira();
      fireEvent.pointerMove(window, { clientX: 680, clientY: 200 });
      expect(document.querySelector(".lateral-fantasma")?.classList.contains("is-sobre-layout")).toBe(true);
      expect(avisos.at(-1)).toEqual({ tab: { paneId: "lateral", tabId: "s3" }, target: { paneId: "pane-1", zone: "right" } });
      expect(nomesDasConversas()).toEqual(["Conversa s1", "Conversa s2", "Conversa s3"]);
      fireEvent.pointerUp(window, { clientX: 680, clientY: 200 });
      expect(onSoltarNoLayout).toHaveBeenCalledWith("api", expect.objectContaining({ id: "s3" }), { paneId: "pane-1", zone: "right" });
      expect(avisos.at(-1)).toBeNull();
      expect(localStorage.getItem("omni-ordem-lateral")).toBeNull();
    } finally {
      document.elementFromPoint = original;
      window.removeEventListener("omni-tab-drag", ouvir);
      painel.remove();
    }
  });

  // Conversa de qualquer projeto pode ir para o layout na tela (a aba mostra de qual projeto é).
  it("conversa de um projeto que não está aberto também vai para o layout", () => {
    const original = document.elementFromPoint;
    const painel = document.createElement("section");
    painel.dataset.dropPane = "pane-1";
    document.body.appendChild(painel);
    document.elementFromPoint = () => painel;
    try {
      const { onSoltarNoLayout } = renderizar();
      // "web" não é o projeto ativo; abre a pasta dele para arrastar a conversa.
      fireEvent.click(screen.getByTitle(/^\/p\/web/));
      const conversa = screen.getByText("Conversa w1");
      fireEvent.pointerDown(conversa, { button: 0, clientX: 10, clientY: 5 });
      fireEvent.pointerMove(window, { clientX: 600, clientY: 100 });
      fireEvent.pointerUp(window, { clientX: 600, clientY: 100 });
      expect(onSoltarNoLayout).toHaveBeenCalledWith("web", expect.objectContaining({ id: "w1" }), expect.objectContaining({ paneId: "pane-1" }));
    } finally {
      document.elementFromPoint = original;
      painel.remove();
    }
  });
});
