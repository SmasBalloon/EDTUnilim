import { Show, createEffect, createSignal, onCleanup, type Component, type JSX } from 'solid-js';

import { useBottomSheet } from '../lib/media';
import { Spring, Velocity, project, rubberband } from '../lib/motion';

interface Props {
  open: boolean;
  onClose: () => void;
  /** Nom de la boîte, pour les lecteurs d'écran. */
  label: string;
  /** Classes ajoutées au panneau. */
  class?: string;
  children: JSX.Element;
}

/** Au-delà de cette part de sa hauteur, la feuille est lâchée dehors. */
const DISMISS = 0.4;
/** Élan qui emporte la feuille quel que soit l'endroit où le doigt la lâche. */
const FLING = 900;
/** Avant ce déplacement, le doigt n'a encore rien décidé. */
const THRESHOLD = 8;

/**
 * Le plateau des boîtes de dialogue : il monte du bas au doigt, se pose au
 * centre à la souris.
 *
 * Tout passe par un ressort plutôt que par une transition CSS : la feuille
 * peut être rattrapée en pleine course, tirée, relâchée, et elle repart de là
 * où elle est avec la vitesse qu'on lui a donnée.
 */
export const Sheet: Component<Props> = (props) => {
  const [visible, setVisible] = createSignal(props.open);
  const bottom = useBottomSheet();

  let panel: HTMLDivElement | undefined;
  let backdrop: HTMLDivElement | undefined;

  /** Hauteur du panneau, relevée au début du geste. */
  let height = 0;
  /** Ce qu'il reste à faire une fois la feuille redescendue. */
  let settled: (() => void) | null = null;

  /** `1` : posée. `0` : sortie de l'écran. */
  const paint = (presence: number) => {
    const shown = Math.max(0, Math.min(1, presence));
    if (backdrop) backdrop.style.setProperty('--presence', `${shown}`);
    if (!panel) return;
    if (bottom()) {
      const full = height || panel.offsetHeight;
      panel.style.transform = `translate3d(0, ${(1 - presence) * full}px, 0)`;
      panel.style.opacity = '1';
    } else {
      // À la souris, la feuille se matérialise sur place : elle grandit et se
      // révèle depuis le point où on l'a demandée.
      panel.style.transform = `scale(${0.94 + 0.06 * shown})`;
      panel.style.opacity = `${shown}`;
    }
  };

  const spring = new Spring(0, (presence) => {
    paint(presence);
    if (presence === 0 && settled) {
      const done = settled;
      settled = null;
      done();
    }
  });
  onCleanup(() => spring.stop());

  /** Range la feuille, puis exécute la suite. */
  const retract = (after: () => void, velocity?: number) => {
    settled = after;
    spring.to(0, { damping: 1, response: 0.3, velocity });
  };

  createEffect(() => {
    if (props.open) {
      settled = null;
      setVisible(true);
      // Le panneau n'existe qu'après le rendu : on le pose dehors, puis on
      // l'amène. Sans ce premier passage, la feuille apparaîtrait en place.
      queueMicrotask(() => {
        height = panel?.offsetHeight ?? 0;
        spring.set(0);
        spring.to(1, { damping: 0.85, response: 0.35 });
      });
    } else if (visible()) {
      retract(() => setVisible(false));
    }
  });

  // --- Geste : la feuille suit le doigt, au pixel près ---------------------

  const travel = new Velocity();
  let origin: { x: number; y: number; presence: number } | null = null;
  let dragging = false;

  const onPointerDown = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!bottom() || event.button !== 0) return;
    // Le doigt posé sur une commande lui appartient ; et une feuille déjà
    // défilée se défile, elle ne se tire pas.
    const target = event.target as HTMLElement;
    if (target.closest('button, a, input, select, textarea')) return;
    if ((panel?.scrollTop ?? 0) > 0) return;

    height = panel?.offsetHeight ?? 0;
    origin = { x: event.clientX, y: event.clientY, presence: spring.value };
    dragging = false;
    travel.clear();
  };

  const onPointerMove = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!origin || height === 0) return;
    const dy = event.clientY - origin.y;
    const dx = event.clientX - origin.x;

    if (!dragging) {
      // On laisse le doigt dire ce qu'il fait avant de lui prendre la main.
      if (Math.abs(dy) < THRESHOLD && Math.abs(dx) < THRESHOLD) return;
      if (dy <= 0 || Math.abs(dx) > Math.abs(dy)) {
        origin = null;
        return;
      }
      dragging = true;
      spring.stop();
      event.currentTarget.setPointerCapture(event.pointerId);
    }

    // Vers le haut il n'y a rien de plus à voir : le bord résiste.
    const offset = dy >= 0 ? dy : -rubberband(-dy, height);
    travel.add(offset);
    spring.set(origin.presence - offset / height);
  };

  const release = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!origin) return;
    const wasDragging = dragging;
    origin = null;
    dragging = false;
    if (!wasDragging) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }

    const speed = travel.get();
    // On vise là où le geste allait, pas l'endroit où le doigt s'est levé.
    const landing = (1 - spring.value) * height + project(speed);
    const presence = speed / height;

    if (landing > height * DISMISS || speed > FLING) {
      retract(() => {
        setVisible(false);
        props.onClose();
      }, -presence);
    } else {
      spring.to(1, { damping: 0.8, response: 0.35, velocity: -presence });
    }
  };

  return (
    <Show when={visible()}>
      <div
        class="sheet-backdrop"
        ref={backdrop}
        onClick={() => retract(() => { setVisible(false); props.onClose(); })}
      >
        <div
          class={`sheet ${props.class ?? ''}`}
          ref={panel}
          role="dialog"
          aria-modal="true"
          aria-label={props.label}
          onClick={(event) => event.stopPropagation()}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={release}
          onPointerCancel={release}
        >
          <Show when={bottom()}>
            <div class="sheet__grip" aria-hidden="true" />
          </Show>
          {props.children}
        </div>
      </div>
    </Show>
  );
};
