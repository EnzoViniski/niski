import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cancelarQuadro, definirPausaDeEnergia, energiaPausada, proximoQuadro } from "../lib/energia";

describe("energia (pausa dos fundos WebGL)", () => {
  beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "requestAnimationFrame", "cancelAnimationFrame", "performance"] }));
  afterEach(() => { definirPausaDeEnergia(false); vi.useRealTimers(); });

  it("pausado guarda o pedido, retoma ao despausar e marca o <html>", () => {
    const cb = vi.fn();
    definirPausaDeEnergia(true);
    expect(energiaPausada()).toBe(true);
    expect(document.documentElement.dataset.energia).toBe("pausa");
    proximoQuadro(cb);
    vi.advanceTimersByTime(200);
    expect(cb).not.toHaveBeenCalled();
    definirPausaDeEnergia(false);
    expect(document.documentElement.dataset.energia).toBeUndefined();
    vi.advanceTimersByTime(100);
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it("o id guardado na pausa ainda cancela depois de retomar (desmontar não deixa laço vivo)", () => {
    const cb = vi.fn();
    definirPausaDeEnergia(true);
    const id = proximoQuadro(cb);
    expect(id).not.toBe(0);
    definirPausaDeEnergia(false);
    cancelarQuadro(id);
    vi.advanceTimersByTime(200);
    expect(cb).not.toHaveBeenCalled();
  });

  it("limita a ~30 quadros por segundo", () => {
    let quadros = 0;
    const laco = () => { quadros += 1; proximoQuadro(laco); };
    const id = proximoQuadro(laco);
    vi.advanceTimersByTime(1_000);
    cancelarQuadro(id);
    expect(quadros).toBeGreaterThan(15);
    expect(quadros).toBeLessThanOrEqual(31);
  });
});
