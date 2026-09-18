import { Show, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { homework, onCourse } from '../lib/homework';
import { KIND_LABEL } from '../lib/layout';

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
    class={`course-block kind-${props.course.kind}`}
    classList={{
      'course-block--live': props.live,
      'course-block--together': props.together,
      'course-block--changed': props.changed !== undefined,
    }}
    style={props.style}
    title={props.together ? 'Les deux classes y sont ensemble' : undefined}
    onClick={props.onSelect}
  >
    <Show when={props.changed}>
      {(change) => (
        <span class="course-block__flag" title={`Cours ${change() === 'added' ? 'ajouté' : 'modifié'} depuis ta dernière visite`} />
      )}
    </Show>

    <span class="course-block__head">
      <span class="course-block__time">
        {props.course.start}
        <span> – {props.course.end}</span>
      </span>
      <span class="course-block__kind">{KIND_LABEL[props.course.kind]}</span>
    </span>

    <Show when={props.course.resource?.code}>
      {(code) => <span class="course-block__code">{code()}</span>}
    </Show>

    <Show when={!props.dense}>
      <span class="course-block__subject">
        {props.course.resource?.name ?? props.course.title ?? ''}
      </span>
    </Show>

    <span class="course-block__meta">
      <Show when={noted()}>
        <span class="course-block__noted" title="Un devoir est rattaché à ce cours">✎</span>
      </Show>
      {props.describe
        ? props.describe(props.course)
        : [props.course.room, props.course.teacher].filter(Boolean).join(' · ')}
    </span>
  </button>
  );
};
