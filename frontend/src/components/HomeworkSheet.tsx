import { For, Show, createEffect, createSignal, on, type Component } from 'solid-js';
import { Link2Off, X } from 'lucide-solid';

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
import { Sheet } from './Sheet';

interface Props {
  open: boolean;
  onClose: () => void;
  /** Le devoir à corriger, ou les valeurs de départ d'un nouveau. */
  item?: Homework | null;
  initial?: Partial<Draft> | null;
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
 * Le formulaire d'un devoir.
 *
 * La portée se choisit à chaque fois, parce qu'elle change avec la nature du
 * cours : un TP ne concerne qu'un demi-groupe, un amphi toute l'année. Quand
 * le devoir naît depuis un cours, elle est proposée d'après lui.
 */
export const HomeworkSheet: Component<Props> = (props) => {
  const [title, setTitle] = createSignal('');
  const [resource, setResource] = createSignal('');
  const [due, setDue] = createSignal('');
  const [details, setDetails] = createSignal('');
  const [audience, setAudience] = createSignal<Audience>({ kind: 'personal' });
  const [course, setCourse] = createSignal<CourseRef | null>(null);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // À l'ouverture, le formulaire prend l'état du devoir ou celui du cours d'où
  // il vient ; il ne garde rien de la fois précédente.
  createEffect(
    on(
      () => props.open,
      (open) => {
        if (!open) return;
        const item = props.item;
        const start = props.initial;
        setTitle(item?.title ?? start?.title ?? '');
        setResource(item?.resource ?? start?.resource ?? '');
        setDue(item?.due ?? start?.due ?? '');
        setDetails(item?.details ?? start?.details ?? '');
        setAudience(item?.audience ?? start?.audience ?? { kind: 'personal' });
        setCourse(item?.course ?? start?.course ?? null);
        setError(null);
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
      props.onClose();
    } catch (failure) {
      setError(failure instanceof ApiError ? failure.message : 'enregistrement impossible');
    } finally {
      setSaving(false);
    }
  };

  return (
    <Sheet open={props.open} onClose={props.onClose} label="Noter un devoir">
      <header class="sheet__header">
        <h2 class="sheet__title">{props.item ? 'Corriger le devoir' : 'Noter un devoir'}</h2>
        <button type="button" class="button" onClick={props.onClose} aria-label="Fermer">
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <form onSubmit={save}>
        <section class="field">
          <h3 class="field__label">À faire</h3>
          <input
            class="input input--block"
            type="text"
            required
            maxLength={200}
            placeholder="Exercices 3 à 7"
            value={title()}
            onInput={(event) => setTitle(event.currentTarget.value)}
          />
        </section>

        <section class="field">
          <h3 class="field__label">Matière et échéance</h3>
          <div class="field__row">
            <input
              class="input"
              type="text"
              maxLength={16}
              placeholder="R1.05"
              aria-label="Code de la ressource"
              value={resource()}
              onInput={(event) => setResource(event.currentTarget.value)}
            />
            <input
              class="input"
              type="date"
              aria-label="Date de rendu"
              value={due()}
              onInput={(event) => setDue(event.currentTarget.value)}
            />
          </div>
        </section>

        <section class="field">
          <h3 class="field__label">Qui le voit</h3>
          <div class="field__row">
            <For each={choices(settings().year, settings().group)}>
              {(choice) => (
                <button
                  type="button"
                  class="chip"
                  aria-pressed={same(audience(), choice.audience)}
                  onClick={() => setAudience(choice.audience)}
                >
                  {choice.label}
                </button>
              )}
            </For>
          </div>
          <p class="option__hint">
            {audience().kind === 'personal'
              ? 'Personne d’autre ne le verra.'
              : 'Il apparaîtra chez les autres une fois que l’un d’eux l’aura confirmé.'}
          </p>
        </section>

        <Show when={course()}>
          {(linked) => (
            <section class="field">
              <h3 class="field__label">Cours rattaché</h3>
              <div class="homework__linked">
                <span>{`${longDate(linked().date)} à ${linked().start}`}</span>
                <button
                  type="button"
                  class="button"
                  onClick={() => setCourse(null)}
                  title="Détacher du cours"
                  aria-label="Détacher du cours"
                >
                  <Link2Off size={16} aria-hidden="true" />
                </button>
              </div>
            </section>
          )}
        </Show>

        <section class="field">
          <h3 class="field__label">Précisions</h3>
          <textarea
            class="input input--block"
            rows="3"
            maxLength={2000}
            placeholder="Page 42, à rendre sur Moodle…"
            value={details()}
            onInput={(event) => setDetails(event.currentTarget.value)}
          />
        </section>

        <Show when={error()}>{(message) => <p class="notice notice--error">{message()}</p>}</Show>

        <div class="sheet__footer">
          <button type="submit" class="button button--block button--primary" disabled={saving()}>
            {saving() ? 'Enregistrement…' : props.item ? 'Enregistrer' : 'Noter le devoir'}
          </button>
        </div>
      </form>
    </Sheet>
  );
};
