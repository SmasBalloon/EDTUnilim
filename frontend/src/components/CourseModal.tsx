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
import { badge, button, fieldLabel } from '../lib/ui';
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
      class={`kind-${shown()?.kind ?? 'controle'}`}
    >
      <Show when={shown()}>
        {(course) => (
          <>
            <header class="mb-5 flex items-start justify-between gap-4">
              <div>
                {/* « lundi 7 septembre » : seule l'initiale se capitalise en français. */}
                <p class="text-sm font-semibold first-letter:uppercase">{longDate(course().date)}</p>
                <p class="mt-0.5 text-sm text-muted tabular-nums">
                  {`${course().start} – ${course().end} · ${humanDuration(course().duration_minutes)}`}
                </p>
              </div>
              <button type="button" class={button.plain} onClick={props.onClose} aria-label="Fermer">
                <X size={16} aria-hidden="true" />
              </button>
            </header>

            <div class="flex items-center gap-2.5">
              <span class={badge.kind}>{KIND_LABEL[course().kind]}</span>
              <h2 class="text-2xl font-semibold">
                {course().resource?.code ?? course().title ?? 'Cours'}
              </h2>
            </div>

            <Show when={course().resource?.name ?? course().title}>
              {(subject) => <p class="mt-1.5 text-base">{subject()}</p>}
            </Show>

            <Show when={props.slot}>
              {(slot) => (
                <section class="mt-5 border-t border-line pt-4">
                  <h3 class={fieldLabel}>Devoirs</h3>
                  <ul class="mb-2.5 grid gap-1.5">
                    <For each={about(course())}>
                      {(item) => (
                        <li class="flex items-center gap-2 text-sm">
                          <span class="min-w-0 flex-1 truncate">{item.title}</span>
                          {/* Un devoir noté avant que la semaine paraisse
                              attend son créneau : on le lui donne ici. Seul
                              celui qui l'a noté peut le rattacher. */}
                          <Show when={item.mine && !onCourse(item, course())}>
                            <button
                              type="button"
                              class={button.plain}
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
                    class={`${button.plain} w-full`}
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

            <dl class="mt-5 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 border-t border-line pt-4 text-sm [&_dt]:text-muted">
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
                            class="cursor-pointer text-start underline decoration-accent/60 underline-offset-[0.15em] hover:text-accent"
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
