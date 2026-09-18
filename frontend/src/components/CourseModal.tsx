import { For, Show, createSignal, createEffect, type Component } from 'solid-js';
import { Link, NotebookPen, X } from 'lucide-solid';

import type { Course } from '../lib/api';
import {
  about,
  audienceFor,
  courseRef,
  editHomework,
  onCourse,
  type Draft,
  type Homework,
} from '../lib/homework';
import { groupLabel } from '../lib/groups';
import { KIND_LABEL } from '../lib/layout';
import { settings, update } from '../lib/settings';
import { humanDuration, longDate } from '../lib/time';
import { Sheet } from './Sheet';

interface Props {
  /** Le cours à détailler, ou `null` quand le modal est fermé. */
  course: Course | null;
  /**
   * L'année et la semaine affichées, quand on regarde son propre emploi du
   * temps : c'est ce qu'il faut pour rattacher un devoir à ce créneau. Absent
   * ailleurs — on ne note pas de devoir depuis l'emploi du temps d'autrui.
   */
  slot?: { year: number; week: number };
  /** Ouvre le formulaire d'un nouveau devoir, déjà rempli par ce cours. */
  onNote?: (draft: Partial<Draft>) => void;
  onClose: () => void;
}

/** Rattache un devoir déjà noté au créneau qu'on regarde. */
async function attach(item: Homework, course: Course, slot: { year: number; week: number }) {
  await editHomework(item.id, {
    title: item.title,
    details: item.details ?? null,
    resource: item.resource ?? null,
    due: item.due ?? null,
    audience: item.audience,
    course: courseRef(course, slot.year, slot.week),
  });
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

            <Show when={props.slot}>
              {(slot) => (
                <section class="course-modal__homework">
                  <h3 class="field__label">Devoirs</h3>
                  <ul class="course-modal__notes">
                    <For each={about(course())}>
                      {(item) => (
                        <li class="course-modal__note">
                          <span class="course-modal__note-title">{item.title}</span>
                          {/* Un devoir noté avant que la semaine paraisse
                              attend son créneau : on le lui donne ici. Seul
                              celui qui l'a noté peut le rattacher. */}
                          <Show when={item.mine && !onCourse(item, course())}>
                            <button
                              type="button"
                              class="button"
                              title="Rattacher à ce cours"
                              onClick={() => void attach(item, course(), slot())}
                            >
                              <Link size={15} aria-hidden="true" />
                              Rattacher
                            </button>
                          </Show>
                        </li>
                      )}
                    </For>
                  </ul>

                  <button
                    type="button"
                    class="button button--block"
                    onClick={() =>
                      props.onNote?.({
                        resource: course().resource?.code ?? null,
                        due: course().date,
                        audience: audienceFor(course(), settings().group),
                        course: courseRef(course(), slot().year, slot().week),
                      })
                    }
                  >
                    <NotebookPen size={16} aria-hidden="true" />
                    Noter un devoir pour ce cours
                  </button>
                </section>
              )}
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
