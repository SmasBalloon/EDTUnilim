import { For, Show, createMemo, createSignal, onMount, type Component } from 'solid-js';
import { Check, Pencil, Plus, Trash2 } from 'lucide-solid';

import {
  audienceLabel,
  confirmHomework,
  deleteHomework,
  homework,
  homeworkError,
  homeworkLoading,
  loadHomework,
  setDone,
  type Homework,
} from '../lib/homework';
import { clock, longDate, parseDate, startOfDay, todayIso } from '../lib/time';
import { HomeworkSheet } from './HomeworkSheet';

/** « pour demain », « pour le lundi 21 septembre », « en retard ». */
function whenDue(item: Homework, now: Date): string | null {
  if (!item.due) return null;
  const days = Math.round((parseDate(item.due).getTime() - startOfDay(now).getTime()) / 86_400_000);
  if (days < 0) return `en retard de ${-days} jour${days < -1 ? 's' : ''}`;
  return ['pour aujourd’hui', 'pour demain', 'pour après-demain'][days] ?? `pour ${longDate(item.due)}`;
}

/**
 * Les devoirs : ce qu'il y a à faire, et ce qui attend d'être confirmé.
 *
 * Un devoir de classe n'est officiel qu'une fois qu'une deuxième personne l'a
 * confirmé — les propositions sont donc montrées à part, en tête, avec de quoi
 * les valider d'un geste.
 */
export const TodoPage: Component = () => {
  const [editing, setEditing] = createSignal<Homework | null>(null);
  const [adding, setAdding] = createSignal(false);
  const [busy, setBusy] = createSignal<string | null>(null);

  onMount(() => void loadHomework());

  const toConfirm = createMemo(() => homework().filter((item) => item.can_confirm));
  const waiting = createMemo(() => homework().filter((item) => item.pending && item.mine));
  const todo = createMemo(() =>
    homework().filter((item) => !item.pending && !item.done_by_me),
  );
  const done = createMemo(() => homework().filter((item) => !item.pending && item.done_by_me));

  /** Ce qui se passe pendant qu'une requête est en vol. */
  const run = async (id: string, action: () => Promise<unknown>) => {
    setBusy(id);
    try {
      await action();
    } finally {
      setBusy(null);
    }
  };

  const row = (item: Homework) => (
    <li class="todo" classList={{ 'todo--done': item.done_by_me, 'todo--pending': item.pending }}>
      <Show
        when={!item.pending}
        fallback={<span class="todo__badge">à confirmer</span>}
      >
        <button
          type="button"
          class="todo__check"
          role="checkbox"
          aria-checked={item.done_by_me}
          aria-label={item.done_by_me ? 'Marquer à faire' : 'Marquer comme fait'}
          disabled={busy() === item.id}
          onClick={() => void run(item.id, () => setDone(item.id, !item.done_by_me))}
        >
          <Show when={item.done_by_me}>
            <Check size={14} aria-hidden="true" />
          </Show>
        </button>
      </Show>

      <div class="todo__body">
        <p class="todo__title">
          <Show when={item.resource}>
            {(code) => <span class="badge badge--kind">{code()}</span>}
          </Show>
          {item.title}
        </p>
        <p class="todo__meta">
          <Show when={whenDue(item, clock())}>
            {(when) => (
              <span classList={{ accent: (item.due ?? '') <= todayIso(clock()) }}>{when()}</span>
            )}
          </Show>
          <span>{audienceLabel(item)}</span>
          <Show when={!item.mine}>
            <span>{`noté par ${item.author_name}`}</span>
          </Show>
        </p>
        <Show when={item.details}>{(text) => <p class="todo__details">{text()}</p>}</Show>
      </div>

      <div class="todo__actions">
        <Show when={item.can_confirm}>
          <button
            type="button"
            class="button"
            disabled={busy() === item.id}
            onClick={() => void run(item.id, () => confirmHomework(item.id))}
          >
            Confirmer
          </button>
        </Show>
        <Show when={item.mine}>
          <button
            type="button"
            class="button button--round"
            title="Corriger"
            aria-label="Corriger"
            onClick={() => setEditing(item)}
          >
            <Pencil size={15} aria-hidden="true" />
          </button>
          <button
            type="button"
            class="button button--round"
            title="Effacer"
            aria-label="Effacer"
            disabled={busy() === item.id}
            onClick={() => void run(item.id, () => deleteHomework(item.id))}
          >
            <Trash2 size={15} aria-hidden="true" />
          </button>
        </Show>
      </div>
    </li>
  );

  const section = (title: string, items: Homework[], hint?: string) => (
    <Show when={items.length > 0}>
      <section class="todo-section">
        <h2 class="rooms-section__title">
          {title} <span class="rooms-section__count">{items.length}</span>
        </h2>
        <Show when={hint}>{(text) => <p class="rooms-state__caption">{text()}</p>}</Show>
        <ul class="todo-list">
          <For each={items}>{row}</For>
        </ul>
      </section>
    </Show>
  );

  return (
    <div class="teachers">
      <div class="summary">
        <div class="card teachers__head">
          <button type="button" class="compare__pick todo__add" onClick={() => setAdding(true)}>
            <Plus size={16} aria-hidden="true" />
            <span class="compare__pick-name">Noter un devoir</span>
          </button>
          <Show when={todo().length > 0}>
            <span class="week-summary__workload">{`${todo().length} à faire`}</span>
          </Show>
        </div>
      </div>

      <main class="app__timetable">
        <Show
          when={!['unauthenticated', 'auth_disabled'].includes(homeworkError()?.code ?? '')}
          fallback={
            <div class="centered">
              <p>
                Les devoirs sont liés à ton compte.{' '}
                <a class="setup__link" href="#compte">
                  Connecte-toi avec Discord
                </a>{' '}
                pour les noter.
              </p>
            </div>
          }
        >
          <Show
            when={homework().length > 0}
            fallback={
              <div class="centered">
                <p>
                  {homeworkLoading()
                    ? 'Chargement…'
                    : 'Rien à faire pour l’instant. Note un devoir, ou ajoute-le depuis un cours.'}
                </p>
              </div>
            }
          >
            <div class="todo-page">
              {section(
                'À confirmer',
                toConfirm(),
                'Quelqu’un de ta classe l’a noté : confirme-le s’il est exact, il apparaîtra alors chez tout le monde.',
              )}
              {section('En attente de confirmation', waiting())}
              {section('À faire', todo())}
              {section('Fait', done())}
            </div>
          </Show>
        </Show>
      </main>

      <HomeworkSheet open={adding()} onClose={() => setAdding(false)} />
      <HomeworkSheet
        open={editing() !== null}
        item={editing()}
        onClose={() => setEditing(null)}
      />
    </div>
  );
};
