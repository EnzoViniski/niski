import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { bauAberto, estrelas, metaDoDia, nivel, xp, xpDoNivel } from "../features/roadmaps/jogo";
import { normalizarCards, normalizarPlano, normalizarProgresso } from "../features/roadmaps/normalizar";
import { concluirEtapa } from "../features/roadmaps/progresso";
import { adicionarCards, responder } from "../features/roadmaps/revisao";
import { definirModoFoco, preferenciasNoFoco, URGENTES } from "../lib/modoFoco";
import { preferenciasDeAvisos } from "../features/atividade/tipos";
import { AttentionPanel } from "../components/AttentionPanel";
import { useLinksEmAba } from "../features/navegador/useLinksEmAba";
import type { AttentionItem } from "../features/terminal/useAttention";
import type { Plano } from "../features/roadmaps/tipos";

const plano = normalizarPlano({
  objetivo: "x", minutosPorDia: 15,
  modulos: [{ id: "m-1", titulo: "M", etapas: [
    { id: "e-a", titulo: "A", minutos: 20, tarefas: [{ id: "t-1", texto: "fazer" }] },
    { id: "e-b", titulo: "B", minutos: 30 },
  ] }],
}) as Plano;

afterEach(() => definirModoFoco(false));

describe("jogo", () => {
  it("nível sobe a cada faixa de XP (300, 450, 600…)", () => {
    expect(nivel(0)).toEqual({ nivel: 1, noNivel: 0, paraOProximo: 300, total: 0 });
    expect(nivel(299).nivel).toBe(1);
    expect(nivel(300)).toMatchObject({ nivel: 2, noNivel: 0, paraOProximo: 450 });
    expect(nivel(300 + 450 + 10)).toMatchObject({ nivel: 3, noNivel: 10 });
    expect(xpDoNivel(4)).toBe(750);
  });

  it("XP de etapas, tarefas e revisões; só o de hoje conta na meta", () => {
    const ontem = new Date(2026, 9, 20, 10), hoje = new Date(2026, 9, 21, 10);
    let progresso = concluirEtapa(normalizarProgresso(null), "e-a", ontem);
    progresso = concluirEtapa(progresso, "e-b", hoje);
    let { cards } = adicionarCards(normalizarCards(null), "e-b", [{ frente: "f", verso: "v" }], hoje);
    cards = responder(cards, cards.cards[0].id, 3, hoje);
    const comTarefa: Plano = { ...plano, modulos: [{ ...plano.modulos[0], etapas: plano.modulos[0].etapas.map((e) => ({ ...e, tarefas: e.tarefas.map((t) => ({ ...t, feita: true })) })) }] };
    expect(xp(comTarefa, progresso, cards, hoje)).toEqual({ total: 200 + 300 + 15 + 5, hoje: 300 + 5 });
    expect(metaDoDia(plano)).toBe(150);
    expect(metaDoDia(null)).toBe(250);
  });

  it("estrelas: concluir, prática feita, cards revisados; baú abre com a região completa", () => {
    const agora = new Date(2026, 9, 21);
    const [a, b] = plano.modulos[0].etapas;
    let progresso = normalizarProgresso(null);
    const vazio = normalizarCards(null);
    expect(estrelas(a, progresso, vazio)).toBe(0);
    progresso = concluirEtapa(progresso, a.id, agora);
    expect(estrelas(a, progresso, vazio)).toBe(1);
    const feita = { ...a, tarefas: a.tarefas.map((t) => ({ ...t, feita: true })) };
    expect(estrelas(feita, progresso, vazio)).toBe(3);
    let { cards } = adicionarCards(vazio, a.id, [{ frente: "f", verso: "v" }], agora);
    expect(estrelas(feita, progresso, cards)).toBe(2);
    cards = responder(cards, cards.cards[0].id, 3, agora);
    expect(estrelas(feita, progresso, cards)).toBe(3);
    expect(bauAberto(plano.modulos[0], progresso)).toBe(false);
    expect(bauAberto(plano.modulos[0], concluirEtapa(progresso, b.id, agora))).toBe(true);
  });
});

describe("modo foco", () => {
  it("só os urgentes avisam (mesmo com a janela em foco); desligado, nada muda", () => {
    const normais = preferenciasDeAvisos();
    expect(preferenciasNoFoco(normais)).toBe(normais);
    definirModoFoco(true);
    const foco = preferenciasNoFoco(normais);
    for (const [tipo, p] of Object.entries(foco)) {
      expect(p.quando).toBe(URGENTES.includes(tipo as never) ? "sempre" : "nunca");
      expect(p.som).toBe(normais[tipo as keyof typeof normais].som);
    }
    expect(foco.precisa_de_voce.quando).toBe("sempre");
    expect(foco.terminou.quando).toBe("nunca");
  });
});

describe("Atenção: limpar tudo", () => {
  const item = (n: number): AttentionItem => ({
    sessionId: `s-${n}`, conversationKey: `c-${n}`, sessionName: `Conversa ${n}`, reason: "answered", state: "answered" as AttentionItem["state"],
    attentionSeq: 1, projectId: "p", projectName: "Projeto", workspaceId: "w", workspaceName: "W",
  });

  it("dispensa todos os avisos de uma vez", () => {
    function Painel() {
      const [itens, setItens] = useState([item(1), item(2), item(3)]);
      return <AttentionPanel items={itens} onFocusSession={() => undefined} onDismiss={(i) => setItens((atual) => atual.filter((x) => x.sessionId !== i.sessionId))} />;
    }
    render(<Painel />);
    expect(screen.getByText("Conversa 3")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Limpar tudo" }));
    expect(screen.queryByText("Conversa 1")).toBeNull();
    expect(screen.queryByText("Atenção")).toBeNull();
  });
});

describe("links do roadmap", () => {
  it("o App não leva para a aba de navegador o link dentro de [data-links-proprios]", () => {
    const dispatch = vi.fn();
    function Tela() {
      useLinksEmAba([], dispatch, () => undefined);
      return <>
        <div data-links-proprios onClick={(e) => e.preventDefault()}><a href="https://exemplo.com/estudo">no roadmap</a></div>
        <a href="https://exemplo.com/fora">fora</a>
      </>;
    }
    render(<Tela />);
    fireEvent.click(screen.getByText("no roadmap"));
    expect(dispatch).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("fora"));
    expect(dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "ABRIR_LINK", url: "https://exemplo.com/fora" }));
  });
});
