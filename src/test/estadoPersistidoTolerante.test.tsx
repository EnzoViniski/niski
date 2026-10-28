import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useTodos } from "../features/todo/TodoList";
import { ajustesDoAgente, contaDoAgente, lembrarAgente, ultimoAgente } from "../lib/escolhasSalvas";
import { loadSettings } from "../services/settingsService";
import { DEFAULT_SETTINGS } from "../types/settings";

// Os outros estados salvos seguem a mesma regra do workspace — uma parte estragada cai sozinha.
beforeEach(() => localStorage.clear());

describe("configurações salvas", () => {
  it("campo com tipo errado volta ao padrão sem levar os outros junto", () => {
    localStorage.setItem(
      "omni-agents-settings",
      JSON.stringify({ theme: "claro", avisos: null, shortcuts: [], backgroundOpacity: "70", notifyAttention: false, campoDoFuturo: { x: 1 } })
    );
    const lidas = loadSettings() as unknown as Record<string, unknown>;
    expect(lidas.theme).toBe("claro");
    expect(lidas.notifyAttention).toBe(false);
    expect(lidas.avisos).toEqual(DEFAULT_SETTINGS.avisos);
    expect(lidas.shortcuts).toEqual({});
    expect(lidas.backgroundOpacity).toBe(DEFAULT_SETTINGS.backgroundOpacity);
    expect(lidas.campoDoFuturo).toEqual({ x: 1 });
  });

  it("valor que nem é objeto vira o padrão", () => {
    localStorage.setItem("omni-agents-settings", "[1,2]");
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });
});

describe("tarefas pessoais", () => {
  it("item inválido sai sozinho", () => {
    localStorage.setItem("omni-todo", JSON.stringify([{ id: "a", texto: "ok", feita: false }, null, { id: 2 }, { id: "b", texto: "x" }]));
    const { result } = renderHook(() => useTodos());
    expect(result.current.todos).toEqual([{ id: "a", texto: "ok", feita: false }]);
  });
});

describe("escolhas salvas", () => {
  it("parte estragada vira 'sem escolha' só nela e chaves novas sobrevivem ao gravar", () => {
    localStorage.setItem(
      "medza-escolhas",
      JSON.stringify({ agente: 42, contas: { claude: "perfil-1", codex: 7 }, ajustes: { claude: "alto", codex: { model: "m", effort: 3 } }, novo: true })
    );
    expect(ultimoAgente()).toBeNull();
    expect(contaDoAgente("claude")).toBe("perfil-1");
    expect(contaDoAgente("codex")).toBeNull();
    expect(ajustesDoAgente("claude")).toBeNull();
    expect(ajustesDoAgente("codex")).toEqual({ model: "m", effort: null });

    lembrarAgente("codex");
    expect(JSON.parse(localStorage.getItem("medza-escolhas")!)).toMatchObject({ agente: "codex", novo: true });
  });
});
