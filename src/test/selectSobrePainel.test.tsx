import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Select } from "../components/ui/Select";

// A lista do Select sai do painel (portal no body, posição fixa) — antes era cortada pelo
// `overflow` do painel e o seletor de modelo/esforço do chat sumia atrás do painel de baixo.
describe("Select dentro de painel com overflow", () => {
  it("abre a lista no body, presa ao botão, e para cima quando falta espaço embaixo", () => {
    const onChange = vi.fn();
    const { container } = render(
      <div style={{ overflow: "hidden", height: 40 }}>
        <Select value="" onChange={onChange} options={[{ value: "", label: "Esforço padrão" }, { value: "high", label: "Alto" }]} />
      </div>,
    );
    const botao = screen.getByRole("combobox");
    botao.parentElement!.getBoundingClientRect = () => ({ left: 10, top: 700, right: 130, bottom: 730, width: 120, height: 30, x: 10, y: 700, toJSON: () => ({}) });
    Object.defineProperty(window, "innerHeight", { configurable: true, value: 768 });
    fireEvent.click(botao);
    const lista = screen.getByRole("listbox");
    expect(container.contains(lista)).toBe(false);
    const pop = lista.parentElement!;
    expect(pop.dataset.lado).toBe("cima");
    expect(pop.style.position).toBe("fixed");
    fireEvent.click(screen.getByRole("option", { name: /Alto/ }));
    expect(onChange).toHaveBeenCalledWith("high");
  });
});
