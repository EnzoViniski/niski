import { describe, expect, it, beforeEach, vi } from "vitest";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { useSettings } from "../hooks/useSettings";
import { DEFAULT_SETTINGS } from "../types/settings";
import { loadSettings, mesclarComPadrao } from "../services/settingsService";
import { EscolhaDeEstilo } from "../components/EscolhaDeEstilo";
import { AgentIcon } from "../components/ui/AgentIcon";
import { ChatIcon, NpmIcon } from "../components/ui/PixelIcon";
import { espelharAgora, esquecerUltimoEspelhado, restaurarDoDisco, type EstadoDaInterface } from "../lib/espelhoDaInterface";

/*
 * Fork: estilo da interface (Clássico × Minimalista). O padrão segue Clássico para quem já usa, a
 * escolha mora no `omni-agents-settings` (logo, no espelho em disco) e vale na hora pelo
 * `data-estilo` do <html>.
 */

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  convertFileSrc: (caminho: string) => caminho,
}));

beforeEach(() => {
  localStorage.clear();
  esquecerUltimoEspelhado();
  delete document.documentElement.dataset.estilo;
});

describe("configuração do estilo", () => {
  it("padrão é o Clássico, também para configuração antiga sem o campo", () => {
    expect(DEFAULT_SETTINGS.estilo).toBe("classico");
    localStorage.setItem("omni-agents-settings", JSON.stringify({ theme: "claro", accent: "verde" }));
    const salvo = loadSettings();
    expect(salvo.estilo).toBe("classico");
    expect(salvo.theme).toBe("claro");
  });

  it("valor de tipo errado cai no padrão; o escolhido é preservado", () => {
    expect(mesclarComPadrao({ estilo: 3 }).estilo).toBe("classico");
    expect(mesclarComPadrao({ estilo: "minimalista" }).estilo).toBe("minimalista");
  });

  it("trocar aplica data-estilo na hora e grava no omni-agents-settings", () => {
    const { result } = renderHook(() => useSettings());
    expect(document.documentElement.dataset.estilo).toBe("classico");

    act(() => result.current.updateSettings({ estilo: "minimalista" }));
    expect(document.documentElement.dataset.estilo).toBe("minimalista");
    expect(JSON.parse(localStorage.getItem("omni-agents-settings")!).estilo).toBe("minimalista");

    act(() => result.current.updateSettings({ estilo: "classico" }));
    expect(document.documentElement.dataset.estilo).toBe("classico");
  });

  it("reabrir o app mantém a escolha", () => {
    localStorage.setItem("omni-agents-settings", JSON.stringify({ ...DEFAULT_SETTINGS, estilo: "minimalista" }));
    const { result } = renderHook(() => useSettings());
    expect(result.current.settings.estilo).toBe("minimalista");
    expect(document.documentElement.dataset.estilo).toBe("minimalista");
  });

  it("valor desconhecido (de uma versão mais nova) fica no Clássico", () => {
    localStorage.setItem("omni-agents-settings", JSON.stringify({ ...DEFAULT_SETTINGS, estilo: "futurista" }));
    renderHook(() => useSettings());
    expect(document.documentElement.dataset.estilo).toBe("classico");
  });

  it("restaurar padrões volta ao Clássico", () => {
    localStorage.setItem("omni-agents-settings", JSON.stringify({ ...DEFAULT_SETTINGS, estilo: "minimalista" }));
    const { result } = renderHook(() => useSettings());
    act(() => result.current.resetSettings());
    expect(result.current.settings.estilo).toBe("classico");
    expect(document.documentElement.dataset.estilo).toBe("classico");
  });

  it("vai para o espelho em disco e volta dele depois de uma atualização", async () => {
    const disco: { estado: EstadoDaInterface | null } = { estado: null };
    localStorage.setItem("omni-agents-settings", JSON.stringify({ ...DEFAULT_SETTINGS, estilo: "minimalista" }));
    await espelharAgora(async (estado) => { disco.estado = estado; }, 9_000);
    localStorage.clear(); // o WebKit perdeu tudo no reinício
    expect(await restaurarDoDisco(async () => disco.estado)).toBe("restaurado");
    expect(loadSettings().estilo).toBe("minimalista");
  });
});

describe("EscolhaDeEstilo", () => {
  it("mostra as três opções com prévia e marca a escolhida", () => {
    const onChange = vi.fn();
    render(<EscolhaDeEstilo valor="classico" onChange={onChange} />);
    const grupo = screen.getByRole("radiogroup", { name: "Estilo da interface" });
    expect(grupo.querySelectorAll(".estilo-previa")).toHaveLength(3);
    expect(screen.getByRole("radio", { name: /Clássico/ })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("radio", { name: /Minimalista/ })).toHaveAttribute("aria-checked", "false");

    fireEvent.click(screen.getByRole("radio", { name: /Minimalista/ }));
    expect(onChange).toHaveBeenCalledWith("minimalista");
  });
});

describe("ícones no Minimalista", () => {
  it("ícone com par desenha pixel e traço; o CSS escolhe qual aparece", () => {
    const { container } = render(<ChatIcon />);
    expect(container.querySelector("path.icone-pixel")).not.toBeNull();
    const traco = container.querySelector("g.icone-traco");
    expect(traco?.getAttribute("stroke")).toBe("currentColor");
    expect(traco?.getAttribute("fill")).toBe("none");
    expect(traco?.children.length).toBeGreaterThan(0);
  });

  it("marca sem par (npm) fica só em pixel nos dois estilos", () => {
    const { container } = render(<NpmIcon />);
    expect(container.querySelector(".icone-traco")).toBeNull();
    expect(container.querySelector("path")?.getAttribute("class")).toBeNull();
  });

  it("ícone do agente leva a cor dele para o quadradinho tingido", () => {
    const { container } = render(<AgentIcon provider="codex" className="shrink-0" />);
    const svg = container.querySelector("svg")!;
    expect(svg.classList.contains("icone-do-agente")).toBe(true);
    expect(svg.classList.contains("shrink-0")).toBe(true);
    expect(svg.style.getPropertyValue("--cor-agente")).toBe("#a78bfa");
  });
});
