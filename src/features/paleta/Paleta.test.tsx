import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Paleta } from "./Paleta";
import { AcoesDaPaleta, useAcoesDaPaleta } from "./acoes";
import { filtrarItens, moverSelecao, type ItemDaPaleta } from "./filtro";
import { buscarNoHistorico, listarConversas } from "./servico";
import { listAllFiles } from "../files/filesService";
import { setShortcutOverrides } from "../../lib/shortcuts";

vi.mock("./servico", () => ({ listarConversas: vi.fn(), buscarNoHistorico: vi.fn() }));
vi.mock("../files/filesService", async (original) => ({ ...await original<typeof import("../files/filesService")>(), resolveIgnoreList: vi.fn().mockResolvedValue([]), listAllFiles: vi.fn() }));
const executar = vi.fn();
function Registro({ titulo = "Novo agente" }: { titulo?: string }) {
  useAcoesDaPaleta("teste", [{ id: "novo", titulo, executar }]);
  return null;
}
const onConversa = vi.fn(), onProjeto = vi.fn(), onArquivo = vi.fn();
const props = { workspaces: [], projectPath: "/projeto", onConversa, onProjeto, onArquivo };
function montar() { return render(<AcoesDaPaleta><Registro /><Paleta {...props} /></AcoesDaPaleta>); }
function abrir() { fireEvent.keyDown(window, { key: "k", ctrlKey: true }); }
const conversa = { id: "conv-outro-projeto", title: "Refatorar navegação", project_id: "outro-projeto", provider: "codex", profile_id: null, state: null, capabilities: null };

beforeEach(() => {
  vi.clearAllMocks();
  setShortcutOverrides({});
  vi.mocked(listarConversas).mockResolvedValue([conversa]);
  vi.mocked(listAllFiles).mockResolvedValue([{ name: "App.tsx", path: "/projeto/src/App.tsx", isDirectory: false }]);
  vi.mocked(buscarNoHistorico).mockResolvedValue({ achados: [{ conversa_id: conversa.id, titulo: conversa.title, papel: "assistant", trecho: "Contexto navegação completo", data: "2026-10-19T12:00:00Z" }], parcial: false });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("filtro da paleta", () => {
  it("fuzzy ignora acentos e mantém seções, incluindo arquivos", () => {
    const itens: ItemDaPaleta[] = [
      { id: "a", titulo: "Configurações", secao: "Ações", executar },
      { id: "b", titulo: "configuracoes.ts", secao: "Arquivos", executar },
      { id: "c", titulo: "Outro", secao: "Projetos", executar },
    ];
    expect(filtrarItens(itens, "cfg").map((i) => i.id)).toEqual(["a", "b"]);
    expect(filtrarItens(itens, "configuracoes").map((i) => i.id)).toEqual(["a", "b"]);
    expect(filtrarItens(itens, "inexistente")).toEqual([]);
  });
  it("setas circulam e lista vazia não gera índice negativo", () => {
    expect(moverSelecao(0, "ArrowUp", 3)).toBe(2);
    expect(moverSelecao(2, "ArrowDown", 3)).toBe(0);
    expect(moverSelecao(0, "ArrowDown", 0)).toBe(0);
  });
});
describe("paleta", () => {
  it("atalho abre, filtro e Enter chamam o handler existente uma vez", async () => {
    montar(); abrir();
    const campo = screen.getByRole("combobox");
    fireEvent.change(campo, { target: { value: "nvag" } });
    fireEvent.keyDown(campo, { key: "Enter" });
    expect(executar).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  it("setas, Escape e reabrir reiniciam a busca", async () => {
    montar(); abrir();
    await screen.findByText(conversa.title);
    const campo = screen.getByRole("combobox");
    fireEvent.keyDown(campo, { key: "ArrowDown" });
    expect(screen.getByRole("option", { selected: true })).toHaveTextContent(conversa.title);
    fireEvent.keyDown(campo, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    abrir(); expect(screen.getByRole("combobox")).toHaveValue("");
    await screen.findByText(conversa.title);
  });
  it("abre conversa de outro projeto e arquivo por nome", async () => {
    montar(); abrir();
    fireEvent.click(await screen.findByText(conversa.title));
    expect(onConversa).toHaveBeenCalledWith(conversa);
    abrir();
    fireEvent.click(await screen.findByText("App.tsx"));
    expect(onArquivo).toHaveBeenCalledWith("/projeto/src/App.tsx");
  });
  it("? busca conteúdo, mostra papel/contexto/data e abre a conversa", async () => {
    montar(); abrir();
    await screen.findByText(conversa.title);
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "? navegação" } });
    await screen.findByText(/Contexto navegação completo/);
    expect(buscarNoHistorico).toHaveBeenCalledWith("navegação");
    expect(screen.getByText(/Agente ·/)).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });
    expect(onConversa).toHaveBeenCalledWith(conversa);
  });
  it("botão No histórico mantém o foco no campo para continuar digitando", () => {
    montar(); abrir();
    fireEvent.click(screen.getByRole("button", { name: "No histórico" }));
    expect(screen.getByRole("combobox")).toHaveValue("?");
    expect(document.activeElement).toBe(screen.getByRole("combobox"));
  });
  it("ignora resposta antiga depois de mudar a consulta", async () => {
    let resolver!: (r: Awaited<ReturnType<typeof buscarNoHistorico>>) => void;
    vi.mocked(buscarNoHistorico).mockImplementationOnce(() => new Promise((resolve) => { resolver = resolve; }));
    montar(); abrir();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "? antiga" } });
    await waitFor(() => expect(buscarNoHistorico).toHaveBeenCalled());
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "arquivo" } });
    await act(async () => resolver({ achados: [{ conversa_id: conversa.id, titulo: "Resultado velho", papel: "user", trecho: "velho", data: null }], parcial: false }));
    expect(screen.queryByText("Resultado velho")).not.toBeInTheDocument();
  });
  it("ações usam a versão atual do handler e removem registros ao desmontar", () => {
    const { rerender } = render(<AcoesDaPaleta><Registro /><Paleta {...props} /></AcoesDaPaleta>);
    rerender(<AcoesDaPaleta><Registro titulo="Novo terminal" /><Paleta {...props} /></AcoesDaPaleta>);
    abrir();
    expect(screen.getByText("Novo terminal")).toBeInTheDocument();
    expect(screen.queryByText("Novo agente")).not.toBeInTheDocument();
    rerender(<AcoesDaPaleta><Paleta {...props} /></AcoesDaPaleta>);
    expect(screen.queryByText("Novo terminal")).not.toBeInTheDocument();
  });
});
