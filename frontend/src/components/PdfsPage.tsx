import { For, Show, createMemo, createResource, createSignal, type Component } from 'solid-js';
import { CircleCheck, FileText, FileUp, Upload, X } from 'lucide-solid';

import { ApiError, clearCache, fetchMe, fetchPdfs, pdfUrl, uploadPdf, type CachedPdf } from '../lib/api';
import { shortDate } from '../lib/time';
import { button, card, cx, input, link, note, notice, page, pill, press, sectionTitle, summary } from '../lib/ui';

const YEARS = [1, 2, 3];

type Status = 'waiting' | 'sending' | 'done' | { error: string };

interface Dropped {
  file: File;
  /** `null` tant que le nom du fichier ne l'a pas dite et que personne ne l'a choisie. */
  year: number | null;
  week: number | null;
  status: Status;
}

/** `A2_S8.pdf` → BUT 2, semaine 8 ; `null` si le nom ne dit rien. */
function weekFromName(name: string): { year: number; week: number } | null {
  const found = /(?:^|[^a-z])A([123])_S(\d{1,3})(?!\d)/i.exec(name);
  if (!found) return null;
  const week = Number(found[2]);
  return week >= 1 && week <= 255 ? { year: Number(found[1]), week } : null;
}

const isPdf = (file: File) => file.type === 'application/pdf' || file.name.toLowerCase().endsWith('.pdf');

const removeButton = cx(
  press,
  'grid size-8 cursor-pointer place-items-center rounded-lg text-muted hover:bg-surface hover:text-text pointer-coarse:size-10',
);

const size = (bytes: number) => `${Math.max(1, Math.round(bytes / 1024))} Ko`;

const savedAt = (iso: string) =>
  new Date(iso).toLocaleString('fr-FR', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });

/**
 * Les PDF gardés par l'API, réservés aux administrateurs : on les consulte, et
 * quand le serveur du département est injoignable, on y dépose les nouvelles
 * versions à la main. Chacune remplace celle de sa semaine.
 */
export const PdfsPage: Component = () => {
  const [me] = createResource(fetchMe);
  const [pdfs, { refetch }] = createResource(() => me()?.admin === true, fetchPdfs);
  const [files, setFiles] = createSignal<Dropped[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [over, setOver] = createSignal(false);
  let picker: HTMLInputElement | undefined;

  const add = (list: FileList | null) => {
    const incoming = [...(list ?? [])];
    const pdfList = incoming.filter(isPdf);
    // Un fichier du même nom remplace l'ancien : on redépose une version corrigée.
    const kept = files().filter((item) => item.status !== 'done' && !pdfList.some((file) => file.name === item.file.name));
    const added = pdfList.map((file): Dropped => {
      const found = weekFromName(file.name);
      return { file, year: found?.year ?? null, week: found?.week ?? null, status: 'waiting' };
    });
    setFiles([...kept, ...added]);
    setError(pdfList.length < incoming.length ? 'Seuls les PDF sont pris.' : null);
  };

  const edit = (index: number, change: Partial<Dropped>) =>
    setFiles(files().map((item, at) => (at === index ? { ...item, ...change, status: 'waiting' } : item)));

  const remove = (index: number) => setFiles(files().filter((_, at) => at !== index));

  const pending = () => files().filter((item) => item.status !== 'done');

  /** Ce qui empêche d'envoyer, dit simplement. */
  const blocker = createMemo(() => {
    const list = pending();
    if (list.length === 0) return 'Dépose au moins un PDF.';
    if (list.some((item) => item.year === null || item.week === null)) return 'Indique l’année et la semaine de chaque PDF.';
    const keys = list.map((item) => `${item.year}:${item.week}`);
    return new Set(keys).size === keys.length ? null : 'Deux PDF pour la même semaine : garde-en un seul.';
  });

  /** Un envoi par PDF : chacun reste sous la limite d'un Mo, et un échec n'arrête pas les autres. */
  const send = async () => {
    if (blocker() !== null) return;
    setBusy(true);
    setError(null);
    const set = (file: File, status: Status) =>
      setFiles(files().map((item) => (item.file === file ? { ...item, status } : item)));
    for (const { file, year, week } of pending()) {
      if (year === null || week === null) continue;
      set(file, 'sending');
      try {
        await uploadPdf(year, week, file);
        set(file, 'done');
      } catch (failure) {
        set(file, { error: failure instanceof ApiError ? failure.message : 'Envoi impossible.' });
      }
    }
    setBusy(false);
    // L'emploi du temps affiché ailleurs doit relire la nouvelle version.
    clearCache();
    void refetch();
  };

  const statusLabel = (status: Status) =>
    status === 'sending' ? (
      <span class="text-[0.8125rem] text-muted">Envoi…</span>
    ) : status === 'done' ? (
      <CircleCheck size={16} class="text-accent-ink" aria-label="Enregistré" />
    ) : typeof status === 'object' ? (
      <span class="text-[0.8125rem] font-medium text-accent-ink">{status.error}</span>
    ) : null;

  const fileRow = (item: Dropped, index: () => number) => (
    <li class="flex min-w-0 flex-wrap items-center gap-1.5 rounded-control border border-line bg-elevated py-0.5 ps-2.5 pe-0.5">
      <span class="min-w-0 max-w-44 truncate text-[0.8125rem]" title={item.file.name}>
        {item.file.name}
      </span>
      <select
        class={input({ invalid: item.year === null })}
        aria-label={`Année de ${item.file.name}`}
        value={item.year ?? ''}
        disabled={busy() || item.status === 'done'}
        onChange={(event) => edit(index(), { year: event.currentTarget.value ? Number(event.currentTarget.value) : null })}
      >
        <option value="">Année…</option>
        <For each={YEARS}>{(year) => <option value={year}>{`BUT ${year}`}</option>}</For>
      </select>
      <input
        type="number"
        min="1"
        max="255"
        inputmode="numeric"
        placeholder="Semaine"
        class={cx(input({ invalid: item.week === null }), 'w-24')}
        aria-label={`Semaine de ${item.file.name}`}
        value={item.week ?? ''}
        disabled={busy() || item.status === 'done'}
        onInput={(event) => {
          const value = Number(event.currentTarget.value);
          edit(index(), { week: Number.isInteger(value) && value >= 1 && value <= 255 ? value : null });
        }}
      />
      {statusLabel(item.status)}
      <button
        type="button"
        class={removeButton}
        title="Retirer"
        aria-label={`Retirer ${item.file.name}`}
        disabled={busy()}
        onClick={() => remove(index())}
      >
        <X size={15} aria-hidden="true" />
      </button>
    </li>
  );

  const pdfRow = (year: number, pdf: CachedPdf) => (
    <li class="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-control bg-elevated px-3 py-2 text-[0.8125rem]">
      <a class={cx(link, 'inline-flex items-center gap-1.5 font-semibold')} href={pdfUrl(year, pdf.week)} target="_blank" rel="noopener">
        <FileText size={14} aria-hidden="true" />
        {`Semaine ${pdf.week}`}
      </a>
      <Show when={pdf.period}>
        {(period) => <span>{`${shortDate(period().start)} – ${shortDate(period().end)}`}</span>}
      </Show>
      <Show when={pdf.courses !== null}>
        <span class="text-muted">{`${pdf.courses} cours`}</span>
      </Show>
      <span class="ms-auto text-muted tabular-nums">{`${size(pdf.bytes)} · ${savedAt(pdf.saved_at)}`}</span>
    </li>
  );

  return (
    <div class={page}>
      <div class={summary}>
        <div
          class={cx(card, 'flex flex-wrap items-center gap-2 p-2 transition-colors', over() && 'border-accent bg-accent-wash')}
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
            if (me()?.admin) add(event.dataTransfer?.files ?? null);
          }}
        >
          <button type="button" class={button.plain} disabled={!me()?.admin || busy()} onClick={() => picker?.click()}>
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
              event.currentTarget.value = '';
            }}
          />

          <Show
            when={files().length > 0}
            fallback={<span class="text-[0.8125rem] text-muted">ou glisse-les ici — nommés A1_S12.pdf, ils se rangent seuls.</span>}
          >
            <ul class="order-last flex w-full min-w-0 flex-wrap gap-2">
              <For each={files()}>{fileRow}</For>
            </ul>
          </Show>

          <div class="ms-auto flex items-center gap-2">
            <Show when={files().length > 0 && blocker()}>
              {(reason) => <span class="text-[0.8125rem] text-muted">{reason()}</span>}
            </Show>
            <button type="button" class={button.primary} disabled={blocker() !== null || busy()} onClick={() => void send()}>
              <Upload size={16} aria-hidden="true" />
              {busy() ? 'Envoi…' : 'Enregistrer'}
            </button>
          </div>
        </div>
      </div>

      <main class="timetable">
        <div class="mx-auto grid w-full max-w-176 gap-gap px-edge pb-6">
          <Show when={error()}>{(message) => <p class={notice(true)}>{message()}</p>}</Show>

          <Show when={!me.loading} fallback={<p class={cx(note, 'text-center')}>Chargement…</p>}>
            <Show
              when={me()?.admin}
              fallback={
                <p class={cx(note, 'px-1 text-center')}>
                  {me() ? 'Cette page est réservée aux administrateurs.' : 'Connecte-toi avec Discord, depuis la page Compte, pour accéder à cette page.'}
                </p>
              }
            >
              <p class={cx(note, 'px-1 text-center')}>
                Un PDF déposé remplace celui de sa semaine et sert aussitôt l’emploi du temps. Dès que le serveur du
                département répond de nouveau, sa version reprend la main.
              </p>

              <Show when={pdfs.error}>
                {(failure) => <p class={notice(true)}>{`Liste indisponible : ${(failure() as Error).message}`}</p>}
              </Show>

              <For each={pdfs()}>
                {(year) => (
                  <section>
                    <h2 class={sectionTitle}>
                      {`BUT ${year.year}`} <span class={pill}>{year.pdfs.length}</span>
                    </h2>
                    <Show when={year.pdfs.length > 0} fallback={<p class={note}>Aucun PDF en cache.</p>}>
                      <ul class="grid gap-1.5">
                        <For each={year.pdfs}>{(pdf) => pdfRow(year.year, pdf)}</For>
                      </ul>
                    </Show>
                  </section>
                )}
              </For>
            </Show>
          </Show>
        </div>
      </main>
    </div>
  );
};
