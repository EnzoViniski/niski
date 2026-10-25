// Fork (Telegram no celular): conectar e ajustar os dois bots pelo celular pareado — o dos avisos dos
// agentes e o da rotina. As mesmas telas do PC; aqui o "Abrir Telegram" é um link que abre o app do
// Telegram direto no celular.
import { TelegramSettings } from "../../features/telegram/TelegramSettings";
import { usarTransporteDoTelegram } from "../../features/telegram/telegramService";
import { TelegramDaRotinaCartao } from "../../features/rotina/AjustesDaRotina";
import { usarTransporteDaRotina } from "../../features/rotina/rotinaService";
import { pelaRede } from "../transportePelaRede";

if (typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window)) {
  usarTransporteDoTelegram(pelaRede);
  usarTransporteDaRotina(pelaRede);
}

export function Telegram() {
  return (
    <div className="m-telegram space-y-4">
      <TelegramSettings />
      <TelegramDaRotinaCartao />
    </div>
  );
}
