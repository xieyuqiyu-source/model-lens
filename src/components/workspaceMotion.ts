// Animate panel snapshots so text and credential cards only lay out once.
export function createWorkspaceMotion(host = document) {
  let active: ViewTransition | undefined;
  let revision = 0;
  const clear = () => host.documentElement.classList.remove("workspace-transitioning");
  return {
    run(update: () => void) {
      const request = ++revision;
      active?.skipTransition();
      const commit = () => {
        if (request === revision) update();
      };
      if (!host.startViewTransition) {
        commit();
        host.querySelector(".workspace")?.animate(
          [{ opacity: 0.45 }, { opacity: 1 }],
          { duration: 180, easing: "ease-out" },
        );
        return;
      }
      host.documentElement.classList.add("workspace-transitioning");
      const transition = host.startViewTransition(commit);
      active = transition;
      // A superseded capture rejects ready; its update must not overwrite the latest click.
      void transition.ready.catch(() => {});
      void transition.finished.catch(() => {}).then(() => {
        if (active === transition) {
          active = undefined;
          clear();
        }
      });
    },
    cancel() {
      revision++;
      active?.skipTransition();
      active = undefined;
      clear();
    },
  };
}
