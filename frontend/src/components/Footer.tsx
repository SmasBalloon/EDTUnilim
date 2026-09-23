import type { ParentComponent } from 'solid-js';

/**
 * Le pied d'une page. Le contenu ne s'arrête pas net dessous : il s'y fond,
 * par un dégradé là seulement où le défilement passe sous la barre.
 */
export const Footer: ParentComponent = (props) => (
  <footer class="relative flex shrink-0 items-center gap-3 bg-bg px-edge pt-2 pb-[max(0.5rem,env(safe-area-inset-bottom))] text-2xs text-muted">
    <div
      class="pointer-events-none absolute inset-x-0 bottom-full h-5 bg-linear-to-t from-bg to-transparent"
      aria-hidden="true"
    />
    {props.children}
  </footer>
);
