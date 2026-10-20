// Fork (Onda 2.0): os mapas que antes eram cópias por tela agora moram em `src/lib`. A tabela
// garante que as quatro plataformas e todos os estados estão em todo mapa, e trava os formatos.
import { describe, expect, it } from "vitest";
import { PLATAFORMAS, nomeDaPlataforma, nomeLongoDaPlataforma, plataforma } from "../lib/plataformasDeAgente";
import { corDoAgente, nomeDoAgente } from "../lib/coresDosAgentes";
import { haQuanto, haQuantoTempo } from "../lib/tempo";
import {
  ROTULO_DA_ATENCAO,
  ROTULO_DO_ESTADO,
  ROTULO_DO_ESTADO_DETALHADO,
  ROTULO_DO_ESTADO_NA_WEB,
  rotuloDoEstado,
  tomDoEstado,
} from "../lib/estadosDaSessao";
import { ATTENTION_LABEL, ATTENTION_ORDER } from "../features/terminal/useAttention";
import { timeAgo } from "../features/terminal/AgentUsageStatus";

const QUATRO = ["claude", "codex", "antigravity", "cursor"] as const;
const ESTADOS = ["working", "answered", "approval_required", "stopped", "orphan", "crashed"] as const;

describe("plataformas de agente", () => {
  it.each(QUATRO)("%s tem nome, nome longo e cor", (id) => {
    const info = PLATAFORMAS[id];
    expect(info.nome).toBeTruthy();
    expect(info.nomeLongo).toBeTruthy();
    expect(info.cor).toMatch(/^#[0-9a-f]{6}$/);
    expect(nomeDaPlataforma(id)).toBe(info.nome);
    expect(nomeLongoDaPlataforma(id)).toBe(info.nomeLongo);
    expect(corDoAgente(id)).toBe(info.cor);
    expect(nomeDoAgente(id)).toBe(info.nome);
  });

  it("só as quatro, com os nomes de sempre", () => {
    expect(Object.keys(PLATAFORMAS).sort()).toEqual([...QUATRO].sort());
    expect(QUATRO.map((id) => nomeDaPlataforma(id))).toEqual(["Claude", "Codex", "Antigravity", "Cursor"]);
    expect(nomeLongoDaPlataforma("claude")).toBe("Claude Code");
  });

  it("id desconhecido fica para a tela decidir", () => {
    for (const id of [undefined, null, "", "gemini", "constructor", "toString"]) {
      expect(plataforma(id)).toBeUndefined();
      expect(nomeDaPlataforma(id)).toBeUndefined();
    }
    expect(nomeDoAgente("gemini")).toBe("gemini");
    expect(nomeDoAgente(null)).toBe("Terminal");
    expect(corDoAgente("gemini")).toBe("rgb(var(--c-accent))");
  });
});

describe("estados da sessão", () => {
  it.each(ESTADOS)("%s tem rótulo em toda variante", (estado) => {
    for (const mapa of [ROTULO_DO_ESTADO, ROTULO_DO_ESTADO_NA_WEB, ROTULO_DO_ESTADO_DETALHADO]) {
      expect(mapa[estado]).toBeTruthy();
    }
    expect(rotuloDoEstado(estado)).toBe(ROTULO_DO_ESTADO[estado]);
    expect(rotuloDoEstado(estado, "web")).toBe(ROTULO_DO_ESTADO_NA_WEB[estado]);
    expect(rotuloDoEstado(estado, "detalhado")).toBe(ROTULO_DO_ESTADO_DETALHADO[estado]);
  });

  it("os textos de cada tela continuam os mesmos", () => {
    expect(rotuloDoEstado("answered")).toBe("Resposta disponível");
    expect(rotuloDoEstado("answered", "web")).toBe("Esperando você");
    expect(rotuloDoEstado("answered", "detalhado")).toBe("Pronta para a próxima mensagem");
    expect(rotuloDoEstado(null)).toBeUndefined();
    expect(rotuloDoEstado("constructor")).toBeUndefined();
  });

  it("tom do ponto de estado", () => {
    expect(tomDoEstado("working", true)).toBe("m-dot-alerta");
    expect(tomDoEstado("approval_required", false)).toBe("m-dot-alerta");
    expect(tomDoEstado("working", false)).toBe("m-dot-ativo");
    expect(tomDoEstado("answered", false)).toBe("m-dot-ok");
    expect(tomDoEstado("crashed", false)).toBe("m-dot-erro");
    expect(tomDoEstado(null, false)).toBe("m-dot-parado");
  });

  it("todo motivo de atenção tem rótulo, e o nome antigo é o mesmo mapa", () => {
    for (const motivo of ATTENTION_ORDER) expect(ROTULO_DA_ATENCAO[motivo]).toBeTruthy();
    expect(ATTENTION_LABEL).toBe(ROTULO_DA_ATENCAO);
  });
});

describe("há quanto tempo", () => {
  const agora = 10 * 24 * 3600_000;
  it.each([
    [0, "agora", "agora"],
    [59_000, "agora", "agora"],
    [5 * 60_000, "5 min", "há 5 min"],
    [3 * 3600_000, "3 h", "há 3 h"],
    [2 * 24 * 3600_000, "2 d", "há 2 d"],
  ])("%i ms atrás", (atras, curto, longo) => {
    expect(haQuantoTempo(agora - atras, agora)).toBe(curto);
    expect(haQuantoTempo(agora - atras, agora, "longo")).toBe(longo);
    expect(timeAgo(agora - atras, agora)).toBe(longo);
  });

  it("futuro conta como agora e sem data não mostra nada", () => {
    expect(haQuantoTempo(agora + 60_000, agora)).toBe("agora");
    expect(haQuanto(undefined)).toBeNull();
    expect(haQuanto(0)).toBeNull();
    expect(haQuanto(agora - 5 * 60_000, agora)).toBe("5 min");
  });
});
