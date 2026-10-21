import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { NaTela, arranjo } from "../features/projects/NaTela";
import { mapaDoLayout, type PainelNaTela } from "../features/projects/telasAbertas";
import type { LayoutNode } from "../types/workspace";

const pane = (id: string): LayoutNode => ({ type: "pane", id, tabs: [], activeTabId: null });

describe("Na tela: conversas da mesma tela juntas", () => {
  it("mapa do layout segue a divisão salva", () => {
    const ladoALado: LayoutNode = { type: "split", id: "s", direction: "horizontal", ratio: 0.6, first: pane("a"), second: pane("b") };
    expect(mapaDoLayout(ladoALado)).toEqual([
      { paneId: "a", posicao: 1, x: 0, y: 0, w: 0.6, h: 1 },
      { paneId: "b", posicao: 2, x: 0.6, y: 0, w: expect.closeTo(0.4, 5), h: 1 },
    ]);
    expect(arranjo(mapaDoLayout(ladoALado))).toBe("lado a lado");
    const empilhado: LayoutNode = { type: "split", id: "s", direction: "vertical", ratio: 0.5, first: pane("a"), second: pane("b") };
    expect(arranjo(mapaDoLayout(empilhado))).toBe("uma em cima da outra");
    const direita: LayoutNode = { type: "split", id: "d", direction: "vertical", ratio: 0.5, first: pane("b"), second: pane("c") };
    const misto: LayoutNode = { type: "split", id: "s", direction: "horizontal", ratio: 0.5, first: pane("a"), second: direita };
    expect(mapaDoLayout(misto).map((a) => [a.paneId, a.x, a.y])).toEqual([["a", 0, 0], ["b", 0.5, 0], ["c", 0.5, 0.5]]);
    expect(arranjo(mapaDoLayout(misto))).toBe("na mesma tela");
  });

  it("dois painéis do mesmo projeto viram um cartão de tela com número e 'agora'; outra tela fica separada", () => {
    const mapa = mapaDoLayout({ type: "split", id: "s", direction: "horizontal", ratio: 0.5, first: pane("p1"), second: pane("p2") });
    const base = { workspaceId: "w", workspace: "Medza", projectId: "omni", projeto: "omni", telaVisivel: true, mapaDaTela: mapa };
    const paineis: PainelNaTela[] = [
      { ...base, paneId: "p1", posicao: 1, emFoco: true, abas: [{ tabId: "t1", titulo: "Roadmap de estudo", sessionId: "s1", provider: "claude", ativa: true, kind: "agent" }] },
      { ...base, paneId: "p2", posicao: 2, emFoco: false, abas: [{ tabId: "t2", titulo: "Passagem de bastão", sessionId: "s2", provider: "codex", ativa: true, kind: "agent" }] },
      { workspaceId: "w", workspace: "Medza", projectId: "site", projeto: "site", telaVisivel: false, mapaDaTela: mapaDoLayout(pane("q")), paneId: "q", posicao: 1, emFoco: false,
        abas: [{ tabId: "t3", titulo: "Outra", sessionId: "s3", provider: "claude", ativa: false, kind: "agent" }] },
    ];
    render(<NaTela paineis={paineis} sessoes={[]} naoVistas={new Set()} onFocar={() => undefined} />);
    const tela = screen.getByRole("group", { name: "Tela omni · 2 painéis · à vista agora" });
    expect(within(tela).getByText("Roadmap de estudo")).toBeInTheDocument();
    expect(within(tela).getByText("Passagem de bastão")).toBeInTheDocument();
    expect(within(tela).getByText("2 abas lado a lado")).toBeInTheDocument();
    expect(within(tela).getByText("agora")).toBeInTheDocument();
    expect(tela.querySelectorAll(".na-tela-mapa rect")).toHaveLength(2);
    const outra = screen.getByRole("group", { name: "Tela site" });
    expect(within(outra).getByText("Outra")).toBeInTheDocument();
    expect(within(outra).queryByText("agora")).toBeNull();
  });
});
