import {
  For,
  Show,
  createMemo,
  createResource,
  createSignal,
  type Component,
} from 'solid-js';
import { ChevronDown, Search } from 'lucide-solid';

import {
  fetchTeachers,
  fetchWeek,
  type Course,
  type Teacher,
  type Week,
  type YearSummary,
} from '../lib/api';
import { groupLabel, weekDays } from '../lib/groups';
import { settings, update } from '../lib/settings';
import { humanDuration, periodLabel } from '../lib/time';
import {
  card,
  centered,
  chip,
  field,
  fieldLabel,
  fieldRow,
  fieldStack,
  note,
  option,
  optionHint,
  optionTitle,
  page,
  pick,
  pickName,
  pill,
  summary,
} from '../lib/ui';
import { Sheet, SheetHeader } from './Sheet';
import { WeekBar } from './WeekBar';
import { WeekView } from './WeekView';

interface Props {
  years: YearSummary[];
  week: number | null;
  current: number | undefined;
  onWeek: (week: number) => void;
  onSelect: (course: Course) => void;
}

/** « BUT 1 · G1A, G2A » — pour un enseignant, la classe est l'information. */
function classOf(course: Course): string {
  const groups = course.groups.map(groupLabel).join(', ');
  const year = course.year === undefined ? '' : `BUT ${course.year}`;
  return [year, groups, course.room].filter(Boolean).join(' · ');
}

/**
 * L'emploi du temps d'un enseignant, les trois années réunies.
 *
 * Une même personne est écrite « Dubreuil L. » ou « LD » selon la place dont
 * dispose la case du PDF ; l'API rattache les deux au même identifiant (voir
 * `data/teachers.csv` côté API), et c'est par lui que les créneaux se
 * rassemblent — sans quoi la moitié de ses cours manquerait à l'appel.
 */
export const TeachersPage: Component<Props> = (props) => {
  const [picking, setPicking] = createSignal(false);
  const [known] = createResource(fetchTeachers);

  /** La semaine entière de chaque année : un enseignant les traverse toutes. */
  const [weeks] = createResource(
    () => (props.week === null ? undefined : { week: props.week, years: props.years }),
    async ({ week, years }) => {
      const loaded = await Promise.all(
        years.map((year) =>
          fetchWeek(year.year, week)
            .then((found) => ({ year: year.year, week: found }))
            .catch(() => null),
        ),
      );
      return loaded.filter((entry): entry is { year: number; week: Week } => entry !== null);
    },
  );

  /** Tous les créneaux de la semaine, l'année attachée à chacun. */
  const courses = createMemo(() =>
    (weeks() ?? []).flatMap(({ year, week }) =>
      week.courses.map((course) => ({ ...course, year })),
    ),
  );

  /**
   * Les enseignants au travail cette semaine. Ceux que le référentiel ne nomme
   * pas encore y figurent sous leur écriture brute : c'est ainsi qu'on voit ce
   * qu'il reste à y ajouter.
   */
  const present = createMemo(() => {
    const seen = new Map<string, { id: string; name: string; count: number }>();
    for (const course of courses()) {
      if (!course.teacher_id || !course.teacher) continue;
      const entry = seen.get(course.teacher_id) ?? {
        id: course.teacher_id,
        name: course.teacher,
        count: 0,
      };
      entry.count += 1;
      seen.set(course.teacher_id, entry);
    }
    return [...seen.values()].sort((a, b) => a.name.localeCompare(b.name, 'fr'));
  });

  /** Nommé au référentiel, ou seulement rencontré dans les PDF. */
  const isKnown = (id: string) => (known() ?? []).some((teacher: Teacher) => teacher.id === id);

  const chosen = () => settings().teacher;
  const name = () => present().find((teacher) => teacher.id === chosen())?.name
    ?? (known() ?? []).find((teacher) => teacher.id === chosen())?.name
    ?? null;

  const mine = createMemo(() => courses().filter((course) => course.teacher_id === chosen()));

  /** Une semaine reconstituée avec les seuls cours de l'enseignant. */
  const week = createMemo<Week | null>(() => {
    const base = weeks()?.[0];
    return base ? { ...base.week, courses: mine(), count: mine().length } : null;
  });

  const workload = () => mine().reduce((sum, course) => sum + course.duration_minutes, 0);

  /** N'importe quelle année suffit à dater la semaine : elles coïncident. */
  const period = () => {
    const first = weeks()?.[0];
    return first ? periodLabel(first.week) : undefined;
  };

  return (
    <div class={page}>
      <div class={summary}>
        <div class={`${card} flex items-center gap-2 p-2`}>
          {/* Le nom ne s'étire pas sur toute la carte : il garde la taille d'un bouton. */}
          <button
            type="button"
            class={`${pick} min-w-48 flex-initial justify-between gap-3`}
            onClick={() => setPicking(true)}
            aria-label="Choisir un enseignant"
          >
            <span class={pickName}>{name() ?? 'Choisir un enseignant'}</span>
            <ChevronDown size={16} aria-hidden="true" />
          </button>
          <Show when={chosen() && mine().length > 0}>
            <span class={pill}>
              {`${mine().length} cours · ${humanDuration(workload())}`}
            </span>
          </Show>
        </div>

        <WeekBar
          week={props.week}
          weeks={props.years[0]?.weeks ?? []}
          current={props.current}
          period={period()}
          onWeek={props.onWeek}
        />
      </div>

      <main class="timetable">
        <Show
          when={chosen() !== null}
          fallback={
            <div class={centered}>
              <p>Choisis un enseignant pour voir sa semaine, les trois années réunies.</p>
            </div>
          }
        >
          <Show
            when={mine().length > 0 ? week() : null}
            fallback={
              <div class={centered}>
                <p>
                  {props.week === null || weeks.loading
                    ? 'Chargement de la semaine…'
                    : `Aucun cours cette semaine pour ${name() ?? 'cet enseignant'}`}
                </p>
              </div>
            }
          >
            {(loaded) => (
              <WeekView
                week={loaded()}
                days={weekDays(loaded())}
                view={settings().view}
                describe={classOf}
                onSelect={props.onSelect}
              />
            )}
          </Show>
        </Show>
      </main>

      <Picker
        open={picking()}
        teachers={present()}
        isKnown={isKnown}
        chosen={chosen()}
        onChoose={(id) => {
          update({ teacher: id });
          setPicking(false);
        }}
        onClose={() => setPicking(false)}
      />
    </div>
  );
};

interface PickerProps {
  open: boolean;
  teachers: { id: string; name: string; count: number }[];
  isKnown: (id: string) => boolean;
  chosen: string | null;
  onChoose: (id: string) => void;
  onClose: () => void;
}

/** La liste des enseignants de la semaine, filtrable au clavier. */
const Picker: Component<PickerProps> = (props) => {
  const [query, setQuery] = createSignal('');

  const matching = createMemo(() => {
    const needle = query().trim().toLowerCase();
    return props.teachers.filter((teacher) => teacher.name.toLowerCase().includes(needle));
  });

  const named = () => matching().filter((teacher) => props.isKnown(teacher.id));
  const unnamed = () => matching().filter((teacher) => !props.isKnown(teacher.id));

  return (
    <Sheet open={props.open} onClose={props.onClose} label="Choisir un enseignant">
      <SheetHeader title="Enseignants" onClose={props.onClose} />

      {/* Le champ de recherche tient la poignée de la feuille : il précède la liste. */}
      <label class="mb-4 flex items-center gap-2 rounded-control border border-line bg-elevated px-2.5 text-muted">
        <Search size={16} aria-hidden="true" />
        <input
          class="min-h-[2.125rem] min-w-0 flex-1 py-1 text-sm text-text outline-none pointer-coarse:min-h-10"
          type="search"
          placeholder="Chercher un nom"
          value={query()}
          onInput={(event) => setQuery(event.currentTarget.value)}
        />
      </label>

      <section class={field}>
        <h3 class={fieldLabel}>Cette semaine</h3>
        <Show when={named().length > 0} fallback={<p class={note}>Personne à ce nom.</p>}>
          <div class={fieldStack}>
            <For each={named()}>
              {(teacher) => (
                <button
                  type="button"
                  class={`${option} flex items-baseline justify-between gap-3`}
                  aria-pressed={props.chosen === teacher.id}
                  onClick={() => props.onChoose(teacher.id)}
                >
                  <span class={optionTitle}>{teacher.name}</span>
                  <span class={optionHint}>{`${teacher.count} cours`}</span>
                </button>
              )}
            </For>
          </div>
        </Show>
      </section>

      {/* Ces écritures-là n'ont pas encore de nom au référentiel de l'API :
          les montrer, c'est dire ce qu'il reste à y ajouter. */}
      <Show when={unnamed().length > 0}>
        <section class={field}>
          <h3 class={fieldLabel}>Écritures sans nom</h3>
          <div class={fieldRow}>
            <For each={unnamed()}>
              {(teacher) => (
                <button
                  type="button"
                  class={chip}
                  aria-pressed={props.chosen === teacher.id}
                  onClick={() => props.onChoose(teacher.id)}
                >
                  {`${teacher.name} · ${teacher.count}`}
                </button>
              )}
            </For>
          </div>
        </section>
      </Show>
    </Sheet>
  );
};
