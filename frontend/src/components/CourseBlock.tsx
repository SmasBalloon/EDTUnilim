import { Show, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { homework, onCourse } from '../lib/homework';
import { KIND_LABEL } from '../lib/layout';
import { cx, kindEdge, press } from '../lib/ui';

interface Props {
  course: Course;
  /** Cours en train de se dérouler : la case est cerclée d'accent. */
  live: boolean;
  /** Créneau trop court pour afficher l'intitulé. */
  dense: boolean;
  /** En comparaison : les deux classes suivent ce cours ensemble. */
  together?: boolean;
  /** Remplace la ligne « salle · enseignant » quand elle n'apprend rien. */
  describe?: (course: Course) => string;
  /** Ce cours a bougé depuis la dernière visite : `added` ou `modified`. */
  changed?: string;
  /** Position et hauteur, calculées par la grille. */
  style: Record<string, string>;
  onSelect: () => void;
}

/** Une case de la grille horaire ; le détail s'ouvre en modal. */
export const CourseBlock: Component<Props> = (props) => {
  /** Un devoir rattaché à ce créneau précis. */
  const noted = () => homework().some((item) => onCourse(item, props.course));

  return (
  <button
    type="button"
    class={cx(
      press,
      `kind-${props.course.kind}`,
      'absolute cursor-pointer overflow-hidden rounded-lg border bg-kind-tint px-2 py-1 text-start text-text hover:shadow-card',
      kindEdge,
      props.together
        ? // Un cours suivi par les deux classes : le même bloc, des deux côtés.
          'border-accent/50 [border-block-style:dashed] [border-inline-end-style:dashed]'
        : props.changed !== undefined
          ? 'border-accent/55'
          : 'border-kind/26',
      props.live && '-outline-offset-1 outline-2 outline-accent',
    )}
    style={props.style}
    title={props.together ? 'Les deux classes y sont ensemble' : undefined}
    onClick={props.onSelect}
  >
    <Show when={props.changed}>
      {(change) => (
        <span
          class="absolute top-1 right-1 h-[0.4375rem] w-[0.4375rem] rounded-full bg-accent"
          title={`Cours ${change() === 'added' ? 'ajouté' : 'modifié'} depuis ta dernière visite`}
        />
      )}
    </Show>

    <span class="flex items-baseline justify-between gap-1">
      <span class="text-2xs font-semibold tabular-nums tracking-[-0.01em]">
        {props.course.start}
        <span class="font-normal text-muted"> – {props.course.end}</span>
      </span>
      <span class="shrink-0 text-[0.625rem] text-kind-ink font-bold uppercase tracking-[0.04em]">
        {KIND_LABEL[props.course.kind]}
      </span>
    </span>

    <Show when={props.course.resource?.code}>
      {(code) => <span class="block text-[0.8125rem] font-semibold leading-tight">{code()}</span>}
    </Show>

    <Show when={!props.dense}>
      <span class="line-clamp-2 text-2xs leading-[1.3]">
        {props.course.resource?.name ?? props.course.title ?? ''}
      </span>
    </Show>

    <span class="block truncate text-2xs text-muted">
      <Show when={noted()}>
        <span class="mr-1 font-bold text-accent" title="Un devoir est rattaché à ce cours">✎</span>
      </Show>
      {props.describe
        ? props.describe(props.course)
        : [props.course.room, props.course.teacher].filter(Boolean).join(' · ')}
    </span>
  </button>
  );
};
