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
import { badge, button, card, centered, cx, link, page, pick, pickName, pill, sectionTitle, summary } from '../lib/ui';
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
    <li
      class={cx(
        'flex items-start gap-3 rounded-card border border-line bg-surface p-3',
        // Un devoir fait s'efface sans disparaître : on doit pouvoir le décocher.
        item.done_by_me && 'opacity-55',
        // Une proposition n'engage encore personne : elle se distingue d'un liseré.
        item.pending && 'border-s-3 border-s-accent/60',
      )}
    >
      <Show
        when={!item.pending}
        fallback={
          <span class="mt-0.5 shrink-0 whitespace-nowrap rounded-full border border-accent/50 px-[0.45rem] py-[0.1rem] text-2xs font-semibold text-accent">
            à confirmer
          </span>
        }
      >
        <button
          type="button"
          class={cx(
            'mt-0.5 grid size-[1.375rem] shrink-0 cursor-pointer place-items-center rounded-lg border-[1.5px] border-muted/60 text-on-accent',
            'transition-[background-color,border-color,scale] duration-(--settle) ease-out active:scale-90 active:duration-(--press)',
            'aria-checked:border-transparent aria-checked:bg-accent',
          )}
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

      <div class="min-w-0 flex-1">
        <p class={cx('flex flex-wrap items-baseline gap-1.5 font-medium', item.done_by_me && 'line-through')}>
          <Show when={item.resource}>
            {(code) => <span class={badge.kind}>{code()}</span>}
          </Show>
          {item.title}
        </p>
        <p class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-[0.8125rem] text-muted">
          <Show when={whenDue(item, clock())}>
            {(when) => (
              <span class={cx((item.due ?? '') <= todayIso(clock()) && 'font-medium text-accent')}>{when()}</span>
            )}
          </Show>
          <span>{audienceLabel(item)}</span>
          <Show when={!item.mine}>
            <span>{`noté par ${item.author_name}`}</span>
          </Show>
        </p>
        <Show when={item.details}>
          {(text) => <p class="mt-1.5 whitespace-pre-line text-[0.8125rem] text-muted">{text()}</p>}
        </Show>
      </div>

      <div class="flex shrink-0 items-center gap-1">
        <Show when={item.can_confirm}>
          <button
            type="button"
            class={button.plain}
            disabled={busy() === item.id}
            onClick={() => void run(item.id, () => confirmHomework(item.id))}
          >
            Confirmer
          </button>
        </Show>
        <Show when={item.mine}>
          <button
            type="button"
            class={button.round}
            title="Corriger"
            aria-label="Corriger"
            onClick={() => setEditing(item)}
          >
            <Pencil size={15} aria-hidden="true" />
          </button>
          <button
            type="button"
            class={button.round}
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
      <section>
        <h2 class={sectionTitle}>
          {title} <span class={pill}>{items.length}</span>
        </h2>
        <Show when={hint}>
          {(text) => <p class="m-0 mb-2 text-sm text-muted first-letter:uppercase">{text()}</p>}
        </Show>
        <ul class="mt-2 grid gap-2">
          <For each={items}>{row}</For>
        </ul>
      </section>
    </Show>
  );

  return (
    <div class={page}>
      <div class={summary}>
        <div class={`${card} flex items-center gap-2 p-2`}>
          <button
            type="button"
            class={`${pick} min-w-48 flex-initial justify-between gap-3`}
            onClick={() => setAdding(true)}
          >
            <Plus size={16} aria-hidden="true" />
            <span class={pickName}>Noter un devoir</span>
          </button>
          <Show when={todo().length > 0}>
            <span class={pill}>{`${todo().length} à faire`}</span>
          </Show>
        </div>
      </div>

      <main class="timetable">
        <Show
          when={!['unauthenticated', 'auth_disabled'].includes(homeworkError()?.code ?? '')}
          fallback={
            <div class={centered}>
              <p>
                Les devoirs sont liés à ton compte.{' '}
                <a class={link} href="#compte">
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
              <div class={centered}>
                <p>
                  {homeworkLoading()
                    ? 'Chargement…'
                    : 'Rien à faire pour l’instant. Note un devoir, ou ajoute-le depuis un cours.'}
                </p>
              </div>
            }
          >
            <div class="mx-auto grid max-w-176 gap-6 px-edge pb-6">
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
