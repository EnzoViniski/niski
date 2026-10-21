// Fork (Sobre mim): tela, salvamento automático, sugestões dos agentes e rota do celular.
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { SobreMimDoDesktop } from "../features/sobreMim/SobreMimDoDesktop";
import { promptParaOrganizar, type EstadoDoSobreMim } from "../features/sobreMim/apiDoSobreMim";
import { engineApi } from "../features/terminal/terminalService";
import { lerRota, urlDaRota } from "../mobile/rota";

vi.mock("../features/terminal/terminalService", () => ({ engineApi: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => undefined) }));

const AREAS = [
  ["quem-sou", "Quem sou"], ["vida-pessoal", "Vida pessoal"], ["trabalho", "Empresas e trabalho"],
  ["estudos", "Estudos"], ["preferencias", "Como trabalhar comigo"], ["outros", "Outros"],
] as const;

let servidor: EstadoDoSobreMim;

function estadoInicial(): EstadoDoSobreMim {
  return {
    pasta: "/dados/sobre-mim",
    ajustes: { ativo: true, sugestoes: true },
    areas: AREAS.map(([id, titulo]) => ({ id, titulo, descricao: `Descrição de ${titulo}`, arquivo: `/dados/sobre-mim/${id}.md`, texto: id === "quem-sou" ? "Perfil sintético" : "" })),
    propostas: [{ linha: "- [estudos] Cursa Administração (Codex, 2026-10-21)", area: "estudos", texto: "Cursa Administração", origem: "Codex, 2026-10-21" },
      { linha: "- [outros] Fato a descartar", area: "outros", texto: "Fato a descartar", origem: null }],
    max_por_area: 10_000,
  };
}

beforeEach(() => {
  servidor = estadoInicial();
  vi.mocked(engineApi).mockReset().mockImplementation(async (method, path, body) => {
    const corpo = body ? JSON.parse(body) as Record<string, string> : {};
    if (method === "POST" && path.startsWith("/sobre-mim/areas/")) {
      const id = decodeURIComponent(path.split("/").pop()!);
      servidor = { ...servidor, areas: servidor.areas.map((a) => a.id === id ? { ...a, texto: corpo.texto } : a) };
    } else if (path === "/sobre-mim/propostas/aceitar") {
      servidor = {
        ...servidor,
        areas: servidor.areas.map((a) => a.id === corpo.area ? { ...a, texto: `${a.texto}- ${corpo.texto}\n` } : a),
        propostas: servidor.propostas.filter((p) => p.linha !== corpo.linha),
      };
    } else if (path === "/sobre-mim/propostas/descartar") {
      servidor = { ...servidor, propostas: servidor.propostas.filter((p) => p.linha !== corpo.linha) };
    } else if (path === "/sobre-mim/ajustes") {
      servidor = { ...servidor, ajustes: corpo as unknown as EstadoDoSobreMim["ajustes"] };
    }
    return { type: "api", status: 200, body: JSON.stringify(servidor) };
  });
});

it("mostra as seis áreas e salva sozinho o que foi digitado", async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  try {
    render(<SobreMimDoDesktop />);
    expect(await screen.findByLabelText("Quem sou")).toHaveValue("Perfil sintético");
    for (const [, titulo] of AREAS) expect(screen.getByLabelText(titulo)).toBeInTheDocument();
    expect(screen.getByText(/recebem este texto/)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Empresas e trabalho"), { target: { value: "Sócio da Medza" } });
    expect(engineApi).toHaveBeenCalledTimes(1);
    await act(async () => { vi.advanceTimersByTime(1_000); });
    await waitFor(() => expect(engineApi).toHaveBeenLastCalledWith("POST", "/sobre-mim/areas/trabalho", JSON.stringify({ texto: "Sócio da Medza" })));
    expect(await screen.findByText("Salvo")).toBeInTheDocument();
    expect(screen.getByLabelText("Empresas e trabalho")).toHaveValue("Sócio da Medza");
  } finally { vi.useRealTimers(); }
});

it("sair do campo grava na hora e o erro mantém o texto digitado", async () => {
  render(<SobreMimDoDesktop />);
  const campo = await screen.findByLabelText("Estudos");
  vi.mocked(engineApi).mockResolvedValueOnce({ type: "api", status: 400, body: JSON.stringify({ error: "Use até 10000 caracteres." }) });
  fireEvent.change(campo, { target: { value: "Colégio sintético" } });
  fireEvent.blur(campo);
  expect(await screen.findByText(/Não salvou: Use até 10000 caracteres/)).toBeInTheDocument();
  expect(campo).toHaveValue("Colégio sintético");
});

it("aceita sugestão editada na área escolhida e descarta outra sem gravar em área nenhuma", async () => {
  render(<SobreMimDoDesktop />);
  const lista = await screen.findByRole("list");
  const itens = within(lista).getAllByRole("listitem");
  expect(itens).toHaveLength(2);
  expect(within(itens[0]).getByText("Sugerido por Codex, 2026-10-21")).toBeInTheDocument();
  fireEvent.change(within(itens[0]).getByLabelText("Texto da sugestão"), { target: { value: "Cursa Administração na Y" } });
  fireEvent.change(within(itens[0]).getByLabelText("Área da sugestão"), { target: { value: "trabalho" } });
  fireEvent.click(within(itens[0]).getByRole("button", { name: "Aceitar" }));
  await screen.findByText("Sugestão aceita.");
  expect(engineApi).toHaveBeenLastCalledWith("POST", "/sobre-mim/propostas/aceitar",
    JSON.stringify({ linha: "- [estudos] Cursa Administração (Codex, 2026-10-21)", area: "trabalho", texto: "Cursa Administração na Y" }));
  expect(screen.getByLabelText("Empresas e trabalho")).toHaveValue("- Cursa Administração na Y\n");
  fireEvent.click(within(screen.getByRole("list")).getByRole("button", { name: "Descartar" }));
  await screen.findByText("Sugestão descartada.");
  expect(engineApi).toHaveBeenLastCalledWith("POST", "/sobre-mim/propostas/descartar", JSON.stringify({ linha: "- [outros] Fato a descartar" }));
  expect(screen.queryByRole("list")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Outros")).toHaveValue("");
});

it("desligar o compartilhamento grava o ajuste", async () => {
  render(<SobreMimDoDesktop />);
  fireEvent.click(await screen.findByLabelText("Compartilhar com meus agentes"));
  await screen.findByText(/Ajuste salvo/);
  expect(engineApi).toHaveBeenLastCalledWith("POST", "/sobre-mim/ajustes", JSON.stringify({ ativo: false, sugestoes: true }));
  expect(screen.getByLabelText(/sugerirem/)).toBeDisabled();
});

it("não oferece campos vazios por cima de um Sobre mim que falhou ao carregar", async () => {
  vi.mocked(engineApi).mockResolvedValue({ type: "api", status: 500, body: JSON.stringify({ error: "Não foi possível ler o Sobre mim." }) });
  render(<SobreMimDoDesktop />);
  expect(await screen.findByRole("alert")).toHaveTextContent("Não foi possível ler o Sobre mim.");
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
});

it("o pedido de organizar aponta a pasta e não deixa o agente gravar", () => {
  const prompt = promptParaOrganizar("/dados/sobre-mim");
  expect(prompt).toContain("/dados/sobre-mim");
  expect(prompt).toContain("Não grave nenhum arquivo");
});

it("o celular tem rota própria para o Sobre mim", () => {
  expect(lerRota("#/sobre-mim")).toEqual({ tela: "sobre-mim" });
  expect(urlDaRota({ tela: "sobre-mim" })).toMatch(/#\/sobre-mim$/);
});
