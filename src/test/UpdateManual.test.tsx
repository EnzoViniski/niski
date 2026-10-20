import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { openUrl } from "@tauri-apps/plugin-opener";
import { UpdateCard } from "../components/UpdateCard";
import { NotificationBell } from "../components/NotificationBell";

const check = vi.fn();
vi.mock("@tauri-apps/plugin-updater", () => ({ check: () => check() }));

it("a distribuição sem chave própria abre os downloads e não consulta o updater", async () => {
  render(<><UpdateCard /><NotificationBell onOpenSettings={vi.fn()} /></>);
  await userEvent.click(screen.getByRole("button", { name: "Ver versões para baixar" }));
  expect(openUrl).toHaveBeenCalledWith("https://github.com/Medza-Primos-Organizacao/medza-agentes/releases");
  expect(check).not.toHaveBeenCalled();
});
