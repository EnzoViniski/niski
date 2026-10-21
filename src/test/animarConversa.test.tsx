// Fork (0.9.4): a entrada da conversa anima só o conteúdo novo e nunca deixa estilo para trás.
import { afterEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { useRef } from "react";
import { animarEntrada, useAnimarAoTrocar } from "../features/workspace/animarConversa";
import { setMotionLevel } from "../lib/motion";

function Painel({ chave }: { chave: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useAnimarAoTrocar(ref, chave);
  return <div ref={ref} data-testid="conteudo">{chave}</div>;
}

function espiarAnimate() {
  const cancel = vi.fn();
  const animate = vi.fn(() => ({ cancel }) as unknown as Animation);
  (HTMLElement.prototype as unknown as { animate: unknown }).animate = animate;
  return { animate, cancel };
}

afterEach(() => {
  setMotionLevel("completas");
  delete document.documentElement.dataset.motion;
  delete (HTMLElement.prototype as unknown as { animate?: unknown }).animate;
});

describe("animação de entrada da conversa", () => {
  it("anima ao nascer e a cada troca de aba, só com opacidade e deslocamento, sem fill", () => {
    const { animate, cancel } = espiarAnimate();
    const { rerender } = render(<Painel chave="aba-1" />);
    expect(animate).toHaveBeenCalledTimes(1);
    rerender(<Painel chave="aba-1" />);
    expect(animate).toHaveBeenCalledTimes(1);
    rerender(<Painel chave="aba-2" />);
    expect(animate).toHaveBeenCalledTimes(2);
    // A anterior é cancelada: nada de duas animações no mesmo painel.
    expect(cancel).toHaveBeenCalledTimes(1);
    const [quadros, opcoes] = animate.mock.calls[1] as unknown as [Keyframe[], KeyframeAnimationOptions];
    expect(JSON.stringify(quadros)).not.toContain("scale");
    expect(opcoes.fill).toBeUndefined();
    expect(opcoes.duration).toBeLessThanOrEqual(320);
  });

  it("respeita animações desligadas e só faz fade nas reduzidas", () => {
    const { animate } = espiarAnimate();
    setMotionLevel("desligadas");
    expect(animarEntrada(document.createElement("div"))).toBeNull();
    setMotionLevel("reduzidas");
    document.documentElement.dataset.motion = "reduzidas";
    animarEntrada(document.createElement("div"));
    const [quadros] = animate.mock.calls[0] as unknown as [Keyframe[]];
    expect(JSON.stringify(quadros)).not.toContain("translate");
  });

  it("sem Web Animations (navegador antigo) não quebra", () => {
    expect(animarEntrada(document.createElement("div"))).toBeNull();
  });
});
