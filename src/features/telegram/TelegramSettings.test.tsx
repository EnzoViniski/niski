import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import userEvent from "../../test/interacoesTelas";
import { TelegramSettings } from "./TelegramSettings";
import { telegram, type TelegramStatus } from "./telegramService";

vi.mock("./telegramService", () => ({ telegram:vi.fn() }));
const desligado: TelegramStatus = { conectado:false, ativo:false, bot:"", tipos:[], incluir_titulos:false, erro:null, parear_url:null, expira_em_ms:null };
beforeEach(() => vi.mocked(telegram).mockReset().mockResolvedValue(desligado));

it("token só vai ao conectar e some do campo antes da resposta; pareamento abre Telegram", async () => {
  render(<TelegramSettings />);
  await waitFor(() => expect(telegram).toHaveBeenCalledWith());
  const campo=screen.getByLabelText("Token do bot");
  expect(campo).toHaveAttribute("type","password");
  await userEvent.type(campo,"123:segredo-sintetico");
  vi.mocked(telegram).mockResolvedValue({ ...desligado, bot:"teste_bot", parear_url:"https://t.me/teste_bot?start=desafio" });
  await userEvent.click(screen.getByRole("button",{name:"Conectar bot"}));
  await screen.findByRole("button",{name:"Abrir Telegram para conectar"});
  expect(telegram).toHaveBeenLastCalledWith("/conectar",{token:"123:segredo-sintetico"});
  expect(screen.queryByLabelText("Token do bot")).not.toBeInTheDocument();
  expect(screen.queryByText(/segredo-sintetico/)).not.toBeInTheDocument();
});

it("salva preferências sem reenviar token e envia teste só pelo botão", async () => {
  const conectado={...desligado,conectado:true,ativo:true,bot:"teste_bot",tipos:["limite"] as TelegramStatus["tipos"]};
  vi.mocked(telegram).mockResolvedValue(conectado);
  render(<TelegramSettings />);
  await screen.findByText("Conectado a @teste_bot");
  expect(telegram).toHaveBeenCalledTimes(1);
  expect(screen.getByLabelText("Limite de uso")).toBeChecked();
  await userEvent.click(screen.getByLabelText("Incluir nome do projeto e título da conversa nos avisos"));
  expect(telegram).toHaveBeenLastCalledWith("",{ativo:true,tipos:["limite"],incluir_titulos:true});
  await waitFor(() => expect(screen.getByRole("button",{name:"Enviar teste"})).toBeEnabled());
  await userEvent.click(screen.getByRole("button",{name:"Enviar teste"}));
  await screen.findByText("Teste enviado ao Telegram.");
  expect(telegram).toHaveBeenCalledWith("/testar",{});
  await userEvent.click(screen.getByRole("button",{name:"Desconectar Telegram"}));
  expect(telegram).toHaveBeenCalledWith("/desconectar",{});
});
