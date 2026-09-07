import { Show, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { KIND_LABEL } from '../lib/layout';

interface Props {
  course: Course;
  /** Cours en train de se dérouler : la case est cerclée d'accent. */
  live: boolean;
  /** Créneau trop court pour afficher l'intitulé. */
  dense: boolean;
  /** Position et hauteur, calculées par la grille. */
  style: Record<string, string>;
  onSelect: () => void;
}

/** Une case de la grille horaire ; le détail s'ouvre en modal. */
export const CourseBlock: Component<Props> = (props) => (
  <button
    type="button"
    class={`course-block kind-${props.course.kind}`}
    classList={{ 'course-block--live': props.live }}
    style={props.style}
    onClick={props.onSelect}
  >
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
      {[props.course.room, props.course.teacher].filter(Boolean).join(' · ')}
    </span>
  </button>
);
