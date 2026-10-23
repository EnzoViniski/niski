// Fork (Rotina): lembrete vencido com o Niski fora de foco (outra janela na frente, minimizado) vira
// notificação do sistema; com o app em foco basta o cartão discreto de `AvisosDaRotina`. Fica tudo no
// computador (título e texto do lembrete); nada vai para o log.
import type { Aviso } from "./tipos";

export async function notificarForaDeFoco(avisos: Aviso[]) {
  if (!avisos.length) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    if (await getCurrentWindow().isFocused()) return;
    const plugin = await import("@tauri-apps/plugin-notification");
    const permitido = await plugin.isPermissionGranted() || await plugin.requestPermission() === "granted";
    if (!permitido) return;
    for (const a of avisos.slice(0, 3)) plugin.sendNotification({ title: a.titulo, body: a.texto || "Lembrete da rotina" });
  } catch { /* Fora do app (navegador, testes): só o cartão. */ }
}
