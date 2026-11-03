import { For, Show, createMemo, createResource, createSignal, type Component } from 'solid-js';
import { CircleCheck, FileText, FileUp, Upload, X } from 'lucide-solid';


import { ApiError, clearCache, fetchMe, fetchPdfs, pdfUrl, uploadPdf, type CachedPdf } from '../lib/api';
import { shortDate } from '../lib/time';
import { button, cx, link, notice, sectionTitle } from '../lib/ui';
import { Empty, PageHead } from './Shell';

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
      <span class="text-[13px] text-muted">Envoi…</span>
    ) : status === 'done' ? (
      <CircleCheck size={18} class="text-ok" aria-label="Enregistré" />
    ) : typeof status === 'object' ? (
      <span class="text-[13px] font-medium text-accent-ink">{status.error}</span>
    ) : null;

  const fileRow = (item: Dropped, index: () => number) => (
    <li class="ana-file flex-wrap py-1">
      <span class="name" title={item.file.name}>
        {item.file.name}
      </span>
      <select
        aria-invalid={item.year === null}
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
        class="min-h-[34px] w-24 rounded-[9px] border border-line bg-surface px-2 text-[13px] font-semibold tabular-nums aria-invalid:border-accent"
        aria-invalid={item.week === null}
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
        class="x"
        title="Retirer"
        aria-label={`Retirer ${item.file.name}`}
        disabled={busy()}
        onClick={() => remove(index())}
      >
        <X size={16} aria-hidden="true" />
      </button>
    </li>
  );

  const pdfRow = (year: number, pdf: CachedPdf) => (
    <li class="card flex flex-wrap items-center gap-x-4 gap-y-1 px-[18px] py-3 text-sm">
      <a class={cx(link, 'inline-flex items-center gap-2 font-semibold')} href={pdfUrl(year, pdf.week)} target="_blank" rel="noopener">
        <FileText size={18} aria-hidden="true" />
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
    <>
      <PageHead title="PDF en cache" sub="Les emplois du temps gardés par le serveur, et ceux déposés à la main." />

      <Show when={!me.loading} fallback={<Empty>Chargement…</Empty>}>
        <Show
          when={me()?.admin}
          fallback={
            <Empty title="Accès réservé">
              {me()
                ? 'Cette page est réservée aux administrateurs.'
                : 'Connecte-toi avec Discord, depuis la page Compte, pour accéder à cette page.'}
            </Empty>
          }
        >
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
            <h2>Dépose les nouvelles versions</h2>
            <p>
              Nommés A1_S12.pdf, ils se rangent seuls. Un PDF déposé remplace celui de sa semaine et sert aussitôt
              l’emploi du temps ; dès que le serveur du département répond de nouveau, sa version reprend la main.
            </p>
            <div class="row mt-1.5">
              <button type="button" class={button.plain} disabled={busy()} onClick={() => picker?.click()}>
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
                event.currentTarget.value = '';
              }}
            />
          </div>

          <Show when={files().length > 0}>
            <div class="row justify-between">
              <ul class="row min-w-0">
                <For each={files()}>{fileRow}</For>
              </ul>
              <div class="row">
                <Show when={blocker()}>{(reason) => <span class="text-sm text-muted">{reason()}</span>}</Show>
                <button
                  type="button"
                  class={`${button.primary} min-h-12!`}
                  disabled={blocker() !== null || busy()}
                  onClick={() => void send()}
                >
                  <Upload size={20} aria-hidden="true" />
                  {busy() ? 'Envoi…' : 'Enregistrer'}
                </button>
              </div>
            </div>
          </Show>

          <Show when={error()}>{(message) => <p class={notice(true)}>{message()}</p>}</Show>

          <Show when={pdfs.error}>
            {(failure) => <p class={notice(true)}>{`Liste indisponible : ${(failure() as Error).message}`}</p>}
          </Show>

          <For each={pdfs()}>
            {(year) => (
              <section class="flex flex-col gap-3.5">
                <div class="row gap-2.5">
                  <h2 class={sectionTitle}>{`BUT ${year.year}`}</h2>
                  <span class={`tag y${year.year}`}>{year.pdfs.length}</span>
                </div>
                <Show when={year.pdfs.length > 0} fallback={<p class="text-sm text-muted">Aucun PDF en cache.</p>}>
                  <ul class="flex flex-col gap-2">
                    <For each={year.pdfs}>{(pdf) => pdfRow(year.year, pdf)}</For>
                  </ul>
                </Show>
              </section>
            )}
          </For>
        </Show>
      </Show>
    </>
  );
};
