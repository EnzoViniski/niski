import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PastasDeProjetos } from "../features/projects/PastasDeProjetos";
import { MarcaDaAba } from "../features/orquestracao/MarcaDaAba";
import { SessionProvidersContext } from "../features/terminal/sessionProviders";
import { _zerarResumo } from "../features/orquestracao/orquestracaoService";
import { transporteDoEngine } from "../features/conversa/transporteDoEngine";
import { usarTransporte } from "../mobile/api";
import type { Project } from "../types/workspace";
import type { TerminalSession } from "../features/terminal/terminalService";

// Fork: na lateral, os subagentes ficam embaixo da orquestradora (como subpasta); nas abas, ORQ/SUB.
const engine = vi.hoisted(() => ({ engineApi: vi.fn() }));
vi.mock("../features/terminal/terminalService", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../features/terminal/terminalService")>()),
  ...engine,
}));

const RESUMO = { limite: 3, orquestradores: [{ orquestrador: "c-orq", ativo: true, agentes: 2, subagentes: ["c-sub1", "c-sub2"] }] };

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  _zerarResumo();
  usarTransporte(transporteDoEngine);
  engine.engineApi.mockImplementation(async () => ({ type: "api", status: 200, body: JSON.stringify(RESUMO) }));
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

const projeto = (id: string): Project => ({ id, name: id, path: `/p/${id}` } as Project);
const sessao = (id: string): TerminalSession =>
  ({ id, conversation_id: `c-${id}`, project_id: "api", provider: "claude", name: `Conversa ${id}`, status: "running" } as TerminalSession);

function renderizar() {
  render(<PastasDeProjetos projects={[projeto("api")]} activeProjectId="api" activeSessionId={null}
    agentSessions={[sessao("sub1"), sessao("solta"), sessao("orq"), sessao("sub2")]} naoVistas={new Set()}
    onSelectProject={vi.fn()} onCloseProject={vi.fn()} onAbrirConversa={vi.fn()}
    onBifurcar={vi.fn()} onReiniciar={vi.fn()} onFechar={vi.fn()} />);
}
const linhas = () => within(screen.getByRole("list", { name: "Conversas de api" })).getAllByRole("listitem");

describe("orquestradora na lateral", () => {
  it("subagentes logo abaixo da orquestradora, recuados; ela segue como conversa normal", async () => {
    renderizar();
    await waitFor(() => expect(linhas().some((li) => li.classList.contains("is-subagente"))).toBe(true));
    const ordem = linhas().map((li) => li.textContent ?? "");
    const nomes = ordem.map((t) => /Conversa (orq|sub1|sub2|solta)/.exec(t)?.[1]);
    expect(nomes.indexOf("sub1")).toBe(nomes.indexOf("orq") + 1);
    expect(nomes.indexOf("sub2")).toBe(nomes.indexOf("orq") + 2);
    const sub = linhas()[nomes.indexOf("sub1")];
    expect(sub).toHaveClass("is-subagente");
    expect(sub).not.toHaveAttribute("data-reordenar");
    expect(linhas()[nomes.indexOf("sub2")]).toHaveClass("is-ultimo");
    expect(linhas()[nomes.indexOf("orq")]).not.toHaveClass("is-subagente");
    expect(screen.getByText("4")).toBeInTheDocument(); // a contagem da pasta continua com todas
  });

  it("recolher esconde os subagentes e fica lembrado", async () => {
    renderizar();
    await userEvent.click(await screen.findByRole("button", { name: "Esconder os subagentes de Conversa orq" }));
    expect(screen.queryByText("Conversa sub1")).not.toBeInTheDocument();
    expect(JSON.parse(localStorage.getItem("medza-orquestradoras-recolhidas") ?? "[]")).toEqual(["c-orq"]);
    await userEvent.click(screen.getByRole("button", { name: "Mostrar os subagentes de Conversa orq" }));
    expect(screen.getByText("Conversa sub1")).toBeInTheDocument();
  });
});

describe("marca na aba do layout", () => {
  it("ORQ na orquestradora, SUB no subagente, nada nas outras", async () => {
    const sessoes = { s1: sessao("orq"), s2: sessao("sub1"), s3: sessao("solta") };
    const { rerender } = render(<SessionProvidersContext.Provider value={sessoes}><MarcaDaAba sessionId="s1" /></SessionProvidersContext.Provider>);
    expect(await screen.findByText("ORQ")).toBeInTheDocument();
    rerender(<SessionProvidersContext.Provider value={sessoes}><MarcaDaAba sessionId="s2" /></SessionProvidersContext.Provider>);
    expect(screen.getByText("SUB")).toBeInTheDocument();
    rerender(<SessionProvidersContext.Provider value={sessoes}><MarcaDaAba sessionId="s3" /></SessionProvidersContext.Provider>);
    expect(screen.queryByText(/ORQ|SUB/)).not.toBeInTheDocument();
  });
});
