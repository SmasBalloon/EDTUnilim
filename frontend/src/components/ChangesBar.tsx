import { For, Show, createSignal, type Component } from 'solid-js';
import { Check } from 'lucide-solid';

import type { Course } from '../lib/api';
import { differences, type Change } from '../lib/changes';
import { groupLabel } from '../lib/groups';
import { KIND_LABEL } from '../lib/layout';
import { humanDuration, longDate } from '../lib/time';
import { button, cx, kindCard, kindTag } from '../lib/ui';
import { Sheet, SheetHeader } from './Sheet';

interface Props {
  changes: Change[];
  /** L'utilisateur a pris connaissance : cette version devient la référence. */
  onSeen: () => void;
}

const TITLES: Record<Change['kind'], string> = {
  added: 'Ajouté',
  removed: 'Annulé',
  modified: 'Modifié',
};

const TONES: Record<Change['kind'], string> = {
  added: 't-sae',
  removed: 't-room',
  modified: 't-td',
};

/** « mardi 15 septembre, 10:00 – 12:00 » */
function when(course: Course): string {
  return `${longDate(course.date)}, ${course.start} – ${course.end}`;
}

/** « R1.05 · G1A · 208 » : la nature est dite par son tag. */
function what(course: Course): string {
  return [
    course.resource?.code ?? course.title,
    course.groups.map(groupLabel).join(', '),
    course.room,
  ]
    .filter(Boolean)
    .join(' · ');
}

/** Combien de changements, dit comme on le dirait. */
function summarise(changes: Change[]): string {
  const count = (kind: Change['kind']) => changes.filter((change) => change.kind === kind).length;
  const parts = [
    count('added') > 0 && `${count('added')} ajouté${count('added') > 1 ? 's' : ''}`,
    count('modified') > 0 && `${count('modified')} modifié${count('modified') > 1 ? 's' : ''}`,
    count('removed') > 0 && `${count('removed')} annulé${count('removed') > 1 ? 's' : ''}`,
  ].filter(Boolean);
  return parts.join(', ');
}

/**
 * Le bandeau des changements : ce qui a bougé dans cette semaine depuis la
 * dernière fois qu'on l'a regardée.
 *
 * Il reste tant qu'on ne l'a pas écarté — un emploi du temps se consulte en
 * marchant, mieux vaut ne pas faire disparaître la nouvelle avant qu'elle soit
 * lue.
 */
export const ChangesBar: Component<Props> = (props) => {
  const [open, setOpen] = createSignal(false);

  return (
    <>
      <div class="card status border-accent-line! bg-accent-soft! py-2.5! pr-2.5!" role="status">
        <span class="dot bg-accent!" aria-hidden="true" />
        <p class="min-w-0 flex-1 text-sm font-medium">
          {`Cette semaine a changé depuis ta dernière visite : ${summarise(props.changes)}.`}
        </p>
        <button type="button" class={button.plain} onClick={() => setOpen(true)}>
          Voir
        </button>
        <button
          type="button"
          class={button.icon}
          onClick={props.onSeen}
          title="Marquer comme vu"
          aria-label="Marquer comme vu"
        >
          <Check size={20} aria-hidden="true" />
        </button>
      </div>

      <Sheet open={open()} onClose={() => setOpen(false)} label="Changements de la semaine">
        <SheetHeader title="Ce qui a changé" onClose={() => setOpen(false)} />

        <ul class="grid gap-2.5">
          <For each={props.changes}>
            {(change) => (
              <li class={cx('rounded-xl border px-3.5 py-3', kindCard(change.course.kind))}>
                <p class="flex flex-wrap items-center gap-2">
                  <span class={cx('tag', TONES[change.kind])}>{TITLES[change.kind]}</span>
                  <span class={kindTag(change.course.kind)}>{KIND_LABEL[change.course.kind]}</span>
                  {/* « mardi 15 septembre » : seule l'initiale se capitalise. */}
                  <span class="text-[13px] text-muted tabular-nums first-letter:uppercase">
                    {when(change.course)}
                  </span>
                </p>
                <p
                  class={cx(
                    'mt-1.5 font-semibold',
                    change.kind === 'removed' && 'line-through decoration-accent/70',
                  )}
                >
                  {what(change.course)}
                </p>

                <Show when={change.before}>
                  {(before) => (
                    <ul class="mt-1 list-disc pl-4 text-[13px] text-muted">
                      <For each={differences(before(), change.course)}>
                        {(detail) => <li>{detail}</li>}
                      </For>
                    </ul>
                  )}
                </Show>

                <Show when={change.kind !== 'modified'}>
                  <p class="mt-1 text-[13px] text-muted">{humanDuration(change.course.duration_minutes)}</p>
                </Show>
              </li>
            )}
          </For>
        </ul>

        <div class="mt-6">
          <button
            type="button"
            class={`${button.primary} wide`}
            onClick={() => {
              props.onSeen();
              setOpen(false);
            }}
          >
            J'ai vu
          </button>
        </div>
      </Sheet>
    </>
  );
};
