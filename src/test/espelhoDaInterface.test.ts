import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  CHAVE_DO_CARIMBO,
  espelharAgora,
  esquecerUltimoEspelhado,
  restaurarDoDisco,
  type EstadoDaInterface,
} from "../lib/espelhoDaInterface";

/*
 * Fork: aparência e workspaces também em disco. O WebKit perde as últimas gravações do localStorage
 * quando o app sai sem esperar (reinício da atualização) — ao abrir, a cópia mais nova vence.
 */

const verde = JSON.stringify({ accent: "verde", background: "pixel-grid" });
const laranja = JSON.stringify({ accent: "laranja", background: "gradient-waves" });

beforeEach(() => {
  localStorage.clear();
  esquecerUltimoEspelhado();
});

describe("restaurarDoDisco", () => {
  it("disco mais novo que o localStorage: restaura as chaves dele", async () => {
    localStorage.setItem("omni-agents-settings", laranja);
    localStorage.setItem("omni-todo", "[1]");
    const resultado = await restaurarDoDisco(async () => ({ salvo_em: 100, chaves: { "omni-agents-settings": verde, "omni-token": "x" } }));
    expect(resultado).toBe("restaurado");
    expect(localStorage.getItem("omni-agents-settings")).toBe(verde);
    expect(localStorage.getItem("omni-todo")).toBe("[1]");
    expect(localStorage.getItem("omni-token")).toBeNull();
    expect(localStorage.getItem(CHAVE_DO_CARIMBO)).toBe("100");
  });

  it("localStorage mais novo: fica como está", async () => {
    localStorage.setItem("omni-agents-settings", laranja);
    localStorage.setItem(CHAVE_DO_CARIMBO, "200");
    expect(await restaurarDoDisco(async () => ({ salvo_em: 100, chaves: { "omni-agents-settings": verde } }))).toBe("local");
    expect(localStorage.getItem("omni-agents-settings")).toBe(laranja);
  });

  it("sem cópia, erro ou disco lento: abre com o localStorage", async () => {
    expect(await restaurarDoDisco(async () => null)).toBe("sem-copia");
    expect(await restaurarDoDisco(async () => { throw new Error("fora do Tauri"); })).toBe("sem-copia");
    expect(await restaurarDoDisco(() => new Promise(() => undefined), 20)).toBe("sem-copia");
  });
});

describe("espelharAgora", () => {
  it("grava o que mudou, com carimbo crescente, e não repete o que já foi", async () => {
    const gravar = vi.fn(async (_estado: EstadoDaInterface) => true);
    localStorage.setItem("omni-agents-settings", verde);
    localStorage.setItem("omni-token", "nunca");
    await espelharAgora(gravar, 1_000);
    expect(gravar).toHaveBeenCalledWith({ salvo_em: 1_000, chaves: { "omni-agents-settings": verde } });
    expect(localStorage.getItem(CHAVE_DO_CARIMBO)).toBe("1000");

    await espelharAgora(gravar, 2_000);
    expect(gravar).toHaveBeenCalledTimes(1);

    localStorage.setItem("omni-agents-settings", laranja);
    await espelharAgora(gravar, 500); // relógio voltou: o carimbo ainda sobe
    expect(gravar).toHaveBeenLastCalledWith({ salvo_em: 1_001, chaves: { "omni-agents-settings": laranja } });
  });

  it("falha ao gravar tenta de novo na próxima volta", async () => {
    const gravar = vi.fn().mockRejectedValueOnce(new Error("disco")).mockResolvedValue(true);
    localStorage.setItem("omni-todo", "[]");
    await espelharAgora(gravar, 10);
    await espelharAgora(gravar, 20);
    expect(gravar).toHaveBeenCalledTimes(2);
  });

  it("perdeu as últimas gravações do localStorage: a próxima abertura restaura do disco", async () => {
    const disco: { estado: EstadoDaInterface | null } = { estado: null };
    localStorage.setItem("omni-agents-settings", verde);
    await espelharAgora(async (estado) => { disco.estado = estado; }, 5_000);
    // O WebKit "esqueceu" a mudança e o carimbo (saída sem esperar a gravação).
    localStorage.setItem("omni-agents-settings", laranja);
    localStorage.setItem(CHAVE_DO_CARIMBO, "4000");
    expect(await restaurarDoDisco(async () => disco.estado)).toBe("restaurado");
    expect(localStorage.getItem("omni-agents-settings")).toBe(verde);
  });
});
