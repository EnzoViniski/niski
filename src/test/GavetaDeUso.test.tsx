import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { GavetaDeUso } from "../features/terminal/GavetaDeUso";

describe("GavetaDeUso", () => {
  it("abre pela barra e fecha no Esc e no X", async () => {
    render(<GavetaDeUso />);
    const gaveta = document.getElementById("gaveta-uso")!;
    const botao = screen.getByRole("button", { name: "Uso das contas" });
    expect(gaveta).not.toHaveClass("is-aberta");

    await userEvent.click(botao);
    expect(gaveta).toHaveClass("is-aberta");
    expect(botao).toHaveAttribute("aria-expanded", "true");
    await userEvent.keyboard("{Escape}");
    expect(gaveta).not.toHaveClass("is-aberta");

    await userEvent.click(botao);
    await userEvent.click(screen.getByRole("button", { name: "Fechar uso das contas" }));
    expect(gaveta).not.toHaveClass("is-aberta");
  });
});
