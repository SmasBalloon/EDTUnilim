import { For, Show, createSignal, type Component } from 'solid-js';
import { Check, X } from 'lucide-solid';

import type { Course } from '../lib/api';
import { differences, type Change } from '../lib/changes';
import { groupLabel } from '../lib/groups';
import { KIND_LABEL } from '../lib/layout';
import { humanDuration, longDate } from '../lib/time';
import { Sheet } from './Sheet';

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

/** « mardi 15 septembre, 10:00 – 12:00 » */
function when(course: Course): string {
  return `${longDate(course.date)}, ${course.start} – ${course.end}`;
}

/** « TD R1.05 · G1A · 208 » */
function what(course: Course): string {
  return [
    KIND_LABEL[course.kind],
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
      <div class="changes" role="status">
        <span class="changes__dot" aria-hidden="true" />
        <p class="changes__text">
          {`Cette semaine a changé depuis ta dernière visite : ${summarise(props.changes)}.`}
        </p>
        <button type="button" class="button" onClick={() => setOpen(true)}>
          Voir
        </button>
        <button
          type="button"
          class="button button--round"
          onClick={props.onSeen}
          title="Marquer comme vu"
          aria-label="Marquer comme vu"
        >
          <Check size={16} aria-hidden="true" />
        </button>
      </div>

      <Sheet open={open()} onClose={() => setOpen(false)} label="Changements de la semaine">
        <header class="sheet__header">
          <h2 class="sheet__title">Ce qui a changé</h2>
          <button type="button" class="button" onClick={() => setOpen(false)} aria-label="Fermer">
            <X size={16} aria-hidden="true" />
          </button>
        </header>

        <ul class="change-list">
          <For each={props.changes}>
            {(change) => (
              <li class={`change change--${change.kind} kind-${change.course.kind}`}>
                <p class="change__head">
                  <span class="change__kind">{TITLES[change.kind]}</span>
                  <span class="change__when">{when(change.course)}</span>
                </p>
                <p class="change__what">{what(change.course)}</p>

                <Show when={change.before}>
                  {(before) => (
                    <ul class="change__details">
                      <For each={differences(before(), change.course)}>
                        {(detail) => <li>{detail}</li>}
                      </For>
                    </ul>
                  )}
                </Show>

                <Show when={change.kind !== 'modified'}>
                  <p class="change__details">{humanDuration(change.course.duration_minutes)}</p>
                </Show>
              </li>
            )}
          </For>
        </ul>

        <div class="sheet__footer">
          <button
            type="button"
            class="button button--block button--primary"
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
