import { For, createEffect, on, onCleanup, onMount, type JSX } from 'solid-js';

import { Spring, Velocity, project, rubberband } from '../lib/motion';
import { cx } from '../lib/ui';

interface Props<T> {
  /** Les pages, dans l'ordre où on les feuillette. */
  items: T[];
  index: number;
  /** Page demandée par le geste ; la vitesse du doigt l'accompagne. */
  onIndex: (index: number, velocity: number) => void;
  /** Faux sur grand écran : tout tient à l'écran, il n'y a rien à feuilleter. */
  enabled: boolean;
  children: (item: T, index: number) => JSX.Element;
}

/** Avant ce déplacement, le doigt n'a encore rien décidé. */
const THRESHOLD = 10;

/**
 * Des pages rangées côte à côte, que le doigt fait défiler.
 *
 * La piste suit le doigt au pixel près, résiste aux deux bouts, et part vers
 * la page que le geste visait — d'après l'élan, pas d'après l'endroit où le
 * doigt s'est levé — en reprenant sa vitesse. Le mouvement est mené par un
 * ressort : il peut être rattrapé et renvoyé en pleine course.
 */
export function Pager<T>(props: Props<T>): JSX.Element {
  let frame: HTMLDivElement | undefined;
  let track: HTMLDivElement | undefined;
  /** Largeur d'une page, relevée sur le cadre. */
  let width = 0;
  /** Vitesse du doigt, à remettre au ressort quand la page aura changé. */
  let handoff: number | undefined;

  const slide = new Spring(
    0,
    (x) => {
      if (track) track.style.transform = `translate3d(${x}px, 0, 0)`;
    },
    0.05,
  );
  onCleanup(() => slide.stop());

  const settle = (velocity?: number) => {
    slide.to(-props.index * width, {
      // Le rebond n'a de sens que derrière un geste qui portait de l'élan.
      damping: velocity === undefined ? 1 : 0.85,
      response: velocity === undefined ? 0.4 : 0.35,
      velocity,
    });
  };

  const measure = () => {
    width = frame?.clientWidth ?? 0;
    slide.set(-props.index * width);
  };

  /*
   * La largeur est relevée par un observateur plutôt qu'une fois pour toutes :
   * le cadre peut n'être attaché au document qu'après le premier passage des
   * effets — il mesurerait alors zéro, et la piste resterait sur le lundi.
   */
  onMount(() => {
    if (!frame) return;
    const observer = new ResizeObserver(() => measure());
    observer.observe(frame);
    onCleanup(() => observer.disconnect());
  });

  // Une autre semaine, ou l'écran qui s'élargit : la page ouverte se repose à
  // sa place, sans glisser — il n'y a pas de geste derrière ce changement.
  createEffect(on([() => props.enabled, () => props.items], () => measure()));

  createEffect(
    on(
      () => props.index,
      () => {
        settle(handoff);
        handoff = undefined;
      },
      { defer: true },
    ),
  );

  // --- Geste ---------------------------------------------------------------

  const travel = new Velocity();
  let origin: { x: number; y: number; at: number } | null = null;
  let dragging = false;

  const onPointerDown = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!props.enabled || event.button !== 0) return;
    origin = { x: event.clientX, y: event.clientY, at: slide.value };
    dragging = false;
    travel.clear();
  };

  const onPointerMove = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!origin || width === 0) return;
    const dx = event.clientX - origin.x;
    const dy = event.clientY - origin.y;

    if (!dragging) {
      // On laisse le doigt dire s'il feuillette ou s'il fait défiler.
      if (Math.abs(dx) < THRESHOLD && Math.abs(dy) < THRESHOLD) return;
      if (Math.abs(dx) <= Math.abs(dy)) {
        origin = null;
        return;
      }
      dragging = true;
      slide.stop();
      event.currentTarget.setPointerCapture(event.pointerId);
    }

    let x = origin.at + dx;
    // Aux deux bouts, le bord résiste au lieu de bloquer.
    const min = -(props.items.length - 1) * width;
    if (x > 0) x = rubberband(x, width);
    else if (x < min) x = min - rubberband(min - x, width);

    travel.add(x);
    slide.set(x);
  };

  const release = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!origin) return;
    const wasDragging = dragging;
    origin = null;
    dragging = false;
    if (!wasDragging || width === 0) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }

    const speed = travel.get();
    // On vise la page que le geste projetait d'atteindre, sans jamais en sauter
    // plus d'une : un élan ne doit pas faire perdre la semaine de vue.
    const aimed = Math.round(-(slide.value + project(speed)) / width);
    const next = Math.max(
      0,
      Math.min(props.items.length - 1, Math.max(props.index - 1, Math.min(props.index + 1, aimed))),
    );

    if (next === props.index) {
      settle(speed);
      return;
    }
    handoff = speed;
    props.onIndex(next, speed);
  };

  return (
    // `overflow-x: clip` découpe les pages voisines sans créer de conteneur de
    // défilement, ce qui laisse les en-têtes d'heures continuer de coller au
    // haut de l'écran.
    <div
      class="relative overflow-x-clip"
      ref={frame}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={release}
      onPointerCancel={release}
    >
      <div class="relative will-change-transform" ref={track}>
        <For each={props.items}>
          {(item, position) => (
            // Chaque page est décalée de sa place dans la semaine. Seule la page
            // ouverte reste dans le flux : c'est elle qui donne sa hauteur à la
            // zone, les autres ne l'étirent pas à la hauteur du jour le plus
            // chargé.
            <div
              class={cx(
                'left-[calc(var(--page)*100%)] w-full',
                position() === props.index ? 'relative' : 'absolute top-0',
              )}
              style={{ '--page': String(position()) }}
              aria-hidden={position() === props.index ? undefined : 'true'}
            >
              {props.children(item, position())}
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
