import { For, Show, createEffect, createSignal, on, type Component } from 'solid-js';

import { Link2Off } from 'lucide-solid';

import { ApiError } from '../lib/api';
import {
  base,
  createHomework,
  editHomework,
  type Audience,
  type CourseRef,
  type Draft,
  type Homework,
} from '../lib/homework';
import { settings } from '../lib/settings';
import { longDate } from '../lib/time';
import { button, input, label, notice, segment } from '../lib/ui';
import { Sheet, SheetHeader } from './Sheet';

interface Props {
  open: boolean;
  onClose: () => void;
  /** Le devoir à corriger, ou les valeurs de départ d'un nouveau. */
  item?: Homework | null;
  initial?: Partial<Draft> | null;
}

interface FormProps {
  /** Repart de zéro (ou de `item` / `initial`) chaque fois qu'il devient vrai. */
  active: boolean;
  item?: Homework | null;
  initial?: Partial<Draft> | null;
  /** Le devoir est enregistré. */
  onDone: () => void;
  /** Pour placer le focus depuis l'extérieur (bouton « Noter un devoir »). */
  ref?: (title: HTMLInputElement) => void;
}

/** Les portées proposées, de la plus étroite à la plus large. */
function choices(year: number | null, group: string | null): { audience: Audience; label: string }[] {
  const list: { audience: Audience; label: string }[] = [
    { audience: { kind: 'personal' }, label: 'Pour moi' },
  ];
  if (group) list.push({ audience: { kind: 'class', group }, label: group.toUpperCase() });
  const whole = base(group);
  if (whole && whole !== group) list.push({ audience: { kind: 'class', group: whole }, label: whole.toUpperCase() });
  if (year !== null) list.push({ audience: { kind: 'class' }, label: `BUT ${year}` });
  return list;
}

function same(a: Audience, b: Audience): boolean {
  if (a.kind !== b.kind) return false;
  return a.kind === 'personal' || (a.group ?? null) === ((b as { group?: string }).group ?? null);
}

/**
 * Le formulaire d'un devoir (`devoirs.html`, colonne de droite).
 *
 * La portée se choisit à chaque fois, parce qu'elle change avec la nature du
 * cours : un TP ne concerne qu'un demi-groupe, un amphi toute l'année. Quand
 * le devoir naît depuis un cours, elle est proposée d'après lui.
 */
export const HomeworkForm: Component<FormProps> = (props) => {
  const [title, setTitle] = createSignal('');
  const [resource, setResource] = createSignal('');
  const [due, setDue] = createSignal('');
  const [details, setDetails] = createSignal('');
  const [audience, setAudience] = createSignal<Audience>({ kind: 'personal' });
  const [course, setCourse] = createSignal<CourseRef | null>(null);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  /** Le formulaire prend l'état du devoir ou celui du cours d'où il vient. */
  const reset = () => {
    const item = props.item;
    const start = props.initial;
    setTitle(item?.title ?? start?.title ?? '');
    setResource(item?.resource ?? start?.resource ?? '');
    setDue(item?.due ?? start?.due ?? '');
    setDetails(item?.details ?? start?.details ?? '');
    setAudience(item?.audience ?? start?.audience ?? { kind: 'personal' });
    setCourse(item?.course ?? start?.course ?? null);
    setError(null);
  };

  createEffect(
    on(
      () => props.active,
      (active) => {
        if (active) reset();
      },
    ),
  );

  const save = async (event: Event) => {
    event.preventDefault();
    if (saving()) return;
    setSaving(true);
    setError(null);

    const draft: Draft = {
      title: title().trim(),
      resource: resource().trim() || null,
      due: due() || null,
      details: details().trim() || null,
      audience: audience(),
      course: course(),
    };
    try {
      const item = props.item;
      if (item) await editHomework(item.id, draft);
      else await createHomework(draft);
      reset();
      props.onDone();
    } catch (failure) {
      setError(failure instanceof ApiError ? failure.message : 'enregistrement impossible');
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class="flex flex-col gap-4" onSubmit={save}>
      <label class="hw-label">
        <span class={label}>À faire</span>
        <input
          ref={props.ref}
          class={input}
          type="text"
          required
          maxLength={200}
          placeholder="Exercices 3 à 7"
          value={title()}
          onInput={(event) => setTitle(event.currentTarget.value)}
        />
      </label>

      <div class="grid grid-cols-2 gap-3">
        <label class="hw-label">
          <span class={label}>Matière</span>
          <input
            class={`${input} min-w-0`}
            type="text"
            maxLength={16}
            placeholder="R1.05"
            value={resource()}
            onInput={(event) => setResource(event.currentTarget.value)}
          />
        </label>
        <label class="hw-label">
          <span class={label}>Pour le</span>
          <input
            class={`${input} min-w-0`}
            type="date"
            value={due()}
            onInput={(event) => setDue(event.currentTarget.value)}
          />
        </label>
      </div>

      <label class="hw-label">
        <span class={label}>Description</span>
        <textarea
          class={input}
          rows="3"
          maxLength={2000}
          placeholder="Ce qu’il faut faire…"
          value={details()}
          onInput={(event) => setDetails(event.currentTarget.value)}
        />
      </label>

      <div class="hw-label">
        <span class={label}>Qui le voit</span>
        <div class={`${segment} flex-wrap self-start`} role="group" aria-label="Qui le voit">
          <For each={choices(settings().year, settings().group)}>
            {(choice) => (
              <button
                type="button"
                aria-pressed={same(audience(), choice.audience)}
                onClick={() => setAudience(choice.audience)}
              >
                {choice.label}
              </button>
            )}
          </For>
        </div>
        <p class="text-[13px] text-muted">
          {audience().kind === 'personal'
            ? 'Personne d’autre ne le verra.'
            : 'Il apparaîtra chez les autres une fois que l’un d’eux l’aura confirmé.'}
        </p>
      </div>

      <Show when={course()}>
        {(linked) => (
          <div class="hw-label">
            <span class={label}>Cours rattaché</span>
            <div class="field justify-between first-letter:uppercase">
              <span>{`${longDate(linked().date)} à ${linked().start}`}</span>
              <button
                type="button"
                class="-mr-2 grid size-9 cursor-pointer place-items-center rounded-lg text-muted hover:bg-raised hover:text-text"
                onClick={() => setCourse(null)}
                title="Détacher du cours"
                aria-label="Détacher du cours"
              >
                <Link2Off size={18} aria-hidden="true" />
              </button>
            </div>
          </div>
        )}
      </Show>

      <Show when={error()}>{(message) => <p class={notice(true)}>{message()}</p>}</Show>

      <button type="submit" class={`${button.primary} wide`} disabled={saving()}>
        {saving() ? 'Enregistrement…' : 'Enregistrer'}
      </button>
    </form>
  );
};

/** Le même formulaire, dans une feuille : depuis un cours, ou pour corriger. */
export const HomeworkSheet: Component<Props> = (props) => (
  <Sheet open={props.open} onClose={props.onClose} label="Noter un devoir">
    <SheetHeader title={props.item ? 'Corriger le devoir' : 'Noter un devoir'} onClose={props.onClose} />
    <HomeworkForm active={props.open} item={props.item} initial={props.initial} onDone={props.onClose} />
  </Sheet>
);
