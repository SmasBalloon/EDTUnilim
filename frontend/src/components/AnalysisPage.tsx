import { For, Show, createMemo, createSignal, type Component } from 'solid-js';
import { CircleCheck, FileUp, ScanSearch, X } from 'lucide-solid';

import { ApiError, analyse, type Analysis, type Course, type RoomConflict } from '../lib/api';
import { KIND_LABEL } from '../lib/layout';
import { longDate, shortDate } from '../lib/time';
import {
  badge,
  button,
  card,
  cx,
  input,
  kindEdge,
  note,
  notice,
  page,
  pill,
  press,
  sectionTitle,
  summary,
} from '../lib/ui';

/** Un PDF par année : au-delà, deux versions d'une même année se disputeraient leurs salles. */
const MAX_FILES = 3;
const YEARS = [1, 2, 3];

interface Dropped {
  file: File;
  /** Année choisie ; `null` tant que le nom du fichier ne l'a pas dite et que personne ne l'a choisie. */
  year: number | null;
}

/** `A2_S8.pdf` → 2, `EDT BUT 3.pdf` → 3 ; `null` si le nom ne dit rien. */
function yearFromName(name: string): number | null {
  const found = /(?:^|[^a-z])A([123])_/i.exec(name) ?? /BUT\s*([123])/i.exec(name);
  return found ? Number(found[1]) : null;
}

const isPdf = (file: File) => file.type === 'application/pdf' || file.name.toLowerCase().endsWith('.pdf');

/** Retirer un fichier : discret, dans la pastille du fichier. */
const removeButton = cx(
  press,
  'grid size-8 cursor-pointer place-items-center rounded-lg text-muted hover:bg-surface hover:text-text pointer-coarse:size-10',
);

/** Libellé de salle comparable, comme côté API. */
const normalise = (room: string) => room.trim().toUpperCase();

/**
 * L'enseignant, sauf quand le PDF n'en donne pas : la case d'une SAÉ porte
 * souvent son groupe à la place, que la ligne affiche déjà.
 */
const teacherOf = (course: Course): string | undefined =>
  course.teacher && !course.groups.some((group) => normalise(group) === normalise(course.teacher ?? ''))
    ? course.teacher
    : undefined;

/**
 * L'analyse de PDF déposés : on y glisse les emplois du temps des trois années
 * — publiés ou pas encore — et elle dit quelles salles sont réservées par
 * plusieurs cours en même temps. Rien n'est conservé.
 */
export const AnalysisPage: Component = () => {
  const [files, setFiles] = createSignal<Dropped[]>([]);
  const [result, setResult] = createSignal<Analysis | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [over, setOver] = createSignal(false);
  let picker: HTMLInputElement | undefined;

  /** Un dépôt efface le résultat précédent : il ne correspondrait plus aux fichiers. */
  const changed = (next: Dropped[]) => {
    setFiles(next);
    setResult(null);
    setError(null);
  };

  const add = (list: FileList | null) => {
    const incoming = [...(list ?? [])];
    const pdfs = incoming.filter(isPdf);
    // Un fichier du même nom remplace l'ancien : on redépose une version corrigée.
    const kept = files().filter((item) => !pdfs.some((file) => file.name === item.file.name));
    const next = [...kept, ...pdfs.map((file) => ({ file, year: yearFromName(file.name) }))];
    changed(next.slice(0, MAX_FILES));
    if (pdfs.length < incoming.length) setError('Seuls les PDF sont pris.');
    else if (next.length > MAX_FILES)
      setError(`${MAX_FILES} PDF au plus, un par année : les suivants sont laissés de côté.`);
  };

  const setYear = (index: number, year: number | null) =>
    changed(files().map((item, at) => (at === index ? { ...item, year } : item)));

  const remove = (index: number) => changed(files().filter((_, at) => at !== index));

  /** Ce qui empêche de lancer l'analyse, dit simplement. */
  const blocker = createMemo(() => {
    const list = files();
    if (list.length === 0) return 'Dépose au moins un PDF.';
    if (list.some((item) => item.year === null)) return 'Choisis l’année de chaque PDF.';
    const twice = YEARS.find((year) => list.filter((item) => item.year === year).length > 1);
    return twice === undefined ? null : `Deux PDF pour le BUT ${twice} : garde-en un seul.`;
  });

  const run = async () => {
    if (blocker() !== null) return;
    setBusy(true);
    setError(null);
    try {
      setResult(await analyse(files().map((item) => ({ year: item.year ?? 0, file: item.file }))));
    } catch (failure) {
      setResult(null);
      setError(failure instanceof ApiError ? failure.message : 'Analyse impossible.');
    } finally {
      setBusy(false);
    }
  };

  const conflictCard = (conflict: RoomConflict) => (
    <li class={`${card} overflow-hidden`}>
      <div class="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2">
        <span class={badge.accent}>{`Salle ${conflict.room}`}</span>
        <span class="text-sm font-medium first-letter:uppercase">{longDate(conflict.date)}</span>
      </div>
      <ul class="grid gap-1.5 p-2">
        <For each={conflict.bookings}>
          {({ year, course }) => (
            <li
              class={cx(
                `kind-${course.kind}`,
                kindEdge,
                'grid gap-0.5 rounded-control bg-elevated px-3 py-2 text-[0.8125rem]',
              )}
            >
              <p class="flex flex-wrap items-center gap-x-2 gap-y-1">
                <span class="font-semibold tabular-nums">{`${course.start}–${course.end}`}</span>
                <span class={pill}>{`BUT ${year}`}</span>
                <span class={badge.kind}>{KIND_LABEL[course.kind]}</span>
                <span class="font-medium">{course.resource?.code ?? course.title ?? ''}</span>
              </p>
              <p class="flex flex-wrap gap-x-3 text-muted">
                <span>{course.groups.join(', ')}</span>
                <Show when={teacherOf(course)}>{(teacher) => <span>{teacher()}</span>}</Show>
                {/* La salle telle que le PDF l'écrit, sur chaque cours : `108` face à `108-9`
                    montre d'un coup d'œil d'où vient le conflit. */}
                <Show when={course.room}>{(room) => <span>{`salle ${room()}`}</span>}</Show>
              </p>
            </li>
          )}
        </For>
      </ul>
    </li>
  );

  /** Un fichier déposé : son nom, son année, et de quoi le retirer. */
  const fileChip = (item: Dropped, index: () => number) => (
    <li class="flex min-w-0 items-center gap-1.5 rounded-control border border-line bg-elevated py-0.5 ps-2.5 pe-0.5">
      <span class="min-w-0 max-w-44 truncate text-[0.8125rem]" title={item.file.name}>
        {item.file.name}
      </span>
      <select
        class={input({ invalid: item.year === null })}
        aria-label={`Année de ${item.file.name}`}
        value={item.year ?? ''}
        onChange={(event) => setYear(index(), event.currentTarget.value ? Number(event.currentTarget.value) : null)}
      >
        <option value="">Année…</option>
        <For each={YEARS}>{(year) => <option value={year}>{`BUT ${year}`}</option>}</For>
      </select>
      <button
        type="button"
        class={removeButton}
        title="Retirer"
        aria-label={`Retirer ${item.file.name}`}
        onClick={() => remove(index())}
      >
        <X size={15} aria-hidden="true" />
      </button>
    </li>
  );

  return (
    <div class={page}>
      {/* La carte de dépôt reste en tête, posée sur le bandeau comme sur les
          autres pages ; seuls les résultats défilent, sous le bandeau. */}
      <div class={summary}>
        <div
          class={cx(
            card,
            'flex flex-wrap items-center gap-2 p-2 transition-colors',
            over() && 'border-accent bg-accent-wash',
          )}
          onDragOver={(event) => {
            event.preventDefault();
            setOver(true);
          }}
          onDragLeave={(event) => {
            // Passer d'un enfant à l'autre n'est pas quitter la carte.
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setOver(false);
          }}
          onDrop={(event) => {
            event.preventDefault();
            setOver(false);
            add(event.dataTransfer?.files ?? null);
          }}
        >
          <button type="button" class={button.plain} onClick={() => picker?.click()}>
            <FileUp size={16} aria-hidden="true" />
            Choisir des PDF
          </button>
          <input
            ref={picker}
            type="file"
            accept=".pdf,application/pdf"
            multiple
            class="hidden"
            onChange={(event) => {
              add(event.currentTarget.files);
              // Le même fichier pourra être redéposé après un retrait.
              event.currentTarget.value = '';
            }}
          />

          <Show
            when={files().length > 0}
            fallback={<span class="text-[0.8125rem] text-muted">ou glisse-les ici — un par année, trois au plus.</span>}
          >
            <ul class="order-last flex w-full min-w-0 flex-wrap gap-2">
              <For each={files()}>{fileChip}</For>
            </ul>
          </Show>

          <div class="ms-auto flex items-center gap-2">
            <Show when={files().length > 0 && blocker()}>
              {(reason) => <span class="text-[0.8125rem] text-muted">{reason()}</span>}
            </Show>
            <button
              type="button"
              class={button.primary}
              disabled={blocker() !== null || busy()}
              onClick={() => void run()}
            >
              <ScanSearch size={16} aria-hidden="true" />
              {busy() ? 'Analyse…' : 'Analyser'}
            </button>
          </div>
        </div>
      </div>

      <main class="timetable">
        <div class="mx-auto grid w-full max-w-176 gap-gap px-edge pb-6">
          {/* La lecture des PDF reste une interprétation : on le dit, discrètement, avant tout résultat. */}
          <p class="text-center text-2xs text-muted">
            Logiciel non officiel : une erreur n’est pas impossible, et un conflit peut ne pas être trouvé.
          </p>

          <Show when={error()}>{(message) => <p class={notice(true)}>{message()}</p>}</Show>

          <Show
            when={result()}
            fallback={
              <Show when={!error()}>
                <p class={cx(note, 'px-1 text-center')}>
                  Dépose les emplois du temps d’une même semaine — publiés ou pas encore — pour voir quelles salles sont
                  réservées par plusieurs cours en même temps. Rien n’est conservé.
                </p>
              </Show>
            }
          >
            {(analysis) => (
              <>
                <ul class={cx(notice(), 'grid gap-1')}>
                  <For each={analysis().weeks}>
                    {(week) => (
                      <li class="min-w-0 truncate">
                        <span class="font-semibold">{`BUT ${week.year}`}</span>
                        <span class="text-muted">
                          {` · ${week.period.week_number ? `semaine ${week.period.week_number} · ` : ''}${shortDate(week.period.start)} – ${shortDate(week.period.end)} · ${week.courses} cours · ${week.file}`}
                        </span>
                      </li>
                    )}
                  </For>
                </ul>

                <Show when={!analysis().same_dates}>
                  <p class={notice(true)}>
                    Ces PDF ne couvrent pas les mêmes dates : leurs cours ne peuvent pas se chevaucher. Vérifie que ce
                    sont bien les emplois du temps d’une même semaine.
                  </p>
                </Show>

                <Show
                  when={analysis().conflicts.length > 0}
                  fallback={
                    <div class={cx(note, 'grid place-items-center gap-2 py-6 text-center')}>
                      <CircleCheck size={28} aria-hidden="true" />
                      <p>Aucune salle réservée deux fois dans ces PDF.</p>
                    </div>
                  }
                >
                  <section>
                    <h2 class={sectionTitle}>
                      Salles réservées plusieurs fois <span class={pill}>{analysis().conflicts.length}</span>
                    </h2>
                    <ul class="mt-2 grid gap-3">
                      <For each={analysis().conflicts}>{conflictCard}</For>
                    </ul>
                  </section>
                </Show>
              </>
            )}
          </Show>
        </div>
      </main>
    </div>
  );
};
