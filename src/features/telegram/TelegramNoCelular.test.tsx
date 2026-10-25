// Fork (Telegram no celular): link abre no app do Mac (capability com t.me) e a tela do celular fala
// com o engine pela rede, para os dois bots.
import { readFileSync } from "node:fs";
import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import userEvent from "../../test/interacoesTelas";
import { LinkDoTelegram } from "./LinkDoTelegram";

const { openUrl, request } = vi.hoisted(() => ({ openUrl: vi.fn(), request: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl }));
vi.mock("../../mobile/api", () => ({ request, ApiError: class extends Error {} }));

afterEach(() => { delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; openUrl.mockReset(); request.mockReset(); });

it("o app do Mac pode abrir links do Telegram", () => {
  const capability = readFileSync("src-tauri/capabilities/default.json", "utf8");
  expect(capability).toContain('"https://t.me/*"');
});

it("no navegador do celular é um link que abre o Telegram em outra aba", () => {
  render(<LinkDoTelegram url="https://t.me/BotFather" onFalha={vi.fn()}>@BotFather</LinkDoTelegram>);
  const link = screen.getByRole("link", { name: "@BotFather" });
  expect(link).toHaveAttribute("href", "https://t.me/BotFather");
  expect(link).toHaveAttribute("target", "_blank");
});

it("no app do Mac abre pelo sistema e avisa quando falha", async () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
  openUrl.mockRejectedValue(new Error("not allowed"));
  const onFalha = vi.fn();
  render(<LinkDoTelegram url="https://t.me/bot?start=x" onFalha={onFalha}>Abrir</LinkDoTelegram>);
  await userEvent.click(screen.getByRole("link", { name: "Abrir" }));
  expect(openUrl).toHaveBeenCalledWith("https://t.me/bot?start=x");
  await waitFor(() => expect(onFalha).toHaveBeenCalledWith(expect.stringContaining("https://t.me/bot?start=x")));
});

it("a tela do celular consulta os dois bots pela rede e mostra os links de conectar", async () => {
  request.mockImplementation(async (caminho: string) => caminho === "/telegram"
    ? { conectado: false, ativo: false, bot: "avisos_bot", tipos: [], incluir_titulos: false, erro: null, parear_url: "https://t.me/avisos_bot?start=a", expira_em_ms: null }
    : caminho === "/rotina/telegram" ? { conectado: false, ativo: false, bot: "rotina_bot", erro: null, parear_url: "https://t.me/rotina_bot?start=b" }
    : {});
  const { Telegram } = await import("../../mobile/telas/Telegram");
  render(<Telegram />);
  const links = await screen.findAllByRole("link", { name: "Abrir Telegram para conectar" });
  await waitFor(() => expect(links.map((l) => l.getAttribute("href")).sort()).toEqual(["https://t.me/avisos_bot?start=a", "https://t.me/rotina_bot?start=b"]));
  expect(request).toHaveBeenCalledWith("/telegram", expect.objectContaining({ method: "GET" }));
  expect(request).toHaveBeenCalledWith("/rotina/telegram", expect.objectContaining({ method: "GET" }));
});
