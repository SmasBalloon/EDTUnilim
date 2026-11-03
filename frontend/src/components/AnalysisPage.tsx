import { For, Show, createMemo, createSignal, type Component } from 'solid-js';
import { FileUp, X } from 'lucide-solid';

import { ApiError, analyse, type Analysis, type Course, type RoomConflict } from '../lib/api';
import { KIND_LABEL } from '../lib/layout';
import { longDate, shortDate, toMinutes } from '../lib/time';
import { formatMinutes } from '../lib/rooms';
import { button, cx, kindTag, notice, sectionTitle } from '../lib/ui';
import { Empty, Foot, PageHead } from './Shell';

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

/** `412 Ko` */
const size = (bytes: number) => `${Math.max(1, Math.round(bytes / 1024))} Ko`;

/** Le créneau où les réservations se chevauchent : `10:00 – 11:30`. */
function overlap(conflict: RoomConflict): string {
  const starts = conflict.bookings.map(({ course }) => toMinutes(course.start));
  const ends = conflict.bookings.map(({ course }) => toMinutes(course.end));
  const from = Math.max(...starts);
  const to = Math.min(...ends);
  return to > from ? `${formatMinutes(from)} – ${formatMinutes(to)}` : '';
}

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
    <li class="card ana-cf">
      <div>
        <p class="room">{conflict.room}</p>
        <p class="when first-letter:uppercase">{[longDate(conflict.date), overlap(conflict)].filter(Boolean).join(' · ')}</p>
      </div>
      <For each={conflict.bookings}>
        {({ year, course }) => (
          <div class="ana-cc">
            <div class="row justify-between gap-2">
              <b class="min-w-0 truncate">
                {[course.resource?.code ?? course.title, course.resource?.name].filter(Boolean).join(' · ') || 'Cours'}
              </b>
              <span class="row gap-1.5">
                <span class={kindTag(course.kind)}>{KIND_LABEL[course.kind]}</span>
                <span class={`tag y${year}`}>{`BUT ${year}`}</span>
              </span>
            </div>
            {/* La salle telle que le PDF l'écrit, sur chaque cours : `108` face à `108-9`
                montre d'un coup d'œil d'où vient le conflit. */}
            <small>
              {[
                `${course.start} – ${course.end}`,
                course.groups.join(', '),
                teacherOf(course),
                course.room && `salle ${course.room}`,
              ]
                .filter(Boolean)
                .join(' · ')}
            </small>
          </div>
        )}
      </For>
    </li>
  );

  /** Un fichier déposé : son nom, son poids, son année, et de quoi le retirer. */
  const fileChip = (item: Dropped, index: () => number) => (
    <li class="ana-file">
      <span class="name" title={item.file.name}>
        {item.file.name}
      </span>
      <small>{size(item.file.size)}</small>
      <select
        aria-invalid={item.year === null}
        aria-label={`Année de ${item.file.name}`}
        value={item.year ?? ''}
        onChange={(event) => setYear(index(), event.currentTarget.value ? Number(event.currentTarget.value) : null)}
      >
        <option value="">Année…</option>
        <For each={YEARS}>{(year) => <option value={year}>{`BUT ${year}`}</option>}</For>
      </select>
      <button type="button" class="x" title="Retirer" aria-label={`Retirer ${item.file.name}`} onClick={() => remove(index())}>
        <X size={16} aria-hidden="true" />
      </button>
    </li>
  );

  return (
    <>
      <PageHead title="Analyse" sub="Les salles réservées deux fois dans les PDF que tu déposes." />

      <div
        class={cx('card ana-drop', over() && 'over')}
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
        <FileUp size={36} strokeWidth={1.5} class="text-muted" aria-hidden="true" />
        <h2>Dépose les emplois du temps d’une même semaine</h2>
        <p>
          Publiés ou pas encore — un par année, trois au plus. Rien n’est conservé. Un logiciel peut se tromper : un
          conflit non détecté n’est pas impossible.
        </p>
        <div class="row mt-1.5">
          <button type="button" class={button.plain} onClick={() => picker?.click()}>
            Choisir des PDF
          </button>
        </div>
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
      </div>

      <Show when={files().length > 0}>
        <div class="row justify-between">
          <ul class="row min-w-0">
            <For each={files()}>{fileChip}</For>
          </ul>
          <div class="row">
            <Show when={blocker()}>{(reason) => <span class="text-sm text-muted">{reason()}</span>}</Show>
            <button
              type="button"
              class={`${button.primary} min-h-12!`}
              disabled={blocker() !== null || busy()}
              onClick={() => void run()}
            >
              {busy() ? 'Analyse…' : 'Analyser'}
            </button>
          </div>
        </div>
      </Show>

      <Show when={error()}>{(message) => <p class={notice(true)}>{message()}</p>}</Show>

      <Show when={result()}>
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
                Ces PDF ne couvrent pas les mêmes dates : leurs cours ne peuvent pas se chevaucher. Vérifie que ce sont
                bien les emplois du temps d’une même semaine.
              </p>
            </Show>

            <Show
              when={analysis().conflicts.length > 0}
              fallback={<Empty title="Aucun conflit">Aucune salle réservée deux fois dans ces PDF.</Empty>}
            >
              <section class="flex flex-col gap-3.5">
                <div class="row gap-2.5">
                  <h2 class={sectionTitle}>Conflits de salle</h2>
                  <span class="tag t-room">
                    {`${analysis().conflicts.length} trouvé${analysis().conflicts.length > 1 ? 's' : ''}`}
                  </span>
                </div>
                <ul class="flex flex-col gap-3.5">
                  <For each={analysis().conflicts}>{conflictCard}</For>
                </ul>
              </section>
            </Show>
          </>
        )}
      </Show>

      <Foot>
        <span>Logiciel non officiel : une erreur n’est pas impossible, et un conflit peut ne pas être trouvé.</span>
      </Foot>
    </>
  );
};
