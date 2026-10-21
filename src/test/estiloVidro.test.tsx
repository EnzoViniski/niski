/**
 * Fork: estilo Vidro (Enzo, 21/10/2026). Liga o Minimalista mais a variante `vidro` no <html>, vale
 * na hora no PC e no celular (que segue o tema publicado pelo PC) e sai limpo ao voltar.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { useRef } from "react";
import { useSettings } from "../hooks/useSettings";
import { DEFAULT_SETTINGS } from "../types/settings";
import { mesclarComPadrao } from "../services/settingsService";
import { aplicarEstilo, estiloValido, usaFormaMinimalista } from "../lib/estiloDaInterface";
import { aplicarTema } from "../mobile/tema";
import { EscolhaDeEstilo } from "../components/EscolhaDeEstilo";
import { useEntradaDaTela } from "../mobile/entradaDaTela";

const root = document.documentElement;

afterEach(() => {
  localStorage.clear();
  delete root.dataset.estilo;
  delete root.dataset.variante;
  delete (HTMLElement.prototype as unknown as { animate?: unknown }).animate;
});

describe("regra do estilo", () => {
  it("Vidro usa a forma do Minimalista; valor desconhecido cai no Clássico", () => {
    expect(estiloValido("vidro")).toBe("vidro");
    expect(estiloValido("futurista")).toBe("classico");
    expect(estiloValido(undefined)).toBe("classico");
    expect(usaFormaMinimalista("vidro")).toBe(true);
    expect(usaFormaMinimalista("minimalista")).toBe(true);
    expect(usaFormaMinimalista("classico")).toBe(false);
  });

  it("liga e desliga a variante sem sobrar atributo", () => {
    aplicarEstilo(root, "vidro");
    expect(root.dataset.estilo).toBe("minimalista");
    expect(root.dataset.variante).toBe("vidro");
    aplicarEstilo(root, "minimalista");
    expect(root.dataset.estilo).toBe("minimalista");
    expect(root.dataset.variante).toBeUndefined();
    aplicarEstilo(root, "vidro");
    aplicarEstilo(root, "classico");
    expect(root.dataset.estilo).toBe("classico");
    expect(root.dataset.variante).toBeUndefined();
  });

  it("configuração salva com Vidro continua Vidro", () => {
    expect(mesclarComPadrao({ estilo: "vidro" }).estilo).toBe("vidro");
  });
});

describe("no PC", () => {
  it("trocar para Vidro aplica na hora e grava; voltar ao Clássico limpa", () => {
    const { result } = renderHook(() => useSettings());
    act(() => result.current.updateSettings({ estilo: "vidro" }));
    expect(root.dataset.estilo).toBe("minimalista");
    expect(root.dataset.variante).toBe("vidro");
    expect(JSON.parse(localStorage.getItem("omni-agents-settings")!).estilo).toBe("vidro");
    act(() => result.current.updateSettings({ estilo: "classico" }));
    expect(root.dataset.variante).toBeUndefined();
  });

  it("abre já no Vidro quando ele está salvo", () => {
    localStorage.setItem("omni-agents-settings", JSON.stringify({ ...DEFAULT_SETTINGS, estilo: "vidro" }));
    renderHook(() => useSettings());
    expect(root.dataset.variante).toBe("vidro");
  });

  it("a escolha oferece o Vidro", () => {
    const onChange = vi.fn();
    render(<EscolhaDeEstilo valor="vidro" onChange={onChange} />);
    expect(screen.getByRole("radio", { name: /Vidro/ })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(screen.getByRole("radio", { name: /Clássico/ }));
    expect(onChange).toHaveBeenCalledWith("classico");
  });
});

describe("no celular", () => {
  it("segue o Vidro publicado pelo PC e volta quando o PC troca", () => {
    aplicarTema({ estilo: "vidro", theme: "escuro" });
    expect(root.dataset.estilo).toBe("minimalista");
    expect(root.dataset.variante).toBe("vidro");
    aplicarTema({ estilo: "minimalista", theme: "escuro" });
    expect(root.dataset.variante).toBeUndefined();
  });

  it("troca de tela anima só ao mudar de rota, nunca na primeira tela, e cancela a anterior", () => {
    const cancel = vi.fn();
    const animate = vi.fn(() => ({ cancel }) as unknown as Animation);
    (HTMLElement.prototype as unknown as { animate: unknown }).animate = animate;
    function Tela({ rota }: { rota: string }) {
      const ref = useRef<HTMLElement>(null);
      useEntradaDaTela(ref, rota);
      return <main ref={ref}>{rota}</main>;
    }
    const { rerender } = render(<Tela rota="inicio" />);
    expect(animate).not.toHaveBeenCalled();
    rerender(<Tela rota="projeto" />);
    expect(animate).toHaveBeenCalledTimes(1);
    const [quadros, opcoes] = animate.mock.calls[0] as unknown as [Keyframe[], KeyframeAnimationOptions];
    expect(JSON.stringify(quadros)).not.toMatch(/scale|blur/);
    expect(opcoes.fill).toBeUndefined();
    rerender(<Tela rota="conversa" />);
    expect(cancel).toHaveBeenCalledTimes(1);
  });
});

describe("vidro.css", () => {
  const css = readFileSync(resolve(__dirname, "../styles/vidro.css"), "utf8");

  it("só pinta com a variante ligada, fora as seções do Minimalista e a prévia", () => {
    const regras = css.replace(/\/\*[\s\S]*?\*\//g, "").split("}").map((r) => r.split("{")[0].trim()).filter(Boolean);
    const soltas = regras.filter((sel) => !/^(@|from|to|\d)/.test(sel) && !sel.includes(":root[data-variante=\"vidro\"]")
      && !sel.includes(":root[data-estilo=\"minimalista\"]") && !sel.startsWith(".estilo-"));
    expect(soltas).toEqual([]);
  });

  it("o vidro do campo de mensagem fica no ::before (backdrop-filter no campo prenderia os menus fixos)", () => {
    const doCampo = css.match(/:is\(\.composer, \.aba-conversa-tela \.composer\) \{[^}]*\}/)?.[0] ?? "";
    expect(doCampo).not.toContain("backdrop-filter");
    expect(css).toMatch(/\.composer\)::before \{[^}]*backdrop-filter/);
  });

  it("o movimento respeita o reduzir movimento do sistema", () => {
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
  });
});
