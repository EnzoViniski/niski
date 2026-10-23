// Fork (desempenho, 2026-10-22): a releitura de 5 s não publica estado novo quando a pasta não mudou.
import { beforeEach, describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { recarregar, reiniciarParaTeste, salvar, transporteEmMemoria, usarTransporteDasTarefas, useTarefas, type TransporteDasTarefas } from "./loja";
import { criarTarefa } from "./dominio";

let pasta: TransporteDasTarefas;
beforeEach(() => {
  localStorage.clear();
  pasta = transporteEmMemoria();
  usarTransporteDasTarefas(pasta);
  reiniciarParaTeste();
});

describe("releitura das tarefas", () => {
  it("pasta igual: quem assina não redesenha; mudança por fora: redesenha", async () => {
    const t = criarTarefa({ titulo: "Ligar para o dentista" }, "");
    await pasta.gravar(null, "tarefas", t.id, JSON.stringify(t));
    let desenhos = 0;
    const { result } = renderHook(() => { desenhos += 1; return useTarefas(); });
    await act(async () => { await recarregar(); });
    expect(result.current.tarefas.map((x) => x.titulo)).toEqual(["Ligar para o dentista"]);
    const antes = result.current;
    const desenhosAntes = desenhos;

    await act(async () => { await recarregar(); await recarregar(); });
    expect(result.current).toBe(antes);
    expect(desenhos).toBe(desenhosAntes);

    const outra = criarTarefa({ titulo: "Pagar a luz" }, "");
    await pasta.gravar(null, "tarefas", outra.id, JSON.stringify(outra));
    await act(async () => { await recarregar(); });
    expect(result.current.tarefas.map((x) => x.titulo).sort()).toEqual(["Ligar para o dentista", "Pagar a luz"]);
  });

  it("mudança feita na tela (memória) ainda é conferida com o disco na próxima releitura", async () => {
    const t = criarTarefa({ titulo: "A" }, "");
    await pasta.gravar(null, "tarefas", t.id, JSON.stringify(t));
    const { result } = renderHook(() => useTarefas());
    await act(async () => { await recarregar(); });
    await act(async () => { await salvar({ ...result.current.tarefas[0], titulo: "B" }); });
    await act(async () => { await recarregar(); });
    expect(result.current.tarefas[0].titulo).toBe("B");
  });
});
