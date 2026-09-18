import { Show, createSignal, createEffect, type Component } from 'solid-js';
import { X } from 'lucide-solid';

import type { Course } from '../lib/api';
import { groupLabel } from '../lib/groups';
import { KIND_LABEL } from '../lib/layout';
import { update } from '../lib/settings';
import { humanDuration, longDate } from '../lib/time';
import { Sheet } from './Sheet';

interface Props {
  /** Le cours à détailler, ou `null` quand le modal est fermé. */
  course: Course | null;
  onClose: () => void;
}

/** Le détail d'un cours, en modal par-dessus l'emploi du temps. */
export const CourseModal: Component<Props> = (props) => {
  // Le cours reste affiché le temps que la feuille redescende : sans cela,
  // elle se viderait avant d'avoir quitté l'écran.
  const [shown, setShown] = createSignal<Course | null>(props.course);
  createEffect(() => {
    if (props.course) setShown(props.course);
  });

  return (
    <Sheet
      open={props.course !== null}
      onClose={props.onClose}
      label="Détail du cours"
      class={`course-modal kind-${shown()?.kind ?? 'controle'}`}
    >
      <Show when={shown()}>
        {(course) => (
          <>
            <header class="course-modal__header">
              <div>
                <p class="course-modal__day">{longDate(course().date)}</p>
                <p class="course-modal__time">
                  {`${course().start} – ${course().end} · ${humanDuration(course().duration_minutes)}`}
                </p>
              </div>
              <button type="button" class="button" onClick={props.onClose} aria-label="Fermer">
                <X size={16} aria-hidden="true" />
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
                    <dd>
                      {/* Son nom mène à sa semaine : c'est de là qu'on se
                          demande le plus souvent où il enseigne par ailleurs. */}
                      <Show when={course().teacher_id} fallback={teacher()}>
                        {(id) => (
                          <button
                            type="button"
                            class="course-modal__teacher"
                            onClick={() => {
                              update({ teacher: id() });
                              props.onClose();
                              location.hash = 'enseignants';
                            }}
                          >
                            {teacher()}
                          </button>
                        )}
                      </Show>
                    </dd>
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
          </>
        )}
      </Show>
    </Sheet>
  );
};
