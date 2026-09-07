import { Show, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { groupLabel } from '../lib/groups';
import { KIND_LABEL } from '../lib/layout';
import { humanDuration, longDate } from '../lib/time';

interface Props {
  /** Le cours à détailler, ou `null` quand le modal est fermé. */
  course: Course | null;
  onClose: () => void;
}

/** Le détail d'un cours, en modal par-dessus l'emploi du temps. */
export const CourseModal: Component<Props> = (props) => (
  <Show when={props.course}>
    {(course) => (
      <div class="sheet-backdrop" onClick={props.onClose}>
        <div
          class={`sheet course-modal kind-${course().kind}`}
          role="dialog"
          aria-modal="true"
          aria-label="Détail du cours"
          onClick={(event) => event.stopPropagation()}
        >
          <header class="course-modal__header">
            <div>
              <p class="course-modal__day">{longDate(course().date)}</p>
              <p class="course-modal__time">
                {`${course().start} – ${course().end} · ${humanDuration(course().duration_minutes)}`}
              </p>
            </div>
            <button type="button" class="button" onClick={props.onClose} aria-label="Fermer">
              ✕
            </button>
          </header>

          <div class="course-modal__title">
            <span class="badge badge--kind">{KIND_LABEL[course().kind]}</span>
            <h2 class="course-modal__code">
              {course().resource?.code ?? course().title ?? 'Cours'}
            </h2>
          </div>

          <Show when={course().resource?.name ?? course().title}>
            {(subject) => <p class="course-modal__subject">{subject()}</p>}
          </Show>

          <dl class="course-modal__facts">
            <Show when={course().teacher}>
              {(teacher) => (
                <>
                  <dt>Enseignant</dt>
                  <dd>{teacher()}</dd>
                </>
              )}
            </Show>
            <Show when={course().room}>
              {(room) => (
                <>
                  <dt>Salle</dt>
                  <dd>{room()}</dd>
                </>
              )}
            </Show>
            <dt>Groupes</dt>
            <dd>{course().groups.map(groupLabel).join(' · ')}</dd>
          </dl>
        </div>
      </div>
    )}
  </Show>
);
