export function dismissSurface(element: HTMLElement | null, done: () => void) {
  if (!element) {
    done();
    return null;
  }
  const animation = element.animate(
    [
      { opacity: 1, translate: "0 0", scale: 1 },
      { opacity: 0, translate: "0 4px", scale: 0.985 },
    ],
    { duration: 140, easing: "cubic-bezier(.4,0,1,1)", fill: "forwards" },
  );
  void animation.finished.then(done, () => {});
  return animation;
}
