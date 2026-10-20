import { act, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { NavegadorPane } from "../features/navegador/NavegadorPane";
import { TAB_DRAG_EVENT } from "../features/workspace/tabPointerDrag";

// A capacidade é decidida ao importar o módulo, como no app Tauri.
vi.hoisted(() => { Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} }); });
type EventoNativo = { payload: { rotulo: string; url?: string; titulo?: string } };
let eventos: Record<string, (evento: EventoNativo) => void>;
let desligar: ReturnType<typeof vi.fn>;
const tab = { id: "pagina", kind: "web" as const, title: "Página", resourceId: "https://example.test" };
beforeEach(() => {
  eventos = {};
  desligar = vi.fn();
  vi.mocked(invoke).mockReset().mockResolvedValue(undefined);
  vi.mocked(listen).mockReset().mockImplementation(async (nome, callback) => {
    eventos[String(nome)] = callback as unknown as (evento: EventoNativo) => void;
    return desligar;
  });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ x: 10, y: 20, left: 10, top: 20, right: 810, bottom: 620, width: 800, height: 600, toJSON: () => ({}) });
});
afterEach(() => { vi.restoreAllMocks(); });
afterAll(() => { Reflect.deleteProperty(window, "__TAURI_INTERNALS__"); });

it("abre o webview na área visível e o esconde ao sair da aba", async () => {
  const { unmount } = render(<NavegadorPane tab={tab} />);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("navegador_abrir", { rotulo: "nav-pagina", x: 10, y: 20, largura: 800, altura: 600, url: tab.resourceId }));
  await waitFor(() => expect(eventos["navegador-titulo"]).toBeDefined());
  expect(screen.queryByText(/só funciona no app/)).not.toBeInTheDocument();
  unmount();
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("navegador_posicionar", expect.objectContaining({ rotulo: "nav-pagina", visivel: false })));
  expect(desligar).toHaveBeenCalledTimes(2);
});
it("acompanha endereço e título recebidos apenas para esta aba", async () => {
  const onTitulo = vi.fn();
  render(<NavegadorPane tab={tab} onTitulo={onTitulo} />);
  await waitFor(() => expect(eventos["navegador-titulo"]).toBeDefined());
  act(() => {
    eventos["navegador-endereco"]({ payload: { rotulo: "outra-aba", url: "https://other.test" } });
    eventos["navegador-titulo"]({ payload: { rotulo: "outra-aba", titulo: "Outra" } });
  });
  expect(screen.getByRole("textbox", { name: "Endereço" })).toHaveValue(tab.resourceId);
  expect(onTitulo).not.toHaveBeenCalled();
  act(() => {
    eventos["navegador-endereco"]({ payload: { rotulo: "nav-pagina", url: "https://example.test/ajuda" } });
    eventos["navegador-titulo"]({ payload: { rotulo: "nav-pagina", titulo: "Ajuda" } });
  });
  expect(screen.getByRole("textbox", { name: "Endereço" })).toHaveValue("https://example.test/ajuda");
  expect(onTitulo).toHaveBeenCalledWith("Ajuda");
});
it("esconde a página durante o arraste e volta a mostrá-la ao terminar", async () => {
  render(<NavegadorPane tab={tab} />);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("navegador_abrir", expect.anything()));
  act(() => { window.dispatchEvent(new CustomEvent(TAB_DRAG_EVENT, { detail: { tabId: "pagina" } })); });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("navegador_posicionar", expect.objectContaining({ visivel: false })));
  act(() => { window.dispatchEvent(new CustomEvent(TAB_DRAG_EVENT, { detail: null })); });
  await waitFor(() => expect(invoke).toHaveBeenLastCalledWith("navegador_posicionar", expect.objectContaining({ visivel: true })));
});
