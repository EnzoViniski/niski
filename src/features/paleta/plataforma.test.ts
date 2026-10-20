import { describe, expect, it, vi } from "vitest";
vi.mock("../../lib/plataforma", async (original) => {
  const real = await original<typeof import("../../lib/plataforma")>();
  return { ...real, NO_MAC: true,
    comboDaPlataforma: (combo: string) => real.comboDaPlataforma(combo, true),
    rotuloDoAtalho: (combo: string) => real.rotuloDoAtalho(combo, true),
    teclaModificadora: (evento: KeyboardEvent, noMac = true) => real.teclaModificadora(evento, noMac),
  };
});
import { handleTerminalKey } from "../terminal/keyBindings";
import { matchesShortcut, setShortcutOverrides, shortcutFor, shortcutLabel } from "../../lib/shortcuts";
import { teclaModificadora, rotuloDoAtalho } from "../../lib/plataforma";

describe("macOS", () => {
  it("Cmd usa o padrão e converte preferências antigas com Ctrl", () => {
    expect(shortcutFor("paleta")).toBe("Meta+K");
    expect(shortcutLabel("paleta")).toBe("⌘+K");
    expect(matchesShortcut(new KeyboardEvent("keydown", { key: "k", metaKey: true }), "paleta")).toBe(true);
    expect(matchesShortcut(new KeyboardEvent("keydown", { key: "k", ctrlKey: true }), "paleta")).toBe(false);
    setShortcutOverrides({ quickOpen: "Ctrl+Shift+F" });
    expect(matchesShortcut(new KeyboardEvent("keydown", { key: "F", metaKey: true, shiftKey: true }), "quickOpen")).toBe(true);
    setShortcutOverrides({});
    expect(rotuloDoAtalho("Ctrl+S", true)).toBe("⌘+S");
  });
  it("helper distingue Cmd no Mac e Ctrl fora", () => {
    expect(teclaModificadora({ metaKey: true, ctrlKey: false }, true)).toBe(true);
    expect(teclaModificadora({ metaKey: true, ctrlKey: false }, false)).toBe(false);
    expect(teclaModificadora({ metaKey: false, ctrlKey: true }, false)).toBe(true);
  });
  it("Cmd+C copia, Cmd+V deixa colar nativamente e Ctrl+C segue para o xterm (SIGINT)", () => {
    const ctx = { hasSelection: () => true, copySelection: vi.fn(), write: vi.fn() };
    const copiar = new KeyboardEvent("keydown", { key: "c", metaKey: true, cancelable: true });
    expect(handleTerminalKey(copiar, ctx)).toBe(false);
    expect(ctx.copySelection).toHaveBeenCalledOnce();
    const colar = new KeyboardEvent("keydown", { key: "v", metaKey: true, cancelable: true });
    expect(handleTerminalKey(colar, ctx)).toBe(false);
    expect(colar.defaultPrevented).toBe(false);
    const interromper = new KeyboardEvent("keydown", { key: "c", ctrlKey: true, cancelable: true });
    // Ctrl+C segue para o xterm, que manda o ^C (SIGINT) à PTY.
    expect(handleTerminalKey(interromper, ctx)).toBe(true);
    expect(interromper.defaultPrevented).toBe(false);
  });
});
