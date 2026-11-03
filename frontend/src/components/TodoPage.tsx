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
import { useMedia } from '../lib/media';
import { clock, dayStamp, longDate, parseDate, startOfDay, todayIso } from '../lib/time';
import { button, cx, label, link, segment } from '../lib/ui';
import { HomeworkForm, HomeworkSheet } from './HomeworkSheet';
import { Empty, Foot, PageHead } from './Shell';

/** Titre d'un groupe de devoirs : « Pour demain · mardi 03/11 ». */
function dueGroup(due: string | undefined, now: Date): string {
  if (!due) return 'Sans échéance';
  const days = Math.round((parseDate(due).getTime() - startOfDay(now).getTime()) / 86_400_000);
  const weekday = longDate(due).split(' ')[0] ?? '';
  const stamp = `${weekday} ${dayStamp(due)}`;
  if (days < 0) return `En retard · ${stamp}`;
  const near = ['Pour aujourd’hui', 'Pour demain', 'Pour après-demain'][days];
  return near ? `${near} · ${stamp}` : `Pour ${stamp}`;
}

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

  /** Le formulaire est posé à droite sur grand écran, en feuille ailleurs. */
  const narrow = useMedia('(max-width: 1100px)');
  const [tab, setTab] = createSignal<'todo' | 'done'>('todo');
  let titleField: HTMLInputElement | undefined;

  /** Les devoirs à faire, rangés par échéance. */
  const byDue = createMemo(() => {
    const groups = new Map<string, Homework[]>();
    const sorted = [...todo()].sort((a, b) => (a.due ?? '9999').localeCompare(b.due ?? '9999'));
    for (const item of sorted) {
      const key = dueGroup(item.due, clock());
      groups.set(key, [...(groups.get(key) ?? []), item]);
    }
    return [...groups.entries()];
  });

  const row = (item: Homework) => (
    <li class={cx('card hw-row', item.pending && 'pending')}>
      <Show
        when={!item.pending}
        fallback={<span class="hw-ck wait" title="En attente de confirmation" aria-label="En attente de confirmation" />}
      >
        <button
          type="button"
          class="hw-ck"
          role="checkbox"
          aria-checked={item.done_by_me}
          aria-label={item.done_by_me ? 'Marquer comme à faire' : 'Marquer comme fait'}
          disabled={busy() === item.id}
          onClick={() => void run(item.id, () => setDone(item.id, !item.done_by_me))}
        >
          <Show when={item.done_by_me}>
            <Check size={16} strokeWidth={3} aria-hidden="true" />
          </Show>
        </button>
      </Show>

      <div class="min-w-0">
        <b class={cx(item.done_by_me && 'done')}>{item.title}</b>
        <p>
          {[
            item.resource,
            audienceLabel(item),
            !item.mine && `noté par ${item.author_name}`,
          ]
            .filter(Boolean)
            .join(' · ')}
          <Show when={whenDue(item, clock())}>
            {(when) => (
              <span class={cx((item.due ?? '') <= todayIso(clock()) && !item.done_by_me && 'hl')}>{` · ${when()}`}</span>
            )}
          </Show>
        </p>
        <Show when={item.details}>
          {(text) => <p class="whitespace-pre-line">{text()}</p>}
        </Show>
      </div>

      <div class="actions">
        <Show
          when={item.pending}
          fallback={
            <span class={item.audience.kind === 'personal' ? 'tag t-gray' : 'tag t-room'}>
              {item.audience.kind === 'personal' ? 'perso' : 'classe'}
            </span>
          }
        >
          <span class="tag t-room">à confirmer</span>
        </Show>
        <Show when={item.can_confirm}>
          <button
            type="button"
            class={button.small}
            disabled={busy() === item.id}
            onClick={() => void run(item.id, () => confirmHomework(item.id))}
          >
            Confirmer
          </button>
        </Show>
        <Show when={item.mine}>
          <button
            type="button"
            class={button.smallIcon}
            title="Corriger"
            aria-label="Corriger"
            onClick={() => setEditing(item)}
          >
            <Pencil size={16} aria-hidden="true" />
          </button>
          <button
            type="button"
            class={button.smallIcon}
            title="Effacer"
            aria-label="Effacer"
            disabled={busy() === item.id}
            onClick={() => void run(item.id, () => deleteHomework(item.id))}
          >
            <Trash2 size={16} aria-hidden="true" />
          </button>
        </Show>
      </div>
    </li>
  );

  const section = (title: string, items: Homework[], hint?: string, faded = false) => (
    <Show when={items.length > 0}>
      <section class={cx('hw-section', faded && 'faded')}>
        <span class={label}>{title}</span>
        <Show when={hint}>{(text) => <p class="text-sm text-muted">{text()}</p>}</Show>
        <ul class="flex flex-col gap-2.5">
          <For each={items}>{row}</For>
        </ul>
      </section>
    </Show>
  );

  const signedOut = () => ['unauthenticated', 'auth_disabled'].includes(homeworkError()?.code ?? '');

  return (
    <>
      <PageHead title="Devoirs" sub="Ce qu’il y a à faire, pour toi ou pour ta classe.">
        <Show when={!signedOut()}>
          <button
            type="button"
            class={button.primary}
            onClick={() => {
              if (narrow()) setAdding(true);
              else titleField?.focus();
            }}
          >
            <Plus size={20} aria-hidden="true" />
            Noter un devoir
          </button>
        </Show>
      </PageHead>

      <Show
        when={!signedOut()}
        fallback={
          <Empty title="Connecte-toi pour noter tes devoirs">
            Les devoirs sont liés à ton compte. <a class={link} href="#compte">Connecte-toi avec Discord</a> pour les
            noter.
          </Empty>
        }
      >
        <div class="hw-layout">
          <div class="hw-list">
            <div class={`${segment} self-start`} role="group" aria-label="Devoirs affichés">
              <button type="button" aria-pressed={tab() === 'todo'} onClick={() => setTab('todo')}>
                {`À faire · ${todo().length + toConfirm().length}`}
              </button>
              <button type="button" aria-pressed={tab() === 'done'} onClick={() => setTab('done')}>
                {`Terminés · ${done().length}`}
              </button>
            </div>

            <Show
              when={homework().length > 0}
              fallback={
                <Empty>
                  {homeworkLoading()
                    ? 'Chargement…'
                    : 'Rien à faire pour l’instant. Note un devoir, ou ajoute-le depuis un cours.'}
                </Empty>
              }
            >
              <Show
                when={tab() === 'todo'}
                fallback={
                  <Show when={done().length > 0} fallback={<Empty>Aucun devoir terminé.</Empty>}>
                    {section('Terminé', done(), undefined, true)}
                  </Show>
                }
              >
                {section(
                  'À confirmer',
                  toConfirm(),
                  'Quelqu’un de ta classe l’a noté : confirme-le s’il est exact, il apparaîtra alors chez tout le monde.',
                )}
                {section('En attente de confirmation', waiting())}
                <For each={byDue()}>{([title, items]) => section(title, items)}</For>
                <Show when={todo().length === 0 && toConfirm().length === 0 && waiting().length === 0}>
                  <Empty title="Tout est fait">Rien à faire pour l’instant.</Empty>
                </Show>
              </Show>
            </Show>
          </div>

          <aside class="card hw-aside">
            <h2>Noter un devoir</h2>
            <HomeworkForm active onDone={() => undefined} ref={(field) => (titleField = field)} />
          </aside>
        </div>

        <Foot>
          <span>Les devoirs sont liés à ton compte Discord.</span>
        </Foot>
      </Show>

      <HomeworkSheet open={adding()} onClose={() => setAdding(false)} />
      <HomeworkSheet open={editing() !== null} item={editing()} onClose={() => setEditing(null)} />
    </>
  );
};
