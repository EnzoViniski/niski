// Fork (Google Agenda): cartão dos Ajustes (conectar pelo navegador) e "Novo compromisso".
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import userEvent from "../../test/interacoesTelas";
import { openUrl } from "@tauri-apps/plugin-opener";
import { GoogleAgendaCartao } from "./GoogleAgendaAjustes";
import { NovoCompromisso } from "./NovoCompromisso";
import * as servico from "./googleAgendaService";

vi.mock("./googleAgendaService", () => ({
  lerGoogleAgenda: vi.fn(), conectarGoogleAgenda: vi.fn(), desconectarGoogleAgenda: vi.fn(), sincronizarGoogleAgenda: vi.fn(), criarCompromisso: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));

const desligado = { conectado: false, conectando: false, conta: "", sincronizado_ms: 0, eventos: 0, erro: null };

beforeEach(() => {
  vi.mocked(servico.lerGoogleAgenda).mockReset().mockResolvedValue(desligado);
  vi.mocked(servico.conectarGoogleAgenda).mockReset();
  vi.mocked(servico.criarCompromisso).mockReset().mockResolvedValue({ ok: true, id: "novo1" });
  vi.mocked(openUrl).mockClear();
});

it("conectar manda o ID e a chave ao engine, limpa a chave e abre o login do Google", async () => {
  vi.mocked(servico.conectarGoogleAgenda).mockResolvedValue({ ...desligado, conectando: true, url: "https://accounts.google.com/o/oauth2/v2/auth?x=1" });
  render(<GoogleAgendaCartao />);
  const conectar = await screen.findByRole("button", { name: "Conectar" });
  expect(conectar).toBeDisabled();
  await userEvent.type(screen.getByLabelText("ID do cliente"), "123.apps.googleusercontent.com");
  await userEvent.type(screen.getByLabelText("Chave secreta do cliente"), "segredo");
  await userEvent.click(conectar);
  expect(servico.conectarGoogleAgenda).toHaveBeenCalledWith("123.apps.googleusercontent.com", "segredo");
  expect(openUrl).toHaveBeenCalledWith("https://accounts.google.com/o/oauth2/v2/auth?x=1");
  expect(await screen.findByText(/Abri a página do Google no navegador/)).toBeInTheDocument();
  expect(screen.queryByLabelText("Chave secreta do cliente")).not.toBeInTheDocument();
});

it("conectado mostra a conta e o erro da última sincronização", async () => {
  vi.mocked(servico.lerGoogleAgenda).mockResolvedValue({ ...desligado, conectado: true, conta: "eu@gmail.com", eventos: 12,
    sincronizado_ms: Date.now(), erro: "O Google pediu para conectar de novo (a autorização expirou ou foi removida)." });
  render(<GoogleAgendaCartao />);
  expect(await screen.findByText("Conectado a eu@gmail.com")).toBeInTheDocument();
  expect(screen.getByText(/12 compromissos/)).toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent("conectar de novo");
});

it("novo compromisso cria no Google com dia, hora e local e avisa a tela", async () => {
  const onCriado = vi.fn();
  render(<NovoCompromisso onCriado={onCriado} onErro={() => undefined} />);
  await userEvent.click(screen.getByRole("button", { name: "Novo compromisso" }));
  await userEvent.type(screen.getByLabelText("Compromisso"), "Dentista");
  await userEvent.clear(screen.getByLabelText("Data"));
  await userEvent.type(screen.getByLabelText("Data"), "2026-11-03");
  await userEvent.type(screen.getByLabelText("Início"), "14:00");
  await userEvent.type(screen.getByLabelText("Local"), "Centro");
  await userEvent.click(screen.getByRole("button", { name: "Criar no Google" }));
  await waitFor(() => expect(onCriado).toHaveBeenCalled());
  expect(servico.criarCompromisso).toHaveBeenCalledWith({ titulo: "Dentista", data: "2026-11-03", inicio: "14:00", fim: "", local: "Centro", notas: "" });
  expect(screen.getByRole("status")).toHaveTextContent("“Dentista” está no seu Google Agenda.");
});

it("erro do Google vai para a tela e o formulário continua aberto", async () => {
  vi.mocked(servico.criarCompromisso).mockRejectedValue(new Error("Conecte o Google Agenda nos Ajustes da Rotina."));
  const onErro = vi.fn();
  render(<NovoCompromisso onCriado={() => undefined} onErro={onErro} />);
  await userEvent.click(screen.getByRole("button", { name: "Novo compromisso" }));
  await userEvent.type(screen.getByLabelText("Compromisso"), "Reunião");
  await userEvent.click(screen.getByRole("button", { name: "Criar no Google" }));
  await waitFor(() => expect(onErro).toHaveBeenCalledWith("Conecte o Google Agenda nos Ajustes da Rotina."));
  expect(screen.getByLabelText("Compromisso")).toHaveValue("Reunião");
});

it("o app tem permissão de abrir o login do Google e o Google Cloud no navegador", async () => {
  const { readFileSync } = await import("node:fs");
  const capacidade = JSON.parse(readFileSync("src-tauri/capabilities/default.json", "utf8")) as { permissions: (string | { identifier: string; allow?: { url: string }[] })[] };
  const abrir = capacidade.permissions.find((p) => typeof p !== "string" && p.identifier === "opener:allow-open-url") as { allow: { url: string }[] };
  const urls = abrir.allow.map((a) => a.url);
  expect(urls).toContain("https://accounts.google.com/*");
  expect(urls).toContain("https://console.cloud.google.com/*");
});

it("sem navegador, o link do login fica na tela para copiar", async () => {
  vi.mocked(servico.conectarGoogleAgenda).mockResolvedValue({ ...desligado, conectando: true, url: "https://accounts.google.com/o/oauth2/v2/auth?x=2" });
  vi.mocked(openUrl).mockRejectedValueOnce(new Error("not allowed"));
  render(<GoogleAgendaCartao />);
  await userEvent.type(await screen.findByLabelText("ID do cliente"), "123.apps.googleusercontent.com");
  await userEvent.type(screen.getByLabelText("Chave secreta do cliente"), "segredo");
  await userEvent.click(screen.getByRole("button", { name: "Conectar" }));
  expect(await screen.findByLabelText("Link para abrir no navegador")).toHaveValue("https://accounts.google.com/o/oauth2/v2/auth?x=2");
  expect(screen.getByRole("alert")).toHaveTextContent("Copie o link");
});
