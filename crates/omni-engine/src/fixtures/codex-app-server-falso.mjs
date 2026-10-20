// Fixture sintética JSONL; nunca inicia Codex nem acessa credenciais.
import { createInterface } from "node:readline";
import { appendFileSync } from "node:fs";
const emitir = (m) => process.stdout.write(`${JSON.stringify(m)}\n`);
let thread = "thread-falso", turn = 0;
for await (const line of createInterface({ input: process.stdin })) {
  const m = JSON.parse(line);
  if (process.env.MEDZA_FAKE_LOG) appendFileSync(process.env.MEDZA_FAKE_LOG, `${JSON.stringify(m)}\n`);
  if (m.result) {
    emitir({ method: "serverRequest/resolved", params: { threadId: thread, requestId: m.id } });
    emitir({ method: "turn/completed", params: { threadId: thread, turn: { id: `turn-${turn}`, status: "completed" } } });
    continue;
  }
  if (m.method === "initialized") continue;
  if (m.method === "initialize") {
    emitir(process.env.MEDZA_FAKE_INITIALIZE_FAIL ? { id: m.id, error: { code: -32601, message: "Sintético" } } : { id: m.id, result: { userAgent: "fake/0.163.0" } });
  } else if (m.method.startsWith("thread/")) {
    if (m.method === "thread/resume") thread = m.params.threadId;
    if (m.method === "thread/fork") thread = `fork-${m.params.threadId}`;
    emitir({ id: m.id, result: { thread: { id: thread, path: process.env.MEDZA_FAKE_ROLLOUT ?? null, turns: [] } } });
  } else if (m.method === "turn/start") {
    const id = `turn-${++turn}`, params = { threadId: thread, turnId: id };
    emitir({ id: m.id, result: { turn: { id, status: "inProgress" } } });
    emitir({ method: "turn/started", params: { threadId: thread, turn: { id } } });
    const text = m.params.input[0].text;
    if (text === "cair") process.exit(2);
    emitir({ method: "turn/plan/updated", params: { ...params, plan: [{ step: "Testar fixture", status: "in_progress" }] } });
    emitir({ method: "thread/tokenUsage/updated", params: { ...params, tokenUsage: { last: { totalTokens: 123, outputTokens: 23 }, modelContextWindow: 1000 } } });
    emitir({ method: "account/rateLimits/updated", params: { rateLimits: { primary: { usedPercent: 12, windowDurationMins: 300, resetsAt: 9999999999 } } } });
    emitir({ method: "futuro/seguro", params: { ...params, desconhecido: true } });
    if (text === "concluir") {
      for (let i = 0; i < 2; i++) emitir({ method: "turn/completed", params: { threadId: thread, turn: { id, status: "completed" } } });
    } else if (text !== "esperar") {
      emitir({ id: `pedido-${turn}`, method: text === "arquivo" ? "item/fileChange/requestApproval" : "item/commandExecution/requestApproval",
        params: { ...params, itemId: "item-1", command: text === "longo" ? `echo ${"x".repeat(600)} && rm -rf build` : "echo fixture", reason: "Pedido sintético" } });
    }
  } else if (m.method === "turn/steer") {
    emitir({ id: m.id, result: { turnId: m.params.expectedTurnId } });
  } else if (m.method === "turn/interrupt") {
    emitir({ id: m.id, result: {} });
    emitir({ method: "turn/completed", params: { threadId: thread, turn: { id: m.params.turnId, status: "interrupted" } } });
  } else emitir({ id: m.id, error: { code: -32601, message: "Método sintético desconhecido" } });
}
