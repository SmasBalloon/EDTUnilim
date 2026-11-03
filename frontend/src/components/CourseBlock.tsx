import { Show, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { homework, onCourse } from '../lib/homework';
import { KIND_LABEL } from '../lib/layout';
import { cx, kindCard, kindTag } from '../lib/ui';

interface Props {
  course: Course;
  /** Cours en train de se dérouler : la case est cerclée d'accent. */
  live: boolean;
  /** Créneau d'une heure ou moins : titre, tag et ligne horaire seulement. */
  dense: boolean;
  /** En comparaison : les deux classes suivent ce cours ensemble. */
  together?: boolean;
  /** Remplace l'enseignant en fin de ligne quand il n'apprend rien. */
  describe?: (course: Course) => string;
  /** Ce cours a bougé depuis la dernière visite : `added` ou `modified`. */
  changed?: string;
  /** Position et hauteur, calculées par la grille. */
  style: Record<string, string>;
  onSelect: () => void;
}

/**
 * Une case de la grille horaire (`.blk` de la maquette) : code et type en
 * tête, matière, puis « 15:30 – 17:00 · 205 · Crosio G. ». Le détail s'ouvre
 * en modal.
 */
export const CourseBlock: Component<Props> = (props) => {
  /** Un devoir rattaché à ce créneau précis. */
  const noted = () => homework().some((item) => onCourse(item, props.course));

  const meta = () =>
    [
      `${props.course.start} – ${props.course.end}`,
      props.course.room,
      props.describe ? props.describe(props.course) : props.course.teacher,
    ]
      .filter(Boolean)
      .join(' · ');

  return (
    <button
      type="button"
      class={cx(
        'blk',
        kindCard(props.course.kind),
        props.together && 'cmp-both',
        props.changed !== undefined && 'is-changed',
        props.live && 'is-live',
      )}
      style={props.style}
      title={props.together ? 'Les deux classes y sont ensemble' : undefined}
      onClick={props.onSelect}
    >
      <Show when={props.changed}>
        {(change) => (
          <span
            class="mark-changed"
            title={`Cours ${change() === 'added' ? 'ajouté' : 'modifié'} depuis ta dernière visite`}
          />
        )}
      </Show>

      <div class="hd">
        <b class="truncate">{props.course.resource?.code ?? props.course.title ?? 'Cours'}</b>
        <span class={kindTag(props.course.kind)}>{KIND_LABEL[props.course.kind]}</span>
      </div>

      <Show when={!props.dense && props.course.resource?.name}>
        {(name) => <small class="line-clamp-2">{name()}</small>}
      </Show>

      <small class="m">
        <Show when={noted()}>
          <span class="mark-hw" title="Un devoir est rattaché à ce cours">✎</span>
        </Show>
        {meta()}
      </small>
    </button>
  );
};
