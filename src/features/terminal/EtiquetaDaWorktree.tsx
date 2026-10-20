import { GitBranchIcon } from "../../components/ui/PixelIcon";
import { worktreeDaPasta } from "./worktreeIsolada";

/** Fork: na aba, a branch da worktree isolada em que o agente roda (`medza/<nome>`). */
export function EtiquetaDaWorktree({ cwd, className = "" }: { cwd: string | null | undefined; className?: string }) {
  const worktree = worktreeDaPasta(cwd);
  if (!worktree) return null;
  return (
    <span
      className={`mr-1.5 inline-flex items-center gap-0.5 border border-accent/40 px-1 align-[1px] text-[9px] text-accent ${className}`}
      title={`Worktree isolada: ${worktree.raiz} (branch ${worktree.branch})`}
      aria-label={`Worktree isolada na branch ${worktree.branch}`}
    >
      <GitBranchIcon className="h-2.5 w-2.5" aria-hidden />
      {worktree.branch}
    </span>
  );
}
